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

### Priorität 2: Liquidation-Routing / 6005-Retry — ERLEDIGT

**Invariante:** Nach 6005 mark_pumpfun_complete_for_mint; Liquidation Phase 2 überspringt PumpFun (complete) und nutzt PumpSwap AMM.

**Test-Ansatz:**
- `invariants_liquidation_flow.rs`: mark_pumpfun_complete_for_mint, find_pump_amm_pool_by_base_mint.
- `golden_replay_liquidation_6005_retry`: Replay-Determinismus für 6005-Fixture (Iron_crab).

**Zieldatei:** `tests/invariants_liquidation_flow.rs`, `golden_replay_blackbox.rs`

---

### Priorität 3: Hot-Path-RPC-Freiheit (I-4, I-7) — ERLEDIGT

**Invariante:** DEX-Connectors liefern bei Cache-Miss None/Err ohne RPC (Hot Path).

**Test-Ansatz:** Dummy-RPC-URL (`http://127.0.0.1:0`). Bei Cache-Miss wird vor RPC-Fallback abgebrochen → keine Netzwerk-Anfrage.

**Zieldatei:** `tests/invariants_hot_path_no_rpc.rs`

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
| I-4 / I-7 | Hot Path RPC-Freiheit | ✅ Eval-getestet (`invariants_hot_path_no_rpc.rs`) |
| I-14 | tokens_per_sol Konvention | Leitlinie, kein Eval-Test |

---

## 4. Implementierungs-Checkliste

| # | Test | Priorität | Zieldatei | Status |
|---|------|-----------|-----------|--------|
| 1 | Pool-Matching (I-13) | P1 | `invariants_pool_matching.rs` | erledigt |
| 2 | Liquidation 6005-Retry Flow | P2 | `invariants_liquidation_flow.rs`, `golden_replay_blackbox.rs` | erledigt |
| 3 | Hot-Path RPC-Freiheit | P3 | `invariants_hot_path_no_rpc.rs` | erledigt |
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
