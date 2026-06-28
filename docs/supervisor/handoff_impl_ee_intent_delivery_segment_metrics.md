WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoest, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff — Impl Agent: EE Intent-Delivery Segment-Metriken (Test C, Evidenz-Grundlage)

## Task-Beschreibung

**Ziel:** Drei Prometheus-Histogramme einfuehren, die die **bereits prod-belegte** Gesamt-Latenz `execution_intent_header_to_receive_ms` in Segmente zerlegen. **Kein Verhaltens-Fix** in diesem PR — nur Observability, damit Root Cause (Channel-Stau vs. `interval.tick`-Starvation vs. JetStream-Fetch) **attributierbar** wird.

**Prod-Evidenz (2026-06-28, `2ad96d7`):**
- `momentum_intent_header_to_publish_ms`: Ø ~0,2 ms (103 Samples)
- `execution_intent_header_to_receive_ms`: Ø ~4942 ms (103 Samples)
- `intent_rejection_by_reason{ttl_expired}` = 39 = exakt Intents mit Delivery >5000 ms
- Beispiel `int-4a4bf70f-000038`: 16284 ms Header→EE-Receive, 63 min nach Deploy

Referenz: `Iron_crab-eval/docs/supervisor/handoff_eval_intent_delivery_latency_slo.md`

## Relevante Invarianten (Volltext)

**I-7 Hot-Path RPC-Freiheit:** Keine neuen RPC-Calls. Metriken nur aus vorhandenen Wall-Clock-/Instant-Timestamps.

**I-9 Simulate-gated:** Unveraendert — dieser PR aendert keine Send-/Sim-Logik.

**I-12 Decision Record:** Unveraendert.

**I-23 Keine ad-hoc NATS Topics:** Keine neuen Subjects; optional interne Sidecar-Map fuer Enqueue-Zeit (nicht on-wire).

## Bestehendes Pattern

Histogramme wie `try_record_execution_intent_header_to_receive_ms` in `src/metrics.rs` (Atomic bucket counts + sum/count, Export in Prometheus-Block ~5344). Aufnahmepunkte in `src/bin/execution_engine.rs`:
- JetStream Intent Fetch-Task (~7832–7870): `intent_consumer.fetch()` → deserialize → `intent_tx.send()`
- Main `select!` (~8327): `intent_rx.recv()` → spawn `process_intent`
- `interval.tick`-Arm (~8543–8695): PoolCache batch + WalletSnapshot batch

## Erlaubte Dateien

- `Iron_crab/src/metrics.rs`
- `Iron_crab/src/bin/execution_engine.rs`
- `Iron_crab/docs/RUNBOOK_PROD.md` (kurzer Abschnitt zu den 3 neuen Metriken)

## Verboten

- `DeliverPolicy`-Aenderung (folgt separater Eval/Impl-Scope)
- TTL-, Queue- oder Sync-BUY-Aenderungen
- Refactor Pool/Wallet aus `select!` (nur messen)
- RPC im Hot Path
- Aenderungen an `Iron_crab-eval/`

## Implementierung (Pflicht)

### 1. `execution_intent_jetstream_to_channel_ms`

**Start:** JetStream-Fetch-Task, sobald `TradeIntent` aus `msg.payload` deserialisiert ist (`Ok(intent)`).

**Ende:** unmittelbar nach erfolgreichem `intent_tx.send(intent).await` (vor `msg.ack()`).

Misst: JetStream-Message → Channel-Enqueue (Fetch-Task-intern).

### 2. `execution_intent_channel_wait_ms`

**Enqueue-Zeit:** Beim `send` in Fetch-Task: `wall_clock_unix_ms_now()` in **interner** Struktur speichern — z.B. `HashMap<String, u64>` keyed by `intent_id` (mit LRU/evict nach recv oder cap 256), **nicht** in `TradeIntent.metadata` (Wire-Contract).

**Ende:** Beim `intent_rx.recv()` in Main-Loop, vor `process_intent`-Spawn: `now - enqueue_ms` histogrammen, Map-Eintrag entfernen.

Misst: Zeit im `intent_rx`-Channel + Wartezeit bis `select!` den Intent-Arm waehlt.

### 3. `execution_engine_interval_tick_duration_ms`

**Start:** Erste Zeile im `interval.tick`-Arm **nach** `try_join_next`-Drain (~8546).

**Ende:** Nach PoolCache- **und** WalletSnapshot-Block (nach beiden `fetch`-while-Schleifen, vor MVP dry-run Test ~8697).

Misst: Wall-Zeit des schweren JetStream-Batch-Blocks im selben `select!` wie `intent_rx`.

### 4. Prometheus + Runbook

- Buckets analog `execution_intent_header_to_receive_ms` / `MOMENTUM_LATENCY`-Sets (ms, bis 60000).
- RUNBOOK: Segment A+B+C summieren sich approx. zu Header→Receive; hohes C + hohes B → Starvation-Hypothese testbar.

### 5. Unit-Test (Impl-Repo)

Minimaler Test in `execution_engine.rs` `#[cfg(test)]` oder `metrics.rs`: Recording-Funktionen inkrementieren Count bei bekanntem Delta (kein NATS noetig).

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Definition of Done

- CI gruen (fmt, clippy, unit tests)
- Drei neue Metrik-Familien auf `:9804/metrics` sichtbar
- Kein Verhaltens-Change ausser Metrik-Recording
