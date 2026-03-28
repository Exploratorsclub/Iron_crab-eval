//! PumpFun Bonding-Curve Cold-Path-Recovery: Blackbox-Vertrag (IPC + Hot-Path-Grenze).
//!
//! Abgedeckte Invarianten (Eval, öffentliche API / On-Wire):
//! - `ControlRequestKind::EnsurePumpfunBondingCurve` + `force_refresh_pumpfun` serialisieren
//!   den **Force-Refresh**-Intent (kein reines Cache-first für diesen Request).
//! - Manueller Cold-Path **sell_all=true**: beobachtbarer Wire-Vertrag aligned mit
//!   execution-engine Cold-Path-Klassifikation: `metadata.sell_all`, `metadata.dex`,
//!   `resources.pools` Länge 1 mit Bonding-Curve-Pubkey (aus `PumpFunDex::derive_bonding_curve_static`
//!   zum `input_mint`), nicht der Literal-String `pumpfun` als Pool.
//! - Regulärer PumpFun-**Bonding-Curve**-Hot-Path: `PumpFunDex::quote_exact_in` bei Cache-Miss
//!   liefert `Ok(None)` ohne blockierende Control-Plane-Recovery (I-7, vgl. `PumpFunDex`-Docs zu
//!   `allow_rpc_fallback` auf dem Swap-Build-Pfad; hier: reines Quote-Cache-Miss-Verhalten).
//! - **Cold-Path-Recovery-Semantik (API-Grenze, vgl. I-24d/I-24e für PumpSwap):** Autoritativer
//!   Zustand gelangt über `PoolCacheUpdate` (dex=`pumpfun`) in den SLAVE-Cache (sichtbar für den
//!   Folgeversuch; bounded Retry auf EE-Ebene ist nicht als Zähler in der API abbildbar).
//!   `force_refresh_pumpfun` auf dem Control-Wire ist nicht cache-first. Hot-Path-
//!   `build_swap_ix_async_with_slippage` mit `allow_rpc_fallback=false` bleibt **ohne**
//!   Recovery-Warten blockierend begrenzt (kein Hot-Path-Recovery-Vertrag).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Impl unter `Iron_crab/src/`;
//! Assertions passen zur dokumentierten Semantik der öffentlichen Typen.
//!
//! Blackbox-Grenze: Kein Zähler für Ensure-Requests oder Engine-Retries im öffentlichen Schema;
//! JetStream-Publish und execution-engine-interne Retry-Zähler sind hier nicht separat beobachtbar.

mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use common::request_reply_e2e_harness::RequestReplyE2eHarness;
use futures::StreamExt;
use ironcrab::execution::live_pool_cache::{
    create_shared_cache, CachedPoolState, LivePoolCache, PumpFunState,
};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::ipc::{
    ControlRequest, ControlRequestKind, ControlResponse, ControlResponseStatus, ExplicitAmount,
    IntentOrigin, IntentTier, PoolCacheUpdate, PoolCacheUpdateType, RecordHeader, TradeIntent,
    TradeResources, TradeSide, TradingRegime,
};
use ironcrab::nats::topics::{TOPIC_CONTROL_REQUESTS, TOPIC_CONTROL_RESPONSES};
use ironcrab::solana::dex::pumpfun::PumpFunDex;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
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

