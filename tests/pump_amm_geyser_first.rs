//! Blackbox: LivePoolCache / Quote-Calculator für PumpAmm (Geyser-First)
//!
//! Verifiziert: Cache-Hit liefert Quote und pool_accounts ohne RPC.
//! Sowie DEX-Connector-Ebene (PumpFunAmmDex) mit Cache – INVARIANTS.md §1.4

use ironcrab::execution::live_pool_cache::{
    create_shared_cache, CachedPoolState, LivePoolCache, PumpAmmState, SharedLivePoolCache,
};
use ironcrab::execution::quote_calculator::quote_output_amount;
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

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
        base_reserve: Some(1_000_000_000), // 1B tokens
        quote_reserve: Some(100_000_000),  // 0.1 SOL (lamports)
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
    assert!(
        amount_out.is_ok(),
        "quote_output_amount sollte Some liefern"
    );
    let out = amount_out.unwrap();
    assert!(out > 0, "amount_out sollte positiv sein");
}

#[test]
fn pool_accounts_from_cache() {
    let (cache, _pool_market, base_mint) = setup_cache_with_pump_amm();
    let result = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "pool_accounts sollte bei Cache-Hit geliefert werden"
    );
    let accounts = result.unwrap();
    assert!(
        accounts.len() >= 12,
        "PumpAmm braucht mindestens 12 pool_accounts"
    );
}

#[test]
fn cache_miss_returns_none() {
    let cache: SharedLivePoolCache = create_shared_cache();
    let unknown_mint = Pubkey::new_from_array([99u8; 32]);
    let result = cache.get_pump_amm_reserves_by_base_mint(&unknown_mint);
    assert!(result.is_none(), "Cache-Miss sollte None liefern");
}

// --- DEX-Ebene (PumpFunAmmDex) ---

fn make_pump_amm_cache_with_reserves(
    pool_market: Pubkey,
    base_mint: Pubkey,
    base_reserve: u64,
    quote_reserve: u64,
) -> Arc<LivePoolCache> {
    let cache = LivePoolCache::new();
    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: Pubkey::from_str(SOL_MINT).unwrap(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(base_reserve),
            quote_reserve: Some(quote_reserve),
            pool_accounts: vec![],
            creator: None,
        }),
        100,
    );
    Arc::new(cache)
}

fn make_pump_amm_cache_with_pool_accounts(
    pool_market: Pubkey,
    base_mint: Pubkey,
    pool_accounts: Vec<Pubkey>,
) -> Arc<LivePoolCache> {
    let cache = LivePoolCache::new();
    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: Pubkey::from_str(SOL_MINT).unwrap(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(1),
            quote_reserve: Some(1),
            pool_accounts,
            creator: None,
        }),
        100,
    );
    Arc::new(cache)
}

#[tokio::test]
async fn dex_quote_from_cache_no_rpc() {
    let base_mint = Pubkey::new_unique();
    let pool_market = Pubkey::new_unique();
    let cache = make_pump_amm_cache_with_reserves(
        pool_market,
        base_mint,
        1_000_000_000_000,
        50_000_000_000,
    );
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache);

    let base_mint_str = base_mint.to_string();
    let result = dex
        .quote_exact_in(SOL_MINT, &base_mint_str, 1_000_000_000)
        .await;

    let quote = result.expect("quote should succeed");
    assert!(quote.is_some(), "expected Some(Quote) on cache hit");
    let quote = quote.unwrap();
    assert!(quote.amount_out > 0);
    assert!(
        quote.price_impact_bps < 10_000,
        "price_impact_bps should be plausible"
    );
    assert!(quote.route.contains(&pool_market.to_string()));
    assert_eq!(quote.fee_bps, 125);
}

#[tokio::test]
async fn dex_pool_accounts_from_cache_no_rpc() {
    let wsol = Pubkey::from_str(SOL_MINT).unwrap();
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

    let cache =
        make_pump_amm_cache_with_pool_accounts(pool_market, base_mint, pool_accounts.clone());
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache);

    let result = dex.pool_accounts_v1_for_base_mint(base_mint).await;

    assert!(result.is_ok());
    let accounts = result.unwrap();
    assert!(accounts.is_some());
    let accounts = accounts.unwrap();
    assert_eq!(accounts.len(), 14);
    assert_eq!(accounts, pool_accounts);
}
