WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 63: PumpFun BUY Instruction Data muss `track_volume` serialisieren

## Task-Beschreibung

Nach Deploy von `architecture-rebuild` mit Scope 62 werden Momentum-BUY-Intents nicht erfolgreich ausgefuehrt. Es gibt zwei verschiedene Dinge in den Logs:

1. Direkt nach dem ersten Deploy-Start wurden Intents wegen persistiertem `KILL_SWITCH_ACTIVE` rejected.
2. Nach Reset / Neustart laufen reguläre Momentum-BUY-Intents bis zur Simulation, scheitern aber als PumpFun-BUY mit `Custom(6062)`.

Dieser Scope adressiert Punkt 2.

Root-Cause-Hypothese mit harter Evidenz:

- Momentum erzeugt `market_order=true` fuer BUY-Intents.
- Execution Engine baut `pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1`.
- Simulation scheitert bei PumpFun-BUY mit `Custom(6062)`.
- Die aktuelle PumpFun-IDL (`pump-public-docs/main/idl/pump.json`) definiert fuer `buy` und `buy_exact_sol_in` jeweils drei Args:
  - `buy`: `amount: u64`, `max_sol_cost: u64`, `track_volume: OptionBool`
  - `buy_exact_sol_in`: `spendable_sol_in: u64`, `min_tokens_out: u64`, `track_volume: OptionBool`
- Unser Code serialisiert in `build_buy_ix` und `build_buy_exact_sol_ix` nur Discriminator + zwei `u64`; `track_volume` fehlt.

Fix: PumpFun BUY instruction data muss das `track_volume`-Argument korrekt serialisieren. Default fuer Momentum/Hot-Path sollte explizit `false` sein, sofern kein bestehendes Pattern etwas anderes verlangt.

## Runtime-Evidenz

Deployed server commit:

```text
0fd5871 fix(wsol): Pending-Wrap-Guard gegen stale Wallet-Snapshots (Scope 62) (#106)
```

Persistierter KillSwitch direkt nach Start:

```text
2026-05-01T15:20:26Z Received TradeIntent int-c9133cdc-000000
2026-05-01T15:20:26Z Intent rejected reason=KILL_SWITCH_ACTIVE
2026-05-01T15:21:54Z Received TradeIntent int-c9133cdc-000001
2026-05-01T15:21:54Z Intent rejected reason=KILL_SWITCH_ACTIVE
```

Nach Reset / Neustart:

```text
2026-05-01T15:25:48Z Received TradeIntent int-4b4ed4cf-000000 source=momentum-bot
2026-05-01T15:25:48Z pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1 token_mint=CuGA7CMGzhKrHiqASfB7rrdTFzxY3X3GNBCso6MFpump sol_amount=1250000 min_tokens_out=1
2026-05-01T15:25:48Z Running simulation
2026-05-01T15:25:48Z Simulation failed: UiTransactionError(InstructionError(1, Custom(6062)))
```

Second identical pattern:

```text
2026-05-01T15:26:16Z Received TradeIntent int-4b4ed4cf-000001 source=momentum-bot
2026-05-01T15:26:16Z pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1 token_mint=3WzruAcKwTEFoxfzqiX7BarWjpAgGmvokEgsHrsapump sol_amount=1250000 min_tokens_out=1
2026-05-01T15:26:16Z Simulation failed: UiTransactionError(InstructionError(1, Custom(6062)))
```

Later same pattern:

```text
2026-05-01T15:40:51Z pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1 token_mint=7oFJfafNXVhkiWKkTGEPwKecV46f7La5Q7rP9ckDpump sol_amount=1250000 min_tokens_out=1
2026-05-01T15:40:51Z Simulation failed: UiTransactionError(InstructionError(1, Custom(6062)))
```

Current IDL evidence from `https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/idl/pump.json`:

```json
{
  "name": "buy_exact_sol_in",
  "docs": [
    "Given a budget of spendable SOL, buy at least min_tokens_out tokens.",
    "Fees are deducted from spendable_sol_in."
  ],
  "args": [
    { "name": "spendable_sol_in", "type": "u64" },
    { "name": "min_tokens_out", "type": "u64" },
    { "name": "track_volume", "type": { "defined": { "name": "OptionBool" } } }
  ]
}
```

The same IDL also defines `buy` args:

```json
[
  { "name": "amount", "type": "u64" },
  { "name": "max_sol_cost", "type": "u64" },
  { "name": "track_volume", "type": { "defined": { "name": "OptionBool" } } }
]
```

