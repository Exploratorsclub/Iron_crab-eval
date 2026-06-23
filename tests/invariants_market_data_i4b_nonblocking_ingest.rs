//! Invariante I-4b: Market-Data Ingest Non-Blocking (INVARIANTS.md A.45)
//!
//! Lib-Blackbox: Geyser-Ingest darf JSONL nicht blockieren — nur bounded `try_enqueue` /
//! `try_write`; bei voller Queue sofort `false` (Drop + Metrik im Binary), nie warten.
//!
//! **md-state (R2):** `md-state` lebt im `market_data`-Binary (`MdStateCommand`,
//! `spawn_md_state_worker`). Eval linkt das Binary nicht. Verhalten dort ist durch Impl-
//! Unit-Tests abgedeckt: `pr_r2_tx_handler_returns_when_md_state_queue_full`,
//! `pr_r2_burst_coalesces_single_schedule_sync_flag` in `Iron_crab/src/bin/market_data.rs`.
//!
//! **PR233 (Impl PR #233):** `sync_geyser_tracked_accounts_core` (Evict + broadcast tracked sets)
//! darf nur auf dem `md-state` OS-Thread laufen — via coalesced
//! `MdStateCommand::FlushGeyserSyncDebounced`, nicht auf der Tokio-Ingest-Runtime.
//! Source-Contract: `invariants_market_data_tracking_single_writer.rs`.
//!
//! **Phase 1 Hybrid (Impl PR #238):** Ingest/Sidefx ohne `tracked_*` Map-Reads; Register-Verbot
//! im TX-/Account-parse-Pfad. Source-Contract: `invariants_market_data_i4b_ingest_no_tracked_read.rs`.
//!
//! Compile-Time-Dokumentation (öffentliche Impl-Doku / Supervisor-Handoff):
//! - Thread-Namen: `"md-state"`, `"md-ingest-liveness"`, `"md-geyser-sync-debounce"`
//! - Command-Variante: `FlushGeyserSyncDebounced`
//!
//! Eval I-4b lib-seitig = JSONL non-blocking + Spec-Eintrag; kein RPC/Geyser in diesen Tests.

use ironcrab::ipc::{MarketEvent, MarketEventKind};
use ironcrab::storage::jsonl_writer::{JsonlWriterConfig, QueuedJsonlWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn sample_market_event(id: &str) -> MarketEvent {
    MarketEvent::new(
        "market-data",
        "eval-i4b",
        "run-i4b",
        id.to_string(),
        "geyser",
        Some(1),
        MarketEventKind::TransactionDetected {
            signature: format!("sig-{id}"),
            program: "prog".into(),
        },
    )
}

/// Bounded queue (cap 4): fuellen bis voll, weiterer `try_enqueue_json` liefert sofort `false`.
#[test]
fn i4b_jsonl_full_queue_try_enqueue_json_returns_immediately() {
    const CAP: usize = 4;
    let dir = tempdir().expect("tempdir");
    let config = JsonlWriterConfig::new("i4b_fullq")
        .with_log_dir(dir.path())
        .with_flush_each_write(true);

    let writer = QueuedJsonlWriter::spawn(config, CAP).expect("spawn QueuedJsonlWriter");

    let hold = Arc::new(AtomicBool::new(true));
    let hold_writer = Arc::clone(&hold);
    assert!(
        writer.try_write(SlowHoldRecord { hold: hold_writer }),
        "first enqueue should succeed"
    );

    // `try_write` belegt eine Queue-Slot; bei cap 4 bleiben drei weitere Plaetze.
    for i in 0..(CAP - 1) {
        assert!(
            writer.try_enqueue_json(format!("{{\"fill\":{i}}}")),
            "enqueue {i} should succeed while queue not full"
        );
    }

    let t0 = Instant::now();
    assert!(
        !writer.try_enqueue_json("{\"overflow\":true}".to_string()),
        "enqueue on full queue must return false (drop path)"
    );
    assert!(
        t0.elapsed() < Duration::from_millis(200),
        "full-queue try_enqueue_json must not block (got {:?})",
        t0.elapsed()
    );

    hold.store(false, Ordering::Relaxed);
    let _ = writer.flush();
    drop(writer);
}

/// `try_enqueue_market_event` auf voller Queue: sofort `false`, kein Deadlock.
#[test]
fn i4b_jsonl_full_queue_try_enqueue_market_event_returns_immediately() {
    const CAP: usize = 4;
    let dir = tempdir().expect("tempdir");
    let config = JsonlWriterConfig::new("i4b_mkt")
        .with_log_dir(dir.path())
        .with_flush_each_write(true);

    let writer = QueuedJsonlWriter::spawn(config, CAP).expect("spawn QueuedJsonlWriter");

    let hold = Arc::new(AtomicBool::new(true));
    let hold_writer = Arc::clone(&hold);
    assert!(writer.try_write(SlowHoldRecord { hold: hold_writer }));

    for i in 0..(CAP - 1) {
        let event = sample_market_event(&format!("pre-{i}"));
        assert!(
            writer.try_enqueue_market_event(&event),
            "market event {i} should enqueue"
        );
    }

    let overflow = sample_market_event("overflow");
    let t0 = Instant::now();
    assert!(
        !writer.try_enqueue_market_event(&overflow),
        "market event on full queue must return false"
    );
    assert!(
        t0.elapsed() < Duration::from_millis(200),
        "full-queue try_enqueue_market_event must not block (got {:?})",
        t0.elapsed()
    );

    hold.store(false, Ordering::Relaxed);
    let _ = writer.flush();
    drop(writer);
}

/// Erfolgreiche Enqueues werden vom Writer-Thread persistiert (kein stilles Verschlucken bei Platz).
#[test]
fn i4b_jsonl_bounded_enqueue_delivers_when_capacity_available() {
    let dir = tempdir().expect("tempdir");
    let config = JsonlWriterConfig::new("i4b_ok")
        .with_log_dir(dir.path())
        .with_flush_each_write(true);

    let writer = QueuedJsonlWriter::spawn(config, 64).expect("spawn QueuedJsonlWriter");
    let event = sample_market_event("delivered-001");

    assert!(writer.try_enqueue_market_event(&event));
    writer.flush().expect("flush");

    let deadline = Instant::now() + Duration::from_millis(200);
    let (records, bytes) = loop {
        let stats = writer.stats();
        if stats.0 >= 1 || Instant::now() >= deadline {
            break stats;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(records >= 1, "expected at least one record written");
    assert!(bytes > 0);

    drop(writer);
}

/// Serde-Hilfstyp: haelt den jsonl-writer-Thread kurz, damit die Queue zuverlaessig voll wird.
struct SlowHoldRecord {
    hold: Arc<AtomicBool>,
}

impl serde::Serialize for SlowHoldRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        while self.hold.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(2));
        }
        ().serialize(serializer)
    }
}
