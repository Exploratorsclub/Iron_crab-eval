//! Invariante A.32: Cold Path pump_amm degenerate Reserves RPC-Fallback
//!
//! Verifiziert, dass PumpAmm mit degenerate Cache-Reserves (eine Seite=0)
//! korrekt als Fehler behandelt wird, und dass valide Reserves funktionieren.
//! Zusaetzlich: BalanceUpdated Merge-Verhalten bei partial reserves.
//!
//! Invariante I-24d: Cold-Path Discovery nur per Request/Reply ueber market-data.
//! pool_accounts duerfen nur via autoritativem PoolCacheUpdate in den SLAVE Cache;
//! keine lokale Engine-Write als Truth-Quelle. Recovery via autoritativen Update-Pfad.

use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::execution::quote_calculator::quote_output_amount;
use ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType, RecordHeader};
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

/// A.32a: degenerate reserves (quote=0) muessen als Fehler behandelt werden.
#[test]
fn pumpamm_degenerate_cache_reserves_quote_zero_rejected() {
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();

    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(691_000_000_000_000),
        quote_reserve: Some(0),
        pool_accounts: vec![],
        creator: None,
    });

    let result = quote_output_amount(&state, 1_000_000_000, &base_mint);
    assert!(
        result.is_err(),
        "A.32: degenerate reserves (quote=0) must fail with missing reserves error"
    );
    assert!(
        result.unwrap_err().to_string().contains("missing reserves"),
        "Error must indicate missing reserves"
    );
}

/// A.32b: degenerate reserves (base=0) muessen als Fehler behandelt werden.
#[test]
fn pumpamm_degenerate_cache_reserves_base_zero_rejected() {
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();

    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(0),
        quote_reserve: Some(22_000_000_000),
        pool_accounts: vec![],
        creator: None,
    });

    let result = quote_output_amount(&state, 1_000_000_000, &base_mint);
    assert!(
        result.is_err(),
        "A.32: degenerate reserves (base=0) must fail with missing reserves error"
    );
}

/// A.32c: valide reserves (beide > 0) muessen ein Ok mit positivem Wert liefern.
#[test]
fn pumpamm_valid_reserves_quote_succeeds() {
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();

    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(691_000_000_000_000),
        quote_reserve: Some(25_000_000_000),
        pool_accounts: vec![],
        creator: None,
    });

    let result = quote_output_amount(&state, 1_000_000_000, &base_mint);
    assert!(
        result.is_ok(),
        "A.32: valid reserves must produce a successful quote"
    );
    assert!(
        result.unwrap() > 0,
        "Quote output must be positive with valid reserves"
    );
}

/// A.32d: BalanceUpdated Merge mit partial reserves.
/// PoolDiscovered mit (0,0) gefolgt von BalanceUpdated mit (base=691T, quote=0)
/// muss (691T, 0) im Cache ergeben.
#[test]
fn balance_updated_partial_base_only_preserves_value() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_address = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    let discovered = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: "So11111111111111111111111111111111111111112".to_string(),
        base_reserve: 0,
        quote_reserve: 0,
        pool_address: pool_address.to_string(),
        metadata: Some({
            let mut m = HashMap::new();
            m.insert("creator".to_string(), Pubkey::new_unique().to_string());
            m
        }),
        geyser_slot: 100,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &discovered);

    let balance_update = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: "So11111111111111111111111111111111111111112".to_string(),
        base_reserve: 691_000_000_000_000,
        quote_reserve: 0,
        pool_address: pool_address.to_string(),
        metadata: None,
        geyser_slot: 101,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::BalanceUpdated,
    };
    apply_pool_cache_update(&cache, &balance_update);

    let pool_pubkey = Pubkey::from_str(&pool_address.to_string()).unwrap();
    let state = cache.get(&pool_pubkey).expect("pool should be cached");
    match state {
        CachedPoolState::PumpAmm(s) => {
            assert_eq!(
                s.base_reserve,
                Some(691_000_000_000_000),
                "base_reserve should be updated from BalanceUpdated"
            );
            assert_eq!(
                s.quote_reserve,
                Some(0),
                "quote_reserve should remain 0 (no update yet)"
            );
        }
        _ => panic!("Expected PumpAmm state"),
    }
}

// --- I-24d: Cold-Path Discovery nur per Request/Reply ueber market-data ---

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn wsol_mint() -> Pubkey {
    Pubkey::from_str(WSOL_MINT).unwrap()
}

/// I-24d: Pool mit leeren pool_accounts liefert None – kein lokaler Engine-Write als Ersatz.
/// Bei fehlenden pool_accounts muss ein klarer Failure-Outcome entstehen, keine stille Heilung.
#[tokio::test]
async fn i24d_missing_pool_accounts_yields_none_no_local_truth() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: wsol_mint(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(1_000_000_000),
            quote_reserve: Some(100_000_000),
            pool_accounts: vec![],
            creator: None,
        }),
        100,
    );

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache.clone(), false);

    let result = dex.pool_accounts_v1_for_base_mint(base_mint).await;

    assert!(
        result.is_ok(),
        "pool_accounts_v1_for_base_mint sollte Ok liefern"
    );
    assert!(
        result.unwrap().is_none(),
        "I-24d: Fehlende pool_accounts muessen None liefern, kein lokaler Write als Truth"
    );
}

/// I-24d: Recovery via autoritativem PoolCacheUpdate – market-data schreibt pool_accounts.
/// Nach apply_pool_cache_update mit pool_accounts in metadata hat der Cache sie.
#[test]
fn i24d_recovery_via_authoritative_pool_cache_update() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_address = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    let pool_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let accounts_str: Vec<String> = pool_accounts.iter().map(|p| p.to_string()).collect();

    let update = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 1_000_000_000,
        quote_reserve: 100_000_000,
        pool_address: pool_address.to_string(),
        metadata: Some({
            let mut m = HashMap::new();
            m.insert("pool_accounts".to_string(), accounts_str.join(","));
            m
        }),
        geyser_slot: 100,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &update);

    let result = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "I-24d: Nach autoritativem PoolCacheUpdate muessen pool_accounts im Cache sein"
    );
    let accounts = result.unwrap();
    assert_eq!(accounts.len(), 14, "PumpAmm braucht 14 pool_accounts");
    assert_eq!(accounts, pool_accounts);
}

/// I-24d: Nach autoritativem Update kann Retry erfolgreich fortfahren (build_swap_ix).
#[tokio::test]
async fn i24d_after_authoritative_update_retry_succeeds() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let wsol = wsol_mint();

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
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);
    let accounts_opt = dex.pool_accounts_v1_for_base_mint(base_mint).await.unwrap();

    assert!(
        accounts_opt.is_some(),
        "I-24d: Nach autoritativem Update muss pool_accounts_v1_for_base_mint Some liefern"
    );
    let accounts = accounts_opt.unwrap();
    assert_eq!(accounts.len(), 14);

    let user = Pubkey::new_unique();
    let build_result = PumpFunAmmDex::build_swap_ix_from_pool_accounts(
        WSOL_MINT,
        &base_mint.to_string(),
        1_000_000,
        100,
        user,
        &accounts,
        None,
    );
    assert!(
        build_result.is_ok(),
        "I-24d: Nach autoritativem Update muss build_swap_ix erfolgreich sein"
    );
    let ix = build_result.unwrap();
    assert!(!ix.is_empty(), "Swap-Instructions duerfen nicht leer sein");
}
