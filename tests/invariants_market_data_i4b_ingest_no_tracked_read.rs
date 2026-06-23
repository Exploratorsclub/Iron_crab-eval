//! Invariante I-4b / A.45 Phase 1 (Hybrid): Ingest + md-sidefx ohne `tracked_*` Map-Reads.
//!
//! Source-Contract gegen `Iron_crab/src/bin/market_data.rs` (Sibling-Checkout wie CI).
//! Architektur-Grep-Gate: `might_be_relevant`, Account-Dispatch, TX-Handler und md-sidefx
//! duerfen nicht `tracked_vaults.read()` / `tracked_mints.read()` / `tracked_bin_arrays.read()`
//! nutzen; Account-Filter nutzen `SnapshotView` / `TrackedMembershipSnapshot`.
//! Phase-1 Register-Verbot: kein `RegisterReservesAfterTrade` im TX-Pfad, kein
//! `RegisterPoolVaultsFromAccount` aus Sidefx-Account-Flush.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! keine Blackbox-Assertions auf private API — nur dokumentierte Architektur-Strings.

use std::fs;
use std::path::PathBuf;

const TRACKED_MAP_READS: &[&str] = &[
    "tracked_vaults.read()",
    "tracked_mints.read()",
    "tracked_bin_arrays.read()",
];

/// Geschwister-Checkout: `parent/ironcrab-eval` + `parent/Iron_crab` (wie `golden_replay_blackbox.rs`).
fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn market_data_rs_path() -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("bin")
        .join("market_data.rs")
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let root = iron_crab_root();
    let path = market_data_rs_path();
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder market_data.rs nicht lesbar unter {:?}",
            root
        );
        return None;
    }
    Some(root)
}

fn read_market_data_source() -> String {
    let path = market_data_rs_path();
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Extrahiert den Rust-Funktionsblock ab `fn {name}` / `async fn {name}` inkl. geschweifter Klammern.
fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needles = [format!("async fn {fn_name}"), format!("fn {fn_name}")];
    let start = needles
        .iter()
        .find_map(|needle| source.find(needle))
        .unwrap_or_else(|| panic!("expected fn {fn_name} in market_data.rs"));
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

fn assert_no_tracked_map_reads(body: &str, fn_name: &str) {
    for needle in TRACKED_MAP_READS {
        assert!(
            !body.contains(needle),
            "{fn_name} must not contain `{needle}` (Phase1 SnapshotView / TrackedMembershipSnapshot)"
        );
    }
}

fn assert_account_filter_uses_membership_snapshot(body: &str, fn_name: &str) {
    assert!(
        body.contains("tracked_membership") || body.contains("tracked_membership_contains_pubkey"),
        "{fn_name} must use tracked_membership snapshot helper (Phase1 hybrid ingest)"
    );
}

/// Test A: Sibling `Iron_crab/src/bin/market_data.rs` lesbar.
#[test]
fn phase1_market_data_rs_exists() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    assert!(
        source.contains("fn account_geyser_update_might_be_relevant")
            || source.contains("async fn account_geyser_update_might_be_relevant"),
        "market_data.rs must define account_geyser_update_might_be_relevant"
    );
}

/// Test B: Ingest/Sidefx-Funktionskoerper ohne `tracked_*` Map-Reads; Account-Filter mit Snapshot.
#[test]
fn phase1_ingest_sidefx_no_tracked_map_reads() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();

    let might_be_relevant = extract_fn_block(&source, "account_geyser_update_might_be_relevant");
    assert_no_tracked_map_reads(
        &might_be_relevant,
        "account_geyser_update_might_be_relevant",
    );
    assert_account_filter_uses_membership_snapshot(
        &might_be_relevant,
        "account_geyser_update_might_be_relevant",
    );

    let dispatch_high = extract_fn_block(&source, "account_geyser_dispatch_priority_high");
    assert_no_tracked_map_reads(&dispatch_high, "account_geyser_dispatch_priority_high");
    assert_account_filter_uses_membership_snapshot(
        &dispatch_high,
        "account_geyser_dispatch_priority_high",
    );

    let tx_body = extract_fn_block(&source, "handle_geyser_transaction");
    assert_no_tracked_map_reads(&tx_body, "handle_geyser_transaction");

    let vault_tick = extract_fn_block(&source, "md_sidefx_process_vault_balance_tick");
    assert_no_tracked_map_reads(&vault_tick, "md_sidefx_process_vault_balance_tick");

    let sidefx_flush = extract_fn_block(&source, "md_sidefx_flush_pending_md_state_jobs");
    assert_no_tracked_map_reads(&sidefx_flush, "md_sidefx_flush_pending_md_state_jobs");
}

/// Test C: TX-Handler enqueued kein `RegisterReservesAfterTrade`.
#[test]
fn phase1_trade_path_no_register_reserves() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let tx_body = extract_fn_block(&source, "handle_geyser_transaction");
    assert!(
        !tx_body.contains("RegisterReservesAfterTrade"),
        "handle_geyser_transaction must not enqueue RegisterReservesAfterTrade (Phase1 register ban)"
    );
}

/// Test D: Sidefx-Flush enqueued kein `RegisterPoolVaultsFromAccount`.
#[test]
fn phase1_sidefx_no_register_pool_from_account() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let sidefx_flush = extract_fn_block(&source, "md_sidefx_flush_pending_md_state_jobs");
    assert!(
        !sidefx_flush.contains("RegisterPoolVaultsFromAccount"),
        "md_sidefx_flush_pending_md_state_jobs must not enqueue RegisterPoolVaultsFromAccount (Phase1 register ban)"
    );
}

/// Test E: TX-Handler enqueued kein Arb-Reconcile (Phase1 Entkopplung).
#[test]
fn phase1_trade_path_no_arb_reconcile_enqueue() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let tx_body = extract_fn_block(&source, "handle_geyser_transaction");
    assert!(
        !tx_body.contains("try_enqueue_arb_reconcile_for_pool"),
        "handle_geyser_transaction must not call try_enqueue_arb_reconcile_for_pool (Phase1)"
    );
    assert!(
        !tx_body.contains("ArbMultiDexReconcile"),
        "handle_geyser_transaction must not enqueue ArbMultiDexReconcile (Phase1)"
    );
}
