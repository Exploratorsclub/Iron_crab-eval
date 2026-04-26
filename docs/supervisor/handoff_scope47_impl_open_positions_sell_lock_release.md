WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 47: Fix Ghost Open Positions After Confirmed SELL

## Task-Beschreibung

Production runtime evidence on 2026-04-25 shows `execution-engine` reporting `open_positions` > 0 after all trade tokens were sold. The wallet had only WSOL non-zero, `momentum-bot` reported `open_positions=0`, but `execution-engine` reported `open_positions=3` and later `4`.

Root cause on current GitHub branch `architecture-rebuild`:

- `src/bin/execution_engine.rs` confirmed SELL success path clears the sold mint via `ctx.lock_manager.set_available_token_balance(mint, 0)`.
- Later in the same success path, common cleanup calls `ctx.lock_manager.release_locks(&intent.intent_id)`.
- `src/storage/locks.rs::release_locks()` restores every token amount from the intent's capital lock back into `available_tokens`.
- For SELL intents, this resurrects the just-sold token balance, so `count_non_zero_token_balances()` counts a ghost position.

Fix this root cause. Keep the change narrowly scoped.

## Relevante Invarianten (Volltext)

### A.28 Open Positions Counter Konsistenz (Single Source of Truth)

`get_open_positions()` wird aus LockManager `count_non_zero_token_balances()` abgeleitet (nicht als separater Counter). Der Wert stimmt stets mit der Anzahl non-zero Eintraege in `available_tokens` ueberein.

Formal: `get_open_positions() == available_tokens.values().filter(|b| b > 0).count()`. Nach N BUY-Fills: count == N. Nach Sell-All: count == 0. Nach Restart-Recovery: count == tatsaechlicher Bestand.

Kontext: KNOWN_BUG_PATTERNS #5 (Ghost Positions); dual-path tracking (Execution Result + Geyser Balance) verursachte Race Conditions und Counter-Drift.

### A.2 LockManager

`total_locked + available` = initial fuer SOL-Erhaltung ueber Lock/Release. Gleicher Intent-ID darf nicht doppelt gelockt werden (Capital Lock / Idempotency).

Fuer diesen Fix wichtig: Diese SOL-Erhaltung darf nicht blind auf SELL-token locks uebertragen werden, wenn der SELL bereits on-chain bestaetigt wurde. Nach einem bestaetigten SELL ist der verkaufte Token nicht mehr available und darf nicht durch Lock-Release wieder als available Token eingetragen werden.

### A.12 Hot-Path RPC-Freiheit (I-4, I-7)

DEX-Connectors liefern bei Cache-Miss None/Err ohne RPC (Hot Path). Hot Path (Arb, Momentum) darf keine blockierenden RPC-Calls ausfuehren.

Fuer diesen Fix: Keine neuen RPC-Calls, keine Wallet-Scans, keine externe Reconciliation im normalen SELL-Erfolgspfad. Der Fix muss rein im LockManager-/Execution-Cleanup-State erfolgen.

### I-9 Simulation-Gate

Transaktionen duerfen nicht ohne erfolgreiche Simulation gesendet werden. Dieser Scope darf den Simulations-/Send-Pfad nicht lockern, umgehen oder neu klassifizieren.

### I-12 Decision Record

Ein Intent darf nicht ohne Decision Record verworfen werden. Dieser Scope darf keine neuen Silent-Drop-Pfade einfuehren.

## Bestehendes Pattern / Relevante Code-Stellen

Current GitHub `architecture-rebuild` evidence:

- `src/bin/execution_engine.rs`: confirmed SELL clears sold mint balance:
  - `ctx.lock_manager.set_available_token_balance(mint_str.to_string(), 0);`
  - Log: `LockManager: cleared token balance after confirmed SELL`
- Same function later calls common cleanup:
  - `ctx.lock_manager.release_locks(&intent.intent_id);`
- `src/storage/locks.rs::release_locks()` currently restores locked token amounts:
  - `for (mint, amount) in lock.tokens { *available_tokens.entry(mint).or_insert(0) += amount; }`

Known patterns to preserve:

- Pattern #15 LockManager Double-Counting / SELL Race: keep LockManager as the immediate local state source for BUY/SELL preflight and confirmed execution.
- Pattern #24 Open Positions Counter Drift: `open_positions` must remain derived from `LockManager.count_non_zero_token_balances()`; do not reintroduce an atomic counter or a second source of truth.
- Newly observed failure pattern: after confirmed SELL, `release_locks()` re-adds the SELL token lock and resurrects a ghost available token balance.

Preferred implementation direction:

- Add a LockManager API that releases locks after a successful SELL without returning sold token amounts to `available_tokens`, while still releasing SOL/WSOL/resource locks correctly.
- Or equivalently adjust the success cleanup ordering/API so the final LockManager state after confirmed SELL has sold mint balance exactly `0`.
- Keep BUY and rejected/failed SELL behavior unchanged: when an intent fails before confirmation, locked tokens must still be returned.
- Update `OPEN_POSITIONS_GAUGE` after the final LockManager state, not before a later cleanup can change it.

## Erlaubte Dateien

You may modify only these files unless you find a hard compile/test necessity and explain it in the PR:

- `src/storage/locks.rs`
- `src/bin/execution_engine.rs`
- Impl-repo tests that exercise `LockManager` / execution-engine state behavior, preferably existing unit tests in the same modules or a narrowly scoped existing test file.
- Documentation only if needed to describe the behavior, but do not broaden scope.

## Verboten

- No RPC calls or wallet scans in the hot path.
- Do not reintroduce a separate `open_positions` counter.
- Do not modify `Iron_crab-eval` in this Impl PR.
- Do not change public IPC schema or public strategy contracts.
- Do not weaken simulation gating or send behavior.
- Do not change unrelated DEX, router, liquidation, WSOL-manager, or market-data behavior.
- Do not mask the symptom by changing Grafana/dashboard queries.

## Erwartete Tests

Add or update focused tests proving:

1. A SELL capital lock that is released after confirmed SELL does not re-add the sold token to `available_tokens`.
2. Failed/rejected SELL cleanup still returns locked token amounts.
3. `count_non_zero_token_balances()` / open positions remains 0 after sell-all success.
4. Existing BUY lock release / SOL or WSOL release behavior remains intact.

## Pruef-Befehle

Run at minimum:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If the full Eval suite is available in your environment, also run the IronCrab Eval Level-5 equivalent against current `ironcrab-eval`. If not available, state exactly what was not run and why.

## Production Evidence Summary

Server: `ironcrab-prod`, 2026-04-25.

- `execution-engine` metrics: `open_positions 3`, later `open_positions 4`.
- `momentum-bot` metrics: `open_positions 0`.
- Helius `getTokenAccountsByOwner`: only `So11111111111111111111111111111111111111112` (WSOL) non-zero.
- Confirmed SELL logs:
  - `19:10:22` SELL for `6qVy1...YEW8`, followed by `LockManager: cleared token balance after confirmed SELL`.
  - `19:16:51` SELL for `Ke4A...X4kw`, followed by `LockManager: cleared token balance after confirmed SELL`.
- Persistent `trade_logs/execution_state.json` later wrote the same drifted `open_positions` value, proving this is execution-engine state drift, not Grafana cache.
