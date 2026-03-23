# Invarianten

Formale Verhaltens-Invarianten. AI darf diese Regeln **niemals** verletzen. Bei Refactors/Features: Invariants prüfen.

**Legende:**
- **Eval-getestet**: Durch Tests in `ironcrab-eval/tests/` geprüft
- **Leitlinie**: Architektur-Regel, nicht als Eval-Test abgedeckt
- **Ziel**: Noch nicht erfüllt, dokumentiert für Zukunft

**Quellen:** TARGET_ARCHITECTURE.md, DEFINITION_OF_DONE.md, ROLE_SEPARATION.md, Iron_crab/docs/INVARIANTS.md

---

## A. Eval-getestet (P1-Invarianten)

Diese Invarianten werden durch Blackbox-Tests in ironcrab-eval verifiziert.

### A.1 Quote-Monotonie
- **Datei:** `tests/invariants_quote_monotonic.rs`
- **Invariante:** Größeres `amount_in` führt zu mindestens gleichem `amount_out` bei `quote_output_amount`.
- **Formal:** `amount_in1 < amount_in2` → `amount_out1 <= amount_out2`

### A.2 LockManager
- **Datei:** `tests/invariants_lock_manager.rs`
- **Invarianten:**
  - `total_locked + available` = initial (SOL-Erhaltung über Lock/Release) — *I-20*
  - Gleicher Intent-ID nicht doppelt gelockt (Capital Lock) — *I-22 Idempotency*

### A.3 DEX Connector
- **Datei:** `tests/invariants_dex_connector.rs`, `tests/invariants_orca_ix.rs`, `tests/invariants_pumpfun_ix.rs`
- **Invarianten:**
  - **Quote-Monotonie:** `amount_in1 < amount_in2` → `amount_out1 <= amount_out2`
  - **Price-Impact:** Größeres amount_in → mindestens gleicher oder höherer price_impact_bps
  - **Unknown Pair:** Kein Pool für Input/Output-Mint → `None` oder `Ok(None)`
  - **Zero Input:** amount_in = 0 → `None` oder amount_out = 0
  - **Build IX:** `build_swap_ix_from_pool_accounts` liefert nicht-leere Instructions mit korrektem program_id (PumpFunAmmDex)
  - **Orca build_swap_ix:** user signer, user ATAs writable, data nicht leer (DoD §H)
  - **PumpFun build_swap_ix:** 2 IXs (ATA + swap), program_id pump.fun, user bei Index 6 signer+writable (DoD §H)
  - **TxBuilder SELL:** `tx_builder::build_tx_plan` mit PumpFun SELL-Intent (creator + min_out_raw) liefert `TxPlanOutcome::Planned` mit 2 IXs (ATA + pump.fun), program_id pump.fun, User Index 6 signer+writable. Pure Derivation (kein RPC).

### A.4 Geyser-First / Cache-Hit
- **Datei:** `tests/pump_amm_geyser_first.rs`
- **Invariante:** Cache-Hit liefert Quote und pool_accounts ohne RPC-Aufruf.
- **Kontext:** I-4, I-16 — Geyser autoritativ im Hot Path

### A.5 Router Slippage & Best Quote
- **Datei:** `tests/invariants_router_slippage.rs`
- **Invarianten:**
  - `cumulative_min_out`: Slippage auf letztes amount_out
  - **Multi-Hop-Plan:** `build_best_hops2_plan_exact_in` liefert `min_out = expected_out * (10_000 - slippage_bps) / 10_000`; Plan hat 2 Hops und 2 Swap-Instructions
  - **Best Quote Selection:** `best_quote_exact_in` liefert den Quote mit höchstem amount_out unter allen DEXs; wenn nur ein DEX ein Quote liefert, wählt der Router dieses (Raydium+Orca-Szenario)

### A.6 Arbitrage Profit Filter
- **Datei:** `tests/invariants_arbitrage_profit.rs`
- **Invariante:** `compute_net_profit` filtert korrekt nach ROI und tx_cost

### A.7 IPC / Schema (STORAGE_CONVENTIONS §4, DoD §B/E)
- **Datei:** `tests/ipc_schema_serde.rs`
- **Invarianten:**
  1. **RecordHeader:** Nach Serde-Roundtrip sind schema_version, component, build, run_id erhalten; ts_unix_ms > 0.
  2. **ExplicitAmount:** Nach Roundtrip sind raw, decimals, ui erhalten (DoD §B Units).
  3. **MarketEvent:** Nach Roundtrip sind event_id, source, slot, kind erhalten (inkl. TokenMintInfo).
  4. **TradeIntent:** Nach Roundtrip sind intent_id, source, tier, required_capital, resources, origin_type, regime, execution.min_out erhalten.
  5. **DecisionRecord:** Nach Roundtrip sind decision_id, intent_id, checks, outcome, source, primary_reject_reason, simulate (bei SimFailed) erhalten.
  6. **ExecutionResult:** Nach Roundtrip sind execution_id, decision_id, intent_id, signature, status, source, fees, pnl, fill_status, fill_unavailable_reason, error_code erhalten.
  7. **RejectReason:** Jeder Eintrag serialisiert/deserialisiert zu identischem Wert (DoD §J SCREAMING_SNAKE_CASE).
  8. **Intent Causality Chain:** intent_id verknüpft Intent → Decision → Execution.
  9. **JSONL:** Jede Zeile valides JSON; Records zeilenweise parsebar.

