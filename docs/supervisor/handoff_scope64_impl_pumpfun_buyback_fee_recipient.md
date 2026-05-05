WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 64: PumpFun Bonding-Curve BUY/SELL braucht Buyback Fee Recipient Account

## Task-Beschreibung

Nach Scope 63 ist der urspruengliche ABI-Fehler fuer PumpFun BUY-Daten behoben: der Produktionsserver laeuft auf:

```text
05adde2 Scope 63: PumpFun BUY instruction data includes track_volume (OptionBool) (#107)
```

Die aktuelle Simulation scheitert aber weiter bei regulären Momentum-BUY-Intents auf Pump.fun Bonding Curve. Die neue harte Evidenz aus `trade_logs/decisions/decision_records-20260501.jsonl` zeigt:

```text
Intent: int-c8ed26aa-000002
Token: CAxvMhF895FwQBwbYCnqtXMZufcpq55p7H8nHgbZpump
Instruction 0: AToken CreateIdempotent -> success
Instruction 1: Pump.fun BuyExactSolIn -> fails
Error: UiTransactionError(InstructionError(1, Custom(6062)))
AnchorError thrown in programs/pump/src/sell.rs:153.
Error Code: BuybackFeeRecipientMissing.
Error Number: 6062.
Error Message: BuybackFeeRecipientMissing.
```

Zweites identisches Beispiel:

```text
Intent: int-c8ed26aa-000004
Token: ALRHJLWuPaLABMuGXR3iHUFnb9dS44YYmxqTMixmZrcA
Instruction 0: AToken CreateIdempotent -> success
Instruction 1: Pump.fun BuyExactSolIn -> fails
Error: UiTransactionError(InstructionError(1, Custom(6062)))
AnchorError: BuybackFeeRecipientMissing
```

Root Cause: Pump.fun hat am 2026-04-28 ein Breaking Fee Recipient Upgrade fuer Bonding-Curve und AMM ausgerollt. Fuer Bonding Curve muss bei BUY und SELL ein neuer Buyback/Fee-Recipient Account nach `bonding_curve_v2` angehaengt werden. Unser PumpFun Bonding-Curve Builder hat nach Scope 63 weiterhin nur 17 Accounts fuer BUY; erforderlich sind jetzt 18 Accounts.

## Externe Protokoll-Evidenz

Quelle: `https://raw.githubusercontent.com/pump-fun/pump-public-docs/main/docs/BREAKING_FEE_RECIPIENT.md`

Relevante Auszuege:

```text
As part of this change,
- Bonding curve: A new account is required to be added in buys and sells
- AMM: 2 new accounts required to be added in buys and sells

Bonding Curve
- Add any one of the 8 new fee recipients at the end of the buy and sell instructions.
- This new account should be added AFTER the bonding-curve-v2 account for both buys and sells.
- Account needs to be mutable
- Buy instructions should have 18 accounts in total
- Sell instruction should have 16 accounts for non cashback coins
- Sell instruction should have 17 accounts for cashback coins
```

Die 8 dokumentierten neuen Fee Recipients:

```text
5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD
9M4giFFMxmFGXtc3feFzRai56WbBqehoSeRE5GK7gf7
GXPFM2caqTtQYC2cJ5yJRi9VDkpsYZXzYdwYpGnLmtDL
3BpXnfJaUTiwXnJNe7Ej1rcbzqTTQUvLShZaWazebsVR
5cjcW9wExnJJiqgLjq7DEG75Pm6JBgE1hNv4B2vHXUW6
EHAAiTxcdDwQ3U4bU6YcMsQGaekdzLS3B5SmYo46kJtL
5eHhjP8JaYkz83CWwvGU2uMUXefd3AazWGx4gpcuEEYD
A7hAgCzFw14fejgCp387JUJRMNyz4j89JKnhtKU8piqW
```

## Relevante Invarianten (Volltext)

### I-7 Hot Path RPC-Freiheit

HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. Dieser Fix darf keine RPC-Calls im BUY/SELL-Hot-Path einfuehren. Es geht um statisches Instruction-Account-Layout, nicht um Runtime-Discovery.

### I-9 Simulation-Gate

