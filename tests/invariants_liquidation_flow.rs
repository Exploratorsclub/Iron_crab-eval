//! Invariante: Liquidation 6005-Retry Komponenten (INVARIANTS.md A.13)
//!
//! Verifiziert mark_pumpfun_complete_for_mint und find_pump_amm_pool_by_base_mint.
//! Der vollständige run_liquidation_job-Flow (6005 → Retry mit pump_amm) wird durch
//! golden_replay_liquidation_6005_retry getestet.

use ironcrab::execution::live_pool_cache::{
    create_shared_cache, CachedPoolState, PumpAmmState, PumpFunState, SharedLivePoolCache,
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn wsol_mint() -> Pubkey {
    Pubkey::from_str(WSOL_MINT).unwrap()
}

/// Cache mit PumpFun-State (complete=false) für mark_pumpfun_complete-Tests
fn setup_cache_with_pumpfun() -> (SharedLivePoolCache, Pubkey, Pubkey) {
    let cache = create_shared_cache();
    let token_mint = Pubkey::new_from_array([2u8; 32]);
    let bonding_curve = Pubkey::new_from_array([3u8; 32]);
    let associated_bonding_curve = Pubkey::new_from_array([4u8; 32]);
    let creator = Pubkey::new_from_array([5u8; 32]);

    let state = CachedPoolState::PumpFun(PumpFunState {
        token_mint,
        bonding_curve,
        associated_bonding_curve,
        virtual_sol_reserves: 100_000_000,
        virtual_token_reserves: 1_000_000_000,
        real_sol_reserves: 100_000_000,
        real_token_reserves: 1_000_000_000,
        complete: false,
        creator,
    });

    cache.upsert(bonding_curve, state, 0);
    (cache, bonding_curve, token_mint)
}

/// Cache mit PumpAmm-State für find_pump_amm_pool_by_base_mint-Tests
fn setup_cache_with_pump_amm() -> (SharedLivePoolCache, Pubkey, Pubkey) {
    let cache = create_shared_cache();
    let pool_market = Pubkey::new_from_array([1u8; 32]);
    let base_mint = Pubkey::new_from_array([2u8; 32]);
    let quote_mint = wsol_mint();

    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_from_array([3u8; 32]),
        pool_quote_token_account: Pubkey::new_from_array([4u8; 32]),
        base_reserve: Some(1_000_000_000),
        quote_reserve: Some(100_000_000),
        pool_accounts: (0..14).map(|_| Pubkey::new_unique()).collect(),
        creator: None,
    });

    cache.upsert(pool_market, state, 0);
    (cache, pool_market, base_mint)
}

#[test]
fn mark_pumpfun_complete_sets_is_complete_true() {
    let (cache, _bonding_curve, token_mint) = setup_cache_with_pumpfun();
    assert_eq!(cache.is_pumpfun_complete_for_mint(&token_mint), Some(false));

    cache.mark_pumpfun_complete_for_mint(&token_mint);
    assert_eq!(
        cache.is_pumpfun_complete_for_mint(&token_mint),
        Some(true),
        "Nach mark_pumpfun_complete_for_mint muss is_pumpfun_complete_for_mint Some(true) liefern"
    );
}

#[test]
fn mark_pumpfun_complete_returns_true_when_found() {
    let (cache, _bonding_curve, token_mint) = setup_cache_with_pumpfun();
    let result = cache.mark_pumpfun_complete_for_mint(&token_mint);
    assert!(
        result,
        "mark_pumpfun_complete_for_mint muss true zurückgeben wenn Mint gefunden"
    );
}

#[test]
fn mark_pumpfun_complete_returns_false_for_unknown_mint() {
    let cache: SharedLivePoolCache = create_shared_cache();
    let unknown_mint = Pubkey::new_unique();
    let result = cache.mark_pumpfun_complete_for_mint(&unknown_mint);
    assert!(
        !result,
        "mark_pumpfun_complete_for_mint muss false zurückgeben bei unbekanntem Mint"
    );
}

#[test]
fn find_pump_amm_pool_returns_pool_when_cached() {
    let (cache, pool_market, base_mint) = setup_cache_with_pump_amm();
    let result = cache.find_pump_amm_pool_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "find_pump_amm_pool_by_base_mint muss Some liefern bei Cache-Hit"
    );
    assert_eq!(result.unwrap(), pool_market);
}

#[test]
fn get_pump_amm_pool_accounts_by_base_mint_returns_accounts() {
    let (cache, _pool_market, base_mint) = setup_cache_with_pump_amm();
    let result = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "get_pump_amm_pool_accounts_by_base_mint muss Some liefern bei Cache-Hit"
    );
    let accounts = result.unwrap();
    assert_eq!(accounts.len(), 14);
}
