# Handoff Impl M1: pool_quote Foundation + Shadow (I-ARB-1..3)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und `.cursor/rules/ironcrab-core.mdc` BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

## Task-Beschreibung

Milestone **M1** aus `Iron_crab-eval/docs/plans/plan_arb_profit_first_rebuild.md`: Profit-First Arb Rebuild — Fundament.

Implementiere **eine** Quote-Schicht `src/arbitrage/pool_quote.rs` gemaess `Iron_crab-eval/docs/spec/ARB_QUOTE_CONTRACT.md` (nach Merge auf eval `main` oder als Handoff-Referenz).

**Legacy `check_arbitrage` bleibt autoritativ.** Neuer Code nur als Library + Shadow-Metriken (Flag default off).

### I-ARB-1: Modul `pool_quote.rs`

- Export in `src/arbitrage/mod.rs`
- Types: `QuoteKind` (`ExecutableMarginal`, `LastTradeMid`), `PoolQuote`, `QuoteSide` (Buy/Sell)
- API:
  - `quote_exact_in(...)` → `Option<PoolQuote>` mit `amount_out` raw
  - `quote_sol_per_token_for_screening(...)` optional helper
- DEX-Support (SOL-quoted only, wie bestehend):
  - **pump_amm / raydium / orca / meteora_cpmm:** CPMM auf Vault-Reserves (`sol_quoted_vault_reserves` Pattern aus `arb_strategy.rs`)
  - **meteora_dlmm:** Bin-Walker — **extrahiere** aus `arb_strategy.rs`: `dlmm_token_output_from_bins`, `dlmm_sol_output_from_bins`, `DLMM_PROBE_SOL_LAMPORTS` (10_000_000), `dlmm_marginal_price_plausible`, `flatten_bin_array_cache`
- Prioritaet innerhalb eines Pools (Contract):
  1. `ExecutableMarginal` wenn Reserves/Bins plausibel
  2. `LastTradeMid` aus `trade_price_buy/sell` wenn frisch
- Freshness v1 (M1 minimal): Trade TTL 30s; State TTL 120s wenn `vault.updated_at` + unveraenderter reserve snapshot (reuse `VaultBalanceCache` fields)
- **Kein RPC**

### I-ARB-2: Unit-Tests

- Tests in `src/arbitrage/pool_quote.rs` `#[cfg(test)]`
- Mindestens:
  1. pump_amm CPMM round numbers
  2. meteora DLMM marginal vs reserve mid divergence bounded
  3. `QuoteKind` pairing helper: `quotes_pairable(a,b)` true iff same kind
  4. stale trade → falls back or None
- Bestehende `arb_strategy` comparable_price Tests duerfen gruen bleiben (noch nicht entfernen)

### I-ARB-3: Shadow in `check_arbitrage`

- In `build_eligibility_breakdown` oder am Ende von `check_arbitrage` (vor Legacy return):
  - Wenn `config.arb_quote_shadow_mode == true` (neues Feld `ArbConfig`, default **false**, Control Plane key `arb_quote_shadow_mode`):
  - Berechne parallel Round-Trip Profit via `pool_quote` fuer bestes frisches Paar (gleicher QuoteKind)
  - Inkrementiere Metriken (additive in `metrics.rs`):
    - `arb_quote_shadow_round_trip_total`
    - `arb_quote_shadow_round_trip_profit_lamports` (Histogram oder counter buckets)
    - `arb_quote_shadow_incompatible_kind_total`
    - `arb_quote_shadow_legacy_spread_bps` vs `arb_quote_shadow_v2_profit_lamports` (optional gauge)
- **Legacy path unveraendert autoritativ** — Shadow darf Opportunities **nicht** erzeugen

## Relevante Invarianten (VOLLTEXT)

- **I-4**: HOT PATH: GEYSER-ONLY. Keine blockierenden RPC-Calls.
- **I-7**: Nie RPC in Hot Paths ohne explizite Freigabe.
- **I-15**: Amounts explizit: raw vs ui und decimals. Keine impliziten Konventionen.
- **I-16**: Geyser/LivePoolCache autoritativ im Hot Path.
- **I-17**: Typ A Arbitrage: marktgetrieben, erzeugt nur TradeIntent.
- **I-9**: Simulation vor Send (unveraendert; M1 erzeugt keine neuen Intents aus Shadow).

## Bestehendes Pattern

Extrahiere aus `src/bin/arb_strategy.rs` (~559-665):

```rust
// comparable_price_sol_per_token — DLMM marginal > reserve mid > trade mid
// dlmm_token_output_from_bins, DLMM_PROBE_SOL_LAMPORTS = 10_000_000
// is_plausible_sol_per_token_price, reserves_plausible_for_comparable_price
```

Known Bug Pattern #12: keine hardcoded SOL-Quote; quote_mint poolseitig.

## Erlaubte Dateien

- `src/arbitrage/pool_quote.rs` (neu)
- `src/arbitrage/mod.rs`
- `src/bin/arb_strategy.rs` (Shadow + Config field + Control Plane parse)
- `src/metrics.rs` (additive Metriken only)
- Unit-Tests in obigen Dateien

## Verboten

- KEINE Aenderung an `check_arbitrage` Legacy-Entscheidung (Spread/Opportunity) ausser Shadow-Block
- KEINE Aenderung `market_data.rs`, `execution_engine.rs`, `momentum_bot.rs`
- KEIN Entfernen von `comparable_price_sol_per_token` (M5)
- KEINE RPC-Calls
- KEIN Lockern von `MAX_PRICE_AGE_MS`, `max_spread`, `min_profit`
- KEIN `arb_two_hop_v2_enabled` in M1 (kommt M2)

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test pool_quote
```

## PR-Body Pflicht

- Milestone M1 I-ARB-1..3
- Shadow default off; Legacy autoritativ
- Link Plan + ARB_QUOTE_CONTRACT
- Eval Level 5 CI muss gruen sein