### A.8 6005 BondingCurveComplete-Erkennung
- **Datei:** `tests/invariants_6005_detection.rs`
- **Invariante:** `is_6005_bonding_curve_complete(err)` erkennt BondingCurveComplete (6005) in Fehlermeldungen.
- **Formal:** Enthält der Error-String "6005" oder "0x1775" → true; sonst false.
- **Kontext:** Voraussetzung für 6005-Retry in Liquidation (PumpFun → PumpSwap AMM).

### A.9 Raydium Slippage (DoD §H)
- **Datei:** `tests/invariants_raydium_slippage.rs`
- **Invariante:** `Raydium::apply_slippage_min_out(amount_out, slippage_bps)` = amount_out * (10_000 - slippage_bps) / 10_000
- **Randfälle:** slippage_bps = 0 → unverändert; slippage_bps >= 10_000 → 0

### A.10 Replay Determinism
- **Datei:** `tests/golden_replay_blackbox.rs`
- **Invariante:** Dieselbe Intent-Sequenz erzeugt bit-identische Decision-Streams.
- **Formal:** Replay(intents) → decisions; Replay(intents) → decisions'; decisions == decisions'
- **Kontext:** Spawnt execution-engine mit `--replay`; vergleicht gegen Fixtures (rejected_trade, sim_failed, normal_trade_simsucc). DoD G.P1.

### A.11 Pool-Matching (I-13, FIX-38)
- **Datei:** `tests/invariants_pool_matching.rs`
- **Invariante:** `should_apply_position_price_update(position_pool, source_pool)` gibt nur dann true zurück, wenn source_pool == position.pool oder source_pool ist None oder position.pool ist leer.
- **Formal:** Apply iff source_pool.is_none() || position_pool.is_empty() || position_pool == source_pool
- **Kontext:** Verhindert falsche PnL und TAKE_PROFIT bei Multi-Pool-Tokens (Bonding Curve + AMM).

### A.12 Hot-Path RPC-Freiheit (I-4, I-7)
- **Datei:** `tests/invariants_hot_path_no_rpc.rs`
- **Invariante:** DEX-Connectors liefern bei Cache-Miss None/Err ohne RPC (Hot Path).
- **Getestet:** PumpFunAmmDex (quote, pool_accounts), Raydium, RaydiumCpmm, MeteoraDlmm (allow_rpc_on_miss=false). Orca (live_pool_cache gesetzt → bei Vault-Miss statische Reserves, kein RPC).
- **Kontext:** Hot Path (Arb, Momentum) darf keine blockierenden RPC-Calls ausführen.

### A.13 Liquidation 6005-Retry Komponenten (ARCHITECTURE_AUDIT BUG A)
- **Datei:** `tests/invariants_liquidation_flow.rs`
- **Invariante:** Nach 6005 (BondingCurveComplete) wird `mark_pumpfun_complete_for_mint` aufgerufen; danach liefert `is_pumpfun_complete_for_mint` Some(true). Liquidation Phase 2 überspringt damit PumpFun und nutzt Multi-Pool (PumpSwap AMM).
- **Getestet:** mark_pumpfun_complete_for_mint → is_pumpfun_complete_for_mint; find_pump_amm_pool_by_base_mint; get_pump_amm_pool_accounts_by_base_mint.
- **E2E-Test:** `golden_replay_liquidation_6005_retry` prüft Replay-Determinismus für die 6005-Fixture (2 Decisions: SimFailed PumpFun + SimFailed Retry PumpSwap AMM; LockManager-Seeding).
- **Kontext:** I-4, I-7; PumpFun BondingCurve migriert zu PumpSwap AMM → 6005-Retry erforderlich.

### A.14 Arbitrage Edge-Aggregation
- **Datei:** `tests/invariants_arbitrage_engine.rs`
- **Invariante:** `aggregate_best_edges` liefert pro Pair den Quote mit maximalem `amount_out` über alle DEX-Connectors.

### A.15 Arbitrage Cycle-Ranking
- **Datei:** `tests/invariants_arbitrage_engine.rs`
- **Invariante:** `rank_triangular_cycles` sortiert Cycles absteigend nach Profit; höchster Profit zuerst.

### A.16 Arbitrage Pruning
- **Datei:** `tests/invariants_arbitrage_engine.rs`
- **Invariante:** Dominance-Pruning entfernt inferiore parallele Edges, behält aber mindestens einen profitable Cycle.

### A.17 Arbitrage N-Hop-Enumeration
- **Datei:** `tests/invariants_arbitrage_engine.rs`
- **Invariante:** `enumerate_cycles_generic` findet alle Cycles bis max_depth; für 4-Hop-Graph existiert A->B->C->D->A.

