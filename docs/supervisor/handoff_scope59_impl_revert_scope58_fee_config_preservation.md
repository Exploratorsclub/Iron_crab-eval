WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 59: Revert Scope 58 FeeConfig Preservation, Keep Extended SELL Layout

## Task-Beschreibung

Scope 58 hat vermutlich die falsche Hypothese gefixt. Bitte nimm Scope 58 gezielt zurueck, ohne die funktionierenden Recovery-/Extended-Layout-Teile aus Scope 100/101 zu entfernen.

Korrigierter Befund:

- Der Token `y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump` ist migriert; `pump_amm` ist korrekt.
- Der Token braucht PumpSwap Extended SELL Layout mit 24 Accounts.
- Vor Scope 58 scheiterte Liquidation mit `Custom(6023)`.
- Scope 58 hat `fee_config` aus `v14[12]` in die SELL-Instruction uebernommen.
- Nach Scope 58 scheitert derselbe Liquidation-SELL mit `Custom(3002)`.

`Custom(3002)` deutet auf Anchor Account Discriminator / falschen Account-Typ hin. Damit ist sehr wahrscheinlich, dass `v14[12]` **nicht** als SELL `fee_config`-Meta verwendet werden darf. Die globale FeeConfig `5PH...` war fuer SELL wahrscheinlich korrekt. Die alte Diagnose `fee_config_replaced=true` war ein irrefuehrendes Red Flag, nicht die Root Cause.

## Runtime-Evidenz

Server-Stand nach Scope 58:

```text
3df95b7d30b0ff2a5b60c2c09d065f007c9365b1
fix(pumpswap): preserve authoritative v14 fee_config for extended SELL (Scope 58)
```

Wallet:

```text
mint=y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
raw=16650263074
ui=16650.263074
program=Token-2022
```

ExecutionResult after Scope 58:

```json
{
  "intent_id": "liquidation-9c9dadf6-5ef8-4667-ba71-0bde74e17b9a",
  "token_mint": "y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump",
  "status": "failed",
  "error_code": "UiTransactionError(InstructionError(1, Custom(3002)))",
  "metadata": {
    "dex": "pump_amm",
    "sell_routing": "multi_pool",
    "purpose": "liquidation",
    "token_program": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
  }
}
```

Vor Scope 58 war derselbe Fehlerpfad `Custom(6023)`, nicht `3002`.

## Relevante Invarianten (Volltext)

### I-7 Hot Path RPC-Freiheit

Keine neuen blockierenden RPC-Calls im normalen Trading-Hot-Path. Dieser Scope ist ein Builder-/Cache-/Test-Fix; er darf keine neuen Hot-Path-RPCs einfuehren.

### I-9 Simulation-Gate

Keine Transaktion darf ohne erfolgreiche Simulation gesendet werden. Keine Umgehung von `Custom(3002)` oder `Custom(6023)`.

### I-24d Cold Path Recovery via market-data / JetStream

Die bestehende Cold-Path-Recovery aus PR #100/#101 bleibt bestehen. Extended-Sell-Metadaten (`sell_extended`, `third_meta`, readiness) sollen weiter aus market-data/JetStream kommen. Nur die Scope-58-Annahme, dass `v14[12]` als SELL fee_config benutzt werden muss, soll zurueckgenommen werden.

### I-13 Account-/Pool-Matching

Finale PumpSwap SELL Accounts muessen der erfolgreichen Mainnet-Referenz / Programm-Erwartung entsprechen. Nicht blind v14-Werte bewahren, wenn die SELL-Instruktion fuer ein Feld eine globale Konstante erwartet.

## Bestehende Patterns / Known Bugs

### Known Bug #14

PumpFun/PumpSwap Account-Formate unterscheiden sich. BUY, base SELL und extended SELL haben unterschiedliche Account-Layouts.

### Known Bug #35

Warnung vor falscher globaler Kanonisierung bleibt wichtig, aber nicht jedes Feld ist pool-spezifisch. Scope 58 hat gezeigt, dass `fee_config` wahrscheinlich ein Gegenbeispiel ist: globale FeeConfig kann korrekt sein, obwohl v14 eine andere FeeConfig enthaelt.

### Altes funktionierendes Pattern vor Scope 58

`build_swap_ix_from_pool_accounts` nutzte bewusst:

