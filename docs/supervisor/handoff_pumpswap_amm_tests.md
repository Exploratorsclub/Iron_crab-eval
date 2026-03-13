# Handoff: Eval Tests fuer A.32 (PumpSwap AMM Degenerate Cache Reserves)

## Kontext

Invariante A.32: Im Cold Path (allow_rpc_on_miss=true) muss `pump_amm` `quote_exact_in()` bei
degenerate Cache-Reserves (eine Seite=0, amount_out=0) zum RPC-Fallback durchfallen statt `None`
zurueckzugeben. Der Hot Path (allow_rpc_on_miss=false) darf weiterhin `None` zurueckgeben.

Siehe `docs/spec/INVARIANTS.md` Invariante A.32 fuer die vollstaendige Definition.

## Zu erstellende Test-Datei

`tests/invariants_pumpswap_amm_liquidation.rs`

## Test A.32a: Cache mit degenerate Reserves (quote=0)

Verifiziert, dass ein PumpAmmState mit `base_reserve=Some(691_000_000_000)` und `quote_reserve=Some(0)`
im QuoteCalculator korrekt als Fehler behandelt wird ("missing reserves").

```rust
#[test]
fn pumpamm_degenerate_cache_reserves_quote_zero_rejected() {
    // Simuliert: Cache hat Reserves aber quote_reserve=0 (nach Restart, nur base vault update)
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();
    
    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(691_000_000_000_000), // tokens present
        quote_reserve: Some(0),                   // SOL missing — degenerate!
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
```

## Test A.32b: Cache mit degenerate Reserves (base=0)

```rust
#[test]
fn pumpamm_degenerate_cache_reserves_base_zero_rejected() {
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();
    
    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(0),                     // tokens missing — degenerate!
        quote_reserve: Some(22_000_000_000),        // SOL present
        pool_accounts: vec![],
        creator: None,
    });
    
    // Selling tokens into pool (input_mint = base_mint → SELL)
    let result = quote_output_amount(&state, 1_000_000_000, &base_mint);
    assert!(
        result.is_err(),
        "A.32: degenerate reserves (base=0) must fail with missing reserves error"
    );
}
```

## Test A.32c: Valide Reserves funktionieren

```rust
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
```

## Test A.32d: PoolCacheUpdate BalanceUpdated Merge mit partial reserves

Verifiziert, dass wenn ein BalanceUpdated-Event mit `base_reserve > 0, quote_reserve = 0` ankommt
und es keinen vorherigen Eintrag gibt, das Ergebnis korrekt `(base, 0)` im Cache ist (nicht `(0, 0)`).

```rust
#[test]
fn balance_updated_partial_base_only_preserves_value() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_address = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    
    // First: PoolDiscovered with (0, 0) — simulates cold start
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
    
    // Then: BalanceUpdated with base only (quote still 0)
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
            assert_eq!(s.base_reserve, Some(691_000_000_000_000),
                "base_reserve should be updated from BalanceUpdated");
            assert_eq!(s.quote_reserve, Some(0),
                "quote_reserve should remain 0 (no update yet)");
        }
        _ => panic!("Expected PumpAmm state"),
    }
}
```

## Imports

```rust
use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::execution::quote_calculator::quote_output_amount;
use ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType, RecordHeader};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
```

## Nach allen Aenderungen

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --quiet
```
