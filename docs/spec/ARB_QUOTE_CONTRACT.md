# ARB Quote Contract (Profit-First)

**Stand:** 2026-08-22 — **geltender Vertrag**, nicht Entwurf.

Eval: `tests/invariants_arb_quote_contract.rs`, `tests/invariants_tx_account_hard_separation.rs`. Invariante A.48, A.51. Impl: `arb-strategy` / `src/arbitrage/*` auf `architecture-rebuild-next` (nach PRs #416–#420).

**Plan-Datei** `docs/plans/plan_arb_profit_first_rebuild.md` ist historisch; bei Konflikt gilt dieses File plus `INVARIANTS.md`.

**TX/Account-Trennung (User 2026-08-22):** TX = Discovery (+ Layout-Seed für Hot-Pins). Account = alleinige Quote-SSOT für Entry-Size, Exit, Arb-Screening und Exit-Reasons. Kein `LastTradeMid`-Fallback im Hot-Path-Quote-API. Siehe `INVARIANTS.md` A.51.

---

## 1. Zweck

Cross-DEX-Arb vergleicht **keine Mid-Preise unterschiedlicher Herkunft**.  
Screening beantwortet nur:

> Bei Probe-Size `amount_in` (SOL): liefert Route A→B→A mehr SOL zurück als `amount_in + fees`?

Execution-Wahrheit bleibt **I-9 Simulation** (unverändert).

---

## 2. QuoteKind

| Kind | Definition | Wann erlaubt (Cross-DEX Screening) |
|------|------------|-------------------------------------|
| `ExecutableMarginal` | `quote_exact_in(pool, mint_in, mint_out, amount)` mit program-nah Math (CPMM, DLMM Bin-Walker) aus **Account-State** (Reserves/Bins) | **Einzige** erlaubte Quote-Kind für Cross-DEX 2-hop / Multi-hop Screening |
| `LastTradeMid` | Letzter SOL-quoted Trade aus TX-Parse | **Verboten** für Cross-DEX Pairing, Screening und `quote_exact_in`-Fallback im Hot Path |

**Verboten für Cross-DEX 2-hop Pairing und Hot-Path-Quotes:**

- `LastTradeMid` (jede Verwendung — kein Pairing, kein Fallback)
- `ExecutableMarginal` ↔ `LastTradeMid` (historischer Cross-Kind-Fall; beide Kinds dürfen nicht screenen)
- Reserve-Mid ohne Size als Screening-Quote
- Trade auf Pool A vs Reserve auf Pool B ohne Slot/State-Kohärenz

`quotes_pairable(a, b)` gilt nur wenn **beide** `QuoteKind::ExecutableMarginal` sind.

`quote_exact_in` liefert im Hot Path nur `ExecutableMarginal` oder `None` — nie `LastTradeMid`.

---

## 3. PoolQuote (Struktur)

```text
PoolQuote {
  pool_address,
  dex,
  kind: QuoteKind,       // Cross-DEX: immer ExecutableMarginal
  as_of_slot: u64,
  as_of_ts: Instant,
  fresh: bool,           // derived from state TTL rules (ExecutableMarginal)
  amount_in,             // probe lamports (buy leg)
  amount_out,            // tokens or SOL on return leg
}
```

Freshness (nur `ExecutableMarginal`):

- **State:** Vault/Bin **Material-Fingerprint** unverändert seit `as_of_ts` → gültig bis `arb_quote_state_ttl_ms` (default 120s)
- **Material-Slot:** `as_of_slot` ist der Slot der letzten Fingerprint-Änderung (Reserves + DLMM-Bins bzw. `amount_out`-wirksamer State). Cache-Heartbeats mit identischem State dürfen `as_of_slot` / `updated_at` **nicht** vorrücken.
- Ruhe ≠ stale **pro Pool**, solange der Fingerprint unverändert ist
- **Verboten:** ein ruhendes Bein mit einem bewegten Bein über gleiche Heartbeat-Slots zu paaren (`|buy.as_of_slot − sell.as_of_slot| ≤ 2` gilt nur für Material-Slots; zusaetzlich `chain_slot − leg.as_of_slot ≤ arb_max_leg_age_slots`)

Trade-TTL (`arb_quote_trade_ttl_ms`) gilt **nicht** mehr als Screening-Fallback — nur noch Account-State.

---

## 4. Round-Trip 2-hop (Cross-DEX)

Für Mint `M`, Probe `P` lamports:

```text
tokens = quote_exact_in(buy_pool,  SOL → M, P)   // ExecutableMarginal only
sol_back = quote_exact_in(sell_pool, M → SOL, tokens)
profit = sol_back - P - estimated_tx_fees
```

**Pairing-Regel:** `buy_pool.kind == sell_pool.kind == ExecutableMarginal`  
**Slot-Regel (M4):** `|buy.as_of_slot - sell.as_of_slot| ≤ arb_max_leg_slot_delta` (default 2)  
**Age-Regel:** `chain_slot − buy.as_of_slot ≤ arb_max_leg_age_slots` und analog Sell (default 16). `chain_slot` = letzter bekannter Geyser-Head im Prozess, kein RPC.

Reject reasons (Metriken):

- `incompatible_quote_kind` (inkl. `LastTradeMid` oder fehlendes ExecutableMarginal)
- `round_trip_unprofitable`
- `quote_stale`
- `slot_delta_exceeded`
- `leg_slot_too_old`
- `no_executable_quote` (kein Account-State-Quote verfügbar)

**Kein** `spread_too_large` auf Mid-Preisen im v2-Pfad.

---

## 5. Pool-Auswahl pro Mint

Pro DEX maximal ein Pool in den Round-Trip:

- Filter: `fresh && quote_available && kind == ExecutableMarginal`
- Rank: jüngster `as_of_slot`, dann höchste Liquidität
- **Nicht:** global günstigster Mid über alle Pools ignorierend Frische
- **Nicht:** Trade-Mark aus TX als Ersatz für fehlende Account-Quotes

---

## 6. Multi-hop

Gleiche `quote_exact_in` Implementierung wie 2-hop (`pool_quote.rs`).  
Graph-Expansion nur über Pools mit `quote_ready` und `QuoteKind::ExecutableMarginal`.  
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
| `invariants_arb_quote_contract.rs` | M1 + M2 (ExecutableMarginal-only) |
| `invariants_tx_account_hard_separation.rs` | TX/Account-Trennung (A.51) |
| Multi-hop unified quoter | M3 (E-ARB-3) |
