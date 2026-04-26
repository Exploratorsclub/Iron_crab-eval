WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 48: Amount-Aware SELL State For Partial Exits

## Task-Beschreibung

Fix the highest-priority root cause from the 2026-04-25/26 production run: confirmed regular SELLs after probe+scale-in often sold only the probe amount, while the larger scale-in token balance remained on-chain. `execution-engine` then treated the SELL as a full close and cleared/untracked the whole mint. Follow-up Momentum sell retries for the remaining scale-in balance were rejected with `SIM_INSUFFICIENT_BALANCE` because execution state said `available=0`.

Observed examples before kill switch:

- `8pDue...`: BUY probe `25313868645`, BUY scale-in `56323355801`, regular SELL `25313868645`, residual `56323355801` stayed in wallet until liquidation.
- `DhQT...`: BUY probe `7854105469`, BUY scale-in `17768557027`, regular SELL `7854105469`, residual `17768557027` stayed in wallet.
- `2qhr...`: repeated follow-up SELLs rejected with `available=0 < required=61492576042`, while the wallet still held that residual.

Fix the execution/market-data state semantics so a confirmed SELL is amount-aware:

- If the sold amount is less than the current tracked token balance, subtract only the sold amount.
- Do not clear the whole mint and do not untrack the ATA unless the SELL actually closed the token account / sold the full current balance.
- Keep the existing behavior for true sell-all / liquidation / account-close sells.

This scope must not solve slow liquidation or Momentum's max-position gate. It only fixes the authoritative execution/data-plane state after partial confirmed SELLs.

## Relevante Invarianten (Volltext)

### A.28 Open Positions Counter Konsistenz (Single Source of Truth)

`get_open_positions()` wird aus LockManager `count_non_zero_token_balances()` abgeleitet (nicht als separater Counter). Der Wert stimmt stets mit der Anzahl non-zero Eintraege in `available_tokens` ueberein.

Formal: `get_open_positions() == available_tokens.values().filter(|b| b > 0).count()`. Nach N BUY-Fills: count == N. Nach Sell-All: count == 0. Nach Restart-Recovery: count == tatsaechlicher Bestand.

Kontext: KNOWN_BUG_PATTERNS #5 (Ghost Positions); dual-path tracking (Execution Result + Geyser Balance) verursachte Race Conditions und Counter-Drift.

### A.12 Hot-Path RPC-Freiheit (I-4, I-7)

DEX-Connectors liefern bei Cache-Miss None/Err ohne RPC (Hot Path). Hot Path (Arb, Momentum) darf keine blockierenden RPC-Calls ausfuehren.

For this scope: no RPC wallet scan or getTokenAccountsByOwner in the normal confirmed SELL handler. Use data already available in `ExecutionResult`, `TradeIntent`, fill accounting, LockManager, or market-data tracking metadata.

### I-9 Simulation-Gate

Transaktionen duerfen nicht ohne erfolgreiche Simulation gesendet werden. This scope must not change simulation/send gating.

### I-12 Decision Record

Ein Intent darf nicht ohne Decision Record verworfen werden. This scope must not introduce silent drops.

## Bestehendes Pattern / Relevante Code-Stellen

Relevant files on `architecture-rebuild`:

- `src/bin/execution_engine.rs`
  - BUY path accumulates token balance in LockManager.
  - Confirmed SELL currently calls `set_available_token_balance(mint, 0)` and logs `LockManager: cleared token balance after confirmed SELL`.
  - Scope 47 added `release_locks_after_confirmed_sell()` to avoid re-adding sold tokens after full SELL release. Preserve that fix.
- `src/storage/locks.rs`
  - `available_tokens` is the execution-engine source for `open_positions`.
  - Add a narrow helper if useful, e.g. subtract token amount with saturating semantics.
- `src/bin/market_data.rs`
  - ExecutionResult tracking/untracking currently logs `Untracked ATA after confirmed SELL`.
  - Confirmed SELL untracking must become amount-aware or keyed by explicit account-close/full-sell evidence. A partial SELL must leave the ATA/mint tracked.

Known bug patterns to preserve:

- Pattern #15 LockManager Double-Counting / SELL Race.
- Pattern #16 Token-2022 / Custom token_program.
- Pattern #24 Open Positions Counter Drift.
- Scope 47 fix: no ghost open positions after confirmed SELL lock release.

## Erlaubte Dateien

- `src/bin/execution_engine.rs`
- `src/storage/locks.rs`
- `src/bin/market_data.rs`
- Narrow unit tests in the same modules or existing test modules.
- Documentation only if needed for the behavior contract.

## Verboten

- No deploy, no `deploy.sh`, no server/systemd restart.
- No hot-path RPC.
- Do not reintroduce a separate `open_positions` counter.
- Do not change dashboard/Grafana queries to mask the issue.
- Do not change liquidation latency behavior in this scope.
- Do not change public IPC schema unless there is no private/metadata-compatible path; if a schema change seems necessary, stop and explain.

## Erwartete Tests

Add focused tests proving:

1. Given LockManager balance `probe + scale_in`, confirmed SELL with `sold_amount == probe` leaves `scale_in` balance available and `count_non_zero_token_balances() == 1`.
2. Confirmed sell-all leaves balance `0` and `count_non_zero_token_balances() == 0`.
3. Confirmed partial SELL does not cause market-data to untrack the wallet ATA/mint.
4. Confirmed full SELL / liquidation still untracks/closes tracking as before.
5. Scope 47 behavior still holds: lock release after confirmed SELL must not resurrect sold tokens.

## Pruef-Befehle

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If CI provides Eval Level 5, ensure it passes before merge.

## Production Evidence Summary

Before kill switch, execution results reconstructed these residual positions:

- `2qhr...`: probe + scale-in, regular SELL only probe, residual `61492576042`.
- `2cKp...`: residual `60422615523`.
- `8pDue...`: residual `56323355801`.
- `GkYd...`: residual `43641376221`.
- `J7jQ...`: residual `38711432312`.
- `Esgxy...`: residual `38486263385`.
- `DXMG...`: residual `31375847588`.
- `Ct4S...`: residual `21222620546`.
- `4CG7...`: residual `19604487059`.
- `DhQT...`: residual `17768557027`.

Follow-up exits repeatedly failed with `SIM_INSUFFICIENT_BALANCE` because execution-engine had cleared the balance to zero after the earlier partial SELL.
