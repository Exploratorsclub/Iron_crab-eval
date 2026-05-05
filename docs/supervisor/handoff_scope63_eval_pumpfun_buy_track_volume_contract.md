WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/eval-test-authority.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss.

# Scope 63 Eval: PumpFun BUY `track_volume` Contract

## Invariante die getestet wird

### Aktualisierte Invariante A.25 PumpFun Market Order BUY (`buy_exact_sol_in`)

`build_buy_exact_sol_ix()` liefert genau 17 Accounts. Das letzte Account ist die `bonding_curve_v2` PDA. Instruction-Data beginnt mit dem Discriminator `[56, 252, 116, 8, 158, 223, 205, 95]`. `spendable_sol_in` und `min_tokens_out` werden als little-endian `u64` korrekt serialisiert. Die aktuelle PumpFun-IDL verlangt danach ein drittes Argument `track_volume: OptionBool`; fuer den IronCrab-Hot-Path ist der Default `track_volume=false`, serialisiert als finales Byte `0`.

Formal:

- `ix.accounts.len() == 17`
- `ix.accounts.last().unwrap().pubkey == PDA(["bonding-curve-v2", mint], pumpfun_program)`
- `!ix.accounts.last().unwrap().is_signer`
- `!ix.accounts.last().unwrap().is_writable`
- `ix.data[0..8] == [56, 252, 116, 8, 158, 223, 205, 95]`
- `u64::from_le_bytes(ix.data[8..16]) == spendable_sol_in`
- `u64::from_le_bytes(ix.data[16..24]) == min_tokens_out`
- `ix.data.len() == 25`
- `ix.data[24] == 0` (`track_volume=false`)

### Aktualisierte Invariante fuer regulären PumpFun BUY (`global:buy`)

`build_buy_ix()` nutzt weiterhin den `global:buy` Discriminator `[0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea]`, hat weiterhin dasselbe 17-Account Layout wie `buy_exact_sol_in`, und serialisiert nach `amount` und `max_sol_cost` ebenfalls `track_volume=false` als finales Byte `0`.

Formal:

- `ix.data[0..8] == [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea]`
- `u64::from_le_bytes(ix.data[8..16]) == amount`
- `u64::from_le_bytes(ix.data[16..24]) == max_sol_cost`
- `ix.data.len() == 25`
- `ix.data[24] == 0` (`track_volume=false`)

## Warum diese Eval-Aenderung fachlich korrekt ist

Aktuelle Runtime-Regressions-Evidenz auf `Iron_crab`:

```text
2026-05-01T15:25:48Z Received TradeIntent int-4b4ed4cf-000000 source=momentum-bot
2026-05-01T15:25:48Z pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1
2026-05-01T15:25:48Z Simulation failed: UiTransactionError(InstructionError(1, Custom(6062)))
```

Weitere gleiche Failures:

```text
2026-05-01T15:26:16Z pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1
2026-05-01T15:26:16Z Simulation failed: UiTransactionError(InstructionError(1, Custom(6062)))

2026-05-01T15:40:51Z pump.fun MARKET ORDER BUY: exact SOL in, min tokens out = 1
2026-05-01T15:40:51Z Simulation failed: UiTransactionError(InstructionError(1, Custom(6062)))
```

Aktuelle offizielle PumpFun-IDL (`pump-fun/pump-public-docs/main/idl/pump.json`) definiert fuer `buy_exact_sol_in`:

```json
{
  "name": "buy_exact_sol_in",
  "args": [
    { "name": "spendable_sol_in", "type": "u64" },
    { "name": "min_tokens_out", "type": "u64" },
    { "name": "track_volume", "type": { "defined": { "name": "OptionBool" } } }
  ]
}
```

Die gleiche IDL definiert fuer `buy`:

```json
[
  { "name": "amount", "type": "u64" },
  { "name": "max_sol_cost", "type": "u64" },
  { "name": "track_volume", "type": { "defined": { "name": "OptionBool" } } }
]
```

`OptionBool`:

```json
{
  "name": "OptionBool",
  "type": { "kind": "struct", "fields": ["bool"] }
}
```

Impl PR #107 (`Exploratorsclub/Iron_crab`, branch `cursor/scope63-pumpfun-buy-track-volume-arg`) aktualisiert die Impl auf 25 Byte und setzt `track_volume=false`.

Der aktuelle Eval-Test ist deshalb fachlich veraltet:

```text
tests/invariants_pumpfun_market_order.rs:64
assertion failed: Data must be 24 bytes (8 disc + 8 sol + 8 min_tokens)
left: 25
right: 24
```

## Zieldateien

Bitte aendern:

- `tests/invariants_pumpfun_market_order.rs`
- `docs/spec/INVARIANTS.md`

Optional, nur wenn lokal etabliert:

- `docs/Tests_todo.md` fuer eine kurze Notiz, dass A.25 auf aktuelle PumpFun-IDL `track_volume` erweitert wurde.

## Konkrete Anforderungen

### 1. Test `market_order_buy_has_17_accounts`

Aktuell erwartet der Test 24 Bytes. Bitte aktualisieren:

- Erwartung `ix.data.len() == 25`
- Fehlermeldung: `8 disc + 8 spendable_sol_in + 8 min_tokens_out + 1 track_volume OptionBool`
- zusaetzlich `ix.data[24] == 0`

### 2. Test `market_order_buy_data_serialization`

Bitte unveraendert sicherstellen:

- `spendable_sol_in` aus `ix.data[8..16]`
- `min_tokens_out` aus `ix.data[16..24]`

und ergaenzen:

- `ix.data[24] == 0` fuer `track_volume=false`
- optional `ix.data.len() == 25`

### 3. Test fuer regulären `build_buy_ix`

In `regular_buy_uses_different_discriminator` oder einem neuen Test:

- `ix.data.len() == 25`
- `ix.data[24] == 0`
- `u64::from_le_bytes(ix.data[8..16])` entspricht `amount`
- `u64::from_le_bytes(ix.data[16..24])` entspricht `max_sol_cost`

### 4. Spec aktualisieren

In `docs/spec/INVARIANTS.md`:

- A.25 Text von "genau 17 Accounts ... Instruction-Data beginnt mit Discriminator ... sol_amount/min_tokens_out serialisiert" erweitern auf aktuelle IDL:
  - `ix.data.len() == 25`
  - Byte 24 ist `track_volume=false` / `0`
- Falls A.26 nur BondingCurveV2 Position betrifft, unveraendert lassen.

## Verboten

- Kein Aendern von Impl-Code im Eval-Repo.
- Kein Versuch, den Impl-PR zu umgehen oder die Eval-Suite abzuschwaechen.
- Kein Entfernen der 17-Account-Pruefung.
- Kein Entfernen der Discriminator-Pruefung.
- Kein Akzeptieren von 24 **oder** 25; der neue Contract ist 25.
- Kein `git clone` von `Iron_crab`; der Eval-Agent arbeitet nur im Eval-Repo und nutzt die oben gegebene Evidenz.

## Pruef-Befehle

Mindestens Eval-Repo-schlankes Gate:

```bash
cargo fmt -p ironcrab-eval -- --check
cargo check
cargo build
cargo clippy -p ironcrab-eval
```

Wenn moeglich zusaetzlich:

```bash
cargo test -p ironcrab-eval --test invariants_pumpfun_market_order
```

Hinweis: Vollstaendige Eval-Suite laeuft kanonisch im Impl-PR #107 als `Eval (Level 5)` nach Merge dieses Eval-PRs.

## Supervisor-Review-Fokus

- Eval-Contract folgt aktueller PumpFun-IDL.
- `buy_exact_sol_in` ist 25 Byte, nicht 24.
- Finaler `track_volume=false` Byte wird explizit geprueft.
- Account-Layout und `bonding_curve_v2` bleiben unveraendert.
- Keine Impl-Code-Aenderungen im Eval-Repo.