### A.18 Compute-Budget-Estimator
- **Datei:** `tests/invariants_compute_budget.rs`
- **Invarianten:**
  - `estimate_single_swap(notional)` liefert `compute_unit_limit` in [80k, 400k] und `compute_unit_price_micro_lamports >= 1`.
  - Bei `notional_in >= large_notional_threshold` wird `compute_unit_price_micro_lamports` mit `large_notional_multiplier` multipliziert (Default: 3x).

### A.19 tokens_per_sol (I-14)
- **Datei:** `tests/invariants_tokens_per_sol.rs`
- **Invariante:** LOWER tokens_per_sol = token wertvoller. pnl_pct = (entry/current - 1)*100. highest_price = niedrigster tps (bester Preis für Holder).
- **Getestet:** pnl_pct, updated_highest_price, drawdown_from_ath_pct.
- **Kontext:** Verhindert invertierte Exit-Signale (FIX-PNL, BUG-Pattern).

### A.20 DEX Parser PumpSwap BUY/SELL (FIX: Guard-Check)
- **Datei:** `tests/invariants_dex_parser_pumpswap.rs`
- **Invariante:** `parse_pumpfun_amm_transaction()` parst sowohl BUY (23 Accounts) als auch SELL (21 Accounts) korrekt.
- **Formal:** BUY-Discriminator + 23 Accounts → Some(Trade { is_buy: true }). SELL-Discriminator + 21 Accounts → Some(Trade { is_buy: false }). <21 Accounts → None.
- **Kontext:** KNOWN_BUG_PATTERNS #14; Guard-Check war != 23, jetzt < 21.

### A.21 DEX Parser Inner-Instruction Fallback (Aggregator-CPI)
- **Datei:** `tests/invariants_dex_parser_cpi.rs`
- **Invariante:** `parse_dex_transaction()` erkennt DEX-Trades auch wenn sie als Inner Instruction (CPI) ausgeführt werden.
- **Formal:** Top-Level = unbekanntes Programm, Inner Instruction = bekannter DEX-Swap → Some(Trade). Top-Level = bekannter DEX → Some(Trade) (kein Fallback nötig).
- **Kontext:** Aggregator-Trades (Jupiter, etc.) rufen DEX-Programme als CPI auf.

### A.22 PumpFun BUY Account Count (Post-Cashback-Upgrade)
- **Datei:** `tests/invariants_pumpfun_cashback.rs`
- **Invariante:** `build_buy_ix()` liefert genau 17 Accounts. Das letzte Account (Index 16) ist bonding_curve_v2 PDA.
- **Formal:** ix.accounts.len() == 17. ix.accounts[16].pubkey == PDA(['bonding-curve-v2', mint], pumpfun_program). ix.accounts[16].is_signer == false. ix.accounts[16].is_writable == false.

### A.23 PumpFun SELL Account Count (Post-Cashback-Upgrade)
- **Datei:** `tests/invariants_pumpfun_cashback.rs`
- **Invariante:** `build_sell_ix(cashback=false)` liefert 15 Accounts. `build_sell_ix(cashback=true)` liefert 16 Accounts. bonding_curve_v2 ist jeweils das LETZTE Account.
- **Formal:** Non-cashback: ix.accounts.len() == 15, ix.accounts[14] == bonding_curve_v2. Cashback: ix.accounts.len() == 16, ix.accounts[15] == bonding_curve_v2, ix.accounts[14] == user_volume_accumulator.

### A.24 BondingCurveState cashback_enabled Parsing
- **Datei:** `tests/invariants_pumpfun_cashback.rs`
- **Invariante:** BondingCurveState::parse() liest cashback_enabled aus Byte 82.
- **Formal:** 81-Byte data → cashback_enabled == false. 151-Byte data mit data[82]==1 → cashback_enabled == true. 151-Byte data mit data[82]==0 → cashback_enabled == false.

### A.25 PumpFun Market Order BUY (buy_exact_sol_in)
- **Datei:** `tests/invariants_pumpfun_market_order.rs`
- **Invariante:** build_buy_exact_sol_ix() liefert genau 17 Accounts (identisch zu build_buy_ix). Instruction-Data beginnt mit Discriminator [56, 252, 116, 8, 158, 223, 205, 95]. sol_amount und min_tokens_out werden korrekt serialisiert.
- **Formal:** ix.accounts.len() == 17. ix.data[0..8] == [56, 252, 116, 8, 158, 223, 205, 95]. u64::from_le_bytes(ix.data[8..16]) == sol_amount. u64::from_le_bytes(ix.data[16..24]) == min_tokens_out.

### A.26 PumpFun Market Order bonding_curve_v2 Position
- **Datei:** `tests/invariants_pumpfun_market_order.rs`
- **Invariante:** build_buy_exact_sol_ix() hat bonding_curve_v2 als letztes Account (Index 16), identisch zu build_buy_ix().
- **Formal:** ix.accounts.last().unwrap().pubkey == PDA(['bonding-curve-v2', mint], pumpfun_program). !is_signer. !is_writable.