/// Manueller Cold-Path sell_all=true: Wire-Shape entspricht der beobachtbaren Recovery-Vorbedingung
/// (dex + sell_all + genau ein Pool = Bonding-Curve-PDA zum Mint), abgeleitet per öffentlicher API.
#[test]
fn trade_intent_manual_sell_all_pumpfun_route_roundtrip() {
    // Gültiger Mint-Pubkey (SAMPLE_BASE_MINT ist absichtlich nur Wire-Placeholder im ControlRequest-Test).
    let token_mint_pk = Pubkey::new_unique();
    let token_mint = token_mint_pk.to_string();
    let (bonding_curve, _bump) = PumpFunDex::derive_bonding_curve_static(&token_mint_pk);
    let bonding_curve_str = bonding_curve.to_string();

    let resources = TradeResources {
        input_mint: token_mint.clone(),
        output_mint: WSOL_MINT.to_string(),
        pools: vec![bonding_curve_str.clone()],
        accounts: vec![],
        token_program: None,
    };

    let mut intent = TradeIntent::new(
        "ironcrab-eval",
        "wire-test",
        "run-sellall",
        "intent-sell-all-pf".to_string(),
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
        .insert("dex".to_string(), "pumpfun".to_string());

    let json = serde_json::to_string(&intent).expect("serialize TradeIntent");
    assert!(
        json.contains("\"sell_all\":\"true\""),
        "metadata muss sell_all=true für manuellen Sell-All-Cold-Path tragen: {json}"
    );
    assert!(
        json.contains("\"dex\":\"pumpfun\""),
        "metadata muss dex=pumpfun für PumpFun Cold-Path-Klassifikation tragen: {json}"
    );
    assert!(
        json.contains(&bonding_curve_str),
        "resources.pools[0] muss Bonding-Curve-PDA (base58) sein, abgeleitet vom input_mint: {json}"
    );

    let parsed: TradeIntent = serde_json::from_str(&json).expect("deserialize TradeIntent");
    assert_eq!(
        parsed.metadata.get("sell_all").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        parsed.metadata.get("dex").map(String::as_str),
        Some("pumpfun")
    );
    assert_eq!(parsed.resources.pools.len(), 1);
    let pool_pk = Pubkey::from_str(&parsed.resources.pools[0]).expect("pools[0] base58 pubkey");
    assert_eq!(
        pool_pk, bonding_curve,
        "Der einzelne Pool-Eintrag muss dem zum input_mint abgeleiteten Bonding-Curve-PDA entsprechen"
    );
}

/// Hot-Path (Bonding Curve, nicht PumpSwap AMM): Quote bei fehlendem Cache → kein Pflicht-RPC,
/// kein EnsurePumpfunBondingCurve (nur `PumpFunDex` + leeres Cache-Backing).
#[tokio::test]
async fn pumpfun_bonding_curve_hot_path_quote_cache_miss_no_control_plane_recovery() {
    let rpc = Arc::new(SolanaRpc::new(DUMMY_RPC));
    let mut dex = PumpFunDex::new(rpc, None).expect("PumpFunDex::new");
    let wallet = Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet");
    dex.set_user_authority(wallet);

    let unknown_token = Pubkey::new_unique();
    // SELL: Token → WSOL auf der Bonding Curve (Dex::quote_exact_in)
    let result = dex
        .quote_exact_in(&unknown_token.to_string(), WSOL_MINT, 1_000_000)
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// --- PumpFun Bonding-Curve Cold-Path-Recovery (API-Grenze, I-24d-Analog) ---
//
// Modelliert: market-data publiziert autoritativen Zustand als `PoolCacheUpdate`; der Connector
// konsumiert denselben Pfad wie JetStream → SLAVE (`apply_pool_cache_update`). Kein Claim auf
// vollständige EE-Orchestrierung oder Retry-Zähler.

fn pumpfun_pool_cache_update_for_recovery(
    bonding_curve: &Pubkey,
    token_mint: &Pubkey,
    creator: &Pubkey,
    cashback_enabled: bool,
) -> PoolCacheUpdate {
    let mut metadata = HashMap::new();
    metadata.insert("creator".to_string(), creator.to_string());
    metadata.insert(
        "cashback_enabled".to_string(),
        if cashback_enabled { "true" } else { "false" }.to_string(),
    );
    PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval-pf-bc-recovery"),
        dex: "pumpfun".to_string(),
        base_mint: token_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 793_100_000_000_000,
        quote_reserve: 30_000_000_000,
        pool_address: bonding_curve.to_string(),
        metadata: Some(metadata),
        geyser_slot: 100,
        liquidity_lamports: Some(30_000_000_000),
        update_type: PoolCacheUpdateType::PoolDiscovered,
    }
}

/// I-24d-Analog (PumpFun BC): Autoritativer `PoolCacheUpdate` (dex=pumpfun, JetStream-Pfad) macht
/// den **neuen** Bonding-Curve-State im SLAVE-Cache sichtbar — der Folgeversuch in der
/// execution-engine konsumiert denselben Eintrag (ein bounded Retry auf EE-Ebene; hier nur
/// Cache-Postcondition wie bei PumpSwap `i24d_after_authoritative_update_retry_can_proceed`).
///
/// Blackbox-Grenze: `PumpFunDex::build_swap_ix_async_with_slippage` kann im Cold Path zusaetzlich
/// eine RPC-Verifikation von `cashback_enabled` ausloesen; ohne laufenden RPC bleibt der Build auch
/// nach Refresh ggf. Err — das ist kein Widerspruch zum Refresh-Vertrag, solange der SLAVE-State
/// dem Publish entspricht.
#[tokio::test]
async fn pumpfun_bc_authoritative_pool_cache_update_makes_refreshed_state_visible_in_slave() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let creator = Pubkey::new_unique();

    // Stale SLAVE-Cache (vgl. A.41): cashback_enabled=false — Cold Path + RPC unreachable → Err.
    let cache = create_shared_cache();
    cache.upsert(
        bonding_curve,
        CachedPoolState::PumpFun(PumpFunState {
            token_mint,
            bonding_curve,
            associated_bonding_curve,
            virtual_sol_reserves: 100_000_000,
            virtual_token_reserves: 1_000_000_000,
            real_sol_reserves: 100_000_000,
            real_token_reserves: 1_000_000_000,
            complete: false,
            creator,
            cashback_enabled: false,
        }),
        100,
    );

    let mut dex = PumpFunDex::new(rpc, Some(cache.clone())).expect("PumpFunDex::new");
    let wallet = Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet");
    dex.set_user_authority(wallet);

    let first = dex
        .build_swap_ix_async_with_slippage(
            &token_mint.to_string(),
            WSOL_MINT,
            1_000_000,
            100,
            None,
            500,
            None,
            false,
            true,
        )
        .await;
    assert!(
        first.is_err(),
        "Vor autoritativem Refresh: Cold Path + stale cashback + RPC unreachable muss Err sein (kein blindes Ok)"
    );

    // Autoritativer Refresh (force-semantic auf Control-Plane; hier: eingehendes PoolCacheUpdate)
    let update =
        pumpfun_pool_cache_update_for_recovery(&bonding_curve, &token_mint, &creator, true);
    assert!(
        apply_pool_cache_update(&cache, &update),
        "PoolCacheUpdate muss SLAVE-Cache aktualisieren"
    );

    let state = cache
        .get(&bonding_curve)
        .expect("Bonding-Curve-Pool muss nach Update im SLAVE-Cache sein");
    match state {
        CachedPoolState::PumpFun(s) => {
            assert!(
                s.cashback_enabled,
                "Nach autoritativem PoolCacheUpdate muss SLAVE cashback_enabled dem Metadata entsprechen (Refresh sichtbar)"
            );
            assert_eq!(
                s.creator, creator,
                "creator im PumpFunState muss aus dem Refresh stammen"
            );
        }
        _ => panic!("erwartet PumpFun CachedPoolState"),
    }
}

