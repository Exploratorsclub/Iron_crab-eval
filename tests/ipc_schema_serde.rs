//! Blackbox: IPC Schema Serde Roundtrip (STORAGE_CONVENTIONS §4, DoD §B/E)
//!
//! Verifiziert: RecordHeader, ExplicitAmount, MarketEvent, TradeIntent, DecisionRecord,
//! ExecutionResult, RejectReason bleiben bei JSON roundtrip strukturell erhalten.
//!
//! System-Invariante Intent Causality Chain (INVARIANTS.md A.7): Jede Execution
//! rückverfolgbar zu decision_id und intent_id.

use ironcrab::ipc::{
    CheckResult, DecisionOutcome, DecisionRecord, ExecutionFees, ExecutionPnl, ExecutionResult,
    ExecutionStatus, ExplicitAmount, FillStatus, FillUnavailableReason, IntentOrigin, IntentTier,
    MarketEvent, MarketEventKind, RecordHeader, RejectReason, SimulationResult, TradeIntent,
    TradeResources, TradeSide, TradingRegime,
};
use rust_decimal::Decimal;

/// STORAGE_CONVENTIONS §4: RecordHeader Pflichtfelder erhalten sich im Roundtrip.
#[test]
fn record_header_roundtrip() {
    let header = RecordHeader::new("test-component", "v0.1.0", "run-12345");

    let json = serde_json::to_string(&header).unwrap();
    let parsed: RecordHeader = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.schema_version, header.schema_version);
    assert_eq!(parsed.component, header.component);
    assert_eq!(parsed.build, header.build);
    assert_eq!(parsed.run_id, header.run_id);
    assert!(parsed.ts_unix_ms > 0);
}

/// DoD §B: ExplicitAmount – raw, decimals, ui erhalten sich im Roundtrip.
#[test]
fn explicit_amount_units_roundtrip() {
    let sol = ExplicitAmount::new(1_500_000_000, 9);
    let json = serde_json::to_string(&sol).unwrap();
    let parsed: ExplicitAmount = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.raw, sol.raw);
    assert_eq!(parsed.decimals, sol.decimals);
    assert_eq!(parsed.ui, sol.ui);
}

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

