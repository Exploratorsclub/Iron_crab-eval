# Plan: FIX-38 Removal + Market Order fuer Momentum

**Datum:** 2026-03-11
**Status:** Impl + Eval abgeschlossen. Alle Tests bestanden. Bereit fuer Deployment.
**Prioritaet:** P0 — Verhindert fehlerhafte On-Chain Transaktionen

## Hintergrund

### Problem 1: FIX-38 Simulation Bypass
FIX-38 umgeht die Simulation bei bestimmten Fehlern (Custom(2) ATA, Custom(6023) SELL Balance).
Ursache war ein Commitment-Level-Mismatch: Geyser streamt auf `processed`, Simulation nutzt Default `finalized`.
Der Bypass ist zu aggressiv — er ignoriert ALLE Sim-Fehler wenn ATA-Creation (ix 0) fehlschlaegt,
auch wenn der Swap (ix 1) strukturelle Fehler hat (z.B. Custom(6024), Custom(6002)).
Dadurch landen fehlerhafte Transaktionen on-chain und verschwenden Fees.

### Problem 2: Momentum BUY Slippage
Momentum BUY nutzt `global:buy` Instruction mit 3% Slippage (`early_max_slippage_bps=300`).
Bei Momentum-Tokens bewegen sich Preise in Sekunden um 10-50%.
BUY scheitert on-chain mit Custom(6002) "Too much SOL required".
Fuer Momentum ist Slippage-Schutz kontraproduktiv: Preis steigt = Momentum bestaetigt.

## Aenderungen

### 1. Simulation Commitment auf "processed" setzen

**Datei:** src/bin/execution_engine.rs, Funktion `simulate_transaction()`

Aktuell:
```rust
let cfg = RpcSimulateTransactionConfig {
    sig_verify: false,
    replace_recent_blockhash: true,
    ..RpcSimulateTransactionConfig::default()  // commitment = None → "finalized"
};
```

Neu:
```rust
let cfg = RpcSimulateTransactionConfig {
    sig_verify: false,
    replace_recent_blockhash: true,
    commitment: Some(CommitmentConfig::processed()),
    ..RpcSimulateTransactionConfig::default()
};
```

### 2. FIX-38 Bypass komplett entfernen

**Datei:** src/bin/execution_engine.rs, Zeilen ~7228-7288

Den gesamten FIX-38 Block entfernen. Nach `if !sim_result.success {` direkt zu
`emit_sim_failed_decision` gehen, ohne Bypass-Logik.

### 3. `build_buy_exact_sol_ix()` — Market Order fuer PumpFun BUY

**Datei:** src/solana/dex/pumpfun.rs

Neue Funktion in PumpFunDex:
- Discriminator: `[56, 252, 116, 8, 158, 223, 205, 95]` (buy_exact_sol_in)
- Data Layout: discriminator(8) + sol_amount(8) + min_tokens_out(8)
- Account Layout: Identisch zu build_buy_ix (17 Accounts inkl. bonding_curve_v2)
- Parameter: sol_amount, min_tokens_out (fuer Market Order: min_tokens_out = 1)

### 4. `build_swap_ix_async_with_slippage` — market_order Parameter

**Datei:** src/solana/dex/pumpfun.rs

Neuer Parameter `market_order: bool`. Wenn true + buy_token:
- `build_buy_exact_sol_ix(sol_amount=amount_in, min_tokens_out=1)` statt `build_buy_ix`
- Kein Slippage-Calculation noetig

### 5. tx_builder — market_order aus Intent durchreichen

**Datei:** src/execution/tx_builder.rs

`market_order` aus `intent.metadata.get("market_order")` lesen und an
`build_swap_ix_async_with_slippage` weiterreichen.

### 6. Momentum Bot — Market Order fuer BUY aktivieren

**Datei:** src/bin/momentum_bot.rs

Bei BUY-Intent: `intent.metadata.insert("market_order", "true")`.
`max_slippage_bps` kann auf 0 gesetzt werden (wird bei Market Order ignoriert).

### 7. Execution Engine — Slippage-Check fuer Market Orders skippen

**Datei:** src/bin/execution_engine.rs

Check 3b (max_slippage): Wenn `intent.metadata.get("market_order") == Some("true")`,
Slippage-Check skippen (wie bei SELL).
