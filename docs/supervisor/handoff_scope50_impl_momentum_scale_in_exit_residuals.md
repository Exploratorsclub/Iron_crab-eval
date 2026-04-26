WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 50: Momentum Scale-In Residual Exit Lifecycle

## Dependency

Run this scope after Scope 48 and preferably after Scope 49. Scope 48 fixes authoritative execution state after partial SELLs. Scope 49 protects the buy gate. This scope fixes Momentum's own position lifecycle so regular exits sell the full held amount and keep retrying residuals.

## Task-Beschreibung

Fix Momentum's scale-in / exit lifecycle for residual balances.

Production evidence from 2026-04-25/26:

- Many tokens had `probe BUY + scale-in BUY`.
- The next regular exit sold only the probe amount.
- The larger scale-in amount remained in the wallet until kill-switch liquidation.
- Momentum continued generating retry exits for some residuals, but state was inconsistent and after execution cleared balances they failed with `SIM_INSUFFICIENT_BALANCE`.

Examples reconstructed from execution results:

- `J7jQ...`: BUY probe `14099749285`, BUY scale-in `38711432312`, regular TIME_EXIT SELL `14099749285`, residual `38711432312`.
- `2cKp...`: BUY probe `20749527565`, BUY scale-in `60422615523`, regular TAKE_PROFIT SELL `20749527565`, residual `60422615523`.
- `Esgxy...`: BUY probe `13736375743`, BUY scale-in `38486263385`, regular STOP_LOSS SELL `13736375743`, residual `38486263385`.

Required behavior:

- Momentum must track probe + scale-in token amounts as one open position total.
- Exit intent generation must use the latest total held amount, not a stale probe amount.
- If a SELL is confirmed for less than the tracked position total, Momentum must reduce the position and immediately make the residual eligible for retry (`exit_generated=false`), not close the position.
- Resource-lock rejections during scale-in must not leave a stale `exit_generated=true` or stale pending SELL amount that blocks/poisons later exits.
- Max hold / stop loss / trailing stop should continue retrying until the residual is sold or authoritative wallet reconciliation proves zero.

## Relevante Invarianten (Volltext)

### A.28 Open Positions Counter Konsistenz (Single Source of Truth)

`get_open_positions()` wird aus LockManager `count_non_zero_token_balances()` abgeleitet (nicht als separater Counter). Der Wert stimmt stets mit der Anzahl non-zero Eintraege in `available_tokens` ueberein.

Formal: `get_open_positions() == available_tokens.values().filter(|b| b > 0).count()`. Nach N BUY-Fills: count == N. Nach Sell-All: count == 0. Nach Restart-Recovery: count == tatsaechlicher Bestand.

For Momentum in this scope: strategy `positions.len()` is not the canonical execution count, but it must not drop a position while a residual token balance is still known/expected.

### I-12 Decision Record

Ein Intent darf nicht ohne Decision Record verworfen werden. Momentum must not silently stop retrying exits after rejections/failures; any execution-engine rejection already has a DecisionRecord and Momentum must consume the corresponding result/state where available.

### I-9 Simulation-Gate

Transaktionen duerfen nicht ohne erfolgreiche Simulation gesendet werden. Do not bypass simulation to force exits.

### I-7 Hot Path RPC-Freiheit

No RPC calls in Momentum hot path. Use ExecutionResult, pending intent state, existing WalletSnapshotComplete flow, and already available wallet snapshots. RPC-only recovery remains cold path / liquidation.

## Bestehendes Pattern / Relevante Code-Stellen

Relevant current code:

- `src/bin/momentum_bot.rs`
  - `PositionTracker` has `token_amount`, `sol_invested`, `exit_generated`, `exit_generated_at`.
  - `open_position()` adds token amount when a position already exists.
  - `handle_execution_result()` partial SELL branch reduces position if `sold_amount < pos_total`.
  - `generate_and_publish_exit_intent()` and `register_sell_intent()` need to use current total position amount at publish time.
  - `reconcile_timed_exits()` collects retry candidates and publishes exits.
- `src/bin/execution_engine.rs`
  - After Scope 48, partial SELLs should leave execution-side residual balance available.

Known patterns:

- Pattern #6 Orphaned Buy: pending cleanup can remove intent state before result.
- Pattern #7 exit_generated not reset -> no sell retry.
- Pattern #15 LockManager Double-Counting / SELL Race.
- New failure pattern: scale-in residual tokens survived because regular exits sold only probe amount and the residual stopped being counted reliably.

## Erlaubte Dateien

- `src/bin/momentum_bot.rs`
- Narrow tests in existing `momentum_bot.rs` test module or related test files.
- Documentation only if necessary.

## Verboten

- No deploy, no `deploy.sh`, no server/systemd restart.
- No hot-path RPC.
- Do not change DEX instruction builders or pool discovery.
- Do not change liquidation latency behavior in this scope.
- Do not loosen stop-loss/take-profit policy thresholds.

## Erwartete Tests

Add focused tests proving:

1. Probe BUY confirmed creates a position with probe token amount.
2. Scale-in BUY confirmed for the same mint increases the same position to `probe + scale_in`.
3. Exit intent generated after scale-in uses the combined amount.
4. If an older/stale SELL for only the probe amount is confirmed, Momentum reduces the position to the residual and resets `exit_generated=false`.
5. A resource-lock rejected SELL during scale-in does not permanently block later exit retries and does not freeze a stale probe amount.
6. `reconcile_timed_exits()` retries residual positions using current `PositionTracker.token_amount`.

## Pruef-Befehle

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If CI provides Eval Level 5, ensure it passes before merge.

## Production Evidence Summary

Resource-lock rejects were early exit attempts racing with scale-in locks:

- `8pDue...` TAKE_PROFIT rejected: `pool locked by int-eabb2cfc-000016` (scale-in).
- `4CG7...` TRAILING_STOP rejected: `pool locked by int-eabb2cfc-000022`.
- `J7jQ...` MOMENTUM_EXIT rejected: `pool locked by int-eabb2cfc-000063`.
- `2cKp...` TRAILING_STOP rejected: `pool locked by int-eabb2cfc-000067`.
- `DhQT...` TRAILING_STOP rejected: `pool locked by int-eabb2cfc-000089`.
- `Esgxy...` TRAILING_STOP rejected: `pool locked by int-eabb2cfc-000125`.

Later regular exits confirmed but sold only the probe amounts, leaving scale-in residuals until liquidation.
