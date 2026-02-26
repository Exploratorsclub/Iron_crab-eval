# Eval-Test-Kandidaten: Migration von Iron_crab nach ironcrab-eval

**Zweck:** Übersicht aller Tests mit Bewertung:
1. **Testet er an der API-Grenze / als Blackbox?**
2. **Kann er als Invariante aus der Spec formuliert werden?**
3. **Eval-Kandidat** (Blackbox + Spec-Invariante) → Migration nach ironcrab-eval
4. **Bleibt Unit/Integration** → bleibt im Iron_crab Repo

---

## Legende

| Symbol | Bedeutung |
|-------|-----------|
| ✅ | Ja |
| ⚠️ | Teilweise |
| ❌ | Nein |
| 🔄 | Eval-Kandidat (Migrationsziel) |
| 📦 | Bleibt im Impl-Repo (Unit/Integration) |

---

## 1. ironcrab-eval/tests/ (bereits in Eval)

| Datei | Blackbox? | Spec-Invariante? | Status |
|-------|-----------|-----------------|--------|
| `invariants_quote_monotonic.rs` | ✅ | ✅ | Bereits Eval-Invariante |
| `invariants_lock_manager.rs` | ✅ | ✅ | Bereits Eval-Invariante |
| `ipc_schema_serde.rs` | ✅ | ✅ | IPC Schema Spec (STORAGE_CONVENTIONS §4, DoD §B/E) |
| `pump_amm_geyser_first.rs` | ✅ | ✅ | Bereits Eval-Invariante |
| `invariants_6005_detection.rs` | ✅ | ✅ | 6005-Retry Error-Detection |
| `invariants_router_slippage.rs` | ✅ | ✅ | cumulative_min_out + Multi-Hop min_out + Best Quote Selection |
| `invariants_dex_connector.rs` | ✅ | ✅ | DEX Connector Contracts (5 Tests) |
| `invariants_arbitrage_profit.rs` | ✅ | ✅ | Arbitrage Profit-Filter |

---

## 2. Iron_crab/tests/ – Eval-Kandidaten (Migration geplant)

### 2.1 Priorität 1: Klare Blackbox + Spec-Invariante

| Datei | Tests | Blackbox? | Spec-Invariante? | Spec-Referenz |
|-------|-------|-----------|-----------------|---------------|
| `dex_connector_contracts.rs` | `contract_pump_amm_quote_monotonic` | ✅ | ✅ | DoD §H Connector Contracts |
| `dex_connector_contracts.rs` | `contract_pump_amm_price_impact_non_decreasing` | ✅ | ✅ | DoD §H Connector Contracts |
| `dex_connector_contracts.rs` | `contract_pump_amm_unknown_pair_returns_none` | ✅ | ✅ | DoD §H Connector Contracts |
| `dex_connector_contracts.rs` | `contract_pump_amm_zero_input` | ✅ | ✅ | DoD §H Connector Contracts |
| `dex_connector_contracts.rs` | `contract_pump_amm_build_ix_valid_accounts` | ✅ | ✅ | DoD §H Instruction-Builder |
| `pump_amm_geyser_first_test.rs` | `test_quote_from_cache_no_rpc`, `test_pool_accounts_from_cache_no_rpc` | ✅ | ✅ | TARGET_ARCHITECTURE Geyser-First |
| `router_min_out.rs` | `cumulative_min_out_applies_slippage_on_final_amount` | ✅ | ✅ | DoD §C Slippage auf finales Output |
| `arbitrage_profit.rs` | `profit_filter_accepts_and_rejects` | ✅ | ✅ | Arbitrage Profit-Filter Invariante |

**Migrationsplan (Priorität 1):**
- DEX-Connector-Contract-Tests → `tests/invariants_dex_connector.rs`
- Pump-AMM-Geyser-First → bereits in eval (`pump_amm_geyser_first.rs`), Duplikat in Impl prüfen
- Router min_out → `tests/invariants_router_slippage.rs`
- Arbitrage profit → `tests/invariants_arbitrage_profit.rs`

---

### 2.2 Priorität 2: Spec-Invariante, teils nicht Blackbox

