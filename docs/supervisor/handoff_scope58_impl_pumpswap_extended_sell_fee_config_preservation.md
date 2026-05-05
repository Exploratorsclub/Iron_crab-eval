WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 58: PumpSwap Extended SELL Must Preserve Authoritative Account Values

## Task-Beschreibung

Fixe die aktuelle Liquidation fuer den migrierten Token:

`y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump`

Wichtig: Der Token ist migriert. `pump_amm` ist korrekt. Die vorherige Hypothese "aktive PumpFun BC Route" war falsch fuer diesen aktuellen Stand.

Aktueller Stand nach PR #101:

- Liquidation erkennt den Token und seedet Balance korrekt.
- `pump_amm` Quote funktioniert.
- Der erste Build nutzt `sell_extended=false` und scheitert mit `Custom(6023)`.
- Cold-path `EnsurePumpAmmPoolAccounts(force_refresh=true)` laeuft.
- Der Retry baut bereits `sell_extended=true` mit `sell_cashback_third_meta=Some(...)` und 24 Sell-Accounts.
- Trotzdem scheitert die Simulation weiter mit `Custom(6023)`.

Der entscheidende Log-Hinweis ist:

```text
v14_fee_config=DcsvhShq8ZaUyU7NtjskVRfKHW7DrSjdu7mkgpzoHyxB
sell_ix_fee_config_meta=5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx
fee_config_replaced=true
```

Das deutet darauf hin, dass `tx_builder` / `pumpfun_amm` trotz authoritative refresh echte Pool-/TX-Werte aus dem v14/extended Layout durch eine globale Konstante ersetzt. Der Scope soll beweisen und fixen, dass der finale `sell_ix_accounts_csv` fuer extended SELL die authoritative Accountwerte aus `pool_accounts` / metadata bewahrt, insbesondere `fee_config`.

## Aktuelle Runtime-Evidenz

Server:

- Commit: `dea96ba3d50560b631efdd98af9daf45e1778a2d`
- PR #101: `fix(execution-engine): liquidation sim-fail triggers full DEX discovery recovery`

Wallet:

```text
mint=y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
ata=52GrRS6s8DJ43W4KAaWmqUwmnpMFq2ZbiaqdN15eHjU6
raw=16650263074
ui=16650.263074
token_program=Token-2022
```

Liquidation log:

```text
Liquidation: seeded LockManager with RPC balance mint=y7... balance_raw=16650263074
LIQUIDATION: Intent prepared ... dex="pump_amm" routing="multi_pool" ... pools=["HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7"] quote_attempts=pump_amm=ok amount_out=402515 ... accounts_len=14
```

First build before recovery:

```text
sell_extended=false
sell_cashback_third_meta=None
fee_config_replaced=true
Simulation -> Custom(6023)
```

After force-refresh/rebuild:

```text
PumpSwap cold-path recovery: simulation failed - force-refresh pool_accounts (market-data RPC), rebuilding tx (one retry)
sell_extended=true
sell_cashback_third_meta=Some(CASRL2zkwDnppxEFQ4LgdwgR9pdz5Q8R8nEMKVZ9QoLp)
sell_ix_accounts_csv=... 24 accounts ...
v14_fee_config=DcsvhShq8ZaUyU7NtjskVRfKHW7DrSjdu7mkgpzoHyxB
sell_ix_fee_config_meta=5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx
fee_config_replaced=true
Simulation -> Custom(6023)
```

This means the missing-extended-layout problem is only partly fixed. The retry has 24 accounts, but at least one critical account differs from authoritative data.

## Relevante Invarianten (Volltext)

### I-7 Hot Path RPC-Freiheit

Momentum-/Arb-/normaler Execution-Hot-Path darf keine blockierenden RPC-Calls ausfuehren. Dieser Scope darf keine neuen RPCs in den normalen Trading-Hot-Path einfuehren. Existing force-refresh in liquidation / kill-switch is Cold Path and may remain bounded.

### I-9 Simulation-Gate

Keine Transaktion darf ohne erfolgreiche Simulation gesendet werden. Dieser Scope darf `Custom(6023)` nicht ignorieren und keine Simulation umgehen.

### I-24d Cold Path Recovery via market-data / JetStream

Cold-Path Recovery nach strukturellem PumpSwap-Sim-Fail muss authoritative Daten aus `market-data` / JetStream nutzen. Wenn market-data echte Pool-/SELL-Layout-Werte publiziert, muss execution-engine diese Werte end-to-end bis in die finalen Instruction Accounts bewahren.

### I-13 Pool-/Account-Matching

Ein SELL darf nicht mit Accountwerten aus einem anderen Pool oder mit globalisierten Annahmen gebaut werden, wenn authoritative pool-spezifische Werte existieren. Extended PumpSwap SELL muss mit dem beobachteten/authoritativen Pool-Layout uebereinstimmen.

### I-12 Sichtbare Entscheidungen

Wenn ein Account bewusst ersetzt wird, muss der Grund belegt und geloggt sein. Fuer diesen Scope ist `fee_config_replaced=true` ein Red Flag, solange nicht bewiesen ist, dass diese Ersetzung korrekt ist.

