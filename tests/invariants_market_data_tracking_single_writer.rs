//! Invariante I-4b / PR233 + Phase 2a/2b/3: Tracking Single-Writer — Geyser sync + Momentum/Arb active pools auf `md-track-worker`.
//!
//! Architektur-Source-Contract gegen `Iron_crab/src/bin/market_data.rs` (kein `market_data`-Binary-Link).
//! Liest Impl-Quelltext nur als veröffentlichter Vertrags-Marker, nicht als private API.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Änderung an `Iron_crab/src/`;
//! keine Blackbox-Assertions auf interne Datenstrukturen — nur dokumentierte Architektur-Strings.

use std::fs;
use std::path::PathBuf;

const PUBLISH_CALL_MARKER: &str = "nats.publish(TOPIC_ARB_TRACK_REQUESTS";
const SUBSCRIBE_CALL_MARKER: &str = "nats.subscribe(TOPIC_ARB_TRACK_REQUESTS";

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

/// Extrahiert den innersten `fn`-Block, der `needle` enthaelt (Spec-Marker statt festem Funktionsnamen).
fn extract_fn_block_containing(source: &str, needle: &str) -> String {
    let needle_pos = source
        .find(needle)
        .unwrap_or_else(|| panic!("expected `{needle}` in market_data.rs"));
    let fn_start = source[..needle_pos]
        .rfind("fn ")
        .unwrap_or_else(|| panic!("expected enclosing fn for `{needle}` in market_data.rs"));
    let brace_start = source[fn_start..]
        .find('{')
        .map(|i| fn_start + i)
        .expect("expected opening brace for enclosing fn block");
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
    assert!(end > brace_start, "unclosed fn block containing `{needle}`");
    source[fn_start..end].to_string()
}

/// Momentum NATS/coalesce-Pfad: bevorzugt `momentum_tracking_coalesce`, sonst Handler mit Spec-Subject.
fn momentum_active_pools_path_block(source: &str) -> Option<String> {
    if source.contains("fn momentum_tracking_coalesce") {
        return Some(extract_fn_block(source, "momentum_tracking_coalesce"));
    }
    const MOMENTUM_ACTIVE_POOLS_SUBJECT: &str = "ironcrab.v1.momentum.active_pools";
    if source.contains(MOMENTUM_ACTIVE_POOLS_SUBJECT) {
        return Some(extract_fn_block_containing(
            source,
            MOMENTUM_ACTIVE_POOLS_SUBJECT,
        ));
    }
    None
}

fn skip_if_no_phase2b_momentum_path(source: &str) -> Option<String> {
    momentum_active_pools_path_block(source).or_else(|| {
        eprintln!(
            "SKIP: Phase 2b momentum active_pools path not present in sibling market_data.rs \
             (expected fn momentum_tracking_coalesce or NATS subject ironcrab.v1.momentum.active_pools)"
        );
        None
    })
}

fn track_worker_momentum_handler_block(source: &str) -> String {
    if source.contains("fn track_worker_process_job") {
        return extract_fn_block(source, "track_worker_process_job");
    }
    extract_fn_block_containing(source, "TrackWorkerCommand::ApplyMomentumActivePools")
}

/// Production code only — test modules in the same file must not affect grep gates.
fn production_source(source: &str) -> &str {
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source section")
}

/// Bin sources with inline `#[cfg(test)]` hooks before production code ends:
/// strip only trailing `#[cfg(test)] mod …` test modules.
fn production_bin_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod ") {
        return &source[..idx];
    }
    production_source(source)
}

/// Arb NATS/coalesce-Pfad: bevorzugt `spawn_arb_tracking_coalescer`, sonst Handler mit Spec-Subject.
fn arb_track_requests_path_block(source: &str) -> Option<String> {
    if source.contains("fn spawn_arb_tracking_coalescer") {
        return Some(extract_fn_block(source, "spawn_arb_tracking_coalescer"));
    }
    const ARB_TRACK_REQUESTS_SUBJECT: &str = "ironcrab.v1.arb.track_requests";
    if source.contains(ARB_TRACK_REQUESTS_SUBJECT) {
        return Some(extract_fn_block_containing(
            source,
            ARB_TRACK_REQUESTS_SUBJECT,
        ));
    }
    None
}

fn skip_if_no_phase3_arb_path(source: &str) -> Option<String> {
    arb_track_requests_path_block(source).or_else(|| {
        eprintln!(
            "SKIP: Phase 3 arb track_requests path not present in sibling market_data.rs \
             (expected fn spawn_arb_tracking_coalescer or NATS subject ironcrab.v1.arb.track_requests)"
        );
        None
    })
}

