# Eval-Testplan: Intent-Delivery-Latenz (TTL_EXPIRED Root Cause)

**Stand:** 2026-06-28, Prod `ironcrab-prod`, Deploy `2ad96d7` (PR #251), EE-Uptime ~2,5 h zum Messzeitpunkt.

**Zweck:** Roter Eval-Test als Fix-Kriterium — kein Symptom-Fix (kein TTL-Verschieben, kein Sync-BUY bis TX-Confirm).

---

## 1. Harte Prod-Evidenz (messbar, reproduzierbar)

| # | Beobachtung | Quelle | Wert |
|---|-------------|--------|------|
| E1 | Momentum Publish-Latenz (Intent-Header → JetStream-Publish) | `:9802/metrics` `momentum_intent_header_to_publish_ms_*` | count=103, sum=20 → **Ø ~0,2 ms**; 101/103 ≤1 ms |
| E2 | EE Delivery-Latenz (Intent-Header → `process_intent`-Eintritt) | `:9804/metrics` `execution_intent_header_to_receive_ms_*` | count=103, sum=509028 → **Ø ~4942 ms** |
| E3 | TTL-Reject-Korrelation | `:9804/metrics` | `intent_rejection_by_reason{ttl_expired}=39` **=** Intents mit Delivery >5000 ms (103−64 Bucket) |
| E4 | `process_intent` selbst ist schnell | `execution_process_intent_us_*` | 103/103 ≤250 µs |
| E5 | TTL-Reject **nach** langer Laufzeit | `journalctl execution-engine` seit 15:50 CEST | Rejects 16:07 … **18:16** (kein reines Startup-Phänomen) |
| E6 | Beispiel `int-4a4bf70f-000038` | Logs + JSONL | Generate 14:53:51.582Z → EE „Received“ 14:54:07.866Z → **16284 ms** |
| E7 | TTL-Check-Details 000038 | `trade_logs/decisions/decision_records-20260628.jsonl` | `now_ms=1782658447866 intent_ts_ms=1782658431582 ttl_ms=5000` → **16284 ms** |
| E8 | JetStream Consumer steady-state | `nats consumer info TRADE_INTENTS execution-engine` | `Unprocessed Messages: 0` (kein offensichtlicher JS-Backlog **zum Messzeitpunkt**) |

**Abgeleitete Fakten (keine Spekulation):**

1. Die TTL-Expires sind **direkte Folge** von Delivery-Latenz > `ttl_ms` (5000 ms), nicht von Kill-Switch oder TTL-Logik-Fehler.
2. Die Latenz entsteht **nach** Momentum-JetStream-Publish und **vor** EE-`process_intent`.
3. Nur **12,6 %** der Intents erreichen EE innerhalb **250 ms** (13/103 im ≤250 ms-Bucket) — deutlich unter SLO.

**Noch nicht bewiesen (Instrumentation fehlt):**

- Ob die Verzögerung in `intent_rx`-Channel-Stau, EE-`select!`-Starvation, JetStream-Fetch-Task oder Tokio-Scheduling liegt.
- Mechanismus im Code **möglich** (PoolCache + WalletSnapshot bis 600 Msg/synchron pro `interval.tick`-Arm im selben `select!` wie `intent_rx`), aber **ohne Segment-Metriken nicht attributierbar**.

---

## 2. Historie (Git, keine Vermutung)

| Änderung | Commit / PR | Autor | Begründung im Commit |
|----------|-------------|-------|----------------------|
| JetStream `TRADE_INTENTS` + `DeliverPolicy::All` | `d113cd96` 2026-02-22 | Robonuk | „fixes Core NATS fire-and-forget race“ / Startup-Race EE vs Momentum |
| Momentum `intent_publish_worker` (1024-Queue) | PR #218 `1aeb6461` 2026-06-11 | Cloud Agent + Robonuk | dbg_log-I/O aus Hot Path; JSONL+JetStream off Event-Loop |
| EE parallele `process_intent` (FIX-31) | 2026-02-19 | — | Main-Loop blockierte nicht mehr auf TX-Confirm |

`DeliverPolicy::All` für Trade-Intents war **von Anfang an** im selben Commit wie JetStream-Intro — kein separates Review-Commit.

---

## 3. Eval-Tests (rot → grün als Impl-Gate)

### Test A — Consumer-Contract (sofort rot)

**Datei:** `Iron_crab-eval/tests/invariants_trade_intents_consumer_deliver_policy.rs`

**Invariante (neu, eval-seitig):** Der durable EE-Consumer für `TRADE_INTENTS` darf **nur Live-Intents** liefern — `DeliverPolicy::New`. Alte Intents haben keinen Nutzen im Hot Path.

**Implementierung:**

```rust
use ironcrab::nats::trade_intents_consumer_config;
// assert matches!(cfg.deliver_policy, DeliverPolicy::New)
```

**Erwartung heute:** **ROT** (`All` in `src/nats/jetstream.rs`).

**Impl-Fix (minimal):** `DeliverPolicy::New` in `trade_intents_consumer_config()`. Kein zweiter Stream, kein Replay-Pfad.

---

### Test B — Delivery-SLO unter JetStream-Last (E2E, reproduziert Symptom)

**Datei:** `Iron_crab-eval/tests/invariants_intent_delivery_latency_under_load.rs`

**Invariante:** Frisch publizierter `TradeIntent` (Header-Timestamp = Erzeugung) muss EE **`process_intent` innerhalb 250 ms** erreichen, während der EE unter realistischer JetStream-Nebenlast läuft.

**Harness:** Erweiterung `request_reply_e2e_harness.rs` (Blackbox, kein `Iron_crab/src/`).

**Ablauf:**

1. NATS + JetStream-Streams (`TRADE_INTENTS`, `WALLET_SNAPSHOT`, `POOL_CACHE`) wie Prod.
2. `execution-engine` starten (`--dry-run`, Eval-Wallet, Port frei).
3. **Last erzeugen (Pflicht):** Hintergrund-Task publiziert **≥500** `WalletBalanceSnapshot`-Events/s auf JetStream (1 Wallet, viele Token-Mints oder Repeat-Publish) **und** **≥100** `PoolCacheUpdate`/s — reproduziert EE-`interval.tick`-Arbeit (Pool max 100 + Wallet max 500 pro Tick).
4. **Intent injizieren:** Ein BUY-`TradeIntent` mit `ts_unix_ms = now`, `ttl_ms = 5000`, `source = momentum-bot`, minimale gültige Ressourcen (Fixture-Mints/Pools aus `tests/fixtures/request_reply/`).
5. JetStream-Publish auf `ironcrab.v1.trade_intents`, Pub-Ack abwarten.
6. **Messung:** Poll bis Decision Record erscheint (`trade_logs/decisions/` oder NATS `decision_records`) oder `/metrics` `execution_intent_header_to_receive_ms_count` steigt; berechne `decision_ts − intent.header.ts_unix_ms`.
7. **Assert:** `latency_ms ≤ 250`.

**Erwartung heute:** **ROT** (Prod: nur 12,6 % ≤250 ms; Median ~2–5 s).

**Grün-Kriterium Impl:** Latenz ≤250 ms bei gleicher Last **ohne** TTL zu verschieben und **ohne** synchrones BUY bis TX-Confirm.

---

### Test C — Segment-Metriken (Impl, **Zuerst** — Evidenz-Grundlage)

**Priorität 1 (Impl-PR vor Fix):** Handoff `handoff_impl_ee_intent_delivery_segment_metrics.md`. Kein Verhaltens-Fix — nur Observability:

| Metrik | Messpunkt |
|--------|-----------|
| `execution_intent_jetstream_to_channel_ms` | JetStream-Fetch-Task: Message received → `intent_tx.send` |
| `execution_intent_channel_wait_ms` | `send` → `intent_rx.recv` (Timestamp im Intent-Envelope oder Sidecar) |
| `execution_engine_interval_tick_duration_ms` | Dauer Pool+Wallet-Block im `interval.tick`-Arm |

Eval prüft dann: `jetstream_to_channel + channel_wait ≈ header_to_receive` und `interval_tick_p99` korreliert mit Spitzen in `channel_wait`.

---

## 4. Explizit verbotene „Fixes“ (kein Test dafür)

- TTL ab EE-Empfang
- Sync BUY im Momentum-Hot-Path bis TX-Confirm
- Optionaler Replay-Stream für alte Intents
- DeliverPolicy `All` beibehalten

---

## 5. Test Authority Handoff (Pflichtblock)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und `.cursor/rules/eval-test-authority.mdc` BEVOR du eine Datei änderst.

**Invariante (Volltext Test B):** Ein frisch erzeugter Momentum-Tier-1-BUY-Intent muss die Execution Engine innerhalb von **250 ms** nach `RecordHeader.ts_unix_ms` erreichen (`process_intent`-Eintritt), auch wenn Wallet-Snapshot- und PoolCache-JetStream-Updates mit Prod-ähnlichem Volumen parallel laufen.

**Zieldateien:** `tests/invariants_trade_intents_consumer_deliver_policy.rs`, `tests/invariants_intent_delivery_latency_under_load.rs`, ggf. Harness-Erweiterung in `tests/request_reply_e2e_harness.rs`.

**Prüf-Befehle:** `cargo fmt`, `cargo check`, `cargo build`, `cargo clippy -p ironcrab-eval`; lokal `cargo test --test invariants_trade_intents_consumer_deliver_policy --test invariants_intent_delivery_latency_under_load` (Test B `#[ignore]` ohne `nats-server` in CI optional — Workflow „Eval invariant tests (manual)“).

**Reihenfolge:** (1) **Test C Impl-Metriken** deployen/messen, (2) Test A + B parallel (rot), (3) Impl-Fixes nur mit Segment-Evidenz: `DeliverPolicy::New` + Intent-Pfad aus EE-`select!`-Starvation befreien (**ohne** BUY-Sync, **ohne** TTL-Verschiebung). Tests müssen grün werden.

---

## 6. Prod-Verifikation nach Fix (Runbook)

```bash
# Nach Deploy: gleiche Metriken wie E1–E3
curl -s localhost:9802/metrics | grep momentum_intent_header_to_publish_ms_count
curl -s localhost:9804/metrics | grep execution_intent_header_to_receive_ms_
# Erwartung: p99 header_to_receive < 250ms; ttl_expired Rate → 0 bei aktivem Momentum
```
