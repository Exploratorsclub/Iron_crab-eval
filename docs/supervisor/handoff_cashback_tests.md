# HANDOFF: Tests fuer cashback_enabled JetStream-Propagierung (A.30, A.31)

## PFLICHT: Lese vor jeder Aenderung:
- docs/spec/INVARIANTS.md (insbesondere A.30 und A.31)
- AGENTS.md (STOP-CHECK Rules)

## Kontext
PumpFun SELL scheitert mit Custom(6024) weil `cashback_enabled` im SLAVE LivePoolCache immer `false` ist. 

Der Fix im Impl-Repo:
1. `market_data.rs`: propagiert `cashback_enabled` in JetStream metadata
2. `pool_cache_sync.rs`: liest `cashback_enabled` aus metadata statt `false` zu hardcoden
3. `pumpfun.rs`: Cold Path verifiziert `cashback_enabled` IMMER per RPC, auch bei Cache-HIT

## Neue Tests

### Test A.30a: tests/invariants_pumpfun_cashback.rs

Teste dass `build_minimal_pool_state()` (oder `apply_pool_cache_update`) `cashback_enabled=true` liefert wenn die PoolCacheUpdate metadata den Wert "true" enthaelt.

Verwende:
- `ironcrab::execution::pool_cache_sync::build_minimal_pool_state` (falls public)
- Oder: `ironcrab::execution::pool_cache_sync::apply_pool_cache_update` mit einem `LivePoolCache`
- `ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType}`
- `ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpFunState}`

**WICHTIG**: Pruefe zuerst ob `build_minimal_pool_state()` public ist. Falls nicht, nutze `apply_pool_cache_update` mit einem `LivePoolCache`:

```rust
use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType};

#[test]
fn jetstream_metadata_propagates_cashback_enabled_true() {
    let cache = LivePoolCache::new();
    
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("creator".to_string(), "CwXwWhLZHrdS8CiuNHd928HHRvp9U9wT1xAbDvFz6Ujd".to_string());
    metadata.insert("cashback_enabled".to_string(), "true".to_string());
    // ... weitere required fields ...
    
    let update = PoolCacheUpdate { /* ... */ };
    
    let modified = apply_pool_cache_update(&cache, &update);
    assert!(modified);
    
    // Pruefe den Cache-Eintrag
    let pool_addr = solana_sdk::pubkey::Pubkey::from_str(&update.pool_address).unwrap();
    let state = cache.get(&pool_addr).expect("should be cached");
    match state {
        CachedPoolState::PumpFun(s) => {
            assert!(s.cashback_enabled, "A.30: cashback_enabled must be true from JetStream metadata");
        }
        _ => panic!("Expected PumpFun state"),
    }
}
```

### Test A.30b: tests/invariants_pumpfun_cashback.rs

Gleicher Aufbau wie A.30a, aber OHNE `cashback_enabled` in metadata:

```rust
#[test]
fn jetstream_metadata_without_cashback_defaults_to_false() {
    // Gleicher Aufbau, aber metadata ohne "cashback_enabled" Key
    // assert: PumpFunState.cashback_enabled == false (backward compat)
}
```

## STOP-CHECK Hinweise
- **Check 1 (Scope)**: Nur Iron_crab-eval Dateien aendern
- **Check 3 (Repo-Isolation)**: NICHT in Iron_crab/src/ lesen. Nur public API via `use ironcrab::...`
- **Check 4 (Blackbox)**: Tests testen die oeffentliche API
- **Check 5 (Assertions)**: Keine Widersprueche

## Erlaubte Dateien
- `tests/invariants_pumpfun_cashback.rs`

## Nach den Aenderungen
```bash
cargo fmt -p ironcrab-eval -- --check
cargo clippy -p ironcrab-eval --all-targets -- -D warnings
cargo test --verbose
```
