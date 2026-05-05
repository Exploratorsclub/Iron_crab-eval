WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 55: Momentum Exit Price Validation Against Executable Quote

## Task-Beschreibung

Fix a production-observed false STOP_LOSS / wrong exit classification caused by stale or wrong `current_price` in `momentum-bot`.

Production evidence from `2026-04-27`, mint `AfKayRAFuCHvxaEr387ND1BgossGpRBxzEJKpSNb9ek9`:

BUY:

- Intent: `int-0bd84c8a-000002`
- Confirmed at bot record time `17:39:45 CEST`
- Fill: `7159.492133` tokens for `0.00125 SOL`
- Entry price in Momentum log: `entry_price=5727593.7064` tokens/SOL

Momentum exit decision:

- Intent: `int-0bd84c8a-000003`
- Triggered only about 1s after BUY was processed.
- Momentum log:
  - `STOP_LOSS trigger`
  - `entry_price=5727593.7064`
  - `current_price=12736430.818995386`
  - `pnl_pct=-55.0298`
  - Reason: `Hard stop hit: -55.0% loss`

Actual SELL:

- Confirmed.
- Fill: `7159.492133` tokens for `0.001938988 SOL`.
- Executable sell price ≈ `3,692,386` tokens/SOL, which is **better** than entry, not worse.
- Dashboard PnL based on fills: `+55.12%`.

Therefore:

- Execution/fill accounting was correct.
- Dashboard PnL from actual fills was correct.
- Momentum's exit decision used a stale/wrong/non-executable `current_price` and incorrectly classified the exit as STOP_LOSS.

Fix this at the Momentum exit-decision layer.

## Relevante Invarianten (Volltext)

### A.11 Pool-Matching (I-13, FIX-38)

`should_apply_position_price_update(position_pool, source_pool)` gibt nur dann true zurück, wenn `source_pool == position.pool` oder `source_pool` ist `None` oder `position.pool` ist leer.

Formal: Apply iff `source_pool.is_none() || position_pool.is_empty() || position_pool == source_pool`.

Kontext: Verhindert falsche PnL und TAKE_PROFIT/STOP_LOSS bei Multi-Pool-Tokens.

### A.19 tokens_per_sol (I-14)

LOWER `tokens_per_sol` = Token wertvoller. `pnl_pct = (entry/current - 1)*100`. `highest_price` = niedrigster tokens_per_sol (bester Preis für Holder).

This convention must not be inverted.

### A.12 Hot-Path RPC-Freiheit (I-4, I-7)

Hot Path (Momentum/Arb normal trading flow) must not perform blocking RPC calls. Any validation must use LivePoolCache / existing quotes / in-process state, not RPC.

### I-9 Simulation-Gate

Do not bypass simulation or send transactions based on unvalidated state.

### I-12 Decision Record

No intent may be silently dropped. If an exit signal is suppressed or reclassified because executable quote disagrees with `current_price`, log/record the reason clearly.

## Bestehendes Pattern / Relevante Code-Stellen

Primary file:

- `src/bin/momentum_bot.rs`

Relevant areas:

- `PositionTracker`
  - `entry_price`
  - `current_price`
  - `highest_price`
  - `pnl_pct()`
  - trailing/stop-loss logic
- `update_position_price(...)`
  - must preserve pool-matching behavior.
- exit generation paths:
  - `should_exit(...)` / equivalent exit decision logic
  - `generate_and_publish_exit_intent(...)`
  - `reconcile_timed_exits(...)`
- `quote_calculator` / LivePoolCache usage for executable quote where available.

Known bug patterns:

- #1 Wrong-Pool Price Pollution.
- #2 fill_in/fill_out wrong -> false entry price.
- #3 Inverted PnL formula.
- #13 TAKE_PROFIT iteration loop: no symptom fixes without evidence.
- #19 Fix without correct root cause.

## Required Behavior

### A. Do not trust stale `current_price` for hard exits

Before generating exits that depend on price/PnL (`STOP_LOSS`, `TRAILING_STOP`, `TAKE_PROFIT`), Momentum must validate the signal against an executable or at least pool-correct current quote when available.

Acceptable validation source:

- LivePoolCache quote for the exact position pool / DEX.
- Existing quote calculator for position token amount to SOL.
- Existing in-process pool state only.

Not allowed:

- RPC in Momentum hot path.
- Global multi-pool discovery.
- Using a trade price from a different pool.

### B. Suppress or reclassify clearly impossible hard exits

If `current_price` says hard stop loss but executable quote implies positive/neutral PnL, do not emit `STOP_LOSS`.

Possible safe behavior:

- suppress the price-based exit for this tick and log `exit_signal_suppressed_stale_price`.
- update `current_price` from executable quote if the quote is pool-correct.
- allow non-price exits (e.g. max hold time) to proceed, but their reason must not claim hard stop loss.

### C. Keep Time/Max-Hold exits working

`TIME_EXIT` is not price-triggered and may still run when max hold is exceeded. But its details must use the validated/latest price if reporting PnL.

### D. Diagnostics

Add logs with fields:

- `mint`
- `position_pool`
- `entry_price`
- `current_price`
- `executable_tokens_per_sol` or equivalent
- `current_pnl_pct`
- `executable_pnl_pct`
- `exit_type_original`
- `decision = suppress|allow|reclassify`

This is important because previous PnL/exit bugs were repeatedly misdiagnosed.

## Erlaubte Dateien

- `src/bin/momentum_bot.rs`
- small pure helper if useful, preferably in existing helper module if one exists
- tests in existing `momentum_bot.rs` test module

Avoid touching:

- `src/bin/execution_engine.rs`
- DEX builders
- dashboard/trades server

## Verboten

- No deploy, no server restart.
- No RPC in Momentum hot path.
- No simulation bypass.
- Do not change ExecutionResult fill accounting.
- Do not change Dashboard PnL.
- Do not loosen hard stop threshold globally.
- Do not remove pool-matching.
- Do not introduce broad refactors of Momentum.

## Erwartete Tests

Add focused tests:

1. `hard_stop_suppressed_when_executable_quote_profitable`
   - entry tps = `5_727_593`
   - stale current tps = `12_736_430` -> would be -55%
   - executable tps = `3_692_386` -> profitable
   - expected: no STOP_LOSS intent / signal suppressed.

2. `hard_stop_allowed_when_executable_quote_confirms_loss`
   - stale/current and executable both imply loss beyond threshold.
   - expected: STOP_LOSS allowed.

3. `take_profit_not_allowed_from_wrong_pool_or_stale_quote`
   - if executable quote disagrees and source is not pool-correct, suppress.

4. `time_exit_still_allowed_without_price_validation`
   - max hold exceeded should still produce TIME_EXIT if price validation unavailable, but not claim hard stop.

5. `tokens_per_sol_formula_not_inverted`
   - lower executable tps than entry produces positive PnL.

If current code is hard to test directly, extract a pure helper that takes:

- `entry_price`
- `current_price`
- optional `executable_tokens_per_sol`
- requested exit type
- thresholds

and returns an allow/suppress/reclassify decision.

## Pruef-Befehle

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If CI provides Eval Level 5, ensure it passes before merge.

## Acceptance Criteria

- The `AfKay...` class of false hard stop is prevented.
- Momentum cannot emit STOP_LOSS based solely on stale `current_price` when an executable pool-correct quote shows profit.
- No hot-path RPC.
- Logs clearly distinguish stale-price suppression from allowed exits.
- Existing successful exits are not disabled wholesale.
- CI + Eval Level 5 + final Bugbot must pass.
