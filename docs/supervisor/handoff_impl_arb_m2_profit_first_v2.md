# Handoff Impl M2: 2-hop Profit-First v2 (I-ARB-4..7)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und `.cursor/rules/ironcrab-core.mdc` BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

**Voraussetzung:** M1 gemergt (`pool_quote.rs` existiert, Shadow-Metriken).

## Task-Beschreibung

Milestone **M2** — 2-hop Cross-DEX wird **profit-first** via Round-Trip. Spec: `ARB_QUOTE_CONTRACT.md`, Plan `plan_arb_profit_first_rebuild.md`.

### I-ARB-4: Freshness v2 in `pool_quote.rs`

- `PoolQuote.fresh` aus:
  - `LastTradeMid`: `trade_ts` ≤ `arb_quote_trade_ttl_ms` (default 30_000, Config)
  - `ExecutableMarginal`: `state_fingerprint` (hash reserve_base|reserve_quote|active_id|bin_step) unchanged seit `as_of_ts` → TTL `arb_quote_state_ttl_ms` (default 120_000)
- Store `as_of_slot: u64` on quote from vault/trade event slot when available
- Helper `is_quote_fresh(quote, config) -> bool`

### I-ARB-5: Pool-Auswahl pro Mint

Neue Funktion z.B. `select_round_trip_pools(tracker, vaults, bins, config) -> Option<(buy_pool, sell_pool, buy_quote, sell_quote)>`:

- Pro DEX: Pools mit `fresh && quote_exact_in` available
- Waehle **ein** buy-Pool (guenstigster **ExecutableMarginal** SOL/token fuer Buy; bei nur LastTradeMid gleiche Kind-Pools)
- Waehle **ein** sell-Pool (bester Sell-Quote, anderer DEX)
- **Pairing:** `buy_quote.kind == sell_quote.kind` sonst None + Metrik `arb_two_hop_v2_incompatible_kind_total`
- **Nicht:** global min buy / max sell ohne Frische (Legacy-Bug Zeile ~1909)

### I-ARB-6: `check_arbitrage_v2`

- Round-Trip:
  ```text
  probe = config.arb_probe_lamports (default 10_000_000)
  tokens = quote_exact_in(buy, SOL→token, probe)
  sol_back = quote_exact_in(sell, token→SOL, tokens)
  profit = sol_back as i64 - probe as i64 - estimated_fees_lamports
  ```
- Reject Metriken:
  - `arb_two_hop_v2_screen_total`
  - `arb_two_hop_v2_rejected_total{reason="round_trip_unprofitable|quote_stale|incompatible_quote_kind|insufficient_pools"}`
- Opportunity nur wenn `profit >= config.min_profit_lamports` (bestehendes Feld) **und** spread/plausibility gates auf **Round-Trip**, nicht Mid-Spread
- **Kein** `spread_too_large` auf Legacy-Mids im v2-Pfad

### I-ARB-7: Feature Flag + Wiring

- `ArbConfig.arb_two_hop_v2_enabled` (default **false**)
- Control Plane: `arb_two_hop_v2_enabled`, `arb_probe_lamports`, `arb_quote_trade_ttl_ms`, `arb_quote_state_ttl_ms`
- In `check_arbitrage`: if v2 enabled → `check_arbitrage_v2` only; else Legacy unchanged
- Writer path (`spawn_blocking check_arbitrage`) unveraendert — nur innere Logik

## Relevante Invarianten (VOLLTEXT)

- **I-4**: HOT PATH GEYSER-ONLY, kein RPC.
- **I-7**: Kein RPC Hot Path.
- **I-15**: raw/ui/decimals explizit.
- **I-16**: LivePoolCache/Geyser autoritativ.
- **I-17**: Typ A — nur TradeIntent erzeugen.
- **I-9**: Simulation Gate bleibt downstream (execution); v2 erzeugt Intent wie Legacy wenn profit ok.
- **I-19**: Atomic Cross-DEX — unveraendert.
- **A.48 (Eval):** Cross-DEX nur gleicher QuoteKind; Round-Trip nicht Mid-Spread.

## Bestehendes Pattern

- `pool_quote.rs` aus M1
- Legacy `check_arbitrage` (~1846+) als Referenz fuer Opportunity struct / intent cooldown / `finalize_trade_opportunity`
- `ArbOpportunity` Felder beibehalten; `spread_bps` aus Round-Trip ableiten: `(sol_back - probe) * 10000 / probe`

## Erlaubte Dateien

- `src/arbitrage/pool_quote.rs`
- `src/bin/arb_strategy.rs`
- `src/metrics.rs` (additive)
- Tests in obigen

## Verboten

- KEIN Entfernen Legacy `comparable_price` (M5)
- KEIN market_data / execution_engine / momentum_bot
- KEIN Spread/MAX_PRICE_AGE allein lockern
- KEIN Multi-hop Aenderung (M3)
- KEIN Slot-Delta Gate (M4) — optional vorbereiten aber default off

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test arb_strategy
```

## PR-Body

- Milestone M2 I-ARB-4..7
- Flag `arb_two_hop_v2_enabled` default false
- Deploy-Hinweis: erst nach vollem M2-Batch aktivieren
