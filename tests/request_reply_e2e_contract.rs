//! Request/Reply E2E Contract Test (I-24c, I-24d)
//!
//! On-Wire Blackbox-Test: EnsurePumpAmmPoolAccounts (PumpSwap pool_accounts) → market-data → ControlResponse.
//! Beweist den Request/Reply-Contract fuer I-24d ohne Liquidation-E2E.
//!
//! STOP-CHECK: Nur Eval-Repo, kein Impl-Code, Blackbox an API-Grenze.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::request_reply_e2e_harness::RequestReplyE2eHarness;
use futures::StreamExt;
use ironcrab::ipc::{ControlRequest, ControlRequestKind, ControlResponse, ControlResponseStatus};
use ironcrab::nats::topics::{TOPIC_CONTROL_REQUESTS, TOPIC_CONTROL_RESPONSES};

/// Timeout für Response-Empfang (Sekunden).
const RESPONSE_TIMEOUT_SECS: u64 = 15;

/// Absichtlich nicht auflösbare base_mint (EnsurePumpAmmPoolAccounts liefert not_found).
const UNRESOLVABLE_BASE_MINT: &str = "11111111111111111111111111111111";

/// Echter On-Wire Request/Reply Contract: EnsurePumpAmmPoolAccounts publizieren, korrelierte Response prüfen.
#[test]
fn request_reply_contract_market_data_responds() {
    let iron_crab = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab");
    if !iron_crab.join("Cargo.toml").exists() {
        eprintln!("SKIP: Iron_crab nicht als Sibling gefunden.");
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
        "e2e-contract-{}",
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
    let result = rt.block_on(async {
        let client = async_nats::connect(&nats_url)
            .await
            .map_err(|e| format!("connect: {}", e))?;

        let mut sub = client
            .subscribe(TOPIC_CONTROL_RESPONSES.to_string())
            .await
            .map_err(|e| format!("subscribe: {}", e))?;

        client
            .publish(TOPIC_CONTROL_REQUESTS.to_string(), payload.into())
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
                Err(_) => continue, // Keine gültige ControlResponse, weiter pollen
            };

            if resp.request_id != request_id {
                continue; // Andere request_id, weiter pollen
            }
            if resp.target != "market-data" {
                continue; // Falscher target, weiter pollen
            }

            match resp.status {
                ControlResponseStatus::Ok
                | ControlResponseStatus::NotFound
                | ControlResponseStatus::Error => return Ok(()),
            }
        }
    });

    harness.stop();

    result.expect("Request/Reply Contract: market-data muss korreliert antworten");
}