```rust
// CRITICAL FIX: Use the global fee_config constant instead of trusting pool_accounts.
// The fee_config is the SAME for all pools - it's a global account owned by the Fee Program.
// Observed from successful on-chain SELL (21 accounts) and BUY (23 accounts) transactions.
let fee_config = Pubkey::from_str(PUMPFUN_AMM_FEE_CONFIG)?;
let fee_program = expected_fee_program;
```

Bitte stelle dieses Verhalten fuer SELL wieder her, sofern keine echte Referenz-TX das Gegenteil beweist.

## Erlaubte Dateien

Prefer:

- `src/solana/dex/pumpfun_amm.rs`
- `src/execution/tx_builder.rs`
- `src/execution/live_pool_cache.rs`
- `src/bin/market_data.rs`

Allowed:

- focused tests in same modules

Avoid:

- `src/bin/momentum_bot.rs`
- large liquidation routing changes in `execution_engine.rs`
- public IPC struct shape changes

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Kein vollstaendiger Revert von Scope 100/101 Recovery.
- Kein Entfernen von Extended 24-Account Support.
- Kein Entfernen von `sell_extended=true` / `third_meta` Propagation.
- Kein Deploy.

## Konkrete Anforderungen

### 1. Revert Scope 58 fee_config semantics

Rueckgaengig machen:

- `build_swap_ix_from_pool_accounts` soll fuer SELL wieder die globale `PUMPFUN_AMM_FEE_CONFIG` als SELL fee_config verwenden, nicht `pool_accounts[12]`.
- `build_swap_ix` (cache-based path) soll fuer SELL ebenfalls globale FeeConfig verwenden, wenn das vor Scope 58 der Fall war.
- `live_pool_cache` / `market_data` readiness darf extended SELL nicht daran koppeln, dass `v14[12]` eine non-default FeeConfig enthaelt. Readiness fuer extended SELL soll wieder auf `third_meta` + layout ready + pool_accounts/reserves basieren, nicht auf v14 fee_config preservation.

Beibehalten:

- Extended SELL flag.
- Extended third meta.
- 24-account layout.
- Scope44 diagnostics, aber umbenennen/interpretieren als `fee_config_uses_global_constant` statt `fee_config_replaced_vs_v14` wenn hilfreich.

### 2. Tests anpassen

Entferne oder invertiere die Scope-58-Tests, die erwarten, dass `pool_accounts[12]` als SELL fee_config erhalten bleibt.

Neue Tests:

1. `pumpswap_extended_sell_uses_global_fee_config`
   - v14[12] ist absichtlich `Dcsvh...`
   - SELL account meta #19 muss `PUMPFUN_AMM_FEE_CONFIG` / `5PH...` sein
   - Account count bleibt 24 bei `sell_requires_cashback_remaining=true`
   - third_meta bleibt enthalten

2. `extended_sell_readiness_does_not_require_v14_fee_config`
   - extended=true + third_meta present + layout_ready + pool accounts present
   - readiness darf nicht wegen non-global/non-default v14[12] blockieren

3. Regression: Scope58 `Custom(3002)` guard
   - Testname/Kommentar erklaert: using v14[12] as SELL fee_config is wrong because it can provide wrong account type; builder must use global FeeConfig for SELL.

### 3. Continue true root cause investigation via diagnostics

Der urspruengliche `6023` vor Scope 58 bleibt offen. Bitte lasse/verbessere Diagnostik so, dass nach Deploy sichtbar wird:

- `sell_extended=true`
- `sell_ix_account_count=24`
- `third_meta=...`
- `fee_config=5PH...`
- `fee_program=pfee...`
- protocol_fee_recipient/protocol_fee_recipient_ta preserved status
- full sell_ix_accounts_csv

Keine neue grosse Fix-Hypothese fuer `6023` in diesem Scope. Erst die Scope-58-Regressionsaenderung rausnehmen.

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR muss ausserdem Impl CI inkl. Eval Level 5 bestehen.

## Supervisor-Review-Fokus

- `fee_config` fuer SELL ist wieder globale Konstante.
- Extended-v24 bleibt erhalten.
- Kein Hot-Path-RPC.
- Kein Simulation bypass.
- Scope-58 readiness-gate auf v14 fee_config ist entfernt oder korrigiert.
- Tests beweisen die korrigierte Semantik.