Wenn Simulation fehlschlaegt, nie senden. Kein Bypass fuer `Custom(6062)` und kein Special-Casing von `BuybackFeeRecipientMissing`. Der Fix muss die Instruction korrekt bauen, sodass die bestehende Simulation gruen werden kann.

### I-12 Decision Record / Forensik

No silent drops. Sim-Fails muessen weiterhin als Decision Records / Execution Results sichtbar bleiben. Wenn ihr Diagnostics verbessert, dann so, dass `BuybackFeeRecipientMissing` / fehlender Fee-Recipient besser erkennbar wird. Keine Entfernung der aktuellen Sim-Fail-Logs.

### I-15 Amounts explizit

Raw/UI units muessen explizit bleiben. `amount_in`, `min_tokens_out`, `max_sol_cost`, Slippage und `track_volume` duerfen in diesem Scope nicht semantisch geaendert werden. Nur Account-Layout ergaenzen.

### I-16 Geyser/LivePoolCache autoritativ im Hot Path

Token-Programm, Creator, Bonding-Curve-Daten und Cache-Daten muessen weiterhin aus bestehenden Geyser-/LivePoolCache-/Intent-Pfaden kommen. Kein neuer RPC-Fallback fuer Momentum BUY/SELL.

## Bestehendes Pattern / aktueller Stand

Scope 63 hat bereits `track_volume=false` an `build_buy_ix` und `build_buy_exact_sol_ix` angehaengt. Auf dem Server:

```text
build_buy_ix data capacity = 25
build_buy_exact_sol_ix data capacity = 25
data.push(TRACK_VOLUME_OPTION_BOOL_FALSE)
```

Das nicht wieder zurueckbauen.

Aktueller PumpFun Bonding-Curve BUY-Account-Satz in `src/solana/dex/pumpfun.rs` endet sinngemaess so:

```rust
AccountMeta::new_readonly(global_volume_accumulator, false),
AccountMeta::new(user_volume_accumulator, false),
AccountMeta::new_readonly(fee_config, false),
AccountMeta::new_readonly(fee_program, false),
AccountMeta::new_readonly(bonding_curve_v2, false),
```

Neues Pattern fuer Bonding-Curve BUY:

```rust
AccountMeta::new_readonly(bonding_curve_v2, false),
AccountMeta::new(PUMPFUN_BUYBACK_FEE_RECIPIENT, false),
```

Der neue Account muss laut Pump-Doku mutable sein und nach `bonding_curve_v2` kommen. Fuer BUY ergibt das 18 Accounts total.

Fuer SELL ebenfalls nach `bonding_curve_v2` anhaengen:

```rust
// existing SELL tail:
if cashback_enabled {
    accounts.push(AccountMeta::new(user_volume_accumulator, false));
}
accounts.push(AccountMeta::new_readonly(bonding_curve_v2, false));

// new final tail:
accounts.push(AccountMeta::new(PUMPFUN_BUYBACK_FEE_RECIPIENT, false));
```

Nicht mit PumpSwap verwechseln: PumpSwap AMM hat laut Breaking-Doku ein anderes 2-Account-Tail-Pattern. Dieser Scope ist fuer PumpFun Bonding Curve (`src/solana/dex/pumpfun.rs`). PumpSwap nur anfassen, wenn bestehende Tests durch gemeinsam genutzte Konstanten sinnvoll profitieren; keine AMM-Layout-Aenderung in diesem Scope.

## Erlaubte Dateien

Prefer:

- `src/solana/dex/pumpfun.rs`
- fokussierte Tests im selben Modul oder bestehenden PumpFun-Testmodulen

Allowed if necessary:

- `docs/KNOWN_BUG_PATTERNS.md` nur wenn ihr das Pattern dokumentieren wollt

Avoid:

