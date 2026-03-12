# Handoff: I-7 Verstoss in PumpFun cashback_enabled Resolution beheben

## Problem

`build_swap_ix_async_with_slippage()` in `src/solana/dex/pumpfun.rs` fuehrt bei Cache-Miss fuer `cashback_enabled` **bedingungslos** einen RPC-Call (`fetch_bonding_curve_fast()`) aus. Diese Funktion wird sowohl im **Hot Path** (Momentum, Arb via `build_tx_plan`) als auch im **Cold Path** (Liquidation) aufgerufen. RPC im Hot Path ist ein Verstoss gegen Invariante I-7.

## Relevante Invarianten (PFLICHT — nicht verletzen)

### I-4 (Hot Path = GEYSER-ONLY)
> HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain.

### I-7 (Nie RPC in Hot Paths)
> Nie RPC in Hot Paths ohne explizite Freigabe — bricht Latenz-Anforderungen. Verletzung = Architekturverletzung.

### I-5 (Cold Path = RPC erlaubt)
> COLD PATH (Liquidation, Manual Actions, Bootstrap): RPC erlaubt. Safety und correctness vor Speed.

### A.12 (Eval-Test fuer Hot-Path RPC-Freiheit)
> DEX-Connectors liefern bei Cache-Miss None/Err ohne RPC (Hot Path). Getestet: PumpFunAmmDex, Raydium, RaydiumCpmm, MeteoraDlmm (allow_rpc_on_miss=false).

## Bestehendes Pattern (MUSS verwendet werden)

Alle anderen DEX-Connectoren implementieren `allow_rpc_on_miss: bool`:

**PumpFunAmmDex** (`src/solana/dex/pumpfun_amm.rs`, Zeile 125-156):
```rust
/// When LivePoolCache is set and cache miss: if false, return None (Hot Path, no RPC).
/// If true, fall back to RPC discovery (Cold Path, e.g. Liquidation). P3 #12.
allow_rpc_on_miss: bool,

pub fn new_with_cache(rpc, live_pool_cache, allow_rpc_on_miss: bool) -> Self { ... }
```

**build_tx_plan** (`src/execution/tx_builder.rs`, Zeile 240-248):
```rust
/// `allow_rpc_fallback`: When true (Cold Path), Raydium may use RPC on cache miss.
/// When false (Hot Path), reject on cache miss (GEYSER-ONLY).
pub async fn build_tx_plan(
    intent, wallet_pubkey, rpc, cache, sell_balance_hint,
    allow_rpc_fallback: bool,   // <-- wird an Raydium, Orca, Meteora durchgereicht
) -> TxPlanOutcome
```

`build_tx_plan` gibt `allow_rpc_fallback` an Raydium, Orca, Meteora weiter — aber NICHT an PumpFun `build_swap_ix_async_with_slippage`.

## Aufgabe

1. Fuege `allow_rpc_fallback: bool` als Parameter zu `build_swap_ix_async_with_slippage()` hinzu
2. Mache den `fetch_bonding_curve_fast()` RPC-Fallback fuer `cashback_enabled` BEDINGT: nur wenn `allow_rpc_fallback == true`
3. Bei `allow_rpc_fallback == false` (Hot Path) und Cache-Miss: `cashback_enabled = false` (wie `unwrap_or(false)` — Simulation faengt falsche Account-Layouts ab)
4. Aktualisiere alle Aufrufstellen:
   - Convenience-Wrapper `build_swap_ix_async()` in pumpfun.rs: `false` uebergeben (Hot Path Default)
   - `build_tx_plan` PumpFun-Abschnitt in tx_builder.rs: `allow_rpc_fallback` durchreichen
   - Tests in `tests/execution_pumpfun_builder.rs`: `false` uebergeben

## Erlaubte Dateien
- `src/solana/dex/pumpfun.rs`
- `src/execution/tx_builder.rs`
- `tests/execution_pumpfun_builder.rs`

## VERBOTEN
- Keine neuen RPC-Calls in Funktionen die vom Hot Path aufgerufen werden
- Keine Aenderungen an anderen DEX-Connectoren
- Keine Aenderungen an der `build_tx_plan` Signatur (die hat `allow_rpc_fallback` bereits)

## Pruef-Befehle (vor Abschluss ausfuehren)
```
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