### A.27 LockManager Atomic Wallet Updates (SOL/WSOL Entkopplung)
- **Datei:** `tests/invariants_wallet_update.rs`
- **Invariante:** `update_native_sol_only()` aendert nur native SOL; `update_wsol_only()` aendert nur WSOL. Nach simuliertem Wrap ist `total_native_sol() + wsol_balance()` konsistent.
- **Formal:** update_native_sol_only(X) → total_native_sol() aendert sich, wsol_balance() bleibt gleich. update_wsol_only(Y) → wsol_balance() aendert sich, total_native_sol() bleibt gleich. Wrap-Simulation: sol_before + wsol_before == sol_after + wsol_after (modulo Fees).
- **Kontext:** KNOWN_BUG_PATTERNS #23; Non-atomic SOL/WSOL Event-Updates verursachten falsche wallet_total_sol_lamports Metrik.

### A.28 Open Positions Counter Konsistenz (Single Source of Truth)
- **Datei:** `tests/invariants_open_positions.rs`
- **Invariante:** `get_open_positions()` wird aus LockManager `count_non_zero_token_balances()` abgeleitet (nicht als separater Counter). Der Wert stimmt stets mit der Anzahl non-zero Eintraege in `available_tokens` ueberein.
- **Formal:** `get_open_positions() == available_tokens.values().filter(|b| b > 0).count()`. Nach N BUY-Fills: count == N. Nach Sell-All: count == 0. Nach Restart-Recovery: count == tatsaechlicher Bestand.
- **Kontext:** KNOWN_BUG_PATTERNS #5 (Ghost Positions); dual-path tracking (Execution Result + Geyser Balance) verursachte Race Conditions und Counter-Drift.

### A.29 Liquidation Vollstaendigkeit (Kill-Switch SELL)
- **Datei:** `tests/invariants_liquidation_flow.rs` (erweitert)
- **Invariante:** Liquidation erkennt alle non-zero Token im Wallet, baut fuer jeden ein korrektes SELL-Intent und scheitert nicht an fehlenden Cache-Daten (Cold Path: RPC erlaubt). PumpFun-Token mit `cashback_enabled=true` erhalten 16-Account SELL Layout (mit `user_volume_accumulator`).
- **Formal:** RPC-Inventory(N Token) → N Liquidation-Intents. `cashback_enabled` wird per RPC verifiziert wenn Cache-Miss. `run_liquidation_job` blockiert Main-Loop nicht (tokio::spawn).
- **Kontext:** Liquidation scheiterte weil: (a) `cashback_enabled` auf `false` defaulted (pool_cache_sync.rs), (b) `run_liquidation_job().await` Main-Loop blockierte.

### A.30 cashback_enabled JetStream-Propagierung
- **Datei:** `tests/invariants_pumpfun_cashback.rs` (erweitert)
- **Invariante:** `cashback_enabled` muss korrekt von Geyser ueber JetStream PoolCacheUpdate-Metadata zum SLAVE LivePoolCache propagiert werden. JetStream-bootstrapped PumpFun-Pools duerfen `cashback_enabled` NICHT auf `false` hardcoden wenn der Wert im Metadata vorhanden ist.
- **Formal:** PoolCacheUpdate mit metadata.cashback_enabled="true" → build_minimal_pool_state() → PumpFunState.cashback_enabled == true. PoolCacheUpdate OHNE cashback_enabled in metadata → PumpFunState.cashback_enabled == false (backward-compat default).
- **Kontext:** Root Cause des Custom(6024) Overflow: JetStream-Cache hatte cashback_enabled=false, Cache-HIT verhinderte RPC-Fallback, build_sell_ix liess user_volume_accumulator weg.

### A.31 Cold Path cashback_enabled RPC-Verifikation
- **Datei:** `tests/invariants_liquidation_flow.rs` (erweitert)
- **Invariante:** Im Cold Path (allow_rpc_fallback=true, z.B. Liquidation) muss `cashback_enabled` IMMER per RPC verifiziert werden, auch wenn der LivePoolCache einen HIT liefert. Ein Cache-HIT mit cashback_enabled=false darf im Cold Path NICHT blind vertraut werden.
- **Formal:** build_swap_ix_async_with_slippage(allow_rpc_fallback=true) fuer Token mit on-chain cashback_enabled=true → Instruction hat 16 Accounts (mit user_volume_accumulator), unabhaengig vom Cache-Wert.
- **Kontext:** JetStream-Cache liefert cashback_enabled=false (nicht im Metadata), Cache-HIT verhindert RPC-Fallback → falsches Account-Layout → Overflow(6024).

