WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 62: WSOL Manager Pending-Wrap Guard gegen stale Wallet Snapshots

## Task-Beschreibung

Fix the production double-wrap after deploy / kill-switch reset.

Current behavior:

- `WsolManager` has `wrap_in_progress`, but it is cleared as soon as `execute_wrap()` returns.
- After successful wrap, `execute_wrap()` optimistically subtracts SOL and adds WSOL in local atomics.
- A stale `WalletBalanceSnapshot` with the old lower WSOL value can arrive after the successful wrap and overwrite the optimistic WSOL balance.
- After cooldown, the manager sees `wsol_current=0` again and sends another wrap.

Required fix:

- Keep a pending/expected post-wrap state across snapshot lag.
- Do not allow another wrap while a previous successful wrap has not been confirmed by a fresh-enough wallet snapshot or has not timed out into an explicit authoritative resync/failure path.
- Stale snapshots lower than the expected post-wrap WSOL must not trigger another wrap.

## Runtime-Evidenz

Server revision after deploy:

```text
6c47639 fix(pumpswap): Extended SELL tail #21–#23 aus Referenz-TX propagieren (Scope 61) (#105)
```

Startup state:

```text
2026-05-01T01:20:53Z Wallet snapshot bootstrap:
sol_lamports=3249257656
wsol_lamports=Some(0)
```

WSOL config:

```text
wsol_enabled=true
wsol_min_wsol_sol=0.5
wsol_target_wsol_sol=1
wsol_max_wsol_sol=2
wsol_cooldown_secs=30
wsol_dry_run=false
```

Sequence:

```text
2026-05-01T01:23:24.168Z ResetKillSwitch
2026-05-01T01:23:24.169Z Triggered WsolManager after kill switch reset (can wrap now) sol=3.249257656 wsol=0.0
2026-05-01T01:23:24.169Z WSOL below minimum, wrapping wsol_current=0.0 wrap_amount=1.0
2026-05-01T01:23:38.231Z Wrapped SOL -> WSOL signature=2jwxg6m... amount=1.0
```

Then market-data published stale / transition snapshots:

```text
2026-05-01T01:24:33.319Z WalletBalanceSnapshot published sol_lamports=2247213376 wsol_lamports=Some(0)
2026-05-01T01:24:33.319Z WalletBalanceSnapshot published sol_lamports=2247213376 wsol_lamports=Some(1000000000)
```

At nearly the same time, WsolManager already triggered a second wrap based on stale `wsol=0`:

```text
2026-05-01T01:24:33.349Z WSOL below minimum, wrapping wsol_current=0.0 wrap_amount=1.0
2026-05-01T01:24:47.428Z Wrapped SOL -> WSOL signature=3KrM2... amount=1.0
```

Final heartbeats:

```text
2026-05-01T01:24:54Z available_wsol=1000000000 native_sol=2247213376
2026-05-01T01:25:54Z available_wsol=2000000000 native_sol=1247208377
```

Root cause:

`wrap_in_progress` only protects the tx-send window. It does not protect the post-success confirmation window where stale WalletBalanceSnapshot can overwrite the optimistic local state.

## Relevante Invarianten (Volltext)

### I-4 / I-7 Hot Path RPC-Freiheit

HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. Nie RPC in Hot Paths ohne explizite Freigabe. This scope must not add blocking RPC to normal trading hot path.

### I-5 / I-6 Cold Path Correctness

COLD PATH (Liquidation, Manual Actions, Bootstrap): RPC erlaubt. Safety und correctness vor Speed. Nie RPC aus Cold Paths entfernen um zu "optimieren", wenn dadurch safety-kritische Flows brechen. If an authoritative resync is needed after pending wrap timeout, keep it bounded and outside hot path.

### I-9 Simulation-Gate

Wenn Simulation fehlschlaegt, nie senden. This scope must not bypass simulations for trading intents. WSOL wrap tx behavior must remain explicit and auditable.

### I-20 Capital / Balance Conservation

Capital locks and wallet balances must not be double-counted or overbooked. WSOL Manager must not wrap twice from the same stale pre-wrap balance. Available WSOL and native SOL metrics must converge to actual wallet state and avoid false duplicate capital.

### WSOL Lifecycle Pattern

WSOL is not a tradeable position. WsolManager needs wallet balance updates, must not wrap under kill switch, and must avoid races with WalletBalanceSnapshot / ATA lifecycle.

## Bestehendes Pattern / Code-Kontext

