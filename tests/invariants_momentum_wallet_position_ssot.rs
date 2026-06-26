//! Invariante A.48 Phase 4 P3: Momentum Strategy SSOT vs WalletBalanceSnapshot (timed sync).
//!
//! Source-Contract gegen Sibling `Iron_crab/src/bin/momentum_bot.rs` und `Iron_crab/src/metrics.rs`
//! (Impl PR #243 @ 7df6298). Architektur-Grep-Gates auf dokumentierte Phase-4-Marker.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! keine Blackbox-Assertions auf private Datenstrukturen — nur dokumentierte Architektur-Strings.

use std::fs;
use std::path::PathBuf;

/// Impl P2: confirmed BUY/SELL size from `ExecutionResult.fill_out` / `fill_in`, not snapshot.
const PHASE4_CONFIRMED_EXECUTION_FILL_MARKER: &str = "token_amount = fill_out.raw";
/// Impl P2: Live positions — snapshot hint only, no token_amount overwrite.
const PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD: &str = "hint only; no size overwrite";
/// Impl P4: divergence recording between confirmed overlay and snapshot hint.
const PHASE4_DIVERGENCE_RECORD_MARKER: &str = "record_wallet_balance_divergence_if_any";
/// Impl P4: Prometheus export (total + per-mint lamports).
const PHASE4_DIVERGENCE_METRIC_TOTAL: &str = "momentum_wallet_balance_divergence_total";
const PHASE4_DIVERGENCE_METRIC_LAMPORTS: &str = "momentum_wallet_balance_divergence_lamports";

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn momentum_bot_rs_path() -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("bin")
        .join("momentum_bot.rs")
}

fn metrics_rs_path() -> PathBuf {
    iron_crab_root().join("src").join("metrics.rs")
}

fn require_sibling_iron_crab() -> PathBuf {
    let root = iron_crab_root();
    let path = momentum_bot_rs_path();
    assert!(
        path.is_file(),
        "Iron_crab Sibling-Checkout fehlt oder momentum_bot.rs nicht lesbar unter {:?} \
         (CI: dual-checkout neben ironcrab-eval)",
        root
    );
    root
}

fn read_momentum_bot_source() -> String {
    let path = momentum_bot_rs_path();
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_metrics_source() -> String {
    let path = metrics_rs_path();
    assert!(
        path.is_file(),
        "Iron_crab sibling metrics.rs not readable at {}",
        path.display()
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// momentum_bot.rs: strip only the trailing `mod tests { ... }` block (production code follows it).
fn source_excluding_mod_tests_block(source: &str) -> String {
    const MOD_TESTS: &str =
        "#[cfg(test)]\n#[allow(clippy::field_reassign_with_default)]\nmod tests";
    let Some(start) = source.find(MOD_TESTS) else {
        return source.to_string();
    };
    let open = source[start..]
        .find('{')
        .map(|i| start + i)
        .expect("mod tests opening brace");
    let mut depth = 0usize;
    let mut end = open;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = open + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let mut out = String::with_capacity(source.len());
    out.push_str(&source[..start]);
    out.push_str(&source[end..]);
    out
}

/// metrics.rs: inline `#[cfg(test)]` helpers mid-file — strip only trailing test modules.
fn production_metrics_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod momentum_latency_metrics_tests") {
        return &source[..idx];
    }
    source
}

/// Extrahiert den Rust-Funktionsblock ab `fn {name}` / `async fn {name}` inkl. geschweifter Klammern.
fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needle_async = format!("async fn {fn_name}");
    let needle = format!("fn {fn_name}");
    let start = source
        .find(&needle_async)
        .or_else(|| source.find(&needle))
        .unwrap_or_else(|| panic!("expected fn {fn_name} in momentum_bot.rs"));
    let brace_start = source[start..]
        .find('{')
        .map(|i| start + i)
        .expect("expected opening brace for fn block");
    let mut depth = 0usize;
    let mut end = brace_start;
    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = brace_start + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(end > brace_start, "unclosed fn block for {fn_name}");
    source[start..end].to_string()
}

