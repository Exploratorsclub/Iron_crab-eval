//! Blackbox: ipc::schema Serde Roundtrip
//!
//! Verifiziert: MarketEvent, TradeIntent, ExecutionResult, DecisionRecord
//! bleiben bei JSON roundtrip strukturell erhalten.

use ironcrab::ipc::{
    CheckResult, DecisionOutcome, DecisionRecord, ExecutionResult, ExecutionStatus, ExplicitAmount,
    IntentOrigin, IntentTier, MarketEvent, MarketEventKind, TradeIntent, TradeResources, TradeSide,
    TradingRegime,
};
use rust_decimal::Decimal;

#[test]
fn market_event_roundtrip() {
    let event = MarketEvent::new(
        "market-data",
        "v0.1.0",
        "run-abc",
        "evt-001".to_string(),
        "geyser",
        Some(12345),
        MarketEventKind::PoolCreated {
            pool_address: "Pool123".to_string(),
            base_mint: "BaseMint".to_string(),
            quote_mint: "QuoteMint".to_string(),
            dex: "raydium".to_string(),
            initial_liquidity_sol: Some(Decimal::from(100)),
        },
    );

    let json = serde_json::to_string(&event).unwrap();
    let parsed: MarketEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.event_id, event.event_id);
    assert_eq!(parsed.source, event.source);
    assert_eq!(parsed.slot, event.slot);
}

#[test]
fn trade_intent_roundtrip() {
    let mut resources = TradeResources::default();
    resources.pools.push("PoolAddr".to_string());
    resources.input_mint = "So11111111111111111111111111111111111111112".to_string();
    resources.output_mint = "MintAddr".to_string();

    let intent = TradeIntent::new(
        "momentum-bot",
        "v0.1",
        "run-1",
        "intent-001".to_string(),
        "momentum-bot",
        IntentTier::Tier0,
        IntentOrigin::StrategyA,
        ExplicitAmount::new(10_000_000, 9),
        resources,
        0,
        300,
        TradeSide::Buy,
        TradingRegime::Early,
    );

    let json = serde_json::to_string(&intent).unwrap();
    let parsed: TradeIntent = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.intent_id, intent.intent_id);
    assert_eq!(parsed.side, intent.side);
    assert_eq!(parsed.required_capital.raw, intent.required_capital.raw);
}

#[test]
fn execution_result_roundtrip() {
    let result = ExecutionResult::new_sent(
        "exec-engine",
        "v0.1",
        "run-1",
        "exec-001".to_string(),
        "decision-001".to_string(),
        "intent-001".to_string(),
        "momentum-bot".to_string(),
        Some("Mint11111111111111111111111111111111".to_string()),
        Some("sig123".to_string()),
        None,
    );

    let json = serde_json::to_string(&result).unwrap();
    let parsed: ExecutionResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.execution_id, result.execution_id);
    assert_eq!(parsed.status, ExecutionStatus::Sent);
}

#[test]
fn decision_record_roundtrip() {
    let record = DecisionRecord::new_rejected(
        "exec-engine",
        "v0.1",
        "run-1",
        "dec-001".to_string(),
        "intent-001".to_string(),
        "momentum-bot".to_string(),
        IntentOrigin::StrategyA,
        TradingRegime::Early,
        vec![CheckResult {
            check_name: "test_check".to_string(),
            passed: true,
            reason_code: None,
            details: None,
        }],
        "TEST".to_string(),
    );

    let json = serde_json::to_string(&record).unwrap();
    let parsed: DecisionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.decision_id, record.decision_id);
    assert_eq!(parsed.intent_id, record.intent_id);
    assert_eq!(parsed.outcome, DecisionOutcome::Rejected);
}
