# Plan: K Performance / Latenz (DoD §K, P2)

**Zweck:** Umsetzungsplan für die DoD §K Performance-Ziele. K ist P2 (Future Optimization).

**Quellen:** DEFINITION_OF_DONE.md §K, §K.1, TARGET_ARCHITECTURE.md §1, §6

**Stand 2026-03-04:** Phase 3 (TPU) erledigt. Phase 1 (Metriken) offen. Phase 2 optional. Phase 4 dokumentiert.

---

## 1. Übersicht

| Phase | Inhalt | Status |
|-------|--------|--------|
| **1** | Slot-to-Send Metriken + Grafana | ✅ Erledigt (2026-03-04) |
| **2** | Single-Process Mode (NATS optional) | Optional, siehe §3 |
| **3** | TPU Direct | ✅ Erledigt (TxSender, parallel_send TPU+RPC) |
| **4** | Hot Path Allocations | Dokumentiert, siehe §5 |

---

## 2. Phase 1: Metriken & Baseline (delegiert)

**Ziel:** Slot-to-Send Latenz messbar machen. Ohne Metriken keine fundierten Optimierungen.

### 2.1 Anforderungen

- **Metrik:** `tx_slot_to_send_ms` (Histogram) in Prometheus
- **Wo:** execution_engine.rs nach TX Send
- **Berechnung:** `now_ms - slot_timestamp_ms` — Slot-Timestamp muss durch Pipeline propagiert werden (Intent/Event → Send)
- **Grafana:** Dashboard mit P50/P95/P99

### 2.2 Herausforderung

Slot-Timestamp muss verfügbar sein. Prüfen: `TradeIntent`, `MarketEvent` oder `PoolCacheUpdate` haben `slot`? Wenn nicht: von arb-strategy/momentum-bot in Intent-Metadata setzen.

### 2.3 Abnahme

- Histogram in Prometheus sichtbar
- Grafana-Dashboard (oder JSON-Export) existiert

---

## 3. Phase 2: Single-Process Mode — Was ist gemeint?

**Problem:** Aktuell laufen 3 separate Prozesse mit NATS dazwischen:

```
Geyser → market-data (Prozess 1) → NATS → arb-strategy (Prozess 2) → NATS → execution-engine (Prozess 3)
```

Jeder NATS-Hop kostet Latenz (Serialisierung, Netzwerk, Deserialisierung).

**Phase 2-Idee:** Ein **einziger Prozess**, der market-data + arb-strategy + execution-engine in sich vereint:

```
Geyser → [market-data + arb-strategy + execution-engine in einem Prozess]
         └── tokio::mpsc Channels statt NATS für den Hot Path
```

- Feature-Flag `--single-process`: Startet alle drei als Tasks in einem Binary
- In-Process Channels (tokio::mpsc) statt NATS für Events/Intents
- NATS nur noch für Control Plane, Debugging, externe Consumer

**Nutzen:** Weniger Latenz durch Wegfall der Netzwerk-Hops. **Nachteil:** Größerer Refactor, ein Crash betrifft alles.

**Priorität:** Niedrig. Nur sinnvoll wenn Latenz kritisch wird (z.B. gegen HFT-Konkurrenz).

---

## 4. Phase 3: TPU Direct — ✅ Bereits umgesetzt

TxSender in `Iron_crab/src/solana/tx_sender.rs`:
- **TPU Direct** via TpuSubmitter (QUIC zu Slot-Leadern)
- **parallel_send: true** (Default): TPU + RPC **gleichzeitig** — erste Bestätigung gewinnt, Solana dedupliziert
- **Jito** für Bundle-Intents (Arb)
- **RPC** als Fallback

Config: `[execution_engine.tx_submission]` mit `tpu_enabled`, `parallel_send`, etc.

---

## 5. Phase 4: Hot Path Allocations — Detaillierte Erklärung

### 5.1 Was sind „Allocations“?

In Rust (und C/C++) bedeutet **Allocation**: Speicher vom Heap anfordern (`malloc`, `Vec::new()`, `String::from()`, `Box::new()`). Das kostet Zeit:
- malloc kann ~100ns–1µs dauern
- Bei sehr hohen Frequenzen (z.B. 10.000 Requests/Sekunde) summiert sich das
- Zusätzlich: Garbage Collection / Drop-Overhead bei vielen kleinen Allocations

### 5.2 Was ist der „Hot Path“?

Der **Trading Hot Path** ist der kritische Pfad von der Entscheidung bis zum TX-Send:

```
Intent empfangen → Lock → Quote berechnen → TX bauen → Senden
```

Jede Millisekunde zählt, wenn man gegen andere Bots antritt.

### 5.3 Was will Phase 4?

**Ziel:** Im Hot Path **keine** Heap-Allocations. Stattdessen:
- **Stack-Allocation:** Lokale Variablen (automatisch auf dem Stack)
- **Pre-allocated Buffers:** Wieder verwendbare Puffer, die einmal angelegt werden
- **Object Pools:** Pool von Objekten, die wiederverwendet statt neu erstellt werden
- **Arena Allocator:** Ein großer Block, aus dem schnell „sub-allociert“ wird

### 5.4 Wie findet man Allocations?

**Tools:**
- **dhat** (Rust): Heap-Profiler, zeigt wo Allocations passieren
- **heaptrack** (Linux): Externer Heap-Profiler
- **Custom Allocator:** Zähler-Allocator, der jede Allocation loggt

### 5.5 Wann lohnt sich Phase 4?

- **<10ms Latenz-Anforderung:** Dann können selbst kleine Allocations stören
- **Hochfrequenz-Trading:** Tausende Intents/Sekunde
- **Aktuell:** Nicht kritisch. Frankfurt VPS, Jito/TPU, ~300–500ms Slot-to-Send — Allocation-Overhead ist vernachlässigbar.

**Fazit:** Phase 4 ist eine **späte Optimierung** für den Fall, dass Latenz zum Engpass wird. Keine Priorität für normalen Betrieb.

---

## 6. Delegation Phase 1

**Handoff:** `docs/supervisor/context_k_phase1_slot_to_send.md`

**Befehl:**
```
cd Iron_crab && agent -p "[HANDOFF]" --model composer-1.5 --trust
```

**Eval:** Keine Eval-Tests für Metriken geplant (operational, keine Invarianten).
