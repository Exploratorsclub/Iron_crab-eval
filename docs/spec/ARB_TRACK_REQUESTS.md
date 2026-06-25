# Arb Track Requests (Phase 3)

**Status:** Implemented (Eval gate)  
**Topic:** `ironcrab.v1.arb.track_requests` (`TOPIC_ARB_TRACK_REQUESTS`)  
**Plan:** `docs/plans/plan_hybrid_rollback_tracking_architecture_20260623.md` Phase 3  
**Invariante:** I-4e / INVARIANTS.md A.47

## Rolle

`arb-strategy` publiziert pool-zentrische Geyser-Pin-Anfragen (ArbMultiDex-Konsument) per Core-NATS fire-and-forget. `market-data` subscribed, coalesced und wendet Pins auf dem `md-track-worker` an — **nicht** über `md-state`.

## Publisher / Subscriber (I-4e)

| Aktion | Erlaubt in | Verboten in |
|--------|------------|-------------|
| `nats.publish(TOPIC_ARB_TRACK_REQUESTS, …)` | `arb_strategy.rs` | `market_data.rs`, `momentum_bot.rs`, `execution_engine.rs` |
| `nats.subscribe(TOPIC_ARB_TRACK_REQUESTS)` | `market_data.rs` | alle anderen Binaries |

Nur `momentum_bot`, `arb_strategy` und `execution_engine` dürfen Track-Request-**topics** publizieren (Schema-Test I-4e Gesamtarchitektur). Für **dieses** Topic gilt Phase 3: ausschließlich `arb_strategy` publiziert; Momentum nutzt `ironcrab.v1.momentum.active_pools` (Phase 2b).

## MD Track-Worker Pfad

- NATS arb subscriber / coalescer: `spawn_arb_tracking_coalescer` → `track_worker_try_enqueue` + `TrackWorkerCommand::ApplyArbTrackRequests`
- **Verboten:** `md_state_try_enqueue` + `MdStateCommand::ApplyArbTrackRequests` im Arb-Pfad
- Handler: `track_worker_process_job` / `apply_arb_track_requests_on_track_worker`

## Wire Schema (`ArbTrackRequestsUpdate`)

Öffentliche Types: `ironcrab::nats::{ArbTrackRequestsUpdate, ArbTrackActiveEntry, ArbTrackActiveReason, ArbTrackRemovedEntry, ArbTrackRemovedReason}`

### Spec-Sample (JSON)

```json
{
  "version": 1,
  "ts_unix_ms": 1700000000,
  "active": [
    {
      "pool": "Pool111111111111111111111111111111111111111",
      "reason": "multi_dex"
    }
  ],
  "removed": [
    {
      "pool": "Pool222222222222222222222222222222222222222",
      "reason": "cooldown"
    }
  ],
  "reconcile": true
}
```

### Felder

| Feld | Typ | Beschreibung |
|------|-----|--------------|
| `version` | `u32` | Wire-Format-Version (aktuell `1`) |
| `ts_unix_ms` | `u64` | Erzeugungszeitpunkt |
| `active` | `[{ pool, reason }]` | Pools die gepinnt werden sollen |
| `removed` | `[{ pool, reason }]` | Pools deren Arb-Pin entfernt werden soll |
| `reconcile` | `bool` (default `false`) | Wenn `true`: `active` ist autoritatives Vollset — MD unpinned Arb-Pools die nicht in `active` stehen |

### `reason` Enums (snake_case on wire)

**Active:** `baseline`, `multi_dex`, `trade_signal`  
**Removed:** `cooldown`, `stale`, `budget`

## Eval-Gates

| Test | Datei |
|------|-------|
| `phase3_only_arb_strategy_publishes_track_requests_topic` | `tests/invariants_arb_track_requests.rs` |
| `phase3_arb_track_requests_schema_roundtrip` | `tests/invariants_arb_track_requests.rs` |
| `phase3_arb_track_requests_bypasses_md_state` | `tests/invariants_market_data_tracking_single_writer.rs` |
| `phase3_arb_track_requests_uses_track_worker` | `tests/invariants_market_data_tracking_single_writer.rs` |

Regression I-4c (Phase 2c): kein `ArbMultiDexReconcile` in `market_data.rs` — unverändert in `tests/invariants_market_data_i4b_ingest_no_tracked_read.rs`.
