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
| `router_builds_hops2_plan_with_min_out` | router_hops2_plan.rs | ✅ | Migriert (`invariants_router_slippage.rs`) |
| `router_picks_higher_out_amount` | router_best_quote.rs | ✅ | Migriert (`invariants_router_slippage.rs`) |
| `pruning_keeps_profitable_cycle` | arbitrage_cycle_pruning.rs | ✅ | Migriert (`invariants_arbitrage_engine.rs` A.16) |
| `profit_ranking_orders_cycles` | arbitrage_profit_ranking.rs | ✅ | Migriert (`invariants_arbitrage_engine.rs` A.15) |
| `aggregate_picks_higher_output` | arbitrage_edge_aggregate.rs | ✅ | Migriert (`invariants_arbitrage_engine.rs` A.14) |
| `enumerate_4hop_cycle` | arbitrage_cycle_generic.rs | ✅ | Migriert (`invariants_arbitrage_engine.rs` A.17) |
| `test_orca_build_swap_ix_*` | execution_orca_builder.rs | ✅ | Migriert (`invariants_orca_ix.rs` DoD §H) |
| `test_pumpfun_build_*`, `test_tx_builder_supports_pumpfun_sell_pure_derivation` | execution_pumpfun_builder.rs | ✅ | Migriert (`invariants_pumpfun_ix.rs` DoD §H + TxBuilder SELL) |
| `single_swap_estimate_in_range` | compute_budget_estimator.rs | ✅ | Migriert (`invariants_compute_budget.rs` A.18) |
| ConfigUpdate-Tests | hot_reload_smoke_test.rs | ⚠️ | DoD §I Runtime-Config; Schema-Konsistenz; ausgelassen |

---

## 3. Offene Invarianten ohne Eval-Test

Invarianten aus INVARIANTS.md B.x, die **nicht** durch Eval-Tests abgedeckt sind:

| ID | Invariante | Status |
|----|------------|--------|
| I-13 | Pool-Matching (FIX-38) | ✅ Eval-getestet (`invariants_pool_matching.rs`) |
| I-4 / I-7 | Hot Path RPC-Freiheit | ✅ Eval-getestet (`invariants_hot_path_no_rpc.rs`) |
| I-14 | tokens_per_sol Konvention | Eval-getestet (`invariants_tokens_per_sol.rs`) |

---

## 4. Implementierungs-Checkliste