### A.32 Cold Path pump_amm degenerate Reserves RPC-Fallback
- **Datei:** `tests/invariants_pumpswap_amm_liquidation.rs`
- **Invariante:** Im Cold Path (allow_rpc_on_miss=true, z.B. Liquidation) muss `pump_amm` degenerate Cache-Reserves als Problem erkennen. Degenerierter State darf nicht still als "valider Quote-None-Fall" durchgehen.
- **Formal:** (a) quote_output_amount mit base_reserve=0 oder quote_reserve=0 → Err. (b) Cold Path quote_exact_in mit degenerate Cache und RPC unreachable → Err (nicht Ok(None)). (c) Valide Reserves → Ok mit positivem amount_out.
- **Getestet:** pumpamm_degenerate_cache_reserves_quote_zero_rejected; pumpamm_degenerate_cache_reserves_base_zero_rejected; pumpamm_valid_reserves_quote_succeeds; pumpamm_cold_path_degenerate_reserves_yields_err_not_ok_none; balance_updated_partial_base_only_preserves_value.
- **Kontext:** Nach Restart werden PumpSwap AMM Pools mit (0,0) entdeckt. Vault-Balance-Updates kommen asynchron. Cache-HIT mit degenerate Reserves darf RPC-Fallback nicht still verhindern.

### A.38 Cold-Path Discovery nur per Request/Reply (I-24d)
- **Datei:** `tests/invariants_pumpswap_amm_liquidation.rs`
- **Invariante:** Wenn execution-engine im Cold Path (Liquidation, manual actions, 6005-Retry) fuer die Ausfuehrung notwendige pool_accounts fehlen, darf sie hoechstens eine korrelierte Discovery-Anforderung an market-data senden und begrenzt auf die autoritative Antwort warten. execution-engine darf fehlende pool_accounts weder selbst discovern noch lokal als Ersatz-Truth in den SLAVE Cache schreiben.
- **Beobachtbarer Vertrag (Eval):** (a) Fehlende pool_accounts fuehren nicht zu lokaler Engine-Truth-Heilung; Cache-Postcondition: keine lokal geheilten pool_accounts sichtbar. (b) Autoritativer PoolCacheUpdate macht den Zustand verfuegbar. (c) Nach autoritativem Update kann der naechste Versuch fortfahren (14 pool_accounts). (d) not_found liefert Ok(None); externer Fehler (RPC unreachable) liefert Err – belastbar getrennt.
- **Getestet:** i24d_missing_pool_accounts_no_local_healing; i24d_authoritative_update_makes_state_available; i24d_after_authoritative_update_retry_can_proceed; i24d_not_found_clear_failure; i24d_external_failure_clear_failure.
- **On-Wire Contract (I-24d PumpSwap):** `tests/request_reply_e2e_contract.rs`: EnsurePumpAmmPoolAccounts (base_mint) → market-data → korrelierte Response. Test pollt bounded auf TOPIC_CONTROL_RESPONSES und filtert nach request_id; erwartet terminalen Outcome (ok|not_found|error). Beweist nur den PumpSwap pool_accounts Request/Reply-Contract; Externer Fehler weiterhin ueber RPC unreachable approximiert.
- **Kontext:** I-24d; Cold Path darf nur ueber market-data pool_accounts erhalten.

### A.43 PumpSwap Cold-Path Recovery: force_refresh und pool_address_hint (I-24e)
- **Dateien:** `tests/invariants_pumpswap_amm_liquidation.rs`, `tests/ipc_schema_serde.rs`
- **Invariante A (Recovery vs. stale Cache):** Loest die Cold-Path-Recovery nach strukturellem PumpSwap-Simulationsfehler einen Pfad mit `force_refresh` aus, darf dieselbe stale 14er-`pool_accounts`-Liste aus dem SLAVE LivePoolCache nicht unveraendert als Truth zurueckkommen (kein stilles cache-first Wiederverwenden). Hot-Path-Dex (`allow_rpc_on_miss=false`) darf bei `force_refresh` weder Cache noch RPC nutzen — beobachtbar als Ok(None).
- **Invariante B (Hint-Pfad):** Der explizite Pool-Hint aus dem Intent-Modell (`TradeIntent.resources.pools[0]`) wird auf dem Wire-Contract als `ControlRequest.pool_address_hint` gefuehrt (nicht im Enum-Variant-Shape). Ein ungueltiger Hint darf nicht in einen unbounded globalen Discovery-Scan ausweichen — beobachtbar: Fehler nach kurzem Timeout.
- **Blackbox-Grenze:** Die Priorisierung „Intent `pools[0]` vor Cache-Lookup“ in der execution-engine ist ohne E2E-Harness (Intent → korreliertes `EnsurePumpAmmPoolAccounts`) nicht separat von der API `pool_accounts_v1_for_base_mint_with_hint` abgegrenzt; der Wire-Vertrag fuer `pool_address_hint` + `force_refresh` ist in `ipc_schema_serde` abgesichert. Vollstaendige End-to-End-Prioritaet engine-intern: offene Luecke bis ein Blackbox-Einstieg (z.B. erweiterter E2E-Contract) existiert.
- **Getestet:** i24e_force_refresh_skips_stale_livepool_cache_pool_accounts; i24e_force_refresh_refuses_without_cold_path_rpc_permission; i24e_pool_address_hint_parse_fail_errors_without_global_scan; control_request_ensure_pump_amm_pool_accounts_force_refresh_and_pool_hint_roundtrip.
- **Kontext:** PR #31 Merge auf architecture-rebuild; Hot-Path-async-Refresh fuer reguläre Sells bleibt explizit out-of-scope (kein Overclaim in diesen Tests).

