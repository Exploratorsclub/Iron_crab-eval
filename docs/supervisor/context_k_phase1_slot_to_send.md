# Handoff: K Phase 1 — Slot-to-Send Latency Metriken

**Erstellt:** 2026-03-04 | **Quelle:** plan_k_performance.md Phase 1

---

## 1. Aufgabe

Implementiere **Slot-to-Send Latency Metrik** in der execution-engine. Ziel: Messen, wie lange von Geyser-Event/Slot bis TX-Send vergeht.

---

## 2. Anforderungen

### 2.1 Metrik

- **Name:** `tx_slot_to_send_ms` (Histogram)
- **Berechnung:** `now_ms - slot_timestamp_ms` zum Zeitpunkt des TX-Send
- **Wo:** `execution_engine.rs` in `send_transaction_with_fallback` oder direkt danach

### 2.2 Slot-Timestamp

- **Herausforderung:** Slot muss durch die Pipeline verfügbar sein.
- **Prüfen:** Hat `TradeIntent` ein `slot`-Feld? Oder `metadata` mit `slot`?
- **Falls nicht:** arb-strategy / momentum-bot müssen `slot` aus dem auslösenden MarketEvent in Intent-Metadata setzen (`intent.metadata.insert("slot", slot.to_string())`).
- **Fallback:** Wenn Slot nicht verfügbar → Metrik nicht emittieren (oder 0 als Sentinel — dokumentieren).

### 2.3 Prometheus

- Histogram mit sinnvollen Buckets (z.B. 10, 25, 50, 100, 200, 500, 1000, 2000 ms)
- Labels optional: `source` (momentum/arb), `method` (tpu/jito/rpc)

### 2.4 Grafana (optional)

- Dashboard mit P50/P95/P99 für `tx_slot_to_send_ms`
- Oder: JSON-Export für bestehendes Dashboard

---

## 3. Erlaubte Dateien

- `Iron_crab/src/bin/execution_engine.rs`
- `Iron_crab/src/metrics.rs` (falls Histogram-Registrierung)
- `Iron_crab/src/ipc/schema.rs` (falls TradeIntent erweitert)
- `Iron_crab/src/bin/arb_strategy.rs`, `momentum_bot.rs` (falls Slot in Intent)
- `Iron_crab/docs/CONFIG_SCHEMA.md` (falls Config)

---

## 4. Hinweise

- INVARIANTS.md, KNOWN_BUG_PATTERNS.md prüfen
- Kein RPC im Hot Path (I-4, I-7) — Metrik-Emission ist kein Hot-Path-Kritisch
- Bestehende Metriken: `TX_SEND_TPU_TOTAL`, `TX_SEND_SUCCESS_TOTAL` etc. als Referenz

---

## 5. Abnahme

- [ ] Histogram `tx_slot_to_send_ms` in Prometheus sichtbar (curl localhost:9804/metrics)
- [ ] Slot wird korrekt propagiert (oder Fallback dokumentiert)
- [ ] Optional: Grafana-Dashboard
