# Handoff: Pool Discovery & Bootstrap Tests (Test Authority)

**Plan-Referenz:** `docs/plans/plan_fix_pool_discovery_and_bootstrap.md`
**Invarianten:** A.37, A.38, A.39, A.40

## Aufgabe

Erstelle Tests fuer die neuen Invarianten aus dem Plan. Alle Tests in `Iron_crab-eval/tests/`.

---

## Test 1: A.37 — try_parse_pool_static kein getTokenAccountsByOwner (Unit/Eval)

**Datei:** `tests/invariants_pool_discovery.rs` (neu erstellen)

### Test: `test_parse_pool_static_derives_ata_without_rpc_index_calls`

Verifiziere per Source-Code-Analyse dass `try_parse_pool_static_from_market_account_inner` KEINE der folgenden Funktionen aufruft:
- `find_any_token_account_for_owner_and_mint`
- `find_token_account_by_owner_and_mint`
- `get_token_accounts_by_owner`

**Ansatz:** Source-Code des Impl-Repos lesen (`../Iron_crab/src/solana/dex/pumpfun_amm.rs`) und pruefen dass diese Aufrufe NICHT in der Funktion vorkommen. Alternativ: Grep-basierter Source-Scan-Test.

```rust
#[test]
fn test_a37_no_get_token_accounts_by_owner_in_parse_pool_static() {
    let src = std::fs::read_to_string("../Iron_crab/src/solana/dex/pumpfun_amm.rs")
        .expect("Cannot read pumpfun_amm.rs");
    
    // Find the function body
    let fn_start = src.find("async fn try_parse_pool_static_from_market_account_inner")
        .expect("Function not found");
    // Find the next top-level fn (approximation)
    let fn_body = &src[fn_start..];
    let fn_end = fn_body[100..].find("\n    async fn ")
        .or_else(|| fn_body[100..].find("\n    fn "))
        .unwrap_or(fn_body.len());
    let fn_text = &fn_body[..fn_end];
    
    assert!(
        !fn_text.contains("find_any_token_account_for_owner_and_mint"),
        "A.37 VIOLATED: try_parse_pool_static_from_market_account_inner still calls find_any_token_account_for_owner_and_mint"
    );
    assert!(
        !fn_text.contains("find_token_account_by_owner_and_mint"),
        "A.37 VIOLATED: try_parse_pool_static_from_market_account_inner still calls find_token_account_by_owner_and_mint"
    );
}
```

---

## Test 2: A.38 — JetStream pool_accounts Persistenz (Eval)

**Datei:** `tests/invariants_pool_cache_sync.rs` (erweitern)

### Test: `test_a38_trade_parsing_publishes_pool_cache_update_with_pool_accounts`

Verifiziere per Source-Code-Analyse dass nach `set_pump_amm_pool_accounts` in `market_data.rs` ein `PoolCacheUpdate` publiziert wird:

```rust
#[test]
fn test_a38_trade_parsing_publishes_pool_cache_update_with_pool_accounts() {
    let src = std::fs::read_to_string("../Iron_crab/src/bin/market_data.rs")
        .expect("Cannot read market_data.rs");
    
    // Find the trade-parsing section that calls set_pump_amm_pool_accounts
    let set_call = src.find("set_pump_amm_pool_accounts")
        .expect("set_pump_amm_pool_accounts not found");
    
    // Check that within ~100 lines after, a PoolCacheUpdate is published
    let after_set = &src[set_call..std::cmp::min(set_call + 3000, src.len())];
    
    assert!(
        after_set.contains("publish_pool_cache_update") || after_set.contains("PoolCacheUpdate"),
        "A.38 VIOLATED: After set_pump_amm_pool_accounts, no PoolCacheUpdate is published to JetStream"
    );
    assert!(
        after_set.contains("pool_accounts"),
        "A.38 VIOLATED: PoolCacheUpdate after set_pump_amm_pool_accounts does not include pool_accounts in metadata"
    );
}
```

---

## Test 3: A.39 — Liquidation-Quote-Timeout >= 30s (Eval)

**Datei:** `tests/invariants_liquidation_flow.rs` (erweitern)

### Test: `test_a39_liquidation_quote_timeout_minimum_30s`

```rust
#[test]
fn test_a39_liquidation_quote_timeout_minimum_30s() {
    let src = std::fs::read_to_string("../Iron_crab/src/bin/execution_engine.rs")
        .expect("Cannot read execution_engine.rs");
    
    // Find liquidation pump_amm quote timeout lines
    // Pattern: tokio::time::timeout(Duration::from_secs(N), pump_amm.quote_exact_in
    let re = regex::Regex::new(r"timeout\(Duration::from_secs\((\d+)\),\s*\n?\s*pump_amm\.quote_exact_in")
        .expect("Bad regex");
    
    let mut found = false;
    for cap in re.captures_iter(&src) {
        let secs: u64 = cap[1].parse().expect("Not a number");
        found = true;
        assert!(
            secs >= 30,
            "A.39 VIOLATED: Liquidation quote timeout is {}s, must be >= 30s (getProgramAccounts for PumpSwap takes ~26s)",
            secs
        );
    }
    assert!(found, "A.39: Could not find liquidation quote timeout pattern in execution_engine.rs");
}
```

---

## Test 4: A.40 — Startup Seeding (Eval)

**Datei:** `tests/invariants_pool_cache_sync.rs` (erweitern)

### Test: `test_a40_startup_seeds_pool_accounts_for_pump_amm`

```rust
#[test]
fn test_a40_startup_seeds_pool_accounts_for_pump_amm() {
    let src = std::fs::read_to_string("../Iron_crab/src/bin/execution_engine.rs")
        .expect("Cannot read execution_engine.rs");
    
    // After bootstrap_pool_cache_from_jetstream, there should be a seeding step
    let bootstrap_pos = src.find("bootstrap_pool_cache_from_jetstream")
        .expect("bootstrap_pool_cache_from_jetstream not found");
    let after_bootstrap = &src[bootstrap_pos..std::cmp::min(bootstrap_pos + 5000, src.len())];
    
    assert!(
        after_bootstrap.contains("pools_without_accounts") || after_bootstrap.contains("seed") || after_bootstrap.contains("pool_accounts"),
        "A.40 VIOLATED: No pool_accounts seeding step found after bootstrap_pool_cache_from_jetstream"
    );
}
```

---

## Test 5: LivePoolCache get_pump_amm_pools_without_accounts (Unit)

**Datei:** `tests/invariants_pool_cache_sync.rs` (erweitern)

### Test: `test_get_pump_amm_pools_without_accounts`

```rust
#[test]
fn test_get_pump_amm_pools_without_accounts() {
    // Verify that get_pump_amm_pools_without_accounts returns pools with empty pool_accounts
    // and does NOT return pools with populated pool_accounts
    let src = std::fs::read_to_string("../Iron_crab/src/execution/live_pool_cache.rs")
        .expect("Cannot read live_pool_cache.rs");
    
    assert!(
        src.contains("get_pump_amm_pools_without_accounts"),
        "A.40 VIOLATED: get_pump_amm_pools_without_accounts not found in LivePoolCache"
    );
}
```

---

## Hinweise

- `cargo test` im Iron_crab-eval Verzeichnis nach jeder Aenderung
- Regex-Crate muss in Cargo.toml vorhanden sein (fuer A.39 Test)
- Pfade zu `../Iron_crab/src/` relativ zum Iron_crab-eval Verzeichnis
