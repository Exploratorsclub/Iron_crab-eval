# Momentum Active Pools (NATS)

**Status:** L2c Imminent-Entry (Evolution 2026-07-25)  
**Related:** `docs/plans/plan_realtime_slo_processable_set_20260725.md` §3.5 · `docs/supervisor/plan_geyser_stream_stability.md`

## Purpose

`momentum_bot` is the source of truth for which `(mint, pool)` pairs need **pinned** Geyser
explicit accounts (vaults, bin arrays, mint metadata) for **execution freshness** (ProbeBuy /
ScaleIn / open position exits). `market-data` subscribes to lifecycle updates and pins/unpins
via the track-worker — **no debounce** that would hide pin latency on the hot path.

**Not** the discovery channel: Existence, trades, buyers, velocity, and most pre-entry filters
come from the **TX / MarketEvents** path (P1). Pins are for vault/bin freshness around trade
and while a position is open — not for scanning every Discovery tracker.

## Topic

| Constant | Value |
|----------|-------|
| `TOPIC_MOMENTUM_ACTIVE_POOLS` | `ironcrab.v1.momentum.active_pools` |

- **Transport:** Core NATS (fire-and-forget; same class as `TOPIC_MARKET_EVENTS`).
- **Publisher:** `momentum_bot`
- **Subscriber:** `market-data`

## Message schema (JSON)

```json
{
  "version": 1,
  "ts_unix_ms": 1779123456789,
  "active": [
    {
      "mint": "<base58>",
      "pool": "<base58>",
      "dex": "pumpfun",
      "pin_reason": "tracker"
    }
  ],
  "removed": [
    {
      "mint": "<base58>",
      "pool": "<base58>",
      "reason": "stale_discovery"
    }
  ]
}
```

### Fields

| Field | Type | Notes |
|-------|------|-------|
| `version` | `u32` | Always `1` for this schema |
| `ts_unix_ms` | `u64` | Publisher wall clock |
| `active` | array | Pools to **add or reaffirm** pin (idempotent) |
| `removed` | array | Pools to **unpin** and drop from explicit Geyser set when safe |

### `active[]` entry

| Field | Values |
|-------|--------|
| `mint` | Token mint (base58) |
| `pool` | Pool address (base58) |
| `dex` | DEX label (`pumpfun`, `raydium`, …) — informational for logs/metrics |
| `pin_reason` | `tracker` — **imminent entry only** (`WaitHotSet` / `ProbeBuyPending` / `ScaleInPending`); `position` — open position on `(mint, pool)` |

Either array may be empty. Incremental messages are preferred; a periodic **reconcile snapshot**
(≈30 s) may send the full current active set in `active` with empty `removed`.

### `removed[]` entry

| Field | Values |
|-------|--------|
| `mint`, `pool` | base58 |
| `reason` | `rejected`, `stale_discovery`, `closed`, `untracked`, `hot_set_timeout`, `filter_failed` |

## Imminent-Entry contract (normative)

**Strategy goal:** Momentum is not required to be first buyer on new tokens; it trades tokens
with strong movement and captures part of that move. Trading a small amount of **entry latency**
for reliable **exit / quote freshness** (EXEC_HOT SLO) is intentional.

### Lifecycle

```text
Discovery / Validation (filters on TX/MarketEvents)
  — NO active_pools pin
        │
        ▼
Initial filter pass (green)
  — publish active pin_reason=tracker (imminent)
        │
        ▼
WaitHotSet
  — wait until explicit sub + vault/reserves fresh enough
  — pre-entry filters KEEP running on new events
  — if filters go red → removed + back to Validation/Rejected (no intent)
        │
        ▼
Pre-Intent Revalidate (REQUIRED)
  — filters still green? else no intent
  — hot-set / reserves still fresh? else keep waiting / timeout
        │
        ▼
ProbeBuy / ScaleIn intent → execution
        │
        ▼
open position → pin_reason=position (must-hot)
```

### Hard rules

1. **No pin** on `get_or_create_tracker` / pure Discovery / early Validation.  
2. **Pin only after** an initial pre-entry filter pass (entry into imminent / `WaitHotSet`).  
3. **No blind intent:** emit ProbeBuy/ScaleIn only if **(a)** hot-set/reserves are fresh enough **and** **(b)** filters are re-checked green **immediately before** intent emit.  
4. While in `WaitHotSet`, the same pre-entry gates continue to evaluate (trades, LP, dev-sell, velocity, …).  
5. Configurable **timeout** on `WaitHotSet`: on timeout → `removed` with `hot_set_timeout` (or equivalent), no intent, return to WAIT/Reject.  
6. Open-position pins (`pin_reason: position`) and wallet pins are **must-hot** — never shed for EXEC_HOT pressure.  
7. Intent after hot-set ready **without** pre-intent revalidate is a **spec violation**.

## Publisher rules (momentum_bot)

Publish **incremental** updates when:

1. Tracker completes **initial filter pass** and enters imminent / `WaitHotSet` → `active` with `pin_reason: tracker`  
   - **Forbidden:** pin solely because `get_or_create_tracker` created a Discovery tracker.  
2. Tracker enters / stays in `ProbeBuyPending` / `ScaleInPending` → keep/update pin (`tracker`)  
3. `open_position` → `active` with `pin_reason: position`  
4. Tracker → terminal `Rejected` (after existing cooldown) → `removed` reason `rejected`  
5. Position closed and no remaining tracker for `(mint, pool)` → `removed` reason `closed`  
6. Filters fail during `WaitHotSet` → `removed` reason `filter_failed` (or `rejected` if terminal)  
7. `WaitHotSet` timeout → `removed` reason `hot_set_timeout`  
8. Stale cleanup for non-imminent trackers (no pin expected): no-op for pins; optional `removed` if a pin was left by crash/reconcile  
9. Optional reconcile tick (~30 s): full **current** active set in `active`, empty `removed` (must not re-pin Discovery-only trackers)

**Do not** remove pins while: open position, `ProbeBuyPending`, `ScaleInPending`, or active `WaitHotSet` that still has green filters and has not timed out.

## Subscriber rules (market-data)

On each message:

1. For each `removed` entry: unpin pool / clear `GeyserPinReason::MomentumActive` on related tracked accounts; schedule coalesced Geyser explicit-set sync (track-worker).  
2. For each `active` entry: pin pool; register vaults/bin arrays/mints from `LivePoolCache` with Momentum pin. If cache miss: log at debug; reconcile/retry on next message or trade — **no hot-path RPC**.  
3. Wallet pins (`GeyserPinReason::Wallet`) must never be cleared by momentum `removed`.

## Latency targets

| Path | Target |
|------|--------|
| Pin event → NATS → market-data → Geyser subscription update | **&lt; 500 ms p99** (no artificial debounce that hides pin latency) |
| EXEC_HOT account channel lag (must-hot + imminent) | p50 &lt; 50 ms, p99 &lt; 200 ms (prod steady state; see realtime SLO plans) |
| Entry wait (`WaitHotSet`) | Bounded by config timeout; filters continue; no blind fire |

## Evolution note

Previous publisher rule “New tracker via `get_or_create_tracker` → pin” is **withdrawn**. It caused
large EXEC_HOT sets (~thousands of tracker pins) and violated exit/quote freshness under load.
Discovery remains on the TX/MarketEvents path.
