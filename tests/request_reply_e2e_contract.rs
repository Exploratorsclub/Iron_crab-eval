//! Request/Reply E2E Contract Test (I-24c, I-24d)
//!
//! On-Wire Blackbox-Tests:
//! - EnsurePumpAmmPoolAccounts (PumpSwap pool_accounts) → market-data → ControlResponse
//! - EnsureRaydiumAmmPoolState (Raydium AMM v4) → market-data → ControlResponse
//! - EnsureOrcaWhirlpoolPoolState (Orca Whirlpool) → market-data → ControlResponse
//! - EnsureMeteoraDlmmPoolState (Meteora DLMM) → market-data → ControlResponse
//! - EnsurePumpfunBondingCurve (PumpFun Bonding Curve) → market-data → ControlResponse
//! - I-24e / A.43: manueller Sell-All-Cold-Path (`source=ui-manual`, `metadata.sell_all=true`,
//!   `metadata.dex=pump_amm`) → execution-engine → EnsurePumpAmmPoolAccounts mit
//!   `pool_address_hint` (resources.pools[0]) → korrelierte ControlResponse
//!
//! Beweist den Request/Reply-Contract fuer I-24d ohne Liquidation-E2E.
//! Erweiterte Felder (`force_refresh`, `pool_address_hint` auf `ControlRequest`) werden zusaetzlich
//! in `ipc_schema_serde` roundtrip-getestet; die PumpSwap-Hint-Kette wird hier per Intent→Control-Wire
//! abgesichert (kein Claim ueber interne Priorisierung ausserhalb des beobachtbaren NATS-Vertrags).
//!
//! STOP-CHECK: Nur Eval-Repo, kein Impl-Code, Blackbox an API-Grenze.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::request_reply_e2e_harness::RequestReplyE2eHarness;
use futures::StreamExt;
use ironcrab::ipc::{
    ControlRequest, ControlRequestKind, ControlResponse, ControlResponseStatus, ExplicitAmount,
    IntentOrigin, IntentTier, TradeExecutionConstraints, TradeIntent, TradeResources, TradeSide,
    TradingRegime,
};
use ironcrab::nats::topics::{
    TOPIC_CONTROL_REQUESTS, TOPIC_CONTROL_RESPONSES, TOPIC_TRADE_INTENTS,
};
use solana_sdk::pubkey::Pubkey;

/// Timeout für Response-Empfang (Sekunden).
const RESPONSE_TIMEOUT_SECS: u64 = 15;

/// Intent→Control→Response: etwas groesserzuegig (Engine + market-data).
const INTENT_CONTROL_E2E_TIMEOUT_SECS: u64 = 45;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// Absichtlich nicht auflösbare base_mint (Cold-Path Ensure* liefert typischerweise not_found).
const UNRESOLVABLE_BASE_MINT: &str = "11111111111111111111111111111111";

/// Bounded Poll auf `TOPIC_CONTROL_RESPONSES`: wartet auf `ControlResponse` mit passender
/// `request_id`, `target == "market-data"`, und terminalem Status (ok | not_found | error).
async fn wait_for_correlated_market_data_response(
    nats_url: &str,
    request_id: &str,
    request_payload: Vec<u8>,
) -> Result<(), String> {
    let client = async_nats::connect(nats_url)
        .await
        .map_err(|e| format!("connect: {}", e))?;

    let mut sub = client
        .subscribe(TOPIC_CONTROL_RESPONSES.to_string())
        .await
        .map_err(|e| format!("subscribe: {}", e))?;

    client
        .publish(TOPIC_CONTROL_REQUESTS.to_string(), request_payload.into())
        .await
        .map_err(|e| format!("publish: {}", e))?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(RESPONSE_TIMEOUT_SECS);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "timeout: keine korrelierte Response fuer request_id={:?} nach {}s",
                request_id, RESPONSE_TIMEOUT_SECS
            ));
        }

        let msg = match tokio::time::timeout(remaining, sub.next()).await {
            Ok(Some(m)) => m,
            Ok(None) => return Err("stream ended".to_string()),
            Err(_) => {
                return Err(format!(
                    "timeout: keine korrelierte Response fuer request_id={:?} nach {}s",
                    request_id, RESPONSE_TIMEOUT_SECS
                ));
            }
        };

        let resp: ControlResponse = match serde_json::from_slice(msg.payload.as_ref()) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if resp.request_id != request_id {
            continue;
        }
        if resp.target != "market-data" {
            continue;
        }

        match resp.status {
            ControlResponseStatus::Ok
            | ControlResponseStatus::NotFound
            | ControlResponseStatus::Error => return Ok(()),
        }
    }
}