### A.41 PumpFun Bonding Curve Cold-Path: Stale Cache darf nicht blind dominieren
- **Datei:** `tests/invariants_liquidation_flow.rs`
- **Invariante:** Im Cold Path (allow_rpc_fallback=true) fuer eine **aktive** PumpFun Bonding Curve (complete=false): Ein Cache-HIT mit cashback_enabled=false darf NICHT blind vertraut werden. Wenn RPC unreachable ist, muss der Outcome ein klarer Failure (Err) sein – NICHT stilles Ok mit falschem 15-Account-Layout.
- **Formal:** build_swap_ix_async_with_slippage(allow_rpc_fallback=true) mit Cache(PumpFunState{complete=false, cashback_enabled=false}) und RPC unreachable → Err.
- **Getestet:** pumpfun_cold_path_stale_cache_rpc_unreachable_clear_failure.
- **Kontext:** Bug #25 (cashback_enabled defaults to false → falsches Layout → Custom(6024) Overflow). Getrennt von I-24d (PumpSwap pool_accounts Request/Reply). Layout-Baustein (cashback=true → 16 Accounts) bereits in A.23/A.29 abgedeckt.

### A.42 Cross-DEX Cold-Path Reserve-Fallback (Raydium, RaydiumCpmm, MeteoraDlmm)
- **Datei:** `tests/invariants_cross_dex_cold_path_reserves.rs`
- **Invariante:** Bekannter Pool + fehlende Live-Reserves im Cold Path = autoritativer RPC-Fallback oder klarer Failure. Wenn fuer einen bereits bekannten Pool die Reserve-/Vault-Daten im LivePoolCache fehlen, darf der Cold Path den Fall nicht still wie einen harmlosen Cache-Miss behandeln. Er muss entweder den autoritativen Reserve-State per RPC nachladen oder einen klaren Fehler (Err) liefern. Nicht erlaubt: stilles Ok(None) oder verdeckter lokaler Ersatz-Truth.
- **Formal:** Cold Path (allow_rpc_on_miss=true), bekannter Pool, fehlende Reserves, RPC unreachable → Err (nicht Ok(None)).
- **Getestet:** raydium_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err; raydium_cpmm_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err; meteora_dlmm_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err.
- **Scope:** Nur Raydium, RaydiumCpmm, MeteoraDlmm. Orca nicht. Hot Path bleibt GEYSER-ONLY (A.12). Kein Overclaim ueber Request/Reply, PumpFun, Orca oder komplettes IX-Layout.
- **Kontext:** I-5/I-6 Cold-Path-Leitlinie; A.12 Gegenkontext (Hot Path RPC-frei).

---

## B. Architektur-Invarianten (Leitlinien, kein Eval-Test)

Diese Regeln sind aus Iron_crab/docs/INVARIANTS.md übernommen. Sie werden nicht durch Eval-Tests geprüft, gelten aber als verbindliche Architektur-Vorgaben.

### B.1 Sicherheit und Keys (I-1 bis I-3)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-1 | **Single-Signer**: Nur execution-engine lädt Keys und signiert/sendet | Architekturbruch |
| I-2 | **Intent-only**: market-data, momentum-bot, arb-strategy, control-plane sind **keyless** — erzeugen nur TradeIntent oder MarketEvents | Key-Leak-Risiko |
| I-3 | Prozesse außer execution-engine **crashen mit exit(1)** wenn Key-Env-Vars erkannt | DoD §A |

### B.2 Hot Path vs. Cold Path (I-4 bis I-8)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-4 | **HOT PATH** (Discovery, Buy, Sell, Monitoring): **GEYSER-ONLY**. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. | Latenz-Bruch |
| I-5 | **COLD PATH** (Liquidation, Manual Actions, Bootstrap): RPC erlaubt. Safety und correctness vor Speed. getTokenAccountsByOwner, getMultipleAccounts für autoritativen On-Chain-State. | — |
| I-6 | **Nie** RPC aus Cold Paths entfernen um zu "optimieren" — bricht safety-kritische Flows. | Safety-Bruch |
| I-7 | **Nie** RPC in Hot Paths ohne explizite Freigabe — bricht Latenz-Anforderungen. | Architekturverletzung |
| I-8 | Bei RPC-Refactoring: **immer** prüfen ob Hot oder Cold Path betroffen. Änderungen die beide Pfade berühren = explizite Freigabe nötig. | — |

### B.3 Execution und Simulation (I-9 bis I-12)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-9 | **Simulate-gated**: Wenn Simulation fehlschlägt — **nie senden** (besonders Arbitrage). | Kapitalverlust-Risiko |
| I-10 | Einziger Pipeline-Pfad: Intent → Arbitration → Plan → Simulate → (Send) → Confirm → Accounting | Undefiniertes Verhalten |
| I-11 | Jeder Intent endet in **genau einem** Outcome: Rejected, Expired, SimFailed, Sent, Confirmed, FailedConfirmed | DoD §C |
| I-12 | **Decision Record** pro Intent — Inputs, Checks, Outcome. Keine stille Ablehnung. | Forensik-Unmöglich |

