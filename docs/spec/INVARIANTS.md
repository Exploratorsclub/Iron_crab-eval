# Invarianten

Formale Verhaltens-Invarianten, die von Eval-Tests geprüft werden. Keine Implementierungsdetails, keine DoD-Abnahme-Checkliste.

**Abgrenzung:** DoD = „Ist Feature X fertig?“ | INVARIANTS = „Unter welchen Bedingungen muss API Y sich so verhalten?“

---

## 1. P1-Invarianten (Eval-Tests)

### 1.1 Quote-Monotonie

- **Datei:** `tests/invariants_quote_monotonic.rs`
- **Invariante:** Größeres `amount_in` führt zu mindestens gleichem `amount_out` bei `quote_output_amount`.
- **Formal:** `amount_in1 < amount_in2` → `amount_out1 <= amount_out2`

### 1.2 LockManager

- **Datei:** `tests/invariants_lock_manager.rs`
- **Invarianten:**
  - `total_locked + available` = initial (SOL-Erhaltung über Lock/Release)
  - Gleicher Intent-ID nicht doppelt gelockt (Capital Lock)

### 1.3 DEX Connector

- **Datei:** `tests/invariants_dex_connector.rs`
- **Invarianten:**
  - **Quote-Monotonie:** `amount_in1 < amount_in2` → `amount_out1 <= amount_out2`
  - **Price-Impact:** Größeres amount_in → mindestens gleicher oder höherer price_impact_bps
  - **Unknown Pair:** Kein Pool für Input/Output-Mint → `None` oder `Ok(None)`
  - **Zero Input:** amount_in = 0 → `None` oder amount_out = 0
  - **Build IX:** `build_swap_ix_from_pool_accounts` liefert nicht-leere Instructions mit korrektem program_id

### 1.4 Geyser-First / Cache-Hit

- **Datei:** `tests/pump_amm_geyser_first.rs`
- **Invariante:** Cache-Hit liefert Quote und pool_accounts ohne RPC-Aufruf.
- **Kontext:** TARGET_ARCHITECTURE §4.2 – DEX Connectors speichern Pool-State aus MarketEvents, nicht RPC.

### 1.5 Router Slippage

- **Datei:** `tests/invariants_router_slippage.rs`
- **Invariante:** `Router::cumulative_min_out(quotes, slippage_bps)` wendet Slippage auf das **letzte** amount_out an:
  - `min_out = (quotes.last().amount_out * (10_000 - slippage_bps)) / 10_000`

### 1.6 Arbitrage Profit Filter

- **Datei:** `tests/invariants_arbitrage_profit.rs`
- **Invariante:** `compute_net_profit(amount_in, final_out, min_profit_bps, est_tx_cost_lamports)`:
  - Gibt `Some(net)` nur wenn: ROI >= min_profit_bps UND (gross - est_tx_cost) > 0
  - Gibt `None` wenn: ROI < min_profit_bps ODER gross <= est_tx_cost

---

## 2. System-Invarianten (GPT-Empfehlungen, testbar)

| Invariante | Beschreibung | Eval-Test |
|------------|--------------|-----------|
| **Replay Determinism** | Dieselbe Event-History erzeugt bit-identische Decision Streams (intent_ids, Reihenfolge, outcome) | golden_replay |
| **Intent Causality Chain** | Jede Execution rückverfolgbar zu decision_id und intent_id | IPC-Schema (ExecutionResult) |
| **Restart Idempotency** | Verarbeitete Intents werden bei Restart nicht erneut ausgeführt (kein Doppel-Send) | LockManager + processed-intent-Restore |

---

## 3. Architektur-Prinzipien (Leitlinien, kein Eval-Test)

| Prinzip | Beschreibung |
|---------|--------------|
| **Single Writer per Truth Domain** | Jede State-Domäne hat genau eine Autorität (Position, Market State, Locks) |
| **Strategy is Pure Function** | Decision = f(ProjectedState); kein verstecktes evolvierendes Memory |

---

## 4. Ziel-Invarianten (noch nicht erfüllt)

### 4.1 Position Conservation

**Status:** Offen – Diskussionsbedarf

**Kontext:** Streng genommen gehört Position weder in Execution noch in Momentum. War ursprünglich in Execution, wurde wegen Problemen nach Momentum verlagert (Momentum benötigt Position für Strategie-Entscheidung). Beste Lösung noch zu finden.

**Architektur-Entscheidung ausstehend:** Keine feste Zuordnung zu einem Modul, bis Konsens gefunden ist.

### 4.2 Execution Finality Consistency

**Status:** Noch nicht umgesetzt

**Invariante:** Position darf nur aus FINALIZED executions entstehen (nicht confirmed).

**Kontext:** Aktuell wird nicht auf finalized gewartet; macht aber Sinn zur Vermeidung zukünftiger Reorg/Fork-Bugs auf Solana (optimistic confirmations, RPC-Divergenz, dropped forks).

---

## 5. Querbezug

- DoD §H (Connector Contract Tests) verweist auf diese Invarianten
- Invarianten selbst stehen ausschließlich in diesem Dokument
