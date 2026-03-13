# Handoff: Tests fuer Token-2022 PumpSwap Fix (A.33, A.34, A.35)

## Kontext

3 neue Invarianten fuer Bug #28, #29, #30. Tests in bestehende oder neue Test-Dateien einfuegen.

## Invariante A.33: PoolDiscovered darf pool_accounts nicht ueberschreiben

**Datei:** `tests/invariants_pumpswap_amm_liquidation.rs` (erweitern)

**Test A.33a: PoolDiscovered mit leeren pool_accounts überschreibt nicht**
```rust
#[test]
fn test_a33a_pool_discovered_preserves_existing_pool_accounts() {
    let cache = LivePoolCache::new();
    let pool = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();

    // Step 1: Insert pool with empty pool_accounts
    let initial_state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(1000),
        quote_reserve: Some(2000),
        pool_accounts: vec![],
        creator: None,
    });
    cache.upsert(pool, initial_state, 100);

    // Step 2: Set pool_accounts (simulates DexPoolAccounts event)
    let fake_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    cache.set_pump_amm_pool_accounts(&pool, fake_accounts.clone());

    // Verify pool_accounts are set
    assert_eq!(cache.get_pump_amm_pool_accounts(&pool).unwrap().len(), 14);

    // Step 3: Apply PoolDiscovered event WITHOUT pool_accounts
    let update = PoolCacheUpdate {
        pool_address: pool.to_string(),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: quote_mint.to_string(),
        base_reserve: 1100,
        quote_reserve: 2100,
        update_type: PoolCacheUpdateType::PoolDiscovered,
        geyser_slot: 200,
        metadata: None, // No pool_accounts in metadata
    };
    apply_pool_cache_update(&cache, &update);

    // A.33: pool_accounts MUST still be present
    let accounts = cache.get_pump_amm_pool_accounts(&pool).unwrap();
    assert_eq!(accounts.len(), 14);
    assert_eq!(accounts, fake_accounts);
}
```

**Test A.33b: PoolDiscovered mit eigenen pool_accounts überschreibt**
```rust
#[test]
fn test_a33b_pool_discovered_with_new_pool_accounts_updates() {
    let cache = LivePoolCache::new();
    let pool = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();

    // Setup: pool with existing pool_accounts
    let initial_state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(1000),
        quote_reserve: Some(2000),
        pool_accounts: vec![],
        creator: None,
    });
    cache.upsert(pool, initial_state, 100);
    let old_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    cache.set_pump_amm_pool_accounts(&pool, old_accounts.clone());

    // Apply PoolDiscovered WITH new pool_accounts
    let new_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let accounts_str: Vec<String> = new_accounts.iter().map(|p| p.to_string()).collect();
    let mut metadata = HashMap::new();
    metadata.insert("pool_accounts".to_string(), accounts_str.join(","));

    let update = PoolCacheUpdate {
        pool_address: pool.to_string(),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: quote_mint.to_string(),
        base_reserve: 1100,
        quote_reserve: 2100,
        update_type: PoolCacheUpdateType::PoolDiscovered,
        geyser_slot: 200,
        metadata: Some(metadata),
    };
    apply_pool_cache_update(&cache, &update);

    // New pool_accounts should replace old ones
    let accounts = cache.get_pump_amm_pool_accounts(&pool).unwrap();
    assert_eq!(accounts.len(), 14);
    assert_eq!(accounts, new_accounts);
}
```

**Test A.33c: PoolDiscovered preserviert auch creator**
```rust
#[test]
fn test_a33c_pool_discovered_preserves_creator() {
    let cache = LivePoolCache::new();
    let pool = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let creator = Pubkey::new_unique();

    let initial_state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(1000),
        quote_reserve: Some(2000),
        pool_accounts: vec![],
        creator: Some(creator),
    });
    cache.upsert(pool, initial_state, 100);

    // PoolDiscovered without creator
    let update = PoolCacheUpdate {
        pool_address: pool.to_string(),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: quote_mint.to_string(),
        base_reserve: 1100,
        quote_reserve: 2100,
        update_type: PoolCacheUpdateType::PoolDiscovered,
        geyser_slot: 200,
        metadata: None,
    };
    apply_pool_cache_update(&cache, &update);

    // Creator must be preserved
    if let Some(entry) = cache.get(&pool) {
        if let CachedPoolState::PumpAmm(ref s) = entry {
            assert_eq!(s.creator, Some(creator));
        } else { panic!("not PumpAmm"); }
    } else { panic!("pool missing"); }
}
```