/// A.43 / I-24e: `TradeIntent.resources.pools[0]` (PumpSwap-Pool-Hint) erscheint auf dem Wire als
/// `ControlRequest.pool_address_hint` fuer `EnsurePumpAmmPoolAccounts`, danach terminale
/// `ControlResponse` von market-data (kein Overclaim: nur beobachtbare NATS-Korrelation).
///
/// Bricht mit Fehler ab, wenn kein passender ControlRequest/Response innerhalb des Timeouts eintrifft
/// (Engine verarbeitet den Intent-Pfad nicht oder Hint wird nicht gefuehrt).
async fn wait_for_intent_pumpswap_pool_hint_control_plane(
    nats_url: &str,
    base_mint: &str,
    expected_pool_hint: &str,
    intent_payload: Vec<u8>,
) -> Result<(), String> {
    let client = async_nats::connect(nats_url)
        .await
        .map_err(|e| format!("connect: {}", e))?;

    let mut sub_req = client
        .subscribe(TOPIC_CONTROL_REQUESTS.to_string())
        .await
        .map_err(|e| format!("subscribe control_requests: {}", e))?;
    let mut sub_resp = client
        .subscribe(TOPIC_CONTROL_RESPONSES.to_string())
        .await
        .map_err(|e| format!("subscribe control_responses: {}", e))?;

    client
        .publish(TOPIC_TRADE_INTENTS.to_string(), intent_payload.into())
        .await
        .map_err(|e| format!("publish trade_intents: {}", e))?;

    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(INTENT_CONTROL_E2E_TIMEOUT_SECS);
    let mut matched_request_id: Option<String> = None;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "timeout: kein EnsurePumpAmmPoolAccounts mit pool_address_hint={expected_pool_hint:?} und base_mint={base_mint:?} innerhalb von {}s",
                INTENT_CONTROL_E2E_TIMEOUT_SECS
            ));
        }

        if matched_request_id.is_none() {
            let msg = match tokio::time::timeout(remaining, sub_req.next()).await {
                Ok(Some(m)) => m,
                Ok(None) => return Err("NATS control_requests stream ended".to_string()),
                Err(_) => {
                    return Err(format!(
                        "timeout: kein passender EnsurePumpAmmPoolAccounts fuer base_mint={base_mint:?}"
                    ));
                }
            };
            let req: ControlRequest = match serde_json::from_slice(msg.payload.as_ref()) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let ControlRequestKind::EnsurePumpAmmPoolAccounts { base_mint: bm } = &req.kind else {
                continue;
            };
            if bm != base_mint {
                continue;
            }
            if req.pool_address_hint.as_deref() != Some(expected_pool_hint) {
                continue;
            }
            matched_request_id = Some(req.request_id.clone());
            continue;
        }

        let rid = matched_request_id.as_ref().unwrap();
        let msg = match tokio::time::timeout(remaining, sub_resp.next()).await {
            Ok(Some(m)) => m,
            Ok(None) => return Err("NATS control_responses stream ended".to_string()),
            Err(_) => {
                return Err(format!(
                    "timeout: keine korrelierte ControlResponse fuer request_id={rid:?}"
                ));
            }
        };
        let resp: ControlResponse = match serde_json::from_slice(msg.payload.as_ref()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if resp.request_id != *rid {
            continue;
        }
        if resp.target != "market-data" {
            continue;
        }
        match resp.status {
            ControlResponseStatus::Ok
            | ControlResponseStatus::NotFound
            | ControlResponseStatus::Error => return Ok(()),
        }
    }
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let iron_crab = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab");
    if !iron_crab.join("Cargo.toml").exists() {
        eprintln!("SKIP: Iron_crab nicht als Sibling gefunden.");
        return None;
    }
    Some(iron_crab)
}

