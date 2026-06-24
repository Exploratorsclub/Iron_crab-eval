//! Invariante I-4b / PR233 + Phase 2a: Tracking Single-Writer — Geyser sync auf `md-track-worker`.
//!
//! Architektur-Source-Contract gegen `Iron_crab/src/bin/market_data.rs` (kein `market_data`-Binary-Link).
//! Liest Impl-Quelltext nur als veröffentlichter Vertrags-Marker, nicht als private API.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Änderung an `Iron_crab/src/`;
//! keine Blackbox-Assertions auf interne Datenstrukturen — nur dokumentierte Architektur-Strings.

use std::fs;
use std::path::PathBuf;

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

/// Extrahiert den Rust-Funktionsblock ab `fn {name}` inkl. geschweifter Klammern.
fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needle = format!("fn {fn_name}");
    let start = source
        .find(&needle)
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

fn fn_block_contains_batched_flush(fn_block: &str) -> bool {
    fn_block.contains("sync_geyser_tracked_accounts_batched_flush")
        || fn_block.contains("sync_geyser_tracked_accounts_batched_flush_with_deadline")
}

/// PR233: Debounce-Thread enqueued `FlushGeyserSyncDebounced` auf md-state — kein direktes
/// `sync_geyser_tracked_accounts_batched_flush` im Tokio-/Debounce-Pfad.
#[test]
fn md_state_command_includes_flush_geyser_sync() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();

    assert!(
        source.contains("enum MdStateCommand"),
        "MdStateCommand must exist (md-state single-writer contract)"
    );
    assert!(
        source.contains("FlushGeyserSyncDebounced"),
        "MdStateCommand must include FlushGeyserSyncDebounced for Geyser tracked flush scheduling"
    );
    assert!(
        source.contains("md_state_try_enqueue"),
        "md-state bounded enqueue helper must exist"
    );

    let debounce_fn = extract_fn_block(&source, "schedule_geyser_sync_batch_debounced");
    assert!(
        debounce_fn.contains("md_state_try_enqueue"),
        "schedule_geyser_sync_batch_debounced must enqueue md-state work, not flush inline"
    );
    assert!(
        debounce_fn.contains("MdStateCommand::FlushGeyserSyncDebounced"),
        "debounce path must schedule FlushGeyserSyncDebounced on md-state"
    );
    assert!(
        !fn_block_contains_batched_flush(&debounce_fn),
        "schedule_geyser_sync_batch_debounced must not call sync_geyser_tracked_accounts_batched_flush \
         on Tokio/debounce thread (PR233 single-writer)"
    );
}

/// Phase 2a (Impl PR #239): batched Geyser sync auf `md-track-worker`, md-state forward-only.
#[test]
fn phase2a_geyser_sync_on_track_worker_not_md_state_loop() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();

    let process_job_fn = extract_fn_block(&source, "md_state_process_job");
    assert!(
        process_job_fn.contains("FlushGeyserSyncDebounced"),
        "md_state_process_job must handle FlushGeyserSyncDebounced"
    );
    assert!(
        process_job_fn.contains("track_worker_try_enqueue")
            || process_job_fn.contains("ScheduleGeyserPushDebounced"),
        "md_state_process_job must forward FlushGeyserSyncDebounced to track-worker \
         (track_worker_try_enqueue / ScheduleGeyserPushDebounced)"
    );

    let push_fn = extract_fn_block(&source, "track_worker_execute_coalesced_push");
    assert!(
        fn_block_contains_batched_flush(&push_fn),
        "track_worker_execute_coalesced_push must run sync_geyser_tracked_accounts_batched_flush \
         (or _with_deadline) on md-track-worker (Phase 2a)"
    );

    assert!(
        source.contains("spawn_track_worker") || source.contains("md-track-worker"),
        "expected spawn_track_worker or md-track-worker OS thread (Phase 2a track-worker)"
    );

    let worker_fn = extract_fn_block(&source, "md_state_worker_loop");
    assert!(
        !fn_block_contains_batched_flush(&worker_fn),
        "md_state_worker_loop must not call sync_geyser_tracked_accounts_batched_flush \
         (Phase 2a: sync moved to md-track-worker)"
    );
}

/// PR233: Global-Ingest-Liveness auf OS-Thread (`md-ingest-liveness`), nicht nur Tokio-`spawn`.
#[test]
fn global_ingest_liveness_os_thread() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();

    assert!(
        source.contains("md-ingest-liveness"),
        "expected dedicated md-ingest-liveness OS thread name (PR233)"
    );

    let liveness_fn = extract_fn_block(&source, "spawn_market_data_global_ingest_liveness_task");
    assert!(
        liveness_fn.contains("std::thread::Builder"),
        "global ingest liveness must use std::thread::Builder (survives Tokio freeze)"
    );
    assert!(
        liveness_fn.contains("md-ingest-liveness"),
        "liveness thread must be named md-ingest-liveness"
    );
    assert!(
        !liveness_fn.contains("tokio::spawn"),
        "PR167 stall loop must not rely exclusively on tokio::spawn (PR233 OS-thread liveness)"
    );
}

/// PR232-Follow-up: Vault-Touch O(1) — kein `values_mut()` Full-Map-Scan pro Vault.
#[test]
fn touch_tracked_vault_o1_contract() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let touch_fn = extract_fn_block(&source, "touch_tracked_vault_pubkey");

    assert!(
        touch_fn.contains("get_mut"),
        "touch_tracked_vault_pubkey should use targeted get_mut lookups"
    );
    assert!(
        !touch_fn.contains("values_mut()"),
        "touch_tracked_vault_pubkey must not scan full vault map via values_mut() (PR232/PR233 O(1) contract)"
    );
}