/// STORAGE_CONVENTIONS §4.1: MarketEvent TokenMintInfo-Variante Roundtrip.
#[test]
fn market_event_token_mint_info_roundtrip() {
    let event = MarketEvent::new(
        "market-data",
        "v0.1.0",
        "run-abc",
        "evt-mint-001".to_string(),
        "geyser",
        Some(555),
        MarketEventKind::TokenMintInfo {
            mint: "So11111111111111111111111111111111111111112".to_string(),
            token_program: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            decimals: 9,
            supply: 1_000_000_000,
            mint_authority: None,
            freeze_authority: Some("FreezeAuth1111111111111111111111111111111".to_string()),
        },
    );

    let json = serde_json::to_string(&event).unwrap();
    let parsed: MarketEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.event_id, event.event_id);
    assert_eq!(parsed.source, event.source);
    assert_eq!(parsed.slot, event.slot);

    match &parsed.kind {
        MarketEventKind::TokenMintInfo {
            mint,
            token_program,
            decimals,
            supply,
            mint_authority,
            freeze_authority,
        } => {
            assert_eq!(mint, "So11111111111111111111111111111111111111112");
            assert_eq!(token_program, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
            assert_eq!(*decimals, 9);
            assert_eq!(*supply, 1_000_000_000);
            assert!(mint_authority.is_none());
            assert_eq!(
                freeze_authority.as_deref(),
                Some("FreezeAuth1111111111111111111111111111111")
            );
        }
        other => panic!("expected TokenMintInfo, got: {other:?}"),
    }
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

/// STORAGE_CONVENTIONS §4.2: TradeIntent execution.min_out Roundtrip.
#[test]
fn trade_intent_execution_min_out_roundtrip() {
    let json = r#"{
        "schema_version":1,
        "ts_unix_ms":1700000000000,
        "component":"test",
        "build":"test",
        "run_id":"run",
        "intent_id":"intent-typed-1",
        "source":"test",
        "tier":"Tier1",
        "origin_type":"StrategyA",
        "ttl_ms":5000,
        "required_capital":{"raw":1,"decimals":9},
        "resources":{"input_mint":"in","output_mint":"out","pools":["pool"],"accounts":[]},
        "expected_roi_bps":0,
        "max_slippage_bps":0,
        "side":"Sell",
        "regime":"Early",
        "execution":{"min_out":{"raw":42,"decimals":9}}
    }"#;

    let parsed: TradeIntent = serde_json::from_str(json).unwrap();
    let min_out = parsed
        .execution
        .as_ref()
        .and_then(|e| e.min_out.as_ref())
        .expect("execution.min_out should parse");
    assert_eq!(min_out.raw, 42);
    assert_eq!(min_out.decimals, 9);

    let serialized = serde_json::to_string(&parsed).unwrap();
    let roundtrip: TradeIntent = serde_json::from_str(&serialized).unwrap();
    let min_out_2 = roundtrip
        .execution
        .as_ref()
        .and_then(|e| e.min_out.as_ref())
        .unwrap();
    assert_eq!(min_out_2.raw, 42);
    assert_eq!(min_out_2.decimals, 9);
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

/// STORAGE_CONVENTIONS §4.4: ExecutionResult fill_status, fill_unavailable_reason Roundtrip.
#[test]
fn execution_result_fill_unavailable_roundtrip() {
    let result = ExecutionResult::new_sent(
        "execution-engine",
        "v0.1.0",
        "run-789",
        "exe-002".to_string(),
        "dec-002".to_string(),
        "intent-002".to_string(),
        "momentum-bot".to_string(),
        Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
        Some("5abcdef123456...".to_string()),
        None,
    )
    .mark_confirmed(
        12345,
        ExecutionFees {
            network_fee_lamports: 5000,
            tip_lamports: 0,
            compute_units: 150000,
        },
        ExecutionPnl {
            gross_lamports: 0,
            net_lamports: -5000,
            decimals: 9,
        },
        250,
    )
    .with_fills(None, None)
    .with_fill_diagnostics(
        FillStatus::Unavailable,
        Some(FillUnavailableReason::RpcTxFetchFailed),
    );

    let json = serde_json::to_string(&result).unwrap();
    let parsed: ExecutionResult = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.fill_status, Some(FillStatus::Unavailable));
    assert_eq!(
        parsed.fill_unavailable_reason,
        Some(FillUnavailableReason::RpcTxFetchFailed)
    );
}

/// STORAGE_CONVENTIONS §4.4: ExecutionResult error_code Roundtrip.
#[test]
fn execution_result_error_code_roundtrip() {
    let mut result = ExecutionResult::new_sent(
        "execution-engine",
        "v0.1.0",
        "run-789",
        "exe-003".to_string(),
        "dec-003".to_string(),
        "intent-003".to_string(),
        "momentum-bot".to_string(),
        Some("mint123".to_string()),
        None,
        None,
    )
    .with_error_code(Some("Custom(6005)".to_string()));
    result.status = ExecutionStatus::Failed;
    result.error_message = Some("execution_failed".to_string());

    let json = serde_json::to_string(&result).unwrap();
    let parsed: ExecutionResult = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.error_code.as_deref(), Some("Custom(6005)"));
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