Relevant current code in `src/execution/wsol_manager.rs`:

- `WsolManager` has:
  - `wsol_balance: AtomicU64`
  - `sol_balance: AtomicU64`
  - `wsol_initialized: AtomicBool`
  - `last_action_ts: AtomicU64`
  - `wrap_in_progress: AtomicBool`
- `check_and_act()`:
  - reads `wsol` and `sol`;
  - checks cooldown;
  - acquires `wrap_in_progress`;
  - if `wsol < min`, calls `execute_wrap()`.
- `execute_wrap()`:
  - sends wrap tx;
  - logs `"Wrapped SOL → WSOL"`;
  - calls `update_last_action()`;
  - optimistically `fetch_sub` native SOL and `fetch_add` WSOL.
- `apply_balance_update()` currently stores incoming snapshot directly:
  - `self.sol_balance.store(sol, ...)`
  - `self.wsol_balance.store(wsol, ...)`
  - then calls `check_and_act()`.

Problem:

The direct store in `apply_balance_update()` can replace optimistic post-wrap `wsol=1 SOL` with stale observed `wsol=0`, immediately making the manager eligible for another wrap after cooldown.

## Erlaubte Dateien

Prefer:

- `src/execution/wsol_manager.rs`
- focused unit tests in the same module

Allowed if needed:

- `src/bin/execution_engine.rs` only for logging / channel wiring if necessary
- metrics naming only if necessary

Avoid:

- unrelated trading strategy changes
- DEX connector changes
- market-data changes unless you need a narrow diagnostic only

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Kein Deploy.
- Kein globales Deaktivieren des WsolManager.
- Kein Verlassen auf arbitrary sleep-only fixes.
- Kein Zugriff auf `Iron_crab-eval`; do not clone/read/modify eval repo.

## Konkrete Anforderungen

### 1. Track pending expected WSOL after wrap success

Add state to remember a recently successful wrap that may not yet be reflected in WalletBalanceSnapshot.

Acceptable design examples:

- `pending_wrap_expected_wsol: AtomicU64`
- `pending_wrap_started_ts: AtomicU64`
- possibly `pending_wrap_amount_lamports: AtomicU64`

After successful wrap:

- set expected WSOL to at least `wsol_before + amount_lamports` or the new optimistic `wsol_balance`;
- keep this pending state until confirmed by snapshot or timeout.

### 2. Ignore / soften stale lower snapshots

When `apply_balance_update()` receives `wsol_lamports=Some(wsol)`:

- if there is a pending expected WSOL and incoming `wsol < expected`, do not let this lower value cause another wrap;
- either keep local effective WSOL at `max(wsol, expected)` or store observed separately but use effective balance for decisions;
- log a structured diagnostic, e.g. `stale_wsol_snapshot_ignored_due_to_pending_wrap`.

When incoming `wsol >= expected`:

- clear pending wrap state;
- store real observed value.

### 3. Timeout / resync behavior

If pending expected wrap is not confirmed after a bounded period, choose a clear behavior:

- do not wrap repeatedly from stale state;
- either keep blocking additional wraps until a fresh snapshot arrives, or perform one bounded authoritative resync if already present in the codebase and not hot-path;
- log warning with pending age and expected/observed values.

Do not introduce unbounded RPC loops.

### 4. Ensure no double-wrap on stale snapshot sequence

Add a unit test that models production sequence:

1. Initial balance `SOL=3.249 SOL`, `WSOL=0`.
2. Manager wraps 1 SOL and sets optimistic/pending expected WSOL.
3. Snapshot arrives with `WSOL=0`.
4. Even after cooldown, `check_and_act()` must not send a second wrap.
5. Snapshot arrives with `WSOL=1 SOL`; pending clears.

If direct tx-sending is hard to unit-test, factor the decision logic into a testable helper or use existing dry-run/test hooks.

### 5. Preserve existing desired behavior

- If no pending wrap and `wsol < min`, wrap to target.
- If `wsol > max`, still only log (no auto-unwrap), unless existing behavior says otherwise.
- Kill switch active still skips wrapping.
- Cooldown still applies, but cooldown alone is not the only duplicate-wrap protection.

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR must also pass Impl CI including Eval Level 5.

## Supervisor-Review-Fokus

- No second wrap can be triggered by a stale lower WSOL snapshot after a successful wrap.
- Pending state clears when a snapshot confirms expected WSOL.
- No hot-path RPC added.
- WsolManager remains disabled under kill switch.
- Tests cover production stale-snapshot sequence.