| Datei | Tests | Blackbox? | Spec-Invariante? | Anmerkung |
|-------|-------|-----------|-----------------|-----------|
| `ipc_schema_roundtrip.rs` | Alle Schema-Tests | ✅ | ✅ | Migriert → ipc_schema_serde.rs |
| `golden_replay_test.rs` | `golden_replay_*` | ❌ | ✅ | Nutzt `simulate_decision()` – **nicht** echte Execution-Engine API. Spec: DoD §G Replay-Determinismus. |

**Migrationsplan (Priorität 2):**
- **ipc_schema_roundtrip**: Migriert – Schema-Roundtrip-Tests in `tests/ipc_schema_serde.rs` (Spec-getrieben neu implementiert, STORAGE_CONVENTIONS §4, DoD §B/E)
- **golden_replay**: Invariante „deterministisches Replay“ ist Spec-konform, aber aktuell als **Unit-Test** mit Nachbau der Engine-Logik. **Eval-Variante**: Blackbox über echte execution-engine API oder über NATS-Intent → DecisionRecord-End-to-End. Das erfordert entweder:
  - (A) Subprocess/Integration: Intent per NATS senden, Decision per JSONL/Fixture vergleichen
  - (B) Invariante nur dokumentieren, konkreten Blackbox-Test später ergänzen

---

### 2.3 Priorität 3: Teilweise Blackbox, teilweise Spec-Invariante

| Datei | Tests | Blackbox? | Spec-Invariante? | Empfehlung |
|-------|-------|-----------|-----------------|------------|
| `router_hops2_plan.rs` | `router_builds_hops2_plan_with_min_out` | ✅ | ✅ | Migriert → invariants_router_slippage.rs |
| `router_best_quote.rs` | `router_picks_higher_out_amount` | ✅ | ✅ | Migriert → invariants_router_slippage.rs |
| `arbitrage_cycle_pruning.rs` | `pruning_keeps_profitable_cycle` | ✅ | ⚠️ | Arbitrage-Engine; Evtl. Invariante; eher Unit |
| `arbitrage_profit_ranking.rs` | `profit_ranking_orders_cycles` | ✅ | ⚠️ | Evtl. Invariante; eher Unit |
| `arbitrage_edge_aggregate.rs` | `aggregate_picks_higher_output` | ✅ | ⚠️ | Evtl. Invariante; eher Unit |
| `arbitrage_cycle_generic.rs` | `enumerate_4hop_cycle` | ✅ | ⚠️ | N-Hop-Enumeration; eher Unit |
| `execution_orca_builder.rs` | `test_orca_build_swap_ix_*` | ✅ | ⚠️ | DoD §H Instruction-Gültigkeit; Build-IX-Validität |
| `execution_pumpfun_builder.rs` | `test_pumpfun_build_*` | ✅ | ⚠️ | DoD §H Instruction-Gültigkeit |
| `raydium_quote.rs` | `slippage_min_out`, `slippage_bounds` | ✅ | ⚠️ | Slippage-Berechnung; DoD §H |
| `compute_budget_estimator.rs` | `single_swap_estimate_in_range`, `large_notional_*` | ✅ | ⚠️ | CU-Schätzung; eher Config/Policy |
| `hot_reload_smoke_test.rs` | ConfigUpdate-Tests | ✅ | ⚠️ | DoD §I Runtime-Config; Schema-Konsistenz |

**Empfehlung:** Diese Tests bleiben vorerst im Impl-Repo. Bei Bedarf können einzelne als Invarianten in die Spec aufgenommen und in eval neu implementiert werden.

---

## 3. Iron_crab/tests/ – Bleiben im Impl-Repo (Unit/Integration)

### 3.1 Unit-Tests (Implementierungsdetails, keine Spec-Invariante)

| Datei | Tests | Blackbox? | Spec-Invariante? | Begründung |
|-------|-------|-----------|-----------------|------------|
| `dex_parser_orca.rs` | `parse_orca_*` | ⚠️ | ❌ | Parser-Interna, kein API-Vertrag |
| `clamping_logic.rs` | Alle | ✅ | ❌ | Metrics-Clamping, keine Spec-Invariante |
| `treasury_env_fallback_test.rs` | Env-Fallback | ⚠️ | ❌ | Env-Var-Fallback, Implementierungsdetail |
| `token_decimals_fallback.rs` | Decimals-Fallback | ⚠️ | ❌ | Fallback-Logik, keine Spec-Invariante |
| `sniper_partial_exit.rs` | `partial_exit_proportional_*` | ❌ | ❌ | Sniper deprecated, mathematische Logik |
| `raydium_swap_plan.rs` | `swap_plan_without_pools_returns_none` | ⚠️ | ❌ | Fehlende Pools, kein klares Spec |
| `raydium_swap_ix.rs` | `raydium_build_swap_instruction_*` | ⚠️ | ❌ | Instruction-Builder Placeholder |

