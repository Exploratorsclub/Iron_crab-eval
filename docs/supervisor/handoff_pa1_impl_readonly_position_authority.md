WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff PA-1: Read-only PositionAuthority Skeleton

## Task-Beschreibung

Begin the migration toward a real durable position single source of truth (SOT), as described in `docs/plans/plan_position_authority_sot_migration.md` from the eval repo.

This first scope must be **read-only / non-invasive**:

- Add a PositionAuthority reducer/module inside `Iron_crab`.
- It consumes model events in tests (and may expose types/helpers for later wiring).
- It builds a derived `PositionState` from `ExecutionResult`-like and `WalletBalanceSnapshot`-like events.
- It must not affect live trading, risk checks, execution, momentum decisions, dashboard metrics, or LockManager behavior yet.

The goal is to create the durable event-reducer foundation and tests. Wiring to live NATS/JetStream, dashboard, max-open gates, and Momentum read-view are later scopes.

## Relevante Invarianten (Volltext)

### I-24a JetStream = SSOT fuer Bot-Zustand

Wallet-Balances, Positionen, Pool-Cache und Config gehoeren in JetStream / persistente Streams. Konsumenten bootstrappen daraus und holen Live-Updates von dort.

For this scope: create reducer/domain types so a future `position-manager` can own durable position state from persistent events. Do not make ephemeral Momentum or LockManager state the new durable truth.

### I-1 / I-2 Role Separation

Nur `execution-engine` laedt Keys und signiert/sendet. `market-data`, `momentum-bot`, `arb-strategy`, `control-plane` and future `position-manager` remain keyless.

For this scope: no key loading, no signing, no send logic, no treasury/signer access.

### I-7 Hot Path RPC-Freiheit

Hot Path (normal Momentum/Arb/Execution flow) must not perform blocking RPC. Reconciliation RPC is Cold Path only.

For this scope: no RPC at all. Tests should feed synthetic events directly into the reducer.

### I-20/I-21 Locks

Capital locks and resource locks prevent overbooking and self-conflict. LockManager remains required for in-flight reservations and execution safety.

For this scope: do not remove or alter LockManager. PositionAuthority is not yet used for reservations.

### A.28 Open Positions Counter Konsistenz

`get_open_positions()` wird aktuell aus LockManager `count_non_zero_token_balances()` abgeleitet. Nach N BUY-Fills: count == N. Nach Sell-All: count == 0. Nach Restart-Recovery: count == tatsaechlicher Bestand.

For this scope: PositionAuthority may expose its own `open_positions_count()` for tests/diagnostics, but production metrics/gates must remain unchanged until later scopes.

## Bestehendes Pattern / Relevante Code-Stellen

Useful files/patterns to inspect:

- `src/ipc/schema.rs`
  - `ExecutionResult`
  - `MarketEventKind::WalletBalanceSnapshot`
  - `ExplicitAmount`
- `src/bin/execution_engine.rs`
  - Scope 48 amount-aware SELL metadata:
    - `sell_position_delta_applied`
    - `sell_token_account_closed`
    - `sell_untracked_ata`
  - Scope 49 `max_open_positions` still uses LockManager for now.
- `src/bin/momentum_bot.rs`
  - `PositionTracker` remains strategy overlay for now.
  - Do not change Momentum behavior in this scope.
- `src/storage/locks.rs`
  - LockManager remains in-flight reservation state.

Known bug patterns to keep in mind:

- #5 Ghost Positions: stale snapshots / missing zero transitions caused display drift.
- #6 Orphaned Buy: pending cleanup can lose pending intent before result.
- #7 `exit_generated` not reset can block retries.
- #15 LockManager double-counting / SELL races.
- #16 Token-2022 program must be preserved.
- #24 Open Positions Counter Drift: avoid dual independent counters.
- Recent failure pattern: scale-in residual tokens survived because partial SELL was treated as full close by some state layers.

## Design Requirements

Create a small module, e.g. `src/position_authority/`, with pure reducer logic:

### Domain Types

At minimum:

- `PositionAuthority`
- `PositionState`
- `PositionStatus = Open | Closed | ReconcileNeeded` (or equivalent)
- event enum for reducer tests, e.g.:
  - `PositionEvent::BuyConfirmed`
  - `PositionEvent::SellConfirmed`
  - `PositionEvent::WalletBalanceSnapshot`
  - `PositionEvent::WalletSnapshotComplete` if useful

State should track at least:

- `mint`
- `balance_raw`
- `decimals`
- `token_program`
- `ata` if known
- lots or aggregate buy fills sufficient for tests
- `sold_raw_total`
- `status`
- `last_update_source`

### Reducer Behavior

Must support:

1. BUY confirmed increases `balance_raw` and records lot/fill.
2. Scale-in BUY for same mint increases the same position.
3. Partial SELL subtracts `sold_raw`, leaves status `Open`.
4. Full SELL or wallet snapshot zero closes the position.
5. Wallet snapshot with non-zero balance can create/reconcile a position if execution events were missing.
6. Token-2022 `token_program` is preserved.

### No Live Behavior Change

Do not wire this reducer into production paths yet unless it is strictly passive and disabled/no-op by default. Prefer no runtime wiring in PA-1.

If adding a metric/log-only integration seems tempting, stop unless it is clearly read-only and tiny. The preferred PA-1 deliverable is compile-tested reducer + tests.

## Erlaubte Dateien

Preferred:

- `src/position_authority/mod.rs`
- `src/position_authority/state.rs` or similar small files
- `src/lib.rs` module export if needed
- Unit tests in the new module

Allowed only if needed:

- `Cargo.toml` if a new internal module requires no dependencies ideally

Avoid touching:

- `src/bin/execution_engine.rs`
- `src/bin/momentum_bot.rs`
- `src/bin/market_data.rs`

If you must touch any runtime binary, explain why in the PR.

## Verboten

- No deploy, no server restart.
- No RPC.
- No key loading.
- No changes to trading/risk/execution behavior.
- No dashboard changes.
- No Momentum behavior changes.
- Do not remove or alter LockManager.
- Do not make PositionAuthority authoritative for production yet.

## Erwartete Tests

Add focused unit tests for the reducer:

1. `buy_then_scale_in_accumulates_balance`
   - BUY 100, BUY 300 => balance 400, status Open.

2. `partial_sell_keeps_position_open`
   - BUY 400, SELL 100 => balance 300, status Open.

3. `full_sell_closes_position`
   - BUY 400, SELL 400 => balance 0, status Closed.

4. `wallet_zero_snapshot_closes_position`
   - BUY 400, WalletBalanceSnapshot 0 => Closed.

5. `wallet_nonzero_snapshot_recovers_missing_position`
   - WalletBalanceSnapshot 250 for unknown mint => Open/ReconcileNeeded with balance 250.

6. `token_2022_program_preserved`
   - event with Token-2022 program keeps exact program id in state.

7. `sell_more_than_balance_saturates_and_marks_reconcile_needed`
   - BUY 100, SELL 150 => balance 0 and status Closed or ReconcileNeeded; choose conservative behavior and document it in test name.

## Pruef-Befehle

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If CI provides Eval Level 5, ensure it passes before merge.

## PR Summary Requirements

In the PR, state clearly:

- This is PA-1, read-only reducer foundation.
- No production trading behavior changed.
- No runtime process was added yet unless explicitly justified.
- What events and state fields are modeled.
- Which tests prove partial sell / scale-in / wallet snapshot reconciliation.
