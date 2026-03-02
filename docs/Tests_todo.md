# Tests TODO – Noch zu implementierende Eval-Tests

**Zweck:** Zentrales Dokument für Tests, die noch implementiert werden müssen, inkl. priorisierter Empfehlungen aus ARCHITECTURE_AUDIT und EVAL_TEST_CANDIDATES.

**Quellen:** ARCHITECTURE_AUDIT.md (Iron_crab), EVAL_TEST_CANDIDATES.md, INVARIANTS.md

---

## 1. Priorisierte Empfehlungen (ARCHITECTURE_AUDIT-Ableitung)

Diese Empfehlungen wurden aus der Abgleichung von ARCHITECTURE_AUDIT.md, INVARIANTS.md und EVAL_TEST_CANDIDATES.md abgeleitet.

### Priorität 1: Pool-Matching Invariante (I-13 / FIX-38) — ERLEDIGT

**Invariante:** Position-Preis-Updates (Trade, PoolCacheUpdate) nur anwenden, wenn `source_pool == position.pool`.

**Begründung:**
- I-13 ist in INVARIANTS.md als kritisch dokumentiert; Verletzung führt zu falscher PnL und TAKE_PROFIT bei Verlust.
- ARCHITECTURE_AUDIT bestätigt: Eingehalten, aber **kein Eval-Test** verifiziert das.
- Bei Refactors kann dies unbeabsichtigt brechen.

**Test-Ansatz (Blackbox):**
- Position mit `pool = A`.
- Preis-Update mit `source_pool = B` → Update wird **nicht** angewendet.
- Preis-Update mit `source_pool = A` → Update wird angewendet.

**Zieldatei:** `tests/invariants_pool_matching.rs`

**Spec-Ergänzung:** INVARIANTS.md A.11 dokumentieren.

---

### Priorität 2: Liquidation-Routing / 6005-Retry

**Kontext (ARCHITECTURE_AUDIT BUG A):** Bei `run_liquidation_job()` gibt es mehrere Pfade, wo Token übersprungen werden: `min_out_sol.is_none()`, Creator fehlt im Cache, `pool_accounts_v1_for_base_mint()` gibt `None`.

**Implementiert:** 6005-Retry (PumpFun → PumpSwap AMM) ist implementiert. Der Gesamtflow wird aber nicht evaluiert.

**Test-Idee (Blackbox):**
- Wenn ein SELL auf PumpFun mit 6005 (BondingCurveComplete) fehlschlägt, wird ein Retry über PumpSwap AMM versucht.
- Liquidation-Routing: Multi-Pool zuerst, PumpFun als Fallback.

**Hinweis:** Aufwendiger, da der Test stark von LivePoolCache und Konfiguration abhängt. Eventuell mit Fixtures/Mock-State realisierbar.

**Zieldatei:** `tests/invariants_liquidation_flow.rs` (oder Erweiterung von `golden_replay_blackbox.rs` um Liquidation-Fixture)

---

### Priorität 3: Hot-Path-RPC-Freiheit (I-4, I-7)

**Kontext:** ARCHITECTURE_AUDIT – `allow_rpc_on_miss` ist implementiert: Hot Path lehnt bei Cache-Miss ab. Offen bleiben: Orca Tick-Arrays, Raydium Serum-Market.

**Test-Idee (Invariante):**
- DEX-Connectors rufen bei `allow_rpc_on_miss = false` bei Cache-Miss **keinen** RPC auf und geben stattdessen `Err` zurück.

**Hinweis:** Schwer ohne RPC-Mock als Blackbox testbar. Möglicherweise als Unit-Test im Impl-Repo oder mit injizierbarem RPC-Client in eval.

**Zieldatei:** `tests/invariants_hot_path_no_rpc.rs` (falls über API testbar)

---

## 2. Aus EVAL_TEST_CANDIDATES (Priorität 3, optional)

