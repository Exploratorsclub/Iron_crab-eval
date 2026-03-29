//! Request/Reply E2E Contract Test (I-24c, I-24d, A.43 schmaler Slice)
//!
//! On-Wire Blackbox-Tests:
//! - EnsurePumpAmmPoolAccounts (PumpSwap pool_accounts) → market-data → ControlResponse
//! - EnsureRaydiumAmmPoolState (Raydium AMM v4) → market-data → ControlResponse
//! - EnsureOrcaWhirlpoolPoolState (Orca Whirlpool) → market-data → ControlResponse
//! - EnsureMeteoraDlmmPoolState (Meteora DLMM) → market-data → ControlResponse
//! - EnsurePumpfunBondingCurve (PumpFun Bonding Curve) → market-data → ControlResponse
//!
//! Beweist den Request/Reply-Contract fuer I-24d ohne Liquidation-E2E.
//! Erweiterte Felder (`force_refresh`, `pool_address_hint` auf `ControlRequest`) werden separat
//! in `ipc_schema_serde` roundtrip-getestet; der Basis-E2E-Test nutzt weiterhin nur `base_mint` (minimal).
//!
//! **A.43 (schmal, merged PR #67):** Vor dem **ersten** `build_tx_plan` loest die execution-engine
//! bounded PumpSwap-Preplan-Discovery nur unter einem **engeren** Gate (u.a. kein Replay-Modus,
//! noch kein Preplan-Versuch, Cold-Path-Recovery-Sell, `metadata.dex=pump_amm`,
//! `resources.accounts.is_empty()`, `resources.pools.len() == 1`, Hint-Pool-Zeile fuer den
//! tx_builder noch unbrauchbar). Dann: `EnsurePumpAmmPoolAccounts` mit
//! `pool_address_hint = pools[0]`, bounded Wait, ein Retry der Plan/Sim-Schleife.
//! **Blackbox-E2E:** Nur der Wire-Slice Intent → `EnsurePumpAmmPoolAccounts` + korrelierte terminale
//! `ControlResponse` (`market-data`). Nicht-leere `resources.accounts` **unterbinden** das Gate
//! (typischer CI-Timeout: kein Ensure sichtbar).
//!
//! STOP-CHECK: Nur Eval-Repo, kein Impl-Code, Blackbox an API-Grenze.

mod common;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use common::request_reply_e2e_harness::RequestReplyE2eHarness;
use futures::StreamExt;
use ironcrab::ipc::{
    ControlRequest, ControlRequestKind, ControlResponse, ControlResponseStatus, ExplicitAmount,
    IntentOrigin, IntentTier, TradeIntent, TradeResources, TradeSide, TradingRegime,
};
use ironcrab::nats::topics::{
    TOPIC_CONTROL_REQUESTS, TOPIC_CONTROL_RESPONSES, TOPIC_TRADE_INTENTS,
};
use solana_sdk::pubkey::Pubkey;

/// Timeout für Response-Empfang (Sekunden).
const RESPONSE_TIMEOUT_SECS: u64 = 15;

/// Absichtlich nicht auflösbare base_mint (Cold-Path Ensure* liefert typischerweise not_found).
const UNRESOLVABLE_BASE_MINT: &str = "11111111111111111111111111111111";

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

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