/// Hot-Path-SELL (`allow_rpc_fallback=false`): Swap-Build bleibt blockierend begrenzt — kein
/// Recovery-Warten im Sinne eines Cold-Path-Retry (I-7; vgl. PumpFunDex-Dokumentation zu
/// `allow_rpc_fallback`).
#[tokio::test]
async fn pumpfun_bc_hot_path_sell_swap_build_bounded_no_recovery_wait() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let creator = Pubkey::new_unique();

    let cache = Arc::new(LivePoolCache::new());
    cache.upsert(
        bonding_curve,
        CachedPoolState::PumpFun(PumpFunState {
            token_mint,
            bonding_curve,
            associated_bonding_curve,
            virtual_sol_reserves: 100_000_000,
            virtual_token_reserves: 1_000_000_000,
            real_sol_reserves: 100_000_000,
            real_token_reserves: 1_000_000_000,
            complete: false,
            creator,
            cashback_enabled: true,
        }),
        100,
    );

    let mut dex = PumpFunDex::new(rpc, Some(cache)).expect("PumpFunDex::new");
    let wallet = Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet");
    dex.set_user_authority(wallet);

    let token_mint_str = token_mint.to_string();
    let fut = dex.build_swap_ix_async_with_slippage(
        &token_mint_str,
        WSOL_MINT,
        1_000_000,
        100,
        None,
        500,
        None,
        false,
        false,
    );

    let completed = tokio::time::timeout(Duration::from_secs(3), fut)
        .await
        .expect("Hot-Path PumpFun-SELL darf nicht auf Recovery/JetStream warten (muss innerhalb weniger Sekunden terminieren)");

    assert!(
        completed.is_ok(),
        "Hot-Path-Build muss ohne Panic terminieren: {completed:?}"
    );
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