Diese Tests wurden in EVAL_TEST_CANDIDATES als „vorerst im Impl-Repo“ markiert. Bei Bedarf als Invarianten in die Spec aufnehmen und in eval implementieren.

| Kandidat | Quelle | Invariante? | Empfehlung |
|----------|--------|-------------|------------|
| `router_builds_hops2_plan_with_min_out` | router_hops2_plan.rs | ✅ | Nach `invariants_router_slippage.rs` migrieren |
| `router_picks_higher_out_amount` | router_best_quote.rs | ✅ | Nach `invariants_router_slippage.rs` migrieren |
| `pruning_keeps_profitable_cycle` | arbitrage_cycle_pruning.rs | ⚠️ | Arbitrage-Engine; evtl. Invariante; eher Unit |
| `profit_ranking_orders_cycles` | arbitrage_profit_ranking.rs | ⚠️ | Evtl. Invariante; eher Unit |
| `aggregate_picks_higher_output` | arbitrage_edge_aggregate.rs | ⚠️ | Evtl. Invariante; eher Unit |
| `enumerate_4hop_cycle` | arbitrage_cycle_generic.rs | ⚠️ | N-Hop-Enumeration; eher Unit |
| `test_orca_build_swap_ix_*` | execution_orca_builder.rs | ⚠️ | DoD §H Instruction-Gültigkeit |
| `test_pumpfun_build_*` | execution_pumpfun_builder.rs | ⚠️ | DoD §H Instruction-Gültigkeit |
| `single_swap_estimate_in_range` | compute_budget_estimator.rs | ⚠️ | CU-Schätzung; eher Config/Policy |
| ConfigUpdate-Tests | hot_reload_smoke_test.rs | ⚠️ | DoD §I Runtime-Config; Schema-Konsistenz |

---

## 3. Offene Invarianten ohne Eval-Test

Invarianten aus INVARIANTS.md B.x, die **nicht** durch Eval-Tests abgedeckt sind:

| ID | Invariante | Status |
|----|------------|--------|
| I-13 | Pool-Matching (FIX-38) | ✅ Eval-getestet (`invariants_pool_matching.rs`) |
| I-4 / I-7 | Hot Path RPC-Freiheit | ⚠️ Kein Test – siehe Priorität 3 |
| I-14 | tokens_per_sol Konvention | Leitlinie, kein Eval-Test |

---

## 4. Implementierungs-Checkliste

| # | Test | Priorität | Zieldatei | Status |
|---|------|-----------|-----------|--------|
| 1 | Pool-Matching (I-13) | P1 | `invariants_pool_matching.rs` | erledigt |
| 2 | Liquidation 6005-Retry Flow | P2 | `invariants_liquidation_flow.rs` | offen |
| 3 | Hot-Path RPC-Freiheit | P3 | `invariants_hot_path_no_rpc.rs` | offen |
| 4 | Router hops2 + best_quote | optional | `invariants_router_slippage.rs` | offen |

---

## 5. Migrationsplan-Vorlage (pro Test)

Für jeden neuen Eval-Test:

1. **Invariante in Spec formulieren** (docs/spec/INVARIANTS.md oder bestehendes Spec-Dokument)
2. **Test in ironcrab-eval implementieren** (nur über öffentliche API, keine Interna)
3. **Im Impl-Repo:** Original behalten (als Regression) oder entfernen, wenn Eval-Test Deckung übernimmt
4. **CI prüfen:** `cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`

---

## 6. Querbezüge

- **EVAL_TEST_CANDIDATES.md** – Vollständige Kandidaten-Liste
- **ARCHITECTURE_AUDIT.md** (Iron_crab) – Offene Architektur-Themen, BUG A
- **INVARIANTS.md** – Eval-getestete vs. nicht getestete Invarianten
- **DEFINITION_OF_DONE.md** – DoD §G Golden Replays, §H Connector Contracts

---

*Erstellt: Test Authority, ironcrab-eval*