### B.4 Daten und Preise (I-13 bis I-16)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-13 | **Pool-Matching**: Position-Preis-Updates (Trade, PoolCacheUpdate) nur anwenden wenn source_pool == position.pool. Bei Multi-Pool-Tokens sonst falsche PnL und TAKE_PROFIT bei Verlust. | FIX-38 |
| I-15 | **Amounts explizit**: Jede Zahl hat raw vs ui und decimals. Keine impliziten Konventionen. | Falsche Slippage/Quotes |
| I-16 | **Geyser/LivePoolCache** ist autoritativ im Hot Path. RPC/WS nur Fallback (Cold Path). | Latenz + Cache-Inkonsistenz |

**Hinweis:** I-14 (tokens_per_sol) ist Eval-getestet in A.19 und wird hier nicht wiederholt.

### B.5 Arbitrage und MEV (I-17 bis I-19)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-17 | **Typ A (Strategy Arbitrage)**: marktgetrieben, erzeugt nur TradeIntent, keine Parent-Tx vorausgesetzt. | — |
| I-18 | **Typ B (Execution MEV)**: reaktiv, existiert nur relativ zu konkreter Parent-Tx oder Engine-State (z. B. eigene Pending-Tx, Bundle, observed Tx). Kein dauerhaftes Market-Scanning. | — |
| I-19 | **Atomic Arbitrage**: Cross-DEX Arb atomar (Bundle) oder verworfen. Keine Teilfills ohne definiertes Recovery. | Partial-Loss |

### B.6 Locks und Kapital (I-20 bis I-22)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-20 | **Capital Locks**: Keine Überbuchung. LockManager.try_lock_capital(). | Doppelte Ausführung |
| I-21 | **Resource Locks**: Accounts/Pools/ATAs die Konflikte erzeugen werden gelockt. | Race Conditions |
| I-22 | **Idempotency**: Engine vermeidet doppelte Verarbeitung (Intent-ID, Tx-Signature, in-flight Registry). | Doppel-Trades |

### B.7 NATS und Topics (I-23 bis I-24d)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-23 | Keine neuen ad-hoc NATS Topics. An versioned Topics halten oder klar dokumentieren. | Topic-Chaos |
| I-24 | Topics: ironcrab.v1.market_events, ironcrab.v1.trade_intents, ironcrab.v1.execution_results, ironcrab.v1.decision_records (siehe src/nats/topics.rs). | — |
| I-24a | **JetStream = SSOT für Bot-Zustand**: Wallet-Balances, Positionen, Pool-Cache, Config gehören in JetStream (persistent). Konsumenten bootstrappen und holen Live-Updates von dort. | Zustands-Drift |
| I-24b | **Core NATS = Market Events**: Chain-Daten (Trades, Blocks, Preise) als Echtzeit-Events. Kein Bot-Zustand über Core NATS — Datenflut zu hoch, keine Persistenz. | — |
| I-24d | **Cold-Path Discovery nur per Request/Reply**: execution-engine darf fehlende pool_accounts weder selbst discovern noch lokal in den SLAVE Cache schreiben. Discovery, MASTER-Write und JetStream-Publikation bleiben bei market-data. (Eval: A.38) | Architekturbruch |

### B.8 Entwicklungs-Workflow (I-25 bis I-27)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-25 | Plan vor dem Coden. Kleine, isolierte Änderungen bevorzugen. | Side-Effects |
| I-26 | Architektur-Änderungen nur mit expliziter Freigabe. | Scope Creep |
| I-27 | SSH/Server-Befehle nur wenn User explizit anfordert oder genehmigt. | — |

### A.33 PoolDiscovered darf pool_accounts im SLAVE Cache nicht ueberschreiben
- **Datei:** `tests/invariants_pumpswap_amm_liquidation.rs`
- **Invariante:** Wenn der SLAVE LivePoolCache bereits einen PumpAmm-Eintrag mit nicht-leeren pool_accounts hat und ein neues PoolDiscovered Event ohne pool_accounts (oder mit leeren) ankommt, muessen die bestehenden pool_accounts erhalten bleiben.
- **Formal:** apply_pool_cache_update(PoolDiscovered{pool_accounts}) → pool_accounts verfuegbar. apply_pool_cache_update(PoolDiscovered{metadata=None}) → pool_accounts unveraendert.
- **Getestet:** a33_pool_discovered_without_accounts_preserves_existing.
- **Kontext:** Root Cause von Bug #28: PoolDiscovered upsert loeschte pool_accounts, Liquidation scheiterte mit err_discovery.

### A.34 build_swap_ix muss base_token_program fuer Token-2022 korrekt setzen
- **Datei:** `tests/invariants_pumpswap_amm_liquidation.rs` (erweitert)
- **Invariante:** `build_swap_ix()` muss fuer Instruction-Account 11 (base token program) die aus `cached_data` aufgeloeste `base_token_program` verwenden. Fuer Token-2022 Tokens muss dies `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` sein, nicht `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`.
- **Formal:** PumpFunAmmDex mit cached_data["token_program:{base_mint}"] = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb". build_swap_ix(sell) → accounts[11] == TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb.

