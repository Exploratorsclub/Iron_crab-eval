//! Invariante A.32: Cold Path pump_amm degenerate Reserves RPC-Fallback
//!
//! Verifiziert, dass PumpAmm mit degenerate Cache-Reserves (eine Seite=0)
//! korrekt als Fehler behandelt wird, und dass valide Reserves funktionieren.
//! Zusaetzlich: BalanceUpdated Merge-Verhalten bei partial reserves.

use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::execution::quote_calculator::quote_output_amount;
use ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType, RecordHeader};
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