`OptionBool` is defined as:

```json
{
  "name": "OptionBool",
  "type": { "kind": "struct", "fields": ["bool"] }
}
```

Current code evidence:

`src/solana/dex/pumpfun.rs`:

```rust
// build_buy_exact_sol_ix data:
data.extend_from_slice(&[56, 252, 116, 8, 158, 223, 205, 95]);
data.extend_from_slice(&sol_amount.to_le_bytes());
data.extend_from_slice(&min_tokens_out.to_le_bytes());
// missing track_volume OptionBool
```

`build_buy_ix` similarly serializes only:

```rust
data.extend_from_slice(&[102, 6, 61, 18, 1, 218, 235, 234]);
data.extend_from_slice(&amount_in.to_le_bytes());
data.extend_from_slice(&max_sol_cost.to_le_bytes());
// missing track_volume OptionBool
```

Momentum currently sets:

```rust
intent.metadata.insert("market_order".to_string(), "true".to_string());
```

So the failing production path is `build_buy_exact_sol_ix`.

## Relevante Invarianten (Volltext)

### I-7 Hot Path RPC-Freiheit

HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. This fix must be pure builder serialization / tests. No new RPC.

### I-9 Simulation-Gate

Wenn Simulation fehlschlaegt, nie senden. Kein Bypass fuer `Custom(6062)`. The fix must make the instruction correct; do not special-case or bypass the simulation.

### I-12 Decision Record / Forensik

No silent drops. Keep clear sim-fail diagnostics. If useful, add a log with `track_volume=false` for PumpFun BUY builders.

### I-15 Amounts explizit

Raw/UI units must stay explicit. Do not reinterpret `amount_in`, `min_out`, or slippage units. Only append the missing ABI arg.

### I-16 Geyser/LivePoolCache autoritativ im Hot Path

Do not add RPC or runtime discovery. The token program / creator / bonding curve data must continue to come from existing Geyser/cache/intent paths.

## Bestehendes Pattern

- `build_buy_ix` and `build_buy_exact_sol_ix` already have the correct account order according to current IDL.
- They include `global_volume_accumulator`, `user_volume_accumulator`, `fee_config`, `fee_program`, and `bonding_curve_v2`.
- The missing piece is instruction data ABI: append `track_volume: OptionBool`.

Recommended conservative behavior:

- Serialize `track_volume=false` as explicit default for both BUY builders.
- If encoding is a one-field Borsh struct `OptionBool(bool)`, append one byte `0`.
- Add tests locking exact data length and final byte.
- Keep account order unchanged unless tests/current IDL reveal otherwise.

## Erlaubte Dateien

Prefer:

- `src/solana/dex/pumpfun.rs`
- focused tests in same module

Allowed if necessary:

- `src/execution/tx_builder.rs` only if caller needs explicit `track_volume` propagation
- `src/bin/momentum_bot.rs` only if you decide to make `market_order` conditional; do not do this unless necessary.

Avoid:

- WSOL Manager changes
- PumpSwap / PumpAmm code
- Eval repo changes

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Kein Deploy.
- Kein Zugriff auf `Iron_crab-eval`; do not clone/read/modify eval repo.
- Kein blindes Deaktivieren von `market_order=true` als workaround unless you prove the ABI cannot be fixed.

## Konkrete Anforderungen

1. Update PumpFun BUY instruction serialization:
   - `build_buy_ix` appends `track_volume=false`.
   - `build_buy_exact_sol_ix` appends `track_volume=false`.

2. Preserve:
   - Existing account order.
   - Token-2022 support via `token_program`.
   - `bonding_curve_v2` last account.
   - `market_order=true` behavior unless proven otherwise.

3. Tests:
   - Existing `build_buy_exact_sol_ix` / market-order tests must expect data length `25` (8 discriminator + 8 + 8 + 1).
   - Test final byte is `0` for `track_volume=false`.
   - Add/adjust equivalent `build_buy_ix` test.
   - Keep account count tests unchanged unless account layout changed.

4. Verification:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR must also pass Impl CI including Eval Level 5.

## Supervisor-Review-Fokus

- Does PumpFun BUY data match current IDL arg count?
- No new RPC in hot path.
- No simulation bypass.
- Market-order BUY still uses exact SOL in, now with explicit track_volume arg.
- Existing PumpFun sell/cashback behavior unaffected.