fn track_worker_arb_handler_block(source: &str) -> String {
    if source.contains("fn track_worker_process_job") {
        return extract_fn_block(source, "track_worker_process_job");
    }
    extract_fn_block_containing(source, "TrackWorkerCommand::ApplyArbTrackRequests")
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

/// Phase 2b (Hybrid): Momentum `active_pools` NATS/coalesce enqueued auf `md-track-worker` only —
/// kein `MdStateCommand::ApplyMomentumActivePools` / `md_state_try_enqueue` im Momentum-Pfad.
#[test]
fn phase2b_momentum_active_pools_bypasses_md_state() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let Some(momentum_fn) = skip_if_no_phase2b_momentum_path(&source) else {
        return;
    };

    assert!(
        momentum_fn.contains("track_worker_try_enqueue"),
        "momentum active_pools path must enqueue bounded work on md-track-worker \
         (track_worker_try_enqueue)"
    );
    assert!(
        !(momentum_fn.contains("md_state_try_enqueue")
            && momentum_fn.contains("MdStateCommand::ApplyMomentumActivePools")),
        "momentum active_pools path must not enqueue MdStateCommand::ApplyMomentumActivePools \
         via md_state_try_enqueue (Phase 2b: momentum bypasses md-state)"
    );
    assert!(
        !momentum_fn.contains("apply_momentum_active_pools_in_md_state"),
        "momentum NATS/coalesce path must not call apply_momentum_active_pools_in_md_state \
         (Phase 2b)"
    );

    let process_job_fn = extract_fn_block(&source, "md_state_process_job");
    assert!(
        !process_job_fn.contains("ApplyMomentumActivePools"),
        "md_state_process_job must not handle ApplyMomentumActivePools \
         (variant removed from MdStateCommand in Phase 2b)"
    );

    assert!(
        source.contains("TrackWorkerCommand::ApplyMomentumActivePools")
            || (source.contains("enum TrackWorkerCommand")
                && source.contains("ApplyMomentumActivePools")),
        "TrackWorkerCommand must include ApplyMomentumActivePools (Phase 2b track-worker handler)"
    );

    let track_worker_fn = track_worker_momentum_handler_block(&source);
    assert!(
        track_worker_fn.contains("ApplyMomentumActivePools"),
        "track-worker job handler must handle ApplyMomentumActivePools \
         (Phase 2b coalesced Geyser push on md-track-worker)"
    );
}

/// Phase 3 (Hybrid, I-4e): Arb `track_requests` NATS/coalesce enqueued auf `md-track-worker` only —
/// kein `MdStateCommand::ApplyArbTrackRequests` / `md_state_try_enqueue` im Arb-Pfad.
#[test]
fn phase3_arb_track_requests_bypasses_md_state() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let prod = production_bin_source(&source);
    let Some(arb_fn) = skip_if_no_phase3_arb_path(&source) else {
        return;
    };

    assert!(
        arb_fn.contains("track_worker_try_enqueue"),
        "arb track_requests path must enqueue bounded work on md-track-worker \
         (track_worker_try_enqueue)"
    );
    assert!(
        !(arb_fn.contains("md_state_try_enqueue")
            && arb_fn.contains("MdStateCommand::ApplyArbTrackRequests")),
        "arb track_requests path must not enqueue MdStateCommand::ApplyArbTrackRequests \
         via md_state_try_enqueue (Phase 3: arb bypasses md-state)"
    );
    assert!(
        !arb_fn.contains("md_state_try_enqueue"),
        "arb track_requests coalescer must not call md_state_try_enqueue (Phase 3)"
    );
    assert!(
        !prod.contains("MdStateCommand::ApplyArbTrackRequests"),
        "MdStateCommand must not include ApplyArbTrackRequests variant (Phase 3)"
    );

    let process_job_fn = extract_fn_block(&source, "md_state_process_job");
    assert!(
        !process_job_fn.contains("ApplyArbTrackRequests"),
        "md_state_process_job must not handle ApplyArbTrackRequests \
         (variant absent from MdStateCommand in Phase 3)"
    );

    assert!(
        source.contains("TrackWorkerCommand::ApplyArbTrackRequests")
            || (source.contains("enum TrackWorkerCommand")
                && source.contains("ApplyArbTrackRequests")),
        "TrackWorkerCommand must include ApplyArbTrackRequests (Phase 3 track-worker handler)"
    );
}

/// Phase 3 (Hybrid, I-4e): Arb `track_requests` coalescer + track-worker handler — symmetrisch Phase 2b.
#[test]
fn phase3_arb_track_requests_uses_track_worker() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_market_data_source();
    let Some(arb_fn) = skip_if_no_phase3_arb_path(&source) else {
        return;
    };

    assert!(
        arb_fn.contains("TrackWorkerCommand::ApplyArbTrackRequests"),
        "spawn_arb_tracking_coalescer must enqueue TrackWorkerCommand::ApplyArbTrackRequests"
    );
    assert!(
        source.contains(SUBSCRIBE_CALL_MARKER),
        "market_data must subscribe to TOPIC_ARB_TRACK_REQUESTS (Phase 3 consumer)"
    );
    assert!(
        !source.contains(PUBLISH_CALL_MARKER),
        "market_data must not publish TOPIC_ARB_TRACK_REQUESTS (Phase 3)"
    );

    let track_worker_fn = track_worker_arb_handler_block(&source);
    assert!(
        track_worker_fn.contains("ApplyArbTrackRequests"),
        "track-worker job handler must handle ApplyArbTrackRequests \
         (Phase 3 coalesced Geyser push on md-track-worker)"
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
