WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und `.cursor/rules/eval-test-authority.mdc` BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoest, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff — Test Authority: Intent-Delivery Eval-Tests A + B

## Kontext

Supervisor-Dokument (Volltext Evidenz + SLO): `docs/supervisor/handoff_eval_intent_delivery_latency_slo.md`

**Parallel laeuft Impl PR Test C** (Segment-Metriken) — Eval-Tests A+B sind **unabhaengig** und sollen **rot** sein bis Impl-Fixes kommen.

**Prod-Evidenz:** Momentum-Publish Ø 0,2 ms; EE Header→Receive Ø 4942 ms; 39/103 TTL_EXPIRED; nur 12,6 % ≤250 ms.

## Test A — Consumer-Contract

**Invariante (Volltext):** Der durable JetStream-Consumer fuer `TRADE_INTENTS` in der Execution Engine muss `DeliverPolicy::New` verwenden. Alte Intents duerfen nicht in den Live-Hot-Path geliefert werden; kein Replay-Stream erforderlich.

**Zieldatei:** `tests/invariants_trade_intents_consumer_deliver_policy.rs`

**Implementierung:**
- `ironcrab::nats::trade_intents_consumer_config()` importieren
- Assert `deliver_policy == DeliverPolicy::New`
- Assert `durable_name == Some("execution-engine")`

**Erwartung vor Impl-Fix:** Test **FAIL** (aktuell `All`).

## Test B — Delivery-SLO unter JetStream-Last (E2E)

**Invariante (Volltext):** Ein frisch erzeugter Momentum-Tier-1-BUY-Intent muss die Execution Engine innerhalb von **250 ms** nach `RecordHeader.ts_unix_ms` erreichen (`process_intent`-Eintritt bzw. erster Decision Record mit idempotency-Check), auch wenn Wallet-Snapshot- und PoolCache-JetStream-Updates mit Prod-aehnlichem Volumen parallel laufen.

**Zieldateien:**
- `tests/invariants_intent_delivery_latency_under_load.rs`
- Erweiterung `tests/request_reply_e2e_harness.rs` (Last-Generator, Intent-Publish, Latency-Poll)

**Ablauf (Blackbox):**
1. NATS + JetStream (`TRADE_INTENTS`, `WALLET_SNAPSHOT`, `POOL_CACHE`) — Pattern wie `request_reply_e2e_harness.rs`
2. `execution-engine --dry-run` mit Eval-Wallet
3. Hintergrund: ≥500 WalletBalanceSnapshot/s + ≥100 PoolCacheUpdate/s (JetStream publish mit Ack)
4. Ein gueltiger BUY-`TradeIntent` (`ts_unix_ms = now`, `ttl_ms = 5000`, Fixtures)
5. Messung: `decision_ts - intent.header.ts_unix_ms` oder Metrics `execution_intent_header_to_receive_ms`
6. **Assert:** `<= 250`

**CI:** Test B mit `#[ignore]` + Kommentar „requires nats-server“ wenn CI kein NATS hat (wie andere E2E). Schlankes Gate: fmt, check, build, clippy ohne `--all-targets`.

## Verboten

- Tests an `Iron_crab/src/` anpassen
- Symptom-Tests fuer TTL-Verschiebung oder Sync-BUY
- Tests gruen machen durch Assertion-Lockerung ohne Impl-Aenderung

## Pruef-Befehle

```bash
cargo fmt -p ironcrab-eval -- --check
cargo check
cargo build
cargo clippy -p ironcrab-eval
cargo test --test invariants_trade_intents_consumer_deliver_policy
# cargo test --test invariants_intent_delivery_latency_under_load -- --ignored
```

## Definition of Done

- PR mit Tests A (+ B) auf `Iron_crab-eval` `main`
- Test A **rot** gegen aktuelles Impl (`DeliverPolicy::All`) — dokumentiert in PR body
- Test B implementiert; rot oder `#[ignore]` mit klarer Anleitung fuer manuellen Lauf
- Eval „Rust“ Workflow gruen (ohne `--all-targets` clippy)
