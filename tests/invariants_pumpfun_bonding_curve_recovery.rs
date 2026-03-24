//! PumpFun Bonding-Curve Cold-Path-Recovery: Blackbox-Vertrag (IPC + Hot-Path-Grenze).
//!
//! Abgedeckte Invarianten (Eval, öffentliche API / On-Wire):
//! - `ControlRequestKind::EnsurePumpfunBondingCurve` + `force_refresh_pumpfun` serialisieren
//!   den **Force-Refresh**-Intent (kein reines Cache-first für diesen Request).
//! - Regulärer PumpFun-Hot-Path (hier: PumpFun AMM Quote bei Cache-Miss) bleibt ohne
//!   Control-Plane-Recovery; ergänzend zu INVARIANTS.md A.12 / `invariants_hot_path_no_rpc`.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Impl unter `Iron_crab/src/`;
//! Assertions passen zur dokumentierten Semantik der öffentlichen Typen.
//!
//! Rest-Risiko: „genau ein bounded Retry“ und JetStream-`PoolCacheUpdate` sind im öffentlichen
//! `ironcrab::ipc`-Schema nicht als Zähler/Metrik abbildbar — E2E-Verifikation erfordert
//! Prozess-Observability oder Integrationstests ausserhalb dieses schmalen Wire-Vertrags.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::request_reply_e2e_harness::RequestReplyE2eHarness;
use futures::StreamExt;
use ironcrab::ipc::{ControlRequest, ControlRequestKind, ControlResponse, ControlResponseStatus};
use ironcrab::nats::topics::{TOPIC_CONTROL_REQUESTS, TOPIC_CONTROL_RESPONSES};
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
const DUMMY_RPC: &str = "http://127.0.0.1:0";

/// Placeholder base58 mint für Wire-Tests (kein Chain-Lookup nötig).
const SAMPLE_BASE_MINT: &str = "BaseMint11111111111111111111111111111111";

const RESPONSE_TIMEOUT_SECS: u64 = 15;

/// I-24d / PumpFun: On-Wire-Format für `EnsurePumpfunBondingCurve` mit `force_refresh_pumpfun`.
/// Blackbox: JSON muss den Cold-Path-Force-Refresh tragen und roundtrippen.
#[test]
fn control_request_ensure_pumpfun_bonding_curve_force_refresh_roundtrip() {
    let kind = ControlRequestKind::EnsurePumpfunBondingCurve {
        base_mint: SAMPLE_BASE_MINT.to_string(),
    };
    let mut req = ControlRequest::new(
        "ironcrab-eval",
        "wire-test",
        "run-1",
        "req-pumpfun-bc".to_string(),
        "market-data",
        kind,
    );
    req.force_refresh_pumpfun = true;
    // Explizit: PumpSwap-Force-Refresh darf für diesen Vertrag aus sein (orthogonale Flags).
    req.force_refresh = false;

    let json = serde_json::to_string(&req).expect("serialize ControlRequest");
    assert!(
        json.contains("ensure_pumpfun_bonding_curve"),
        "JSON muss kind=ensure_pumpfun_bonding_curve tragen (On-Wire-Tag): {json}"
    );
    assert!(
        json.contains("\"force_refresh_pumpfun\":true"),
        "JSON muss force_refresh_pumpfun=true für Cold-Path-Recovery tragen (nicht cache-only): {json}"
    );

    let parsed: ControlRequest = serde_json::from_str(&json).expect("deserialize ControlRequest");
    assert!(
        parsed.force_refresh_pumpfun,
        "force_refresh_pumpfun muss roundtrippen"
    );
    assert!(
        !parsed.force_refresh,
        "force_refresh (PumpSwap) bleibt false für diesen PumpFun-Vertrag"
    );
    match &parsed.kind {
        ControlRequestKind::EnsurePumpfunBondingCurve { base_mint } => {
            assert_eq!(base_mint, SAMPLE_BASE_MINT);
        }
        other => panic!("expected EnsurePumpfunBondingCurve, got {other:?}"),
    }
}

/// Hot-Path: PumpFun AMM bleibt bei Cache-Miss ohne RPC/Control-Recovery (kein blockierender Cold-Path).
#[tokio::test]
async fn pumpfun_hot_path_amm_quote_cache_miss_no_control_plane_recovery() {
    let cache = Arc::new(ironcrab::execution::live_pool_cache::LivePoolCache::new());
    let rpc = Arc::new(SolanaRpc::new(DUMMY_RPC));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);
    let unknown_mint = Pubkey::new_unique();
    let result = dex
        .quote_exact_in(WSOL_MINT, &unknown_mint.to_string(), 1_000_000)
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

/// Optional: market-data antwortet auf `EnsurePumpfunBondingCurve` (wie andere ControlRequests).
/// Benötigt nats-server, Sibling `Iron_crab` und gebaute Binaries — sonst Skip.
#[test]
fn request_reply_market_data_accepts_ensure_pumpfun_bonding_curve() {
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
        "pumpfun-bc-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let kind = ControlRequestKind::EnsurePumpfunBondingCurve {
        base_mint: SAMPLE_BASE_MINT.to_string(),
    };
    let mut req = ControlRequest::new(
        "ironcrab-eval",
        "pumpfun-bc-recovery",
        "run-e2e",
        request_id.clone(),
        "market-data",
        kind,
    );
    req.force_refresh_pumpfun = true;

    let payload = serde_json::to_vec(&req).expect("serialize EnsurePumpfunBondingCurve");
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
                    "timeout: keine korrelierte Response fuer request_id={request_id:?} nach {RESPONSE_TIMEOUT_SECS}s"
                ));
            }

            let msg = match tokio::time::timeout(remaining, sub.next()).await {
                Ok(Some(m)) => m,
                Ok(None) => return Err("stream ended".to_string()),
                Err(_) => {
                    return Err(format!(
                        "timeout: keine korrelierte Response fuer request_id={request_id:?} nach {RESPONSE_TIMEOUT_SECS}s"
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
    });

    harness.stop();
    result.expect("market-data muss auf EnsurePumpfunBondingCurve korreliert antworten");
}