- `src/solana/dex/pumpfun_amm.rs`
- `src/execution/tx_builder.rs`
- `src/bin/momentum_bot.rs`
- WSOL Manager
- Eval-Repo

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Kein Deploy.
- Kein Zugriff auf `Iron_crab-eval`; do not clone/read/modify eval repo.
- Kein Rueckbau von Scope 63 (`track_volume=false` bleibt erhalten).
- Kein Deaktivieren von `market_order=true` als Workaround.
- Keine dynamische Auswahl per RPC im Hot Path. Waehlt konservativ eine der 8 dokumentierten Fee-Recipient-Adressen als statische Konstante, sofern kein bestehender lokaler Pattern eine deterministische Auswahl vorgibt.

## Konkrete Anforderungen

1. Fuege eine statische Konstante fuer einen dokumentierten Buyback/Fee-Recipient hinzu, z.B.:

```rust
pub const PUMPFUN_BUYBACK_FEE_RECIPIENT: &str =
    "5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD";
```

2. `build_buy_ix`:

- Bestehende Daten-Serialisierung inkl. `track_volume=false` unveraendert lassen.
- Nach `bonding_curve_v2` den neuen Buyback/Fee-Recipient als writable Account anhaengen.
- Erwartete Account-Anzahl: 18.

3. `build_buy_exact_sol_ix`:

- Bestehende Daten-Serialisierung inkl. `track_volume=false` unveraendert lassen.
- Nach `bonding_curve_v2` den neuen Buyback/Fee-Recipient als writable Account anhaengen.
- Erwartete Account-Anzahl: 18.

4. `build_sell_ix`:

- Bestehende SELL-Daten unveraendert lassen.
- Bestehende Cashback-Logik unveraendert lassen.
- Nach `bonding_curve_v2` den neuen Buyback/Fee-Recipient als writable Account anhaengen.
- Erwartete Account-Anzahl:
  - non-cashback: 16
  - cashback: 17

5. Tests:

- BUY exact-sol-in Test: Account count ist 18, letzter Account ist der konfigurierte Buyback/Fee-Recipient, writable, not signer.
- BUY amount/max-cost Test: Account count ist 18, letzter Account ist der konfigurierte Buyback/Fee-Recipient, writable, not signer.
- SELL non-cashback Test: Account count ist 16, letzter Account ist der konfigurierte Buyback/Fee-Recipient, writable, not signer.
- SELL cashback Test: Account count ist 17, letzter Account ist der konfigurierte Buyback/Fee-Recipient, writable, not signer.
- Bestehende Scope-63-Tests fuer data length `25` und final byte `0` muessen weiter bestehen.

6. Optional, wenn einfach:

- Sim-Fail-Diagnostics koennen `BuybackFeeRecipientMissing` besser benennen, aber nur ohne neue Hot-Path-Abhaengigkeit.

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR muss ausserdem Impl-CI inklusive Eval Level 5 bestehen.

## Supervisor-Review-Fokus

- Hat PumpFun Bonding-Curve BUY jetzt 18 Accounts?
- Hat PumpFun Bonding-Curve SELL jetzt 16/17 Accounts?
- Ist der neue Account wirklich nach `bonding_curve_v2` und writable?
- Ist Scope 63 (`track_volume=false`) unveraendert erhalten?
- Kein RPC im Hot Path.
- Kein Simulation-Bypass.
- Keine PumpSwap-AMM-Vermischung.

## Memory / bekannte Failure-Patterns

Open-Brain Failure Pattern wurde gespeichert:

```text
Pump.fun Bonding-Curve BUY/Sell simulation failure after Apr 28 2026 fee-recipient upgrade:
UiTransactionError(InstructionError(1, Custom(6062))) with AnchorError BuybackFeeRecipientMissing.
Root cause: missing newly-required buyback fee recipient remaining account after bonding_curve_v2.
Fix: append one of the documented 8 fee recipients as writable after bonding_curve_v2.
```

`docs/KNOWN_BUG_PATTERNS.md` enthaelt verwandte PumpSwap-/Token-2022-Patterns:

- Bug #29: statische Token-Program-Annahme in PumpSwap statt dynamischer Token-2022-Aufloesung.
- Bug #32: Hot-/Cold-Path darf nicht durch ungeeignete RPC-Fallbacks oder Validator-Index-Abhaengigkeiten gebrochen werden.

Dieses Scope ist ein neues Bonding-Curve-Layout-Pattern, nicht dieselbe Root Cause wie #29/#32.
