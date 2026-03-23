//! PumpSwap Recovery-Semantik nach strukturellem Cache-/Account-Mismatch (Eval-Vertrag)
//!
//! Blackbox an der `ironcrab`-Public-API (`PumpFunAmmDex`, `LivePoolCache`, `PoolCacheUpdate`,
//! `ControlRequest`). Keine Annahmen ueber interne Sleeplogik oder Momentum-Bot-Implementierung.
//!
//! **Abgedeckte Invarianten (beobachtbar):**
//!
//! 1. **Cold Path / force refresh:** `pool_accounts_v1_for_base_mint_with_hint(..., force_refresh=true)`
//!    darf bei nachgewiesen veraltetem LivePoolCache-14er **nicht** denselben Vec stumm als
//!    `Ok(Some(stale))` zurueckgeben (kein cache-first Recovery-Ersatz).
//! 2. **Hot Path / nicht blockierend auf Recovery:** Dieselbe API mit `allow_rpc_on_miss=false`
//!    (Hot-Path-DEX) und `force_refresh=true` liefert **weder** den gecachten 14er **noch** RPC —
//!    beobachtbar als `Ok(None)` in einem synchronen Aufruf (keine Pflicht zum Warten auf
//!    market-data im selben Aufruf).
//! 3. **Autoritativer Zustand:** Nur nach `apply_pool_cache_update` (market-data → PoolCacheUpdate)
//!    kann ein folgender `pool_accounts_v1_for_base_mint`-Versuch die Accounts liefern — der
//!    Refresh-Pfad laeuft ueber publizierten Cache-State, nicht ueber lokales „Heilen“ im
//!    fehlgeschlagenen Intent.
//!
//! **Eng verwandte, bestehende Tests (nicht dupliziert hier):** Wire-Format `force_refresh` auf
//! `ControlRequest` — `tests/ipc_schema_serde.rs`; E2E Request/Reply — `tests/request_reply_e2e_contract.rs`.
//!
//! **Blackbox-Grenze:** Dedupe/Cooldown nach erfolgreichem NATS-Publish und async Hot-Path-Refresh
//! ohne Retry im selben Intent sind **nicht** ueber eine stabile Public-API ohne Observability
//! vertraglich festziehbar; die Grundinvariante (Hot Path blockiert nicht auf Recovery-Warten)
//! wird hier ueber das DEX-Verhalten mit `force_refresh` + `allow_rpc_on_miss=false` abgesichert.

use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::ipc::{
    ControlRequest, ControlRequestKind, PoolCacheUpdate, PoolCacheUpdateType, RecordHeader,
};
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn wsol_mint() -> Pubkey {
    Pubkey::from_str(WSOL_MINT).unwrap()
}

/// Cold-Path-Recovery: `force_refresh=true` ist semantisch **kein** stilles Zurueckgeben des
/// gecachten 14er pool_accounts-Vektors bei RPC-unreachable (hier: kein `Ok(Some)` == stale Vec).
#[tokio::test]
async fn contract_cold_path_force_refresh_not_cache_first_stale_14er() {
    let wsol = wsol_mint();
    let base_mint = Pubkey::new_unique();
    let pool_market = Pubkey::new_unique();
    let pool_accounts: Vec<Pubkey> = vec![
        pool_market,
        Pubkey::new_unique(),
        base_mint,
        wsol,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ];
    assert_eq!(pool_accounts.len(), 14);

    let cache = Arc::new(LivePoolCache::new());
    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: wsol,
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(1_000_000_000),
            quote_reserve: Some(100_000_000),
            pool_accounts: pool_accounts.clone(),
            creator: None,
        }),
        100,
    );

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, true);

    let result = dex
        .pool_accounts_v1_for_base_mint_with_hint(base_mint, None, true)
        .await;

    assert!(
        !matches!(result.as_ref(), Ok(Some(a)) if a == &pool_accounts),
        "force_refresh must not return stale LivePoolCache 14er unchanged as Ok(Some): {:?}",
        result
    );
}

/// Hot-Path: `force_refresh` mit `allow_rpc_on_miss=false` — kein Cache, kein RPC im selben
/// synchronen Aufruf; damit keine blockierende Recovery-Semantik an der DEX-Grenze.
#[tokio::test]
async fn contract_hot_path_force_refresh_non_blocking_no_cache_no_rpc() {
    let wsol = wsol_mint();
    let base_mint = Pubkey::new_unique();
    let pool_market = Pubkey::new_unique();
    let pool_accounts: Vec<Pubkey> = vec![
        pool_market,
        Pubkey::new_unique(),
        base_mint,
        wsol,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ];

    let cache = Arc::new(LivePoolCache::new());
    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: wsol,
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(1_000_000_000),
            quote_reserve: Some(100_000_000),
            pool_accounts: pool_accounts.clone(),
            creator: None,
        }),
        100,
    );

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);

    let result = dex
        .pool_accounts_v1_for_base_mint_with_hint(base_mint, Some(pool_market), true)
        .await;

    assert!(result.is_ok());
    assert!(
        result.unwrap().is_none(),
        "Hot-Path dex + force_refresh must not return cached accounts or use RPC (Ok(None))"
    );
}

/// Autoritativer `PoolCacheUpdate` stellt State bereit; der **folgende** Abruf kann denselben
/// Weg wie ein regulärer Cold-Path-Read nutzen (ohne force_refresh).
#[tokio::test]
async fn contract_pool_cache_update_then_subsequent_read_can_succeed() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    let pool_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let update = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 1_000_000_000,
        quote_reserve: 100_000_000,
        pool_address: pool_market.to_string(),
        metadata: Some({
            let mut m = HashMap::new();
            m.insert(
                "pool_accounts".to_string(),
                pool_accounts
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            m
        }),
        geyser_slot: 100,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &update);

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, true);

    let result = dex.pool_accounts_v1_for_base_mint(base_mint).await;

    assert!(
        result.is_ok(),
        "after PoolCacheUpdate, pool_accounts must be readable"
    );
    let accounts = result.unwrap();
    assert!(
        accounts.as_ref().is_some_and(|v| v.len() == 14),
        "subsequent attempt after authoritative update should see 14 pool_accounts"
    );
}

/// Wire: Cold-Path-Recovery-Requests muessen `force_refresh` tragen koennen (market-data
/// kann cache-first unterscheiden). Ergaenzt den DEX-Vertrag ohne Metrik-Abhaengigkeit.
#[test]
fn contract_control_request_force_refresh_wire_semantic() {
    let mut req = ControlRequest::new(
        "ironcrab-eval",
        "recovery-contract",
        "run-1",
        "req-semantic-recovery".to_string(),
        "market-data",
        ControlRequestKind::EnsurePumpAmmPoolAccounts {
            base_mint: "BaseMint11111111111111111111111111111111".to_string(),
        },
    );
    req.force_refresh = true;

    let json = serde_json::to_string(&req).unwrap();
    assert!(
        json.contains("\"force_refresh\":true"),
        "EnsurePumpAmmPoolAccounts recovery must serialize force_refresh=true: {json}"
    );

    let parsed: ControlRequest = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.force_refresh,
        "roundtrip must preserve force_refresh"
    );
}