## Bestehende Patterns / Known Bugs

### Known Bug #35

`PumpSwap protocol_fee_recipient global kanonisiert statt reale Pool-/TX-Werte zu bewahren`

Symptom:

- PumpSwap Liquidation oder SELL scheitert trotz erfolgreichem Cold-Path-Refresh weiterhin mit `Custom(6023)`.
- Logs zeigen, dass `build_swap_ix_from_pool_accounts` bzw. `set_pool_from_accounts` echte Werte aktiv auf "kanonische" Werte umbiegt.

Fix-Pattern:

- Keine globale Kanonisierung fuer Werte, die aus Geyser/RPC/Referenz-TX autoritativ bekannt sind.
- Reale PumpSwap-Accountwerte end-to-end bewahren.
- Nur echte globale Konstanten normalisieren.
- Regressionstest gegen Mainnet-Sell-Referenz oder bewusst abweichenden Account-Satz.

### Current y7-specific evidence

The current log shows:

- `v14_fee_config=Dcsvh...`
- `sell_ix_fee_config_meta=5PH...`
- `fee_config_replaced=true`

Do not assume the global constant is correct for this pool. Verify against the PumpSwap program / existing parser assumptions and successful reference TXs. If the v14 source is authoritative for SELL, preserve it.

### Code anchors

Likely relevant:

- `src/solana/dex/pumpfun_amm.rs`
  - `build_swap_ix_from_pool_accounts`
  - `PUMPFUN_AMM_FEE_CONFIG`
  - any replacement/canonicalization of fee_config / protocol_fee_recipient / protocol_fee_recipient_ta
  - extended SELL account construction (`sell_requires_cashback_remaining`, trailing metas)

- `src/execution/tx_builder.rs`
  - PumpAmm branch
  - Scope44 diagnostics: `v14_csv`, `sell_ix_accounts_csv`, `fee_config_replaced`
  - `cache.pump_amm_sell_extended_layout`

- `src/execution/live_pool_cache.rs`
  - extended layout metadata storage
  - `pump_amm_sell_extended_layout`
  - readiness gates

- `src/bin/market_data.rs`
  - EnsurePumpAmmPoolAccounts publishes extended metadata
  - only if propagation still truncates authoritative account values

## Erlaubte Dateien

Prefer:

- `src/solana/dex/pumpfun_amm.rs`
- `src/execution/tx_builder.rs`
- `src/execution/live_pool_cache.rs`

Allowed if needed:

- `src/bin/market_data.rs`
- focused unit tests in the same modules

Avoid unless absolutely necessary:

- `src/bin/momentum_bot.rs`
- `src/bin/execution_engine.rs` outside diagnostics/wait-readiness glue
- Public IPC struct shape changes

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Kein "just retry more" ohne fixing the account mismatch.
- Kein Rueckbau von PR #101's force-refresh/rebuild logic.
- Kein globales Ersetzen von authoritative pool-specific account values unless proven by tests/spec.
- Kein Deploy.

## Konkrete Anforderungen

### 1. Prove the mismatch in tests

Add a focused test around the final PumpSwap SELL account construction:

- Input v14/pool accounts include a non-global `fee_config` like `Dcsvh...`.
- Extended SELL required with third meta.
- `build_swap_ix_from_pool_accounts` (or the tx_builder PumpAmm path) must produce final account metas that preserve the authoritative fee_config when that is required by the v14/extended source.
- The test should fail on current behavior where `sell_ix_fee_config_meta` becomes `5PH...` while `v14_fee_config` is `Dcsvh...`.

If the program spec truly requires the global `5PH...`, then prove it with an existing successful reference TX and explain why `Dcsvh...` is present in v14. Do not guess.

### 2. Fix final SELL account construction

For extended SELL:

- Build exactly the required account layout.
- Preserve authoritative v14/extended values end-to-end.
- Ensure `sell_ix_accounts_csv` matches the intended account sequence.
- Keep Token-2022 base token program override intact.
- Keep WSOL quote token program SPL Token.

### 3. Readiness must mean "safe to build extended SELL"

If the cache says `sell_extended=true`, readiness must require:

- third meta present
- layout ready
- pool accounts present
- and any required authoritative account values present

Do not mark ready while later builder still needs to replace values.

### 4. Diagnostics

Keep/improve Scope44 diagnostics:

- log whether fee_config/protocol_fee_recipient/protocol_fee_recipient_ta were preserved or replaced
- include reason if replacement happens
- include account count and extended flag

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR must also pass Impl CI including Eval Level 5.

## Supervisor-Review-Fokus

- Check for new RPC: none in hot path.
- Check simulation gate: no bypass.
- Check final account construction: extended true actually produces complete layout and preserves authoritative fee_config/value(s).
- Check tests use a v14 fee_config different from global constant.
- Check y7-style log mismatch would be impossible after fix (`fee_config_replaced=false` or a clearly proven/correct replacement reason).
