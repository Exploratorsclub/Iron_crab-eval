//! Blackbox: LivePoolCache / Quote-Calculator für PumpAmm (Geyser-First)
//!
//! Verifiziert: Cache-Hit liefert Quote und pool_accounts ohne RPC.

use ironcrab::execution::live_pool_cache::{
    create_shared_cache, CachedPoolState, PumpAmmState, SharedLivePoolCache,
};
use ironcrab::execution::quote_calculator::quote_output_amount;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn sol_mint() -> Pubkey {
    Pubkey::from_str(SOL_MINT).unwrap()
}

/// Cache mit PumpAmm-State befüllen
fn setup_cache_with_pump_amm() -> (SharedLivePoolCache, Pubkey, Pubkey) {
    let cache = create_shared_cache();
    let pool_market = Pubkey::new_from_array([1u8; 32]);
    let base_mint = Pubkey::new_from_array([2u8; 32]);
    let quote_mint = sol_mint();

    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_from_array([3u8; 32]),
        pool_quote_token_account: Pubkey::new_from_array([4u8; 32]),
        base_reserve: Some(1_000_000_000),  // 1B tokens
        quote_reserve: Some(100_000_000),   // 0.1 SOL (lamports)
        pool_accounts: (0..14).map(|_| Pubkey::new_unique()).collect(),
        creator: None,
    });

    cache.upsert(pool_market, state, 12345);
    (cache, pool_market, base_mint)
}

#[test]
fn quote_from_cache_no_rpc() {
    let (cache, pool_market, _base_mint) = setup_cache_with_pump_amm();
    let state = cache.get(&pool_market).expect("pool in cache");
    let amount_in = 10_000_000; // 0.01 SOL

    let amount_out = quote_output_amount(&state, amount_in, &sol_mint());
    assert!(amount_out.is_ok(), "quote_output_amount sollte Some liefern");
    let out = amount_out.unwrap();
    assert!(out > 0, "amount_out sollte positiv sein");
}

#[test]
fn pool_accounts_from_cache() {
    let (cache, _pool_market, base_mint) = setup_cache_with_pump_amm();
    let result = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(result.is_some(), "pool_accounts sollte bei Cache-Hit geliefert werden");
    let accounts = result.unwrap();
    assert!(accounts.len() >= 12, "PumpAmm braucht mindestens 12 pool_accounts");
}

#[test]
fn cache_miss_returns_none() {
    let cache: SharedLivePoolCache = create_shared_cache();
    let unknown_mint = Pubkey::new_from_array([99u8; 32]);
    let result = cache.get_pump_amm_reserves_by_base_mint(&unknown_mint);
    assert!(result.is_none(), "Cache-Miss sollte None liefern");
}
