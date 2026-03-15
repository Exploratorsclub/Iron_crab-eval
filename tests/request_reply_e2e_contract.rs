//! Request/Reply E2E Contract Test (I-24c, I-24d)
//!
//! On-Wire Blackbox-Test: ControlRequest → market-data → ControlResponse.
//! Beweist den Request/Reply-Contract ohne Liquidation-E2E.
//!
//! STOP-CHECK: Nur Eval-Repo, kein Impl-Code, Blackbox an API-Grenze.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::request_reply_e2e_harness::RequestReplyE2eHarness;
use futures::StreamExt;
use ironcrab::ipc::{ControlRequest, ControlRequestKind};
use ironcrab::nats::topics::{TOPIC_CONTROL_REQUESTS, TOPIC_CONTROL_RESPONSES};

/// Timeout für Response-Empfang (Sekunden).
const RESPONSE_TIMEOUT_SECS: u64 = 15;

/// Echter On-Wire Request/Reply Contract: Request publizieren, korrelierte Response prüfen.
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

    // ControlRequest: target=market-data, absichtlich nicht auflösbar (ResetKillSwitch ist execution-engine)
    let req = ControlRequest::new(
        "ironcrab-eval",
        "e2e-contract",
        "run-e2e",
        request_id.clone(),
        "market-data",
        ControlRequestKind::ResetKillSwitch,
    );
    let payload = serde_json::to_vec(&req).expect("serialize ControlRequest");

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

        let msg = tokio::time::timeout(Duration::from_secs(RESPONSE_TIMEOUT_SECS), sub.next())
            .await
            .map_err(|_| format!("timeout: keine Response nach {}s", RESPONSE_TIMEOUT_SECS))?
            .ok_or("stream ended")?;

        let body: serde_json::Value = serde_json::from_slice(msg.payload.as_ref())
            .map_err(|e| format!("parse response: {}", e))?;

        let resp_request_id = body.get("request_id").and_then(|v| v.as_str());
        let resp_target = body.get("target").and_then(|v| v.as_str());
        let status = body
            .get("status")
            .or_else(|| body.get("outcome"))
            .and_then(|v| v.as_str());

        if resp_request_id != Some(request_id.as_str()) {
            return Err(format!(
                "request_id mismatch: expected {:?}, got {:?}",
                request_id, resp_request_id
            ));
        }
        if resp_target.map(|t| t != "market-data").unwrap_or(true) {
            return Err(format!(
                "target mismatch: expected market-data, got {:?}",
                resp_target
            ));
        }
        match status {
            Some("ok") | Some("not_found") | Some("error") => Ok(()),
            other => Err(format!(
                "kein terminaler Outcome: expected ok|not_found|error, got {:?}",
                other
            )),
        }
    });

    harness.stop();

    result.expect("Request/Reply Contract: market-data muss korreliert antworten");
}
