//! Invariant: Der durable JetStream-Consumer fuer `TRADE_INTENTS` in der Execution Engine
//! muss `DeliverPolicy::New` verwenden — nur Live-Intents im Hot Path, kein Replay alter Intents.
//!
//! Erwartung vor Impl-Fix: **rot** (aktuell `DeliverPolicy::All`, vgl. Prod-Evidenz
//! `execution_intent_header_to_receive_ms` Ø ~4942 ms).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; oeffentliche `ironcrab::nats`-API;
//! keine Assertions auf private Implementierungsdetails.

use async_nats::jetstream::consumer::DeliverPolicy;
use ironcrab::nats::trade_intents_consumer_config;

#[test]
fn trade_intents_consumer_uses_new_deliver_policy() {
    let cfg = trade_intents_consumer_config();

    assert_eq!(
        cfg.durable_name.as_deref(),
        Some("execution-engine"),
        "durable TRADE_INTENTS consumer name must be execution-engine"
    );

    assert!(
        matches!(cfg.deliver_policy, DeliverPolicy::New),
        "TRADE_INTENTS consumer must use DeliverPolicy::New (live hot-path only, no stale \
         intent replay); got {:?}",
        cfg.deliver_policy
    );
}
