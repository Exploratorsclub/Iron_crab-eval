//! Gemeinsame Blackbox-Assertions fuer I-24e / PumpSwap `force_refresh` (Cold vs Hot Path).
//! Zentralisiert Setup und API-Aufrufe; Assertion-Texte liegen in den aufrufenden Tests.

use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn wsol_mint() -> Pubkey {
    Pubkey::from_str(WSOL_MINT).unwrap()
}

/// Mit `force_refresh=true` duerfen die 14 pool_accounts aus dem LivePoolCache nicht
/// unveraendert als Truth zurueckkommen (kein stilles Cache-first).
pub async fn assert_force_refresh_skips_stale_livepool_cache_pool_accounts(
    stale_assert_prefix: &'static str,
) {
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
        "{}{:?}",
        stale_assert_prefix,
        result
    );
}

/// Hot-Path-Dex (`allow_rpc_on_miss=false`): bei `force_refresh` weder Cache noch RPC im
/// selben synchronen Aufruf.
pub async fn assert_force_refresh_refuses_without_cold_path_rpc_permission(
    hot_path_assert_msg: &'static str,
) {
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
    assert!(result.unwrap().is_none(), "{hot_path_assert_msg}");
}