### 3.2 Integration-Tests (Live-RPC, Mainnet, Debug)

| Datei | Tests | Blackbox? | Spec-Invariante? | Begründung |
|-------|-------|-----------|-----------------|------------|
| `pumpfun_live_token.rs` | `test_pumpfun_live_token_quote` | ⚠️ | ❌ | Live-RPC, `#[ignore]` |
| `pumpfun_real_tokens.rs` | `test_pumpfun_with_real_tokens` | ⚠️ | ❌ | Live-RPC, Mainnet |
| `cpmm_mainnet_integration.rs` | `test_cpmm_*` | ⚠️ | ❌ | Mainnet Integration, `#[ignore]` |
| `meteora_dlmm_integration.rs` | `test_meteora_dlmm_*` | ⚠️ | ❌ | Mainnet Integration, `#[ignore]` |
| `raydium_simulation.rs` | `raydium_swap_plan_simulation_layout` | ⚠️ | ❌ | Live-RPC, `#[ignore]` |
| `integration_buy_fill_sell.rs` | (falls vorhanden) | ❌ | ❌ | Legacy, ggf. inaktiv |
| `verify_creator_vault.rs` | Debug | ❌ | ❌ | PDA-Debug, kein Test |
| `debug_burunduk_vault.rs` | `test_burunduk_creator_vault` | ❌ | ❌ | PDA-Debug |
| `bench_quote_refresh.rs` | `timing_refresh_and_quote` | ⚠️ | ❌ | Benchmark, kein Invarianten-Test |

### 3.3 Helper / Keine Tests

| Datei | Anmerkung |
|-------|-----------|
| `common.rs` | Helper-Funktionen, keine Tests |

---

## 4. Zusammenfassung: Eval-Kandidaten nach Migration

| Kandidat | Quelle | Ziel in ironcrab-eval |
|----------|--------|------------------------|
| DEX Connector Contracts (5 Tests) | `dex_connector_contracts.rs` | `tests/invariants_dex_connector.rs` |
| Router Slippage | `router_min_out.rs` | `tests/invariants_router_slippage.rs` |
| Arbitrage Profit Filter | `arbitrage_profit.rs` | `tests/invariants_arbitrage_profit.rs` |
| IPC Schema (erweitert) | `ipc_schema_roundtrip.rs` | `tests/ipc_schema_spec.rs` (Merge mit `ipc_schema_serde.rs`) |
| Golden Replay (Blackbox-Variante) | `golden_replay_test.rs` | `tests/golden_replay_blackbox.rs` (neu, über API/NATS) |

**Bereits in eval:** `invariants_quote_monotonic`, `invariants_lock_manager`, `ipc_schema_serde` (14 Tests, STORAGE_CONVENTIONS §4, DoD §B/E), `pump_amm_geyser_first`, `invariants_6005_detection`

---

## 5. Migrationsplan-Vorlage (pro Test)

Für jeden Eval-Kandidaten:

| Schritt | Aktion |
|--------|--------|
| 1 | Invariante in Spec formulieren (docs/spec/ oder bestehendes Spec-Dokument) |
| 2 | Test in ironcrab-eval neu implementieren (nur über öffentliche API, keine Interna) |
| 3 | Im Impl-Repo: Original behalten (als Regression) oder entfernen (wenn Eval-Test Deckung übernimmt) |

---

## 6. Offene Kandidaten (Später prüfen)

- **Router hops2/best_quote**: Sollten diese als Invarianten in die Spec?
- **Arbitrage Engine** (pruning, ranking, aggregate, cycle): Architektur-Spec für Arb-Engine vorhanden?
- **Golden Replay Blackbox**: Braucht Subprocess execution-engine oder NATS-Mock?

---

*Erstellt: Test Authority, ironcrab-eval*