### A.35 Liquidation Retry Scan muss Token-2022 abdecken
- **Datei:** `tests/invariants_liquidation_flow.rs` (erweitert)
- **Invariante:** Der Retry-Diagnostic-Scan im Liquidation-Job muss sowohl SPL Token als auch Token-2022 Accounts per `getTokenAccountsByOwner` abfragen, analog zur initialen Scan-Phase.

### A.36 Bekannte Pool-Adresse = enger Pfad vor globalem Scan
- **Datei:** `tests/invariants_hot_path_no_rpc.rs` (geplant)
- **Invariante:** Wenn der LivePoolCache die Pool-Adresse fuer eine base_mint kennt, muss der Recovery-/Discovery-Pfad den bekannten Pool gezielt behandeln. Globaler Scan ist nur Last-Resort fuer komplett unbekannte Pools.
- **Luecke:** Der Claim (getAccount vs getProgramAccounts) erfordert RPC-Call-Beobachtung und ist ohne Mock-RPC nicht blackbox-testbar. Der beobachtbare Vertrag "bekannte Pool-Adresse + pool_accounts → gezielter Pfad funktioniert" ist ueber i24d_after_authoritative_update_retry_can_proceed abgedeckt.

### Orca Cold Path (geplant, NICHT Eval-getestet)
- **Ziel-Invariante:** Bekannter Orca-Pool + gesetzter LivePoolCache + fehlende/unbrauchbare Live-Reserves + Cold-Path-Aktivierung + RPC unreachable => Err (nicht stilles Ok(None)).
- **Luecke:** Der gemergte Fix (PR #20) haertet den spezifischen Cold Path mit **gesetztem** LivePoolCache und fehlenden Reserves. Orca hat aktuell kein `allow_rpc_on_miss` im Konstruktor (im Gegensatz zu Raydium/RaydiumCpmm/Meteora). Der Contract ist an der `quote_exact_in`-API-Grenze ohne diesen Parameter nicht beobachtbar. Ein Blackbox-Test erfordert entweder die gemergte API (allow_rpc_on_miss o.ae.) oder einen anderen Test-Einstiegspunkt.
- **Nicht** als aktive Eval-Invariante gefuehrt; kein ignorierten Schein-Test.

### A.37-A.40 zurueckgezogen
- Die zuvor vorgeschlagenen Invarianten A.37-A.40 wurden **nicht** als aktive Eval-Invarianten uebernommen.
- Grund: Der dazu erstellte Testansatz basierte ueberwiegend auf Source-Code-Scans und Regex-/String-Matching gegen das Impl-Repo statt auf belastbaren Verhaltens- oder Blackbox-Tests.
- Der lokale Eval-Revert entfernt die zugehoerigen Testdateien wieder. Bis ein sinnvoller verhaltensorientierter Testansatz vorliegt, gelten A.37-A.40 hier **nicht** als aktive Spec-Anforderungen.

---

## C. Architektur-Prinzipien (GPT-Empfehlungen)

| Prinzip | Beschreibung |
|---------|--------------|
| **Single Writer per Truth Domain** | Jede State-Domäne hat genau eine Autorität (Position, Market State, Locks) |
| **Strategy is Pure Function** | Decision = f(ProjectedState); kein verstecktes evolvierendes Memory |
| **Replay Determinism** | Dieselbe Event-History erzeugt bit-identische Decision Streams (golden_replay) |
| **Restart Idempotency** | Verarbeitete Intents werden bei Restart nicht erneut ausgeführt |

---

## D. Ziel-Invarianten

### D.1 Position Conservation
**Status:** ✅ Entscheidung getroffen (2026-03-04)

**Entscheidung:** Position bleibt vorerst in Momentum. Kein separater Positions-Ledger.

Begründung: War ursprünglich in Execution, wurde wegen Problemen nach Momentum verlagert. Ein eigener Positions-Ledger würde zusätzliche Komplexität und Sync-Punkte einführen. Momentum ist aktuell die autoritative Quelle für offene Positionen; ausreichend für den Betrieb.

### D.2 Execution Finality Consistency
**Status:** Umgesetzt (2026-03-04)

**Invariante:** Position darf nur aus FINALIZED executions entstehen (nicht confirmed).

Umsetzung: `confirm_commitment` Config (default: "finalized"). Geyser TX-Subscription und RPC-Polling warten auf Finalized, um Reorg/Fork-Bugs auf Solana zu vermeiden.

---

## E. Checkliste vor PR / AI-Änderung

- [ ] Kein RPC im Hot Path?
- [ ] Pool-Matching bei Preis-Updates für Positionen?
- [ ] tokens_per_sol Konvention eingehalten?
- [ ] Simulation vor jedem Send?
- [ ] Decision Record für jeden Intent?
- [ ] Keine Keys außer in execution-engine?

---

## F. Querbezug

- DoD §H (Connector Contract Tests) verweist auf A.3
- Iron_crab/docs/INVARIANTS.md ist die Quelle für B.1–B.8 (I-1 bis I-27 inkl. I-24a, I-24b)
- Invarianten selbst stehen ausschließlich in diesem Dokument