## Invariante A.34: build_swap_ix Token-2022

**Datei:** `tests/invariants_pumpswap_amm_liquidation.rs` (erweitern)

**Test A.34a: build_swap_ix SELL mit Token-2022 base_token_program**
```rust
#[test]
fn test_a34a_build_swap_ix_sell_token2022_base_program() {
    // Setup PumpFunAmmDex with cached Token-2022 program for base mint
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new(rpc, true, true);
    let base_mint = Pubkey::new_unique();
    let token_2022 = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap();

    // Cache the token program
    dex.cache_extra_data(
        &format!("token_program:{}", base_mint),
        &token_2022.to_string(),
    );

    // Insert a discovered pool
    let pool_static = PumpAmmPoolStatic { /* ... construct with base_mint ... */ };
    dex.pools_by_base.insert(base_mint, pool_static.clone());
    dex.pools_by_market.insert(pool_static.pool_market, base_mint);

    // Build SELL instruction
    let ixs = dex.build_swap_ix(
        &base_mint.to_string(),  // input = token (SELL)
        WSOL_MINT,               // output = WSOL
        1_000_000,
        1_000,
    ).unwrap();

    // Account 11 MUST be Token-2022, NOT spl_token
    let ix = &ixs[0];
    assert_eq!(ix.accounts[11].pubkey, token_2022, "account 11 must be Token-2022 for base token program");
    // Account 12 must be spl_token (WSOL always SPL)
    let spl = Pubkey::new_from_array(spl_token::id().to_bytes());
    assert_eq!(ix.accounts[12].pubkey, spl, "account 12 must be SPL Token for quote (WSOL)");
}
```

**Test A.34b: build_swap_ix SELL mit default SPL base_token_program**
```rust
#[test]
fn test_a34b_build_swap_ix_sell_default_spl_program() {
    // Same as above but WITHOUT caching Token-2022
    // → both accounts 11 and 12 should be spl_token
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new(rpc, true, true);
    let base_mint = Pubkey::new_unique();

    // No token_program cached → defaults to spl_token

    let pool_static = PumpAmmPoolStatic { /* ... construct with base_mint ... */ };
    dex.pools_by_base.insert(base_mint, pool_static.clone());
    dex.pools_by_market.insert(pool_static.pool_market, base_mint);

    let ixs = dex.build_swap_ix(
        &base_mint.to_string(),
        WSOL_MINT,
        1_000_000,
        1_000,
    ).unwrap();

    let spl = Pubkey::new_from_array(spl_token::id().to_bytes());
    assert_eq!(ix.accounts[11].pubkey, spl);
    assert_eq!(ix.accounts[12].pubkey, spl);
}
```

## Invariante A.35: Liquidation Retry Scan Token-2022

Die A.35 Tests können als Integration/End-to-End Test implementiert werden, da sie RPC-Calls benötigen. Vorläufig reicht eine manuelle Verifikation anhand des Codes: die Retry-Scan-Schleife muss `token_2022_program_id` abfragen.

## Erlaubte Dateien

- `tests/invariants_pumpswap_amm_liquidation.rs` (erweitern)
- Ggf. `tests/invariants_liquidation_flow.rs` (erweitern fuer A.35)

## Hinweise

- Imports: `use ironcrab::execution::live_pool_cache::*;`, `use ironcrab::execution::pool_cache_sync::*;`
- Die Test-Snippets oben sind Pseudocode — passe Struct-Felder, Imports und Hilfsfunktionen an die tatsaechliche Codebase an.
- Pruefe `PoolCacheUpdate` Struct-Definition fuer die korrekten Felder.
- `PumpFunAmmDex` hat private Felder — ggf. muss der Test ueber die oeffentliche API gehen oder Test-Helpers genutzt werden.
