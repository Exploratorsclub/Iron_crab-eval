//! Invariante A.48 Phase 4 P3: Momentum Strategy SSOT vs WalletBalanceSnapshot (timed sync).
//!
//! Source-Contract gegen Sibling `Iron_crab/src/bin/momentum_bot.rs` und `Iron_crab/src/metrics.rs`.
//! Architektur-Grep-Gates auf dokumentierte Phase-4-Marker (Impl P2/P3); skip wenn Sibling oder
//! Marker noch nicht gemergt (paralleles Impl Phase 4 P1–P2–P4).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! keine Blackbox-Assertions auf private Datenstrukturen — nur dokumentierte Architektur-Strings.

use std::fs;
use std::path::PathBuf;

/// Impl P2/P3: confirmed ExecutionResult schreibt Strategy-SSOT-Balance (nicht nur open_position).
const PHASE4_CONFIRMED_EXECUTION_BALANCE_MARKER: &str = "record_confirmed_execution_token_balance";
/// Impl P2: WalletBalanceSnapshot darf confirmed balance nicht ohne Guard ueberschreiben.
const PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD: &str =
    "wallet_snapshot_must_not_clobber_confirmed_balance";
/// Impl P3: Divergenz Strategy-SSOT vs Wallet-Snapshot beobachtbar in Metriken.
const PHASE4_DIVERGENCE_METRIC_MARKER: &str = "momentum_wallet_balance_divergence";

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

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let root = iron_crab_root();
    let path = momentum_bot_rs_path();
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder momentum_bot.rs nicht lesbar unter {:?}",
            root
        );
        return None;
    }
    Some(root)
}

fn read_momentum_bot_source() -> String {
    let path = momentum_bot_rs_path();
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn read_metrics_source() -> String {
    let path = metrics_rs_path();
    if !path.is_file() {
        return String::new();
    }
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Production code only — test modules in the same file must not affect grep gates.
fn production_source(source: &str) -> &str {
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source section")
}

/// Bin sources with inline `#[cfg(test)]` hooks before production code ends.
fn production_bin_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod ") {
        return &source[..idx];
    }
    production_source(source)
}

/// Extrahiert den Rust-Funktionsblock ab `fn {name}` inkl. geschweifter Klammern.
fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needle = format!("fn {fn_name}");
    let start = source
        .find(&needle)
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

/// Extrahiert den innersten `match`-Arm-Block, der `needle` enthaelt.
fn extract_match_arm_containing(source: &str, needle: &str) -> String {
    let needle_pos = source
        .find(needle)
        .unwrap_or_else(|| panic!("expected `{needle}` in momentum_bot.rs"));
    let arm_start = source[..needle_pos]
        .rfind("MarketEventKind::")
        .or_else(|| source[..needle_pos].rfind("WalletBalanceSnapshot"))
        .unwrap_or_else(|| panic!("expected match arm for `{needle}` in momentum_bot.rs"));
    let brace_start = source[needle_pos..]
        .find('{')
        .map(|i| needle_pos + i)
        .expect("expected opening brace for WalletBalanceSnapshot arm");
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

fn skip_if_no_phase4_momentum_wallet_ssot_markers(source: &str) -> bool {
    if !source.contains(PHASE4_CONFIRMED_EXECUTION_BALANCE_MARKER) {
        eprintln!(
            "SKIP: Phase 4 P3 `{PHASE4_CONFIRMED_EXECUTION_BALANCE_MARKER}` not present in sibling momentum_bot.rs"
        );
        return true;
    }
    if !source.contains(PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD) {
        eprintln!(
            "SKIP: Phase 4 P2 `{PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD}` not present in sibling momentum_bot.rs"
        );
        return true;
    }
    false
}

fn skip_if_no_phase4_divergence_metric(source: &str) -> bool {
    if !source.contains(PHASE4_DIVERGENCE_METRIC_MARKER) {
        eprintln!(
            "SKIP: Phase 4 P3 `{PHASE4_DIVERGENCE_METRIC_MARKER}` not present in sibling metrics.rs"
        );
        return true;
    }
    false
}

/// Phase 4 P3: confirmed ExecutionResult mutiert Strategy-SSOT; WalletSnapshot mit Guard.
#[test]
fn phase4_momentum_execution_result_mutates_balance_marker() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_momentum_bot_source();
    let prod = production_bin_source(&source);
    if skip_if_no_phase4_momentum_wallet_ssot_markers(prod) {
        return;
    }

    let handle_er = extract_fn_block(prod, "handle_execution_result");
    assert!(
        handle_er.contains("ExecutionStatus::Confirmed"),
        "handle_execution_result must handle confirmed executions (Phase 4 SSOT contract)"
    );
    assert!(
        handle_er.contains(PHASE4_CONFIRMED_EXECUTION_BALANCE_MARKER),
        "confirmed path must record strategy SSOT via `{PHASE4_CONFIRMED_EXECUTION_BALANCE_MARKER}`"
    );
    assert!(
        handle_er.contains("open_position"),
        "confirmed BUY path must still call open_position (Momentum overlay; A.28 extension)"
    );
    assert!(
        handle_er.contains("fill_out") || handle_er.contains("fill_in"),
        "confirmed path must derive balance from ExecutionResult fill fields"
    );

    let wallet_arm = extract_match_arm_containing(prod, "MarketEventKind::WalletBalanceSnapshot");
    assert!(
        wallet_arm.contains(PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD),
        "WalletBalanceSnapshot handler must guard against clobbering confirmed execution balance \
         (`{PHASE4_WALLET_SNAPSHOT_CLOBBER_GUARD}`)"
    );
    assert!(
        wallet_arm.contains("balance_raw"),
        "WalletBalanceSnapshot arm must observe on-chain balance_raw for reconciliation"
    );
}

/// Phase 4 P3: Divergenz-Metrik exportiert (Strategy-SSOT vs Wallet-Snapshot).
#[test]
fn phase4_momentum_wallet_balance_divergence_metric_exported() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let metrics = read_metrics_source();
    if metrics.is_empty() {
        eprintln!("SKIP: sibling metrics.rs not readable");
        return;
    }
    let prod = production_source(&metrics);
    if skip_if_no_phase4_divergence_metric(prod) {
        return;
    }

    assert!(
        prod.contains("line!(") && prod.contains(PHASE4_DIVERGENCE_METRIC_MARKER),
        "metrics render must expose `{PHASE4_DIVERGENCE_METRIC_MARKER}` via line! (Prometheus text)"
    );
}
