# Plan: Grafana CPU-Fix + echter Slot-Rückstand (rev. 2)

**Datum:** 2026-08-18  
**Status:** Vorbereitet (Dateien in `Iron_crab/docs/`, Prod-Deploy ausstehend)

---

## User-Vorgaben (rev. 2)

1. **CPU:** Kein separates Validator-CPU-Panel. Gauge und Graph sollen **dieselbe systemweite Server-CPU** korrekt anzeigen.
2. **Slots Behind:** Kein neues Panel. Bestehende Panels **„Slots Behind“** und **„Slots Behind Over Time“** auf echten Gossip/Mainnet-Lag umstellen (nicht `getHealth`).

---

## 1. CPU-Fix (implementiert)

**Datei:** `Iron_crab/docs/grafana_multiprocess_dashboard.json`

| Panel | Änderung |
|-------|----------|
| CPU Usage (Gauge) | Einheitliche Query `[2m]`, `instant: true`, explizite `reduceOptions.lastNotNull` |
| CPU & RAM Over Time | Gleiche CPU-Query `[2m]` statt `$__rate_interval` |

**Query (beide Panels):**
```promql
100 * (1 - avg(rate(node_cpu_seconds_total{mode="idle",job="node-exporter"}[2m])))
```

---

## 2. Echter Slot-Rückstand (implementiert)

### Sidecar: `validator-lag-exporter`

| Datei | Zweck |
|-------|-------|
| `Iron_crab/docs/validator-lag-exporter.py` | Pollt alle 10s local + mainnet `getSlot` |
| `Iron_crab/docs/validator-lag-exporter.service` | systemd unit (Port 9180) |
| `Iron_crab/docs/prometheus_multiprocess.yml` | Scrape job `validator-lag-exporter` |

**Primäre Metrik:**
```text
ironcrab_validator_slots_behind = reference_slot - local_slot
```

### Dashboard (bestehende Panels, keine neuen)

| Panel | Neue Query |
|-------|------------|
| Slots Behind | `ironcrab_validator_slots_behind{job="validator-lag-exporter"}` |
| Slots Behind Over Time | dieselbe Metrik |

**Schwellwerte:** grün 0–2, gelb ab 3, orange ab 50, rot ab 200

### Alerts

`grafana_alert_rules.json`: Sync-Alerts nutzen jetzt `ironcrab_validator_slots_behind` statt `solana_node_num_slots_behind`.

---

## 3. Prod-Deploy (manuell, nach User-Freigabe)

```bash
# Exporter installieren
sudo cp validator-lag-exporter.py /usr/local/bin/validator-lag-exporter
sudo chmod +x /usr/local/bin/validator-lag-exporter
sudo cp validator-lag-exporter.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now validator-lag-exporter

# Prometheus + Grafana
sudo cp prometheus_multiprocess.yml /etc/prometheus/prometheus.yml   # oder merge
sudo systemctl reload prometheus
sudo cp grafana_multiprocess_dashboard.json /var/lib/grafana/dashboards/
# Alerts ggf. via Grafana UI importieren

# Verifikation
curl -s http://127.0.0.1:9180/metrics | grep ironcrab_validator_slots_behind
```

**Kein Validator-/IronCrab-Restart nötig.**

---

## 4. Erfolgskriterien

- CPU-Gauge und CPU-Graph zeigen denselben Wert (±1 %)
- „Slots Behind“ zeigt ~130 im Steady-State, auch wenn Validator Status = HEALTHY
- „Slots Behind Over Time“ zeichnet denselben Wert auf
