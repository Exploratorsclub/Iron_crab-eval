# Plan: Arb-Modul Profit-First Rebuild

**Status:** Entwurf (Supervisor, 2026-07-02)  
**Priorität:** P0 — 2-hop Cross-DEX produziert 0 Opportunities trotz Ingest-Fixes (#255–#257)  
**Scope:** **Nur Arb-Modul** — `arb-strategy` Binary + `src/arbitrage/*` + Arb-Metriken. **Kein** Rewrite von `market_data`, `momentum_bot`, `execution_engine` (Jito-Pfad bleibt).  
**Repos:** `Exploratorsclub/Iron_crab` (Impl), `Exploratorsclub/Iron_crab-eval` (Spec/Eval)  
**Basis-Prod:** nach Merge #257 (`5d55fc1`), Branch `architecture-rebuild`

---

## 1. Problemstellung (kurz)

| Heute (Research-Eligibility) | Ziel (Profit-First / Production) |
|------------------------------|----------------------------------|
| `comparable_price`: Reserve/Trade/Marginal-Fallback pro Pool | **Eine** Quote-Engine: size-aware, program-nah |
| Spread bps zwischen SOL/token-Mids | **Round-Trip:** `SOL → Token → SOL` minus Fees |
| Freshness = „irgendein Event ≤30s“ | Freshness = **Quote** gültig (Slot/State) |
| Best buy/sell ohne Quote-Kind-Filter | Nur **kompatible**, frische Pool-Paare |
| 2-hop vs Multi-hop unterschiedliche Quoter | **Gemeinsame** `PoolQuote`-Schicht |
| Symptom-Tuning (`stale_price`, Spread) | Milestone-Deploys; kein Einzel-PR-Tuning |

**Infrastruktur ist Production-tauglich** (Frankfurt Co-Location, eigener Validator/Geyser, Jito). Der Engpass ist die **Entscheidungsschicht** in `arb_strategy.rs`, nicht Execution.

---

## 2. Scope-Grenzen

### In Scope

| Pfad | Rolle |
|------|-------|
| `src/bin/arb_strategy.rs` | 2-hop, Tracker, Writer, Event-Routing |
| `src/arbitrage/*` | Multi-hop, Graph, Ranker, **neu:** `pool_quote.rs` |
| `src/arbitrage/arb_slave_sync.rs` | SLAVE LivePoolCache / JetStream |
| `src/metrics.rs` | Nur **additive** Arb-Metriken |
| `Iron_crab-eval/docs/spec/ARB_QUOTE_CONTRACT.md` | Neuer Vertrag (Phase 0) |
| `Iron_crab-eval/tests/invariants_arb_quote_*.rs` | Neue Eval-Gates |

### Out of Scope (explizit unverändert)

- `src/bin/market_data.rs` (Pins werden über bestehendes `arb.track_requests` angestoßen — **kein** MD-Rewrite)
- `src/bin/execution_engine.rs`, Jito-Bundle-Pfad
- `src/bin/momentum_bot.rs`
- `src/execution/quote_calculator.rs` — **Phase 1–2:** unangetastet; **Phase 3:** optional dünner Delegate auf `pool_quote` (separater PR, nur wenn nötig)

### Verbot während des Rollouts (Supervisor / Review)

| Verbot | Grund |
|--------|-------|
| `MAX_PRICE_AGE_MS`, `max_spread`, `min_profit` allein lockern | Symptom-Maskierung |
| Einzel-PR Freshness-Fix für Meteora ohne Milestone M2 | Wird in M2 ersetzt |
| `comparable_price`-Fallback-Kette erweitern | Legacy-Pfad wird entfernt |
| Prod-Deploy **innerhalb** eines Milestones (nur Teil-PRs) | Erwartete Fehlfunktion → Symptom-Fix-Schleife |
| Deploy ohne Eval Level-5 grün + Bugbot | DoD |

---

## 3. Ziel-Architektur (Arb Decision Layer)

```
MarketEvents / JetStream / LivePoolCache (unverändert)
        │
        ▼
┌───────────────────┐     ┌─────────────────────────┐
│ TokenArbTracker   │────▶│ pool_quote::PoolQuote   │  ← NEU (Single SSOT)
│ (Pools, Vaults,   │     │ executable_exact_in/out │
│  Bins, Trades)    │     │ kind, as_of_slot, fresh │
└───────────────────┘     └───────────┬─────────────┘
        │                             │
        ├──────── 2-hop ──────────────┤ round_trip_screen()
        │         check_arbitrage_v2  │ → Kandidat
        │                             │
        └──────── multi-hop ──────────┘ CachedQuoteProvider → PoolQuote
                                      │
                                      ▼
                            TradeIntent (unverändert)
                                      │
                                      ▼
                         execution_engine + Jito (unverändert)
                                      │
                                      ▼
                         I-9 Simulation = Profit-Wahrheit
```

**Profit-First-Regel:** Kein Intent ohne (1) bestandenen Round-Trip-Screen **und** (2) Simulation OK (bestehend I-9).

---

## 4. Milestones, PRs, Merge-Batches, Deploy

**Regel:** Server-Verifikation (Prod-Soak) ist **nur** an Milestone-Grenzen erlaubt — **nachdem der komplette Merge-Batch** auf `architecture-rebuild` ist und CI + Eval Level-5 grün.

---

### Milestone M0 — Spec & Eval-Vertrag (kein Prod-Verhalten)

**Ziel:** Verbindlicher Quote-Contract; noch **keine** Impl-Änderung am Hot Path.

| PR | Repo | Inhalt | Merge alone? |
|----|------|--------|--------------|
| **E-ARB-0** | ironcrab-eval | `docs/spec/ARB_QUOTE_CONTRACT.md` + Verweis in `INVARIANTS.md` (A.48 neu) | Ja |
| **E-ARB-0b** | ironcrab-eval | `docs/plans/plan_arb_profit_first_rebuild.md` (dieses Doc) final | mit E-ARB-0 |

**Prod-Deploy:** **Nein** (nur Doku).  
**Erwartung:** Keine Metrik-Änderung.

**Exit M0:** Spec reviewed; Invariante A.48 Text frozen.

---

### Milestone M1 — Quote-Engine Fundament (Shadow only)

**Ziel:** `PoolQuote` implementiert; **paralleler Shadow-Vergleich** — alter 2-hop-Pfad bleibt **authoritativ**.

| PR | Repo | Inhalt | Abhängigkeit |
|----|------|--------|--------------|
| **I-ARB-1** | iron_crab | Neues Modul `src/arbitrage/pool_quote.rs`: `QuoteKind`, `PoolQuote`, `quote_exact_in` (pump_amm CPMM, meteora DLMM Bin-Walker, orca/raydium über Reserves); Extrakt aus `arb_strategy.rs` (DLMM marginal, reserve mid) | M0 |
| **I-ARB-2** | iron_crab | Unit-Tests in `pool_quote.rs` + `arb_strategy` Tests für Pump↔Meteora **gleicher Slot**, Round-Trip > 0 | I-ARB-1 |
| **E-ARB-1** | ironcrab-eval | `tests/invariants_arb_quote_contract.rs`: QuoteKind-Pairing, Monotonie (A.1 analog), kein Cross-Kind | E-ARB-0 + I-ARB-1 API |
| **I-ARB-3** | iron_crab | Shadow in `check_arbitrage`: Metriken `arb_quote_shadow_*` (old mid spread vs new round_trip_profit); Flag `arb_quote_shadow_mode` (default **false**) | I-ARB-1 |

**Merge-Batch M1 (zusammen mergen, Reihenfolge):**

1. I-ARB-1 → I-ARB-2 (Impl CI)
2. E-ARB-1 (Eval gegen gemergtes Impl via git-dep oder nach Impl-Merge)
3. I-ARB-3

**Prod-Deploy M1:** **Optional** — nur mit `arb_quote_shadow_mode=true` (Control Plane).  
**Erwartung:** `arb_two_hop_opportunities_total` **unverändert** (Legacy autoritativ). Shadow-Metriken sichtbar.  
**Verbot:** Keine Fixes an `comparable_price`, `stale_price`, Spread-Schwellen.  
**Soak-Gate M1 (wenn deployed):** 6 h, Shadow-Samples > 0, kein Writer-Stall, kein CPU-Regression arb-strategy.

---

### Milestone M2 — 2-hop Profit-First (authoritativ)

**Ziel:** Cross-DEX 2-hop nutzt **nur** Round-Trip + Quote-Pairing; Legacy-Pfad hinter Flag aus.

| PR | Repo | Inhalt |
|----|------|--------|
| **I-ARB-4** | iron_crab | **Freshness v2:** `PoolQuote.fresh` aus `as_of_slot` / vault state seq; ruhender Pool mit unverändertem State bleibt gültig (TTL konfigurierbar, default 120s state / 30s trade) |
| **I-ARB-5** | iron_crab | **Pool-Auswahl:** pro Mint pro DEX bester Pool nach `(fresh, quote_kind)` — nicht global min/max Mid |
| **I-ARB-6** | iron_crab | **`check_arbitrage_v2`:** Round-Trip `probe_lamports → token_out → sol_back`; Profit = `sol_back - probe - fee_estimate`; Reject `incompatible_quote_kind`, `round_trip_unprofitable` |
| **I-ARB-7** | iron_crab | Flag `arb_two_hop_v2_enabled`: wenn true, **kein** Legacy-Spread; Metriken `arb_two_hop_v2_*`; Default **false** bis Batch komplett |
| **E-ARB-2** | ironcrab-eval | Invarianten: kein Cross-DEX-Vergleich unterschiedlicher `QuoteKind`; 2-hop Intent nur nach positivem Round-Trip-Screen (Mock-Fixtures) |

**Merge-Batch M2 (alle vor Deploy — kein Einzel-PR):**

```
I-ARB-4 ─┐
I-ARB-5 ─┼─► I-ARB-6 ─► I-ARB-7 ─► E-ARB-2
         ─┘
```

**Prod-Deploy M2:** **Ja — erstes echtes Verifikations-Deploy**  
**Aktivierung:** `arb_two_hop_v2_enabled=true`, `two_hop_enabled=true`, Shadow optional aus.

**Erwartung (realistisch):**

- `stale_price` / Legacy `spread_too_large` **sinken** (Legacy-Metriken evtl. still)
- Neue Rejects: `incompatible_quote_kind`, `round_trip_unprofitable` (normal)
- **Erste** `arb_two_hop_opportunities_total > 0` **oder** klare `round_trip_unprofitable`-Dominanz (Markt zu effizient — ok)
- Intents nur wenn Simulation (I-9) grün — wie bisher

**Soak-Gate M2 (verbindlich, 24 h Frankfurt Prod):**

| Metrik / Check | Kriterium |
|----------------|-----------|
| Writer stall | 0 |
| `arb_strategy_pool_cache_updates_seen` | Δ > 0 sustained |
| `arb_two_hop_v2_screen_total` | > 0 |
| Legacy `comparable_price` path | nicht aktiv (Flag) |
| Symptom-Fix-PRs | **keine** ohne Supervisor-Freigabe |
| Opportunities + Simulation | ≥ 1 simulierte Opportunity **oder** dokumentierter Markt-Nullfall mit `round_trip_unprofitable` > 95% und **kein** `incompatible_quote_kind` > 20% |

**Bei Fail M2:** Follow-up **nur** innerhalb M2-Scope (Quote/Paring/Round-Trip) — **kein** Rückfall auf Spread-Tuning.

---

### Milestone M3 — Multi-hop Angleichung

**Ziel:** Multi-hop nutzt dieselbe `PoolQuote`-Engine; keine CP-Approximation für DLMM in der Suche.

| PR | Repo | Inhalt |
|----|------|--------|
| **I-ARB-8** | iron_crab | `CachedQuoteProvider` → delegiert an `pool_quote` |
| **I-ARB-9** | iron_crab | `pool_ranker` / Beam: nur `quote_ready` Pools mit `QuoteKind::ExecutableMarginal` oder explizit `LastTradeMid` |
| **E-ARB-3** | ironcrab-eval | Multi-hop Cycle-Profit-Tests mit unified quoter (Mock LivePoolCache) |

**Merge-Batch M3:** I-ARB-8 + I-ARB-9 + E-ARB-3 zusammen.

**Prod-Deploy M3:** **Nach** M2-Soak bestanden.  
**Flag:** `multi_hop_enabled` unabhängig; Empfehlung: erst M2 stabil, dann Multi-hop Shadow 24 h.

**Soak-Gate M3 (24 h):**

- `multi_hop_hop_missing_quote_total` / s **< 1%** der Expansion-Edges (nicht 99% Miss wie historisch)
- `multi_hop_cycles_profitable` Δ ≥ 0 mit plausibler Rate

---

### Milestone M4 — Coverage (Co-Location nutzen)

**Ziel:** Genug frische **Executable**-Quotes für Multi-DEX-Mints — ohne MD-Rewrite.

| PR | Repo | Inhalt |
|----|------|--------|
| **I-ARB-10** | iron_crab | Proaktives `arb.track_requests`: bei Tracker-Seed / Multi-DEX-Erkennung **alle** Pools des Mints sofort pinnen (bestehend Topic I-4e) |
| **I-ARB-11** | iron_crab | **Slot-Kohärenz:** Round-Trip nur wenn `|slot_buy - slot_sell| ≤ arb_max_leg_slot_delta` (default 2) |
| **I-ARB-12** | iron_crab | Metriken: `arb_quote_pair_slot_delta`, `arb_track_pin_before_first_screen_ms` |

**Merge-Batch M4:** I-ARB-10 + I-ARB-11 + I-ARB-12 zusammen.

**Prod-Deploy M4:** Nach M3 oder parallel zu M2 wenn 2-hop `incompatible_quote_kind` > 20% (Coverage-Engpass).

**Soak-Gate M4 (24 h):**

- `incompatible_quote_kind` **< 10%** der v2-Screens
- `market_data_arb_registered_vaults` korreliert mit Multi-DEX-Mints
- Kein Pin-Budget-Runaway (bestehende Budget-Metriken)

---

### Milestone M5 — Legacy entfernen

**Ziel:** Kein Dual-Path; Codebase bereinigen.

| PR | Repo | Inhalt |
|----|------|--------|
| **I-ARB-13** | iron_crab | Entfernen: `comparable_price_sol_per_token`, Legacy-Spread in `check_arbitrage`, Flags `arb_quote_shadow_mode`, `arb_two_hop_v2_enabled` (v2 = einziger Pfad) |
| **E-ARB-4** | ironcrab-eval | INVARIANTS A.48 eval-enforced; alte Gate-Metriken dokumentiert deprecated |

**Merge-Batch M5:** I-ARB-13 + E-ARB-4.

**Prod-Deploy M5:** Nach M2–M4 grün. Kein neues Verhalten — Cleanup.

---

## 5. PR-Übersicht (Roadmap)

| ID | Milestone | Kurztitel |
|----|-----------|-----------|
| E-ARB-0 | M0 | Spec ARB_QUOTE_CONTRACT |
| E-ARB-0b | M0 | Dieser Plan |
| I-ARB-1 | M1 | pool_quote Modul |
| I-ARB-2 | M1 | pool_quote Unit-Tests |
| E-ARB-1 | M1 | Eval Quote-Contract Tests |
| I-ARB-3 | M1 | Shadow-Metriken |
| I-ARB-4 | M2 | Freshness v2 |
| I-ARB-5 | M2 | Pool-Auswahl |
| I-ARB-6 | M2 | check_arbitrage_v2 Round-Trip |
| I-ARB-7 | M2 | Feature-Flag + Metriken |
| E-ARB-2 | M2 | Eval 2-hop Invarianten |
| I-ARB-8 | M3 | Multi-hop → PoolQuote |
| I-ARB-9 | M3 | Ranker Quote-Ready v2 |
| E-ARB-3 | M3 | Eval Multi-hop |
| I-ARB-10 | M4 | Proaktive arb.track_requests |
| I-ARB-11 | M4 | Slot-Kohärenz |
| I-ARB-12 | M4 | Coverage-Metriken |
| I-ARB-13 | M5 | Legacy entfernen |
| E-ARB-4 | M5 | Eval final |

**Eval/Impl-Reihenfolge:** Wie bei Phase 3 Arb-Track — **Impl-PRs mit API zuerst**, Eval-PRs mit git-dep auf gemergten Impl-Stand (oder manueller Eval-Lauf vor Merge).

---

## 6. Was bei Prod-Test **nicht** als Bug behandelt wird

Solange Milestone **M2 nicht vollständig** deployed ist:

| Beobachtung | Kein Bug — erwartet |
|-------------|---------------------|
| `stale_price` hoch | Legacy-Pfad aktiv |
| `spread_too_large` pump↔meteora | Mid-Mix Legacy |
| 0 Opportunities | Kein Round-Trip v2 |
| Meteora fresh niedrig ohne M4 | Pin-Timing noch reaktiv |
| Shadow zeigt Spread vs Round-Trip Divergenz | M1 Ziel |

Erst **nach M2 + 24 h Soak** sind „0 Opportunities“ oder dominierende `incompatible_quote_kind` **echte** Follow-up-Kandidaten (M4 Coverage oder Quote-Math).

---

## 7. Konfiguration (Control Plane — nach M2)

| Key | Default (M2 Deploy) | Bedeutung |
|-----|---------------------|-----------|
| `arb_two_hop_v2_enabled` | `false` bis Batch merged; dann `true` | Legacy aus |
| `arb_quote_shadow_mode` | `false` | Nur M1 optional |
| `arb_probe_lamports` | z. B. `10_000_000` (0.01 SOL) | Round-Trip-Size |
| `arb_max_leg_slot_delta` | `2` (M4) | Slot-Kohärenz |
| `arb_quote_state_ttl_ms` | `120_000` | Ruhender State |
| `arb_quote_trade_ttl_ms` | `30_000` | LastTradeMid |
| `two_hop_enabled` | `true` | unverändert |
| `multi_hop_enabled` | bestehend | M3 separat |

---

## 8. CI / Definition of Done pro PR

- **Impl:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, **Eval Level 5** (Impl CI)
- **Eval:** schlankes Rust-Gate + `cargo test` mit passendem Impl-Checkout vor koordiniertem Merge
- **Bugbot** auf jedem PR vor Merge (Supervisor-Gate)
- **Deploy:** nur Supervisor mit **expliziter User-Freigabe**; nie mitten in M2/M3/M4 Batch

---

## 9. Abhängigkeit zu bestehenden Fixes

| Bereits gemergt | Rolle im Plan |
|-----------------|---------------|
| #255 Writer-Stall | Voraussetzung — Tracker muss PoolState anwenden |
| #256 JetStream Live PoolCache | SLAVE known_pools / Updates |
| #257 Meteora PoolStatePublish | Vault/Bin-Frische — **Input** für PoolQuote, nicht Lösung für Mid-Mix |
| I-4e arb.track_requests | M4 baut darauf auf (proaktiver Publish) |

**Nicht wiederholen:** Einzel-PR „Meteora Freshness P0“-Art Fixes ohne M2 — obsolet.

---

## 10. Erfolgskriterium (Production Arb)

Nach **M2 + M4** deployed und 48 h Soak:

1. 2-hop v2 ist **einziger** Cross-DEX-Screening-Pfad (oder M5 merged).
2. Opportunities korrelieren mit **simuliertem** Profit (I-9), nicht mit Mid-Spread.
3. Kein dominanter Reject `incompatible_quote_kind` (< 10%).
4. Jito-Intents nur aus v2-Pipeline (Audit-Log / Decision Record).
5. Multi-hop (M3) optional zweite Profit-Quelle — nicht Blocker für 2-hop.

---

## 11. Nächster Supervisor-Schritt

1. User-Freigabe Plan M0–M5  
2. Handoff **E-ARB-0** (Test Authority / Eval-Doku-PR)  
3. Handoff **I-ARB-1** (Impl Agent) — erst nach E-ARB-0 Merge oder parallel Spec frozen  
4. **Kein** Prod-Deploy bis Merge-Batch M2 komplett
