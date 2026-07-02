# ARB Quote Contract (Profit-First)

**Status:** Entwurf — Milestone M0  
**Plan:** `docs/plans/plan_arb_profit_first_rebuild.md`  
**Scope:** Arb-Modul (`arb-strategy`, `src/arbitrage/*`)  
**Invariante:** A.48 (Eval, ab E-ARB-2)

---

## 1. Zweck

Cross-DEX-Arb vergleicht **keine Mid-Preise unterschiedlicher Herkunft**.  
Screening beantwortet nur:

> Bei Probe-Size `amount_in` (SOL): liefert Route A→B→A mehr SOL zurück als `amount_in + fees`?

Execution-Wahrheit bleibt **I-9 Simulation** (unverändert).

---

## 2. QuoteKind

| Kind | Definition | Wann erlaubt |
|------|------------|--------------|
| `ExecutableMarginal` | `quote_exact_in(pool, mint_in, mint_out, amount)` mit program-nah Math (CPMM, DLMM Bin-Walker) | Reserves/Bins vorhanden und fresh |
| `LastTradeMid` | Letzter SOL-quoted Trade ≤ `arb_quote_trade_ttl_ms` | Kein ExecutableMarginal **oder** expliziter Trade-only-Modus (Coverage-Lücke) |

**Verboten für Cross-DEX 2-hop Pairing:**

- `ExecutableMarginal` ↔ `LastTradeMid`
- Reserve-Mid ohne Size als Screening-Quote
- Trade auf Pool A vs Reserve auf Pool B ohne Slot/State-Kohärenz

---

## 3. PoolQuote (Struktur)

```text
PoolQuote {
  pool_address,
  dex,
  kind: QuoteKind,
  as_of_slot: u64,
  as_of_ts: Instant,
  fresh: bool,           // derived from state/trade TTL rules
  amount_in,             // probe lamports (buy leg)
  amount_out,            // tokens or SOL on return leg
}
```

Freshness:

- **Trade:** `now - trade_ts ≤ arb_quote_trade_ttl_ms` (default 30s)
- **State:** Vault/Bin `state_version` unverändert seit `as_of_ts` → gültig bis `arb_quote_state_ttl_ms` (default 120s)
- Ruhe ≠ stale, solange State unverändert

---

## 4. Round-Trip 2-hop (Cross-DEX)

Für Mint `M`, Probe `P` lamports:

```text
tokens = quote_exact_in(buy_pool,  SOL → M, P)   // ExecutableMarginal oder LastTradeMid
sol_back = quote_exact_in(sell_pool, M → SOL, tokens)
profit = sol_back - P - estimated_tx_fees
```

**Pairing-Regel:** `buy_pool.kind == sell_pool.kind`  
**Slot-Regel (M4):** `|buy.as_of_slot - sell.as_of_slot| ≤ arb_max_leg_slot_delta` (default 2)

Reject reasons (Metriken):

- `incompatible_quote_kind`
- `round_trip_unprofitable`
- `quote_stale`
- `slot_delta_exceeded`

**Kein** `spread_too_large` auf Mid-Preisen im v2-Pfad.

---

## 5. Pool-Auswahl pro Mint

Pro DEX maximal ein Pool in den Round-Trip:

- Filter: `fresh && quote_available`
- Rank: jüngster `as_of_slot`, dann höchste Liquidität
- **Nicht:** global günstigster Mid über alle Pools ignorierend Frische

---

## 6. Multi-hop

Gleiche `quote_exact_in` Implementierung wie 2-hop (`pool_quote.rs`).  
Graph-Expansion nur über Pools mit `quote_ready` und erlaubtem `QuoteKind`.  
Cycle-Profit = produkt der Hop-Quotes minus Fees — kein CP-Approx für DLMM.

---

## 7. Out of Scope

- Änderung Jito / execution_engine Send-Pfad
- market_data Pin-Implementierung (nur arb.track_requests Publish-Timing)
- Spread-/Profit-Schwellen-Tuning als Ersatz für Quote-Fix

---

## 8. Eval-Gates (Referenz)

| Test | Milestone |
|------|-----------|
| `invariants_arb_quote_contract.rs` | M1 |
| 2-hop no Cross-Kind | M2 (E-ARB-2) |
| Multi-hop unified quoter | M3 (E-ARB-3) |