/// Echter On-Wire Request/Reply Contract: EnsurePumpAmmPoolAccounts publizieren, korrelierte Response prüfen.
#[test]
fn request_reply_contract_market_data_responds() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    let nats_url = harness.nats_url().to_string();
    let request_id = format!(
        "e2e-contract-pump-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // EnsurePumpAmmPoolAccounts (PumpSwap pool_accounts): target=market-data, absichtlich nicht auflösbar
    let kind = ControlRequestKind::EnsurePumpAmmPoolAccounts {
        base_mint: UNRESOLVABLE_BASE_MINT.to_string(),
    };
    let req = ControlRequest::new(
        "ironcrab-eval",
        "e2e-contract",
        "run-e2e",
        request_id.clone(),
        "market-data",
        kind,
    );
    let payload = serde_json::to_vec(&req).expect("serialize EnsurePumpAmmPoolAccounts");

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_correlated_market_data_response(
        &nats_url,
        &request_id,
        payload,
    ));

    harness.stop();

    result.expect("Request/Reply Contract: market-data muss korreliert antworten (PumpSwap)");
}

/// A.43 / I-24e: PumpSwap manueller Sell-All-Cold-Path — gleiches Wire-Muster wie
/// `trade_intent_manual_sell_all_pumpfun_route_roundtrip` (Eval), aber DEX `pump_amm` und 14er Accounts.
/// Expliziter Pool-Hint (`resources.pools[0]`) erscheint als `pool_address_hint` auf
/// `EnsurePumpAmmPoolAccounts`; danach terminale `ControlResponse`.
/// Normativ: Iron_crab/docs/MOMENTUM_V2_SPEC.md §10.2 (Pump AMM): genau ein Pool, `accounts[0] == pools[0]`, 14 Accounts.
#[test]
fn request_reply_e2e_pumpswap_intent_pool_hint_on_control_request() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let pool = Pubkey::new_unique();
    let pool_str = pool.to_string();
    let mut accounts: Vec<String> = (0..14).map(|_| Pubkey::new_unique().to_string()).collect();
    accounts[0] = pool_str.clone();

    let base_mint = Pubkey::new_unique();
    let base_mint_str = base_mint.to_string();

    let resources = TradeResources {
        input_mint: base_mint_str.clone(),
        output_mint: WSOL_MINT.to_string(),
        pools: vec![pool_str.clone()],
        accounts,
        token_program: None,
    };

    let mut intent = TradeIntent::new(
        "ironcrab-eval",
        "e2e-pumpswap-hint",
        "run-e2e",
        format!(
            "intent-pumpswap-hint-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ),
        "ui-manual",
        IntentTier::Tier0,
        IntentOrigin::StrategyA,
        ExplicitAmount::new(1_000_000, 6),
        resources,
        0,
        500,
        TradeSide::Sell,
        TradingRegime::Early,
    );
    intent
        .metadata
        .insert("sell_all".to_string(), "true".to_string());
    intent
        .metadata
        .insert("dex".to_string(), "pump_amm".to_string());
    intent.execution = Some(TradeExecutionConstraints {
        min_out: Some(ExplicitAmount::new(1, 9)),
    });

    let intent_payload =
        serde_json::to_vec(&intent).expect("serialize TradeIntent (PumpSwap hint path)");

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    let nats_url = harness.nats_url().to_string();
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_intent_pumpswap_pool_hint_control_plane(
        &nats_url,
        &base_mint_str,
        &pool_str,
        intent_payload,
    ));

    harness.stop();

    result.expect(
        "A.43 Wire: Intent pools[0] muss als pool_address_hint auf EnsurePumpAmmPoolAccounts erscheinen und market-data antwortet terminal",
    );
}

