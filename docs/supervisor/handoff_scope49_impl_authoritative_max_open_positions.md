WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 49: Authoritative Max Open Positions Gate

## Dependency

Run this scope only after Scope 48 is merged. Scope 48 makes execution-engine's LockManager state amount-aware after partial SELLs. This scope relies on that state being meaningful.

## Task-Beschreibung

Fix the max-open-positions gate so the bot cannot keep buying when the wallet/execution layer still has residual token balances.

Production evidence from 2026-04-25/26:

- UI configured `max_open_positions=5`.
- The wallet reached 11 trade-token positions before kill switch.
- Execution metrics showed `intent_rejection_by_reason{reason="risk_limit"} 0`.
- All BUY decisions saw `current_open_positions < 5`; distribution from intent metadata was only `0..3`.
- `current_open_positions` came from `momentum-bot` metadata (`ctx.positions.len()`), which missed residual scale-in tokens after partial exits.

Required behavior:

- For BUY risk checks, execution-engine must not trust only strategy-provided `current_open_positions`.
- Use an authoritative execution-side count from LockManager (`ctx.get_open_positions()`) after Scope 48, or at minimum `max(strategy_metadata_count, ctx.get_open_positions())`.
- The DecisionRecord details must state both values so future incidents are diagnosable.
- If authoritative count is `>= config.max_open_positions`, reject BUY with the existing risk-limit reason.

## Relevante Invarianten (Volltext)

### A.28 Open Positions Counter Konsistenz (Single Source of Truth)

`get_open_positions()` wird aus LockManager `count_non_zero_token_balances()` abgeleitet (nicht als separater Counter). Der Wert stimmt stets mit der Anzahl non-zero Eintraege in `available_tokens` ueberein.

Formal: `get_open_positions() == available_tokens.values().filter(|b| b > 0).count()`. Nach N BUY-Fills: count == N. Nach Sell-All: count == 0. Nach Restart-Recovery: count == tatsaechlicher Bestand.

Kontext: KNOWN_BUG_PATTERNS #5 (Ghost Positions); dual-path tracking (Execution Result + Geyser Balance) verursachte Race Conditions und Counter-Drift.

### I-12 Decision Record

Ein Intent darf nicht ohne Decision Record verworfen werden. For this scope every max-open rejection must produce a DecisionRecord with details including metadata count, authoritative count, effective count, and max.

### I-7 Hot Path RPC-Freiheit

Hot Path = everything called from `process_intent` in the normal trading flow. No RPC calls are allowed in this gate. The authoritative count must come from already-maintained in-process state, not a fresh wallet scan.

## Bestehendes Pattern / Relevante Code-Stellen

Relevant current code:

- `src/bin/execution_engine.rs`
  - `fn get_open_positions(&self) -> usize { self.lock_manager.count_non_zero_token_balances() }`
  - BUY risk check currently reads:
    - `intent.metadata["current_open_positions"]`
    - compares it to `config.max_open_positions`
    - Decision details say `from intent metadata`.
- `src/bin/momentum_bot.rs`
  - BUY intents insert `current_open_positions` from `ctx.positions.read().len()`.
  - Keep this metadata for observability, but it must not be the only enforcement source.

## Erlaubte Dateien

- `src/bin/execution_engine.rs`
- Narrow tests for risk checks / process intent behavior in existing modules.
- Documentation only if necessary.

## Verboten

- No deploy, no `deploy.sh`, no server/systemd restart.
- No hot-path RPC or wallet scan.
- Do not change strategy entry filters.
- Do not change max position config semantics except making enforcement authoritative.
- Do not change dashboard queries.

## Erwartete Tests

Add focused tests proving:

1. BUY with metadata `current_open_positions=1` but LockManager has 5 non-zero token balances is rejected by max-open gate.
2. BUY with metadata `current_open_positions=5` but LockManager has 1 is rejected (metadata still conservative).
3. BUY with both counts below max passes the max-open check.
4. Decision details include both `metadata_current`, `authoritative_current` or equivalent wording.

## Pruef-Befehle

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If CI provides Eval Level 5, ensure it passes before merge.

## Production Evidence Summary

From 2026-04-25/26:

- `max_open_positions` rejection count: `0`.
- BUY metadata distribution before kill switch:
  - `current_open_positions=0`: 37 buys
  - `=1`: 20 buys
  - `=2`: 5 buys
  - `=3`: 3 buys
  - `>=5`: 0 buys
- The wallet had 11 trade-token positions at kill switch because residual scale-in balances were not counted by Momentum state.