| # | Test | Priorität | Zieldatei | Status |
|---|------|-----------|-----------|--------|
| 1 | Pool-Matching (I-13) | P1 | `invariants_pool_matching.rs` | erledigt |
| 2 | Liquidation 6005-Retry Flow | P2 | `invariants_liquidation_flow.rs`, `golden_replay_blackbox.rs` | erledigt |
| 3 | Hot-Path RPC-Freiheit | P3 | `invariants_hot_path_no_rpc.rs` | erledigt |
| 4 | Router hops2 + best_quote | optional | `invariants_router_slippage.rs` | erledigt |
| 5 | Arbitrage Engine (Edge-Agg, Ranking, Pruning, 4-Hop) | optional | `invariants_arbitrage_engine.rs` | erledigt |
| 6 | Orca/PumpFun build_swap_ix (DoD §H) | optional | `invariants_orca_ix.rs`, `invariants_pumpfun_ix.rs` | erledigt |
| 7 | Compute-Budget estimate_single_swap + large_notional | optional | `invariants_compute_budget.rs` | erledigt |
| 8 | tokens_per_sol (I-14) | optional | `invariants_tokens_per_sol.rs` | erledigt |
| 9 | TxBuilder PumpFun SELL | optional | `invariants_pumpfun_ix.rs` | erledigt |
| 10 | DEX Parser PumpSwap BUY/SELL (A.20) | P1 | `invariants_dex_parser_pumpswap.rs` | erledigt |
| 11 | DEX Parser CPI Fallback (A.21) | P1 | `invariants_dex_parser_cpi.rs` | erledigt |
| 12 | PumpFun Cashback-Upgrade (A.22-A.24) | P0 | `invariants_pumpfun_cashback.rs` | erledigt |
| 13 | PumpFun Market Order (A.25-A.26) | P0 | `invariants_pumpfun_market_order.rs` | erledigt |
| 14 | PumpSwap Recovery-Semantik: Cold-Path force refresh, Hot-Path nicht blockieren | P0 | neue/erweiterte PumpSwap/Liquidation Invarianten | erledigt (Impl Scope 1-3 gemergt, Eval-Vertrag in PR #13 gemergt) |

---

## 5. Migrationsplan-Vorlage (pro Test)

Für jeden neuen Eval-Test:

1. **Invariante in Spec formulieren** (docs/spec/INVARIANTS.md oder bestehendes Spec-Dokument)
2. **Test in ironcrab-eval implementieren** (nur über öffentliche API, keine Interna)
3. **Im Impl-Repo:** Original behalten (als Regression) oder entfernen, wenn Eval-Test Deckung übernimmt
4. **CI prüfen:** `cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`

---

## 7. Neue offene Architektur-/Recovery-Invariante

**Thema:** DEX-uebergreifende Recovery-Semantik nach strukturellem Cache-/Account-Mismatch

**Gewuenschte Invariante:**
- Hot Path (regulaere Buys/Sells) bleibt RPC-frei und blockiert nicht auf Recovery-Warten.
- Cold Path (Liquidation / manuelle Recovery) darf bei nachgewiesen stale/invalid cache einen Request an `market-data` senden.
- Dieser Recovery-Request muss semantisch ein **force refresh** sein, nicht cache-first.
- `market-data` publiziert den autoritativen Refresh als PoolCacheUpdate; erst der folgende Versuch nutzt den neuen State.
- Diese Semantik soll nach und nach fuer alle relevanten DEX-Pfade umgesetzt werden, damit sellbare Token nicht wegen stale State in der Wallet liegen bleiben.

**Warum offen:**
- Die aktuelle Architektur trennt Hot Path vs. Cold Path korrekt auf dem Papier, aber die Recovery-Semantik ist bisher nur teilweise und DEX-spezifisch umgesetzt.
- Das Ziel ist eine schrittweise DEX-Ausweitung in kleinen Scopes, nicht ein grosser Refactor.

**Impl-/Eval-Status (2026-03-24):**
- Cold-Path-Recovery mit `force_refresh=true` und bounded Wait/Retry ist im Impl-Repo gemergt.
- Hot-Path fuer regulaere `momentum-bot` PumpSwap-SELLs triggert nach strukturellem Sim-Fail einen nicht-blockierenden async Refresh an `market-data`, ohne Retry im selben Intent.
- Wiederholte Hot-Path-Refreshes werden lokal per Mint gededupliziert; der Cooldown startet erst nach erfolgreichem `nats.publish -> Ok(true)`.
- Der Healing-Pfad ist zusaetzlich ueber Runtime-Metriken beobachtbar (Trigger, suppressed, publish ok/fail, no-NATS).
- Der Eval-Vertrag ist gemergt; Bugbot-Findings zu duplizierten Tests und unnoetiger Ein-Datei-Helper-Abstraktion wurden vor Merge bereinigt.
- PumpSwap ist damit der erste vollstaendige Slice.
- PumpFun Bonding Curve Cold-Path-Recovery ist im Impl-Repo jetzt ebenfalls gemergt.
- Naechster direkter Schritt ist dafuer ein enger Eval-Vertrag fuer force-refresh / autoritativen `market-data`-Refresh / bounded one-retry im Cold Path.

**Empfohlene Test-Richtung:**
- Pro DEX-Slice einen engen Blackbox-Vertrag formulieren statt einen grossen All-at-once-Test.
- Fuer PumpSwap ist dieser Slice bereits erledigt.
- Als naechstes fuer PumpFun Bonding Curve:
  - stale-state / struktureller Sim-Fail im Cold Path
  - autoritativer Refresh ueber `market-data`
  - kein cache-first-Wiederverwenden desselben fehlerhaften States
  - genau ein bounded Retry nach dem Refresh

---

## 6. Querbezüge

- **EVAL_TEST_CANDIDATES.md** – Vollständige Kandidaten-Liste
- **ARCHITECTURE_AUDIT.md** (Iron_crab) – Offene Architektur-Themen, BUG A
- **INVARIANTS.md** – Eval-getestete vs. nicht getestete Invarianten
- **DEFINITION_OF_DONE.md** – DoD §G Golden Replays, §H Connector Contracts

---

*Erstellt: Test Authority, ironcrab-eval*