/// Extrahiert den `WalletBalanceSnapshot`-Match-Arm in `process_market_event`.
fn extract_wallet_balance_snapshot_arm(source: &str) -> String {
    let needle = "MarketEventKind::WalletBalanceSnapshot";
    let needle_pos = source
        .find(needle)
        .unwrap_or_else(|| panic!("expected `{needle}` in momentum_bot.rs"));
    let arm_start = source[..needle_pos].rfind(needle).unwrap_or(needle_pos);
    let brace_start = source[needle_pos..]
        .find("=> {")
        .map(|i| needle_pos + i + 3)
        .expect("expected `=> {` body for WalletBalanceSnapshot match arm");
    let mut depth = 0usize;
    let mut end = brace_start;
    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = brace_start + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(
        end > brace_start,
        "unclosed match arm block containing `{needle}`"
    );
    source[arm_start..end].to_string()
}

/// Phase 4 P3: confirmed ExecutionResult mutiert Strategy-SSOT; WalletSnapshot mit Guard.
#[test]
fn phase4_momentum_execution_result_mutates_balance_marker() {
    require_sibling_iron_crab();
    let source = read_momentum_bot_source();
    let prod = source_excluding_mod_tests_block(&source);

    assert!(
        prod.contains(PHASE4_DIVERGENCE_RECORD_MARKER),
        "momentum_bot must define `{PHASE4_DIVERGENCE_RECORD_MARKER}` (Phase 4 P4 divergence)"
    );

    let handle_er = extract_fn_block(&prod, "handle_execution_result");
    assert!(
        handle_er.contains("ExecutionStatus::Confirmed"),
        "handle_execution_result must handle confirmed executions (Phase 4 SSOT contract)"
    );
    assert!(
        handle_er.contains("open_position"),
        "confirmed BUY path must call open_position (Momentum overlay; A.28 extension)"
    );
    assert!(
        handle_er.contains(PHASE4_CONFIRMED_EXECUTION_FILL_MARKER),
        "confirmed BUY must size position from ExecutionResult fill_out \
         (`{PHASE4_CONFIRMED_EXECUTION_FILL_MARKER}`)"
    );
    assert!(
        handle_er.contains("fill_out") || handle_er.contains("fill_in"),
        "confirmed path must derive balance from ExecutionResult fill fields"
    );

    let wallet_arm =
        extract_wallet_balance_snapshot_arm(&extract_fn_block(&prod, "process_market_event"));
    assert!(
        wallet_arm.contains(PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD),
        "WalletBalanceSnapshot handler must not clobber confirmed position size \
         (`{PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD}`)"
    );
    assert!(
        wallet_arm.contains("balance=0 ignored for Live position"),
        "WalletBalanceSnapshot zero-balance must not close Live/ExecutionResult positions"
    );
    assert!(
        wallet_arm.contains(PHASE4_DIVERGENCE_RECORD_MARKER),
        "WalletBalanceSnapshot path must record divergence via `{PHASE4_DIVERGENCE_RECORD_MARKER}`"
    );
    assert!(
        wallet_arm.contains("balance_raw"),
        "WalletBalanceSnapshot arm must observe on-chain balance_raw for reconciliation"
    );
}

/// Phase 4 P3: Divergenz-Metrik exportiert (Strategy-SSOT vs Wallet-Snapshot).
#[test]
fn phase4_momentum_wallet_balance_divergence_metric_exported() {
    require_sibling_iron_crab();
    let metrics = read_metrics_source();
    let prod = production_metrics_source(&metrics);

    assert!(
        prod.contains(PHASE4_DIVERGENCE_METRIC_TOTAL),
        "metrics must define `{PHASE4_DIVERGENCE_METRIC_TOTAL}`"
    );
    assert!(
        prod.contains(PHASE4_DIVERGENCE_METRIC_LAMPORTS),
        "metrics must define `{PHASE4_DIVERGENCE_METRIC_LAMPORTS}` per-mint gauge"
    );
    assert!(
        prod.contains("line!") && prod.contains(PHASE4_DIVERGENCE_METRIC_TOTAL),
        "metrics render must expose `{PHASE4_DIVERGENCE_METRIC_TOTAL}` via line! (Prometheus text)"
    );
}