/// Raydium AMM v4: EnsureRaydiumAmmPoolState → market-data → korrelierte terminale ControlResponse.
#[test]
fn request_reply_contract_raydium_amm_market_data_responds() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    let nats_url = harness.nats_url().to_string();
    let request_id = format!(
        "e2e-contract-raydium-amm-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let kind = ControlRequestKind::EnsureRaydiumAmmPoolState {
        base_mint: UNRESOLVABLE_BASE_MINT.to_string(),
    };
    let req = ControlRequest::new(
        "ironcrab-eval",
        "e2e-contract-raydium",
        "run-e2e",
        request_id.clone(),
        "market-data",
        kind,
    );
    let payload = serde_json::to_vec(&req).expect("serialize EnsureRaydiumAmmPoolState");

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_correlated_market_data_response(
        &nats_url,
        &request_id,
        payload,
    ));

    harness.stop();

    result.expect("Request/Reply Contract: market-data muss korreliert antworten (Raydium AMM v4)");
}

/// Orca Whirlpool: EnsureOrcaWhirlpoolPoolState → market-data → korrelierte terminale ControlResponse.
#[test]
fn request_reply_contract_orca_whirlpool_market_data_responds() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    let nats_url = harness.nats_url().to_string();
    let request_id = format!(
        "e2e-contract-orca-whirlpool-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let kind = ControlRequestKind::EnsureOrcaWhirlpoolPoolState {
        base_mint: UNRESOLVABLE_BASE_MINT.to_string(),
    };
    let req = ControlRequest::new(
        "ironcrab-eval",
        "e2e-contract-orca-whirlpool",
        "run-e2e",
        request_id.clone(),
        "market-data",
        kind,
    );
    let payload = serde_json::to_vec(&req).expect("serialize EnsureOrcaWhirlpoolPoolState");

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_correlated_market_data_response(
        &nats_url,
        &request_id,
        payload,
    ));

    harness.stop();

    result.expect("Request/Reply Contract: market-data muss korreliert antworten (Orca Whirlpool)");
}

/// Meteora DLMM: EnsureMeteoraDlmmPoolState → market-data → korrelierte terminale ControlResponse.
#[test]
fn request_reply_contract_meteora_dlmm_market_data_responds() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    let nats_url = harness.nats_url().to_string();
    let request_id = format!(
        "e2e-contract-meteora-dlmm-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let kind = ControlRequestKind::EnsureMeteoraDlmmPoolState {
        base_mint: UNRESOLVABLE_BASE_MINT.to_string(),
    };
    let req = ControlRequest::new(
        "ironcrab-eval",
        "e2e-contract-meteora-dlmm",
        "run-e2e",
        request_id.clone(),
        "market-data",
        kind,
    );
    let payload = serde_json::to_vec(&req).expect("serialize EnsureMeteoraDlmmPoolState");

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_correlated_market_data_response(
        &nats_url,
        &request_id,
        payload,
    ));

    harness.stop();

    result.expect("Request/Reply Contract: market-data muss korreliert antworten (Meteora DLMM)");
}

/// PumpFun Bonding Curve: EnsurePumpfunBondingCurve → market-data → korrelierte terminale ControlResponse.
#[test]
fn request_reply_contract_pumpfun_bonding_curve_market_data_responds() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    let nats_url = harness.nats_url().to_string();
    let request_id = format!(
        "e2e-contract-pumpfun-bc-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let kind = ControlRequestKind::EnsurePumpfunBondingCurve {
        base_mint: UNRESOLVABLE_BASE_MINT.to_string(),
    };
    let req = ControlRequest::new(
        "ironcrab-eval",
        "e2e-contract-pumpfun-bc",
        "run-e2e",
        request_id.clone(),
        "market-data",
        kind,
    );
    let payload = serde_json::to_vec(&req).expect("serialize EnsurePumpfunBondingCurve");

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_correlated_market_data_response(
        &nats_url,
        &request_id,
        payload,
    ));

    harness.stop();

    result.expect(
        "Request/Reply Contract: market-data muss korreliert antworten (PumpFun Bonding Curve)",
    );
}
