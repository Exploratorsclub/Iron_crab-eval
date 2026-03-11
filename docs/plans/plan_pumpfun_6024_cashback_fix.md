# Plan: PumpFun 6024 Cashback-Upgrade Fix

**Datum:** 2026-03-11
**Status:** Impl + Eval abgeschlossen. Bereit fuer cargo test & Deployment.
**Prioritaet:** P0 — Blockiert alle PumpFun Bonding Curve Trades

## Problem

Seit dem PumpFun Cashback-Upgrade (Ende Februar 2026) schlagen alle PumpFun Bonding Curve Buy/Sell Transaktionen mit `Custom(6024)` (Overflow) fehl. Ursache: Das Upgrade fuehrte ein neues Pflicht-Account (`bonding_curve_v2` PDA) ein, das in `build_buy_ix` und `build_sell_ix` fehlt.

### Fehlercode
- `InstructionError(1, Custom(6024))` — Anchor Error "Overflow"
- Tritt bei ALLEN PumpFun Bonding Curve Trades auf, unabhaengig von Trade-Groesse
- Ursache: Ohne `bonding_curve_v2` im Account-Array liest das Programm Daten vom falschen Index → u64 Overflow

## Referenz

Vollstaendige Dokumentation des Upgrades: https://allenhark.com/blog/pumpfun-bonding-curve-custom-6024-overflow-fix-cashback-upgrade-guide

### Neue Account-Layouts (nach Upgrade)

**BUY: 17 Accounts (fuer alle Tokens gleich)**
| Index | Account | Mut | Signer |
|-------|---------|-----|--------|
| 0-15  | (wie bisher, unveraendert) | — | — |
| 16    | `bonding_curve_v2` (PDA: `["bonding-curve-v2", mint]`) | No | No |

**SELL non-cashback: 15 Accounts**
| Index | Account | Mut | Signer |
|-------|---------|-----|--------|
| 0-13  | (wie bisher, unveraendert) | — | — |
| 14    | `bonding_curve_v2` (PDA: `["bonding-curve-v2", mint]`) | No | No |

**SELL cashback-enabled: 16 Accounts**
| Index | Account | Mut | Signer |
|-------|---------|-----|--------|
| 0-13  | (wie bisher, unveraendert) | — | — |
| 14    | `user_volume_accumulator` (PDA: `["user_volume_accumulator", user]`) | Yes | No |
| 15    | `bonding_curve_v2` (PDA: `["bonding-curve-v2", mint]`) | No | No |

### cashback_enabled Flag
- Byte[82] im Bonding Curve Account Data
- `data.len() > 82 && data[82] != 0` → cashback enabled
- NICHT von Token-2022 Status ableitbar — muss direkt aus Account Data gelesen werden

### bonding_curve_v2 PDA
```
seeds = ["bonding-curve-v2", mint.as_ref()]
program = 6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P
```
Muss nicht on-chain existieren (kann uninitialisiert sein). Readonly.

## Impl-Aenderungen

### 1. BondingCurveState (src/solana/dex/pumpfun.rs)

**parse()**: Neues Feld `cashback_enabled: bool` hinzufuegen.
- Aktuell: `data.len() < 81` check → Erweiterung: Wenn `data.len() > 82`, `cashback_enabled = data[82] != 0`, sonst `false`
- Struct BondingCurveState: Neues pub Feld `cashback_enabled: bool`
- initial_bonding_curve_state(): `cashback_enabled: false` (neue Tokens starten ohne Cashback)

### 2. derive_bonding_curve_v2 (neue Funktion)

```rust
pub fn derive_bonding_curve_v2(token_mint: &Pubkey) -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(PUMPFUN_PROGRAM_ID).expect("valid pumpfun program id");
    Pubkey::find_program_address(&[b"bonding-curve-v2", token_mint.as_ref()], &program_id)
}
```

### 3. build_buy_ix: bonding_curve_v2 als 17. Account

Nach dem letzten Account (Index 15: fee_program) hinzufuegen:
```rust
// #17 (16): Bonding Curve V2 - readonly (required since Feb 2026 cashback upgrade)
AccountMeta::new_readonly(bonding_curve_v2, false),
```

### 4. build_sell_ix: cashback_enabled Parameter + bonding_curve_v2

- Neuer Parameter: `cashback_enabled: bool`
- Wenn cashback_enabled: `user_volume_accumulator` als Index 14 hinzufuegen (writable)
- `bonding_curve_v2` als LETZTES Account (Index 14 oder 15)

### 5. Caller-Updates

`build_swap_ix` (in pumpfun.rs): Bei SELL den `cashback_enabled` Wert aus dem BondingCurveState an `build_sell_ix` durchreichen.

## Bestehende Tests (Impl-Repo)

`tests/execution_pumpfun_builder.rs`:
- `test_pumpfun_build_buy_ix_pure_derivation`: Muss Account-Count auf 17 pruefen
- `test_tx_builder_supports_pumpfun_sell_pure_derivation`: Muss Account-Count pruefen

## Neue Eval-Tests

### Invariante A.22: PumpFun BUY Account Count (Post-Cashback-Upgrade)
- **Datei:** tests/invariants_pumpfun_cashback.rs
- **Invariante:** build_buy_ix() liefert genau 17 Accounts, bonding_curve_v2 ist das letzte

### Invariante A.23: PumpFun SELL Account Count (Post-Cashback-Upgrade)
- **Datei:** tests/invariants_pumpfun_cashback.rs (gleiche Datei)
- **Invariante:** build_sell_ix(cashback=false) liefert 15 Accounts; build_sell_ix(cashback=true) liefert 16 Accounts. bonding_curve_v2 ist jeweils das letzte Account.

### Invariante A.24: BondingCurveState cashback_enabled Parsing
- **Datei:** tests/invariants_pumpfun_cashback.rs
- **Invariante:** BondingCurveState::parse() liest cashback_enabled korrekt aus Byte 82
