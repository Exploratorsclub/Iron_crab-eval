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
- **Datei:** `tests/invariants_dex_connector.rs`
- **Invarianten:**
  - **Quote-Monotonie:** `amount_in1 < amount_in2` → `amount_out1 <= amount_out2`
  - **Price-Impact:** Größeres amount_in → mindestens gleicher oder höherer price_impact_bps
  - **Unknown Pair:** Kein Pool für Input/Output-Mint → `None` oder `Ok(None)`
  - **Zero Input:** amount_in = 0 → `None` oder amount_out = 0
  - **Build IX:** `build_swap_ix_from_pool_accounts` liefert nicht-leere Instructions mit korrektem program_id

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
| I-14 | **tokens_per_sol** Konvention: LOWER = token wertvoller. pnl_pct = (entry/current - 1)*100. highest_price = niedrigster tps (bester Preis für Holder). | Invertierte Exit-Signale |
| I-15 | **Amounts explizit**: Jede Zahl hat raw vs ui und decimals. Keine impliziten Konventionen. | Falsche Slippage/Quotes |
| I-16 | **Geyser/LivePoolCache** ist autoritativ im Hot Path. RPC/WS nur Fallback (Cold Path). | Latenz + Cache-Inkonsistenz |

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

### B.7 NATS und Topics (I-23 bis I-24b)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-23 | Keine neuen ad-hoc NATS Topics. An versioned Topics halten oder klar dokumentieren. | Topic-Chaos |
| I-24 | Topics: ironcrab.v1.market_events, ironcrab.v1.trade_intents, ironcrab.v1.execution_results, ironcrab.v1.decision_records (siehe src/nats/topics.rs). | — |
| I-24a | **JetStream = SSOT für Bot-Zustand**: Wallet-Balances, Positionen, Pool-Cache, Config gehören in JetStream (persistent). Konsumenten bootstrappen und holen Live-Updates von dort. | Zustands-Drift |
| I-24b | **Core NATS = Market Events**: Chain-Daten (Trades, Blocks, Preise) als Echtzeit-Events. Kein Bot-Zustand über Core NATS — Datenflut zu hoch, keine Persistenz. | — |

### B.8 Entwicklungs-Workflow (I-25 bis I-27)

| ID | Invariante | Verletzung = |
|----|------------|--------------|
| I-25 | Plan vor dem Coden. Kleine, isolierte Änderungen bevorzugen. | Side-Effects |
| I-26 | Architektur-Änderungen nur mit expliziter Freigabe. | Scope Creep |
| I-27 | SSH/Server-Befehle nur wenn User explizit anfordert oder genehmigt. | — |

---

## C. Architektur-Prinzipien (GPT-Empfehlungen)

| Prinzip | Beschreibung |
|---------|--------------|
| **Single Writer per Truth Domain** | Jede State-Domäne hat genau eine Autorität (Position, Market State, Locks) |
| **Strategy is Pure Function** | Decision = f(ProjectedState); kein verstecktes evolvierendes Memory |
| **Replay Determinism** | Dieselbe Event-History erzeugt bit-identische Decision Streams (golden_replay) |
| **Restart Idempotency** | Verarbeitete Intents werden bei Restart nicht erneut ausgeführt |

---

## D. Ziel-Invarianten (noch nicht erfüllt)

### D.1 Position Conservation
**Status:** Offen – Diskussionsbedarf

Streng genommen gehört Position weder in Execution noch in Momentum. War ursprünglich in Execution, wurde wegen Problemen nach Momentum verlagert. Beste Lösung noch zu finden.

### D.2 Execution Finality Consistency
**Status:** Noch nicht umgesetzt

**Invariante:** Position darf nur aus FINALIZED executions entstehen (nicht confirmed).

Aktuell wird nicht auf finalized gewartet; macht aber Sinn zur Vermeidung von Reorg/Fork-Bugs auf Solana.

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