/// STORAGE_CONVENTIONS §4.3, DoD §E: DecisionRecord SimFailed Roundtrip.
#[test]
fn decision_record_sim_failed_roundtrip() {
    let checks = vec![CheckResult {
        check_name: "simulation".to_string(),
        passed: false,
        reason_code: Some("SIM_FAILED".to_string()),
        details: Some("InstructionError".to_string()),
    }];

    let sim_result = SimulationResult {
        success: false,
        error_code: Some("InstructionError".to_string()),
        logs_preview: Some("Program failed: insufficient funds".to_string()),
        compute_units_consumed: Some(50_000),
    };

    let decision = DecisionRecord::new_sim_failed(
        "execution-engine",
        "v0.1.0",
        "run-456",
        "dec-002".to_string(),
        "intent-002".to_string(),
        "arb-strategy".to_string(),
        IntentOrigin::StrategyA,
        TradingRegime::Established,
        checks,
        "plan-hash-xyz".to_string(),
        sim_result,
    );

    let json = serde_json::to_string(&decision).unwrap();
    let parsed: DecisionRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.outcome, DecisionOutcome::SimFailed);
    assert!(parsed.simulate.is_some());
    assert!(!parsed.simulate.as_ref().unwrap().success);
    assert_eq!(parsed.plan_hash.as_deref(), Some("plan-hash-xyz"));
}

/// Intent Causality Chain (INVARIANTS.md A.7): Jede Execution rückverfolgbar zu decision_id und intent_id.
#[test]
fn intent_causality_chain() {
    let intent_id = "intent-corr-001";
    let decision_id = "dec-corr-001";

    let intent = TradeIntent::new(
        "test",
        "v0.1.0",
        "run-test",
        intent_id.to_string(),
        "test-strategy",
        IntentTier::Tier1,
        IntentOrigin::StrategyA,
        ExplicitAmount::new(100, 9),
        TradeResources::default(),
        0,
        100,
        TradeSide::Buy,
        TradingRegime::NotApplicable,
    );

    let decision = DecisionRecord::new_rejected(
        "test",
        "v0.1.0",
        "run-test",
        decision_id.to_string(),
        intent_id.to_string(),
        "test-strategy".to_string(),
        IntentOrigin::StrategyA,
        TradingRegime::NotApplicable,
        vec![],
        "TEST".to_string(),
    );

    let execution = ExecutionResult::new_sent(
        "test",
        "v0.1.0",
        "run-test",
        "exe-corr-001".to_string(),
        decision_id.to_string(),
        intent_id.to_string(),
        "test-strategy".to_string(),
        Some("So11111111111111111111111111111111111111112".to_string()),
        None,
        None,
    );

    assert_eq!(intent.intent_id, intent_id);
    assert_eq!(decision.intent_id, intent_id);
    assert_eq!(decision.decision_id, decision_id);
    assert_eq!(execution.intent_id, intent_id);
    assert_eq!(execution.decision_id, decision_id);
}

/// DoD §J: RejectReason serialisiert und deserialisiert zu identischem Wert.
#[test]
fn reject_reason_roundtrip() {
    let reasons = [
        RejectReason::TtlExpired,
        RejectReason::RiskDailyLossLimit,
        RejectReason::SimFailed,
        RejectReason::LockCapitalConflict,
        RejectReason::MissingDecimals,
    ];

    for reason in reasons {
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: RejectReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, reason, "Roundtrip failed for {:?}", reason);
    }
}

/// STORAGE_CONVENTIONS §4: JSONL – eine Zeile pro Record, zeilenweise parsebar.
#[test]
fn jsonl_line_format() {
    let events = vec![
        MarketEvent::new(
            "test",
            "v1",
            "run",
            "e1".to_string(),
            "test",
            Some(1),
            MarketEventKind::SlotUpdate { current_slot: 1 },
        ),
        MarketEvent::new(
            "test",
            "v1",
            "run",
            "e2".to_string(),
            "test",
            Some(2),
            MarketEventKind::SlotUpdate { current_slot: 2 },
        ),
    ];

    let mut jsonl = String::new();
    for event in &events {
        let line = serde_json::to_string(event).unwrap();
        assert!(!line.contains('\n'), "Each line must not contain newlines");
        jsonl.push_str(&line);
        jsonl.push('\n');
    }

    for (i, line) in jsonl.lines().enumerate() {
        let parsed: MarketEvent = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.event_id, format!("e{}", i + 1));
    }
}