/// A.43: Wenn das merged Preplan-Gate greift (siehe Modul-Doku), erscheint ein korreliertes
/// `EnsurePumpAmmPoolAccounts` mit `pool_address_hint == pools[0]` und terminale `ControlResponse`
/// von `market-data`. Zwei Topics, race-sicher, `tokio::time::timeout` pro Iteration.
async fn wait_for_manual_pumpswap_cold_path_control_roundtrip(
    nats_url: &str,
    base_mint: &str,
    pool_address: &str,
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

    let mut matched_request_ids: HashSet<String> = HashSet::new();
    // Response vor zugehoerigem Ensure-Request (Race auf zwei Topics).
    let mut pending_terminal: HashSet<String> = HashSet::new();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(RESPONSE_TIMEOUT_SECS);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "timeout A.43: kein EnsurePumpAmmPoolAccounts+hint oder keine korrelierte market-data-Response nach {}s (base_mint={base_mint}, pool={pool_address})",
                RESPONSE_TIMEOUT_SECS
            ));
        }

        enum Branch {
            Req(async_nats::Message),
            Resp(async_nats::Message),
        }

        let branch = tokio::time::timeout(remaining, async {
            tokio::select! {
                m = sub_req.next() => {
                    match m {
                        Some(msg) => Ok(Branch::Req(msg)),
                        None => Err("control_requests stream ended".to_string()),
                    }
                }
                m = sub_resp.next() => {
                    match m {
                        Some(msg) => Ok(Branch::Resp(msg)),
                        None => Err("control_responses stream ended".to_string()),
                    }
                }
            }
        })
        .await;

        let branch = match branch {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(format!(
                    "timeout A.43: kein EnsurePumpAmmPoolAccounts+hint oder keine korrelierte market-data-Response nach {}s (base_mint={base_mint}, pool={pool_address})",
                    RESPONSE_TIMEOUT_SECS
                ));
            }
        };

        match branch {
            Branch::Req(msg) => {
                let req: ControlRequest = match serde_json::from_slice(msg.payload.as_ref()) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                if req.target != "market-data" {
                    continue;
                }
                let hint_ok = req.pool_address_hint.as_deref() == Some(pool_address);
                let kind_ok = matches!(
                    &req.kind,
                    ControlRequestKind::EnsurePumpAmmPoolAccounts { base_mint: bm } if bm == base_mint
                );
                if !(hint_ok && kind_ok) {
                    continue;
                }
                if pending_terminal.remove(&req.request_id) {
                    return Ok(());
                }
                matched_request_ids.insert(req.request_id);
            }
            Branch::Resp(msg) => {
                let resp: ControlResponse = match serde_json::from_slice(msg.payload.as_ref()) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                if resp.target != "market-data" {
                    continue;
                }
                let terminal = matches!(
                    resp.status,
                    ControlResponseStatus::Ok
                        | ControlResponseStatus::NotFound
                        | ControlResponseStatus::Error
                );
                if !terminal {
                    continue;
                }
                if matched_request_ids.contains(&resp.request_id) {
                    return Ok(());
                }
                pending_terminal.insert(resp.request_id);
            }
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

/// A.43 E2E: SELL-Intent mit genau dem **oeffentlichen** Shape, das das merged Preplan-Gate erwartet:
/// `metadata.sell_all=true`, `metadata.dex=pump_amm`, `resources.pools.len()==1`, `resources.accounts`
/// **leer** (nicht-leer → Gate greift nicht → Timeout ohne Ensure). `source=sell-all` wie typisches
/// Tooling; kein Claim, dass nur dieser `source` zulaessig ist.
#[test]
fn request_reply_e2e_manual_pumpswap_sell_all_pool_hint_roundtrip() {
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

    let token_mint_pk = Pubkey::new_unique();
    let base_mint = token_mint_pk.to_string();
    let pool_address_pk = Pubkey::new_unique();
    let pool_address = pool_address_pk.to_string();

    let resources = TradeResources {
        input_mint: base_mint.clone(),
        output_mint: WSOL_MINT.to_string(),
        pools: vec![pool_address.clone()],
        accounts: vec![],
        token_program: None,
    };

    let mut intent = TradeIntent::new(
        "ironcrab-eval",
        "e2e-a43",
        "run-e2e",
        format!(
            "intent-a43-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ),
        "sell-all",
        IntentTier::Tier0,
        IntentOrigin::StrategyA,
        ExplicitAmount::new(1_000_000, 6),
        resources,
        0,
        500,
        TradeSide::Sell,
        TradingRegime::Early,
    );
    let mut md = HashMap::new();
    md.insert("sell_all".to_string(), "true".to_string());
    md.insert("dex".to_string(), "pump_amm".to_string());
    intent.metadata = md;

    let intent_payload = serde_json::to_vec(&intent).expect("serialize TradeIntent (A.43)");

    let nats_url = harness.nats_url().to_string();
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let result = rt.block_on(wait_for_manual_pumpswap_cold_path_control_roundtrip(
        &nats_url,
        &base_mint,
        &pool_address,
        intent_payload,
    ));

    harness.stop();

    result.expect(
        "A.43: EnsurePumpAmmPoolAccounts mit pool_address_hint aus pools[0] und market-data-Response",
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
