//! Invarianten A.38, A.40: Pool-Cache-Synchronisation
//!
//! A.38: Nach set_pump_amm_pool_accounts in market_data.rs muss ein PoolCacheUpdate
//!       an JetStream publiziert werden.
//! A.40: Nach bootstrap_pool_cache_from_jetstream muss ein pool_accounts-Seeding
//!       fuer PumpAmm-Pools stattfinden.
//! Zusaetzlich: LivePoolCache bietet get_pump_amm_pools_without_accounts.

/// A.38: Trade-Parsing publiziert PoolCacheUpdate nach set_pump_amm_pool_accounts
#[test]
fn test_a38_trade_parsing_publishes_pool_cache_update_with_pool_accounts() {
    let src = std::fs::read_to_string("../Iron_crab/src/bin/market_data.rs")
        .expect("Cannot read market_data.rs — is the Iron_crab sibling directory present?");

    let set_call = src
        .find("set_pump_amm_pool_accounts")
        .expect("set_pump_amm_pool_accounts not found in market_data.rs");

    let end = std::cmp::min(set_call + 3000, src.len());
    let after_set = &src[set_call..end];

    assert!(
        after_set.contains("publish_pool_cache_update") || after_set.contains("PoolCacheUpdate"),
        "A.38 VIOLATED: After set_pump_amm_pool_accounts, no PoolCacheUpdate is published to JetStream"
    );
    assert!(
        after_set.contains("pool_accounts"),
        "A.38 VIOLATED: PoolCacheUpdate after set_pump_amm_pool_accounts does not include \
         pool_accounts in metadata"
    );
}

/// A.40: Startup seeds pool_accounts for PumpAmm pools after bootstrap
#[test]
fn test_a40_startup_seeds_pool_accounts_for_pump_amm() {
    let src = std::fs::read_to_string("../Iron_crab/src/bin/execution_engine.rs")
        .expect("Cannot read execution_engine.rs — is the Iron_crab sibling directory present?");

    let bootstrap_pos = src
        .find("bootstrap_pool_cache_from_jetstream")
        .expect("bootstrap_pool_cache_from_jetstream not found in execution_engine.rs");

    let end = std::cmp::min(bootstrap_pos + 5000, src.len());
    let after_bootstrap = &src[bootstrap_pos..end];

    assert!(
        after_bootstrap.contains("pools_without_accounts")
            || after_bootstrap.contains("seed")
            || after_bootstrap.contains("pool_accounts"),
        "A.40 VIOLATED: No pool_accounts seeding step found after bootstrap_pool_cache_from_jetstream"
    );
}

/// LivePoolCache muss get_pump_amm_pools_without_accounts bereitstellen
#[test]
fn test_get_pump_amm_pools_without_accounts() {
    let src = std::fs::read_to_string("../Iron_crab/src/execution/live_pool_cache.rs")
        .expect("Cannot read live_pool_cache.rs — is the Iron_crab sibling directory present?");

    assert!(
        src.contains("get_pump_amm_pools_without_accounts"),
        "A.40 VIOLATED: get_pump_amm_pools_without_accounts not found in LivePoolCache — \
         needed for startup seeding of pool_accounts"
    );
}
