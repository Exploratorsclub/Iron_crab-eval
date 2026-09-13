//! Invariante A.52 / I-MD-5 / I-7 / A.48: Ungepinntes Pool-Adressbuch vs MASTER-Pin-Promote.
//!
//! - Ungepinnt: TX-Layout nur im [`PoolAddressBook`] (In-Memory), kein Quote-SSOT.
//! - Pin: Layout nach MASTER (layout-only), Buch-Zeile weg; Account-Parse fuellt nur Luecken, Reserves bleiben.
//! - I-MD-5: Kein neuer Test, der unpinned TX-Subscribe verlangt (bleibt in `invariants_md_explicit_track_requests_only.rs`).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Impl-Aenderung; oeffentliche `ironcrab`-API;
//! Source-Grep nur fuer I-7 auf dokumentiertes Sibling-`pool_address_book.rs`.

use ironcrab::execution::live_pool_cache::{
    merge_account_parse_preserves_existing, CachedPoolState, LivePoolCache, MeteoraState,
    OrcaWhirlpoolState, PumpAmmState, RaydiumCpmmState,
};
use ironcrab::execution::pool_address_book::{
    PoolAddressBook, PoolLayoutKeys, DEFAULT_CAP, DEFAULT_TTL_MS,
};
use solana_sdk::pubkey::Pubkey;
use std::fs;
use std::path::PathBuf;

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn iron_crab_src(rel: &str) -> PathBuf {
    iron_crab_root().join("src").join(rel)
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let book = iron_crab_src("execution/pool_address_book.rs");
    if !book.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder pool_address_book.rs nicht lesbar unter {:?}",
            iron_crab_root()
        );
        return None;
    }
    Some(iron_crab_root())
}

fn sample_pump_layout_keys() -> PoolLayoutKeys {
    PoolLayoutKeys::PumpAmm {
        base_mint: Pubkey::new_unique(),
        quote_mint: Pubkey::new_unique(),
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        pool_accounts: (0..14).map(|_| Pubkey::new_unique()).collect(),
    }
}

/// Compile-/Match-Check: [`PoolLayoutKeys`] traegt keine Reserve-Felder (nur Adress-/Layout-Keys).
fn assert_pool_layout_keys_are_address_only(keys: PoolLayoutKeys) {
    match keys {
        PoolLayoutKeys::PumpAmm {
            base_mint: _,
            quote_mint: _,
            pool_base_token_account: _,
            pool_quote_token_account: _,
            pool_accounts: _,
        }
        | PoolLayoutKeys::Orca {
            token_mint_a: _,
            token_mint_b: _,
            token_vault_a: _,
            token_vault_b: _,
        }
        | PoolLayoutKeys::RaydiumCpmm {
            token_0_mint: _,
            token_1_mint: _,
            token_0_vault: _,
            token_1_vault: _,
        }
        | PoolLayoutKeys::Meteora {
            token_x_mint: _,
            token_y_mint: _,
            reserve_x: _,
            reserve_y: _,
        }
        | PoolLayoutKeys::RaydiumAmm {
            base_mint: _,
            quote_mint: _,
            coin_vault: _,
            pc_vault: _,
        } => {}
    }
}

#[test]
fn pool_layout_keys_variants_have_no_reserve_fields_in_type() {
    assert_pool_layout_keys_are_address_only(sample_pump_layout_keys());
}

#[test]
fn address_book_merge_contains_take_insert_from_demote_lifecycle() {
    let mut book = PoolAddressBook::new();
    let pool = Pubkey::new_unique();
    let keys = sample_pump_layout_keys();

    book.merge(pool, keys.clone(), 1_000);
    assert!(book.contains(&pool));

    let taken = book.take(pool).expect("take after pin promote");
    assert!(!book.contains(&pool));
    assert_eq!(taken, keys);

    book.insert_from_demote(pool, taken, 2_000);
    assert!(book.contains(&pool));
}

#[test]
fn address_book_evict_stale_respects_ttl() {
    let mut book = PoolAddressBook::with_ttl_and_cap(1_000, 32);
    let pool = Pubkey::new_unique();
    book.merge(pool, sample_pump_layout_keys(), 0);
    assert_eq!(book.len(), 1);

    let evicted = book.evict_stale(2_500);
    assert_eq!(evicted, 1);
    assert!(!book.contains(&pool));
}

#[test]
fn address_book_cap_lru_drops_oldest_when_over_cap() {
    let mut book = PoolAddressBook::with_ttl_and_cap(120_000, 2);
    let p0 = Pubkey::new_unique();
    let p1 = Pubkey::new_unique();
    let p2 = Pubkey::new_unique();

    book.merge(p0, sample_pump_layout_keys(), 1);
    book.merge(p1, sample_pump_layout_keys(), 2);
    assert_eq!(book.len(), 2);

    book.merge(p2, sample_pump_layout_keys(), 3);
    assert_eq!(book.len(), 2);
    assert!(
        !book.contains(&p0),
        "LRU: aeltester Eintrag muss bei Cap=2 weg"
    );
    assert!(book.contains(&p1));
    assert!(book.contains(&p2));
}

#[test]
fn default_ttl_and_cap_match_spec() {
    assert_eq!(DEFAULT_TTL_MS, 120_000);
    assert_eq!(DEFAULT_CAP, 32_768);
}

#[test]
fn layout_only_pump_amm_has_no_reserves() {
    let state = sample_pump_layout_keys().into_layout_only_cached_state();
    match state {
        CachedPoolState::PumpAmm(s) => {
            assert!(s.base_reserve.is_none());
            assert!(s.quote_reserve.is_none());
        }
        other => panic!("expected PumpAmm layout-only, got {other:?}"),
    }
}

#[test]
fn layout_only_orca_no_vault_balances_and_quote_not_seeded() {
    let keys = PoolLayoutKeys::Orca {
        token_mint_a: Pubkey::new_unique(),
        token_mint_b: Pubkey::new_unique(),
        token_vault_a: Pubkey::new_unique(),
        token_vault_b: Pubkey::new_unique(),
    };
    match keys.into_layout_only_cached_state() {
        CachedPoolState::Orca(OrcaWhirlpoolState {
            vault_a_balance,
            vault_b_balance,
            whirlpool_quote_account_seeded,
            ..
        }) => {
            assert!(vault_a_balance.is_none());
            assert!(vault_b_balance.is_none());
            assert!(!whirlpool_quote_account_seeded);
        }
        other => panic!("expected Orca layout-only, got {other:?}"),
    }
}

#[test]
fn layout_only_raydium_cpmm_has_no_reserves() {
    let keys = PoolLayoutKeys::RaydiumCpmm {
        token_0_mint: Pubkey::new_unique(),
        token_1_mint: Pubkey::new_unique(),
        token_0_vault: Pubkey::new_unique(),
        token_1_vault: Pubkey::new_unique(),
    };
    match keys.into_layout_only_cached_state() {
        CachedPoolState::RaydiumCpmm(RaydiumCpmmState {
            reserve_0,
            reserve_1,
            ..
        }) => {
            assert!(reserve_0.is_none());
            assert!(reserve_1.is_none());
        }
        other => panic!("expected RaydiumCpmm layout-only, got {other:?}"),
    }
}

#[test]
fn layout_only_meteora_dlmm_no_balances_and_bin_params_not_seeded() {
    let keys = PoolLayoutKeys::Meteora {
        token_x_mint: Pubkey::new_unique(),
        token_y_mint: Pubkey::new_unique(),
        reserve_x: Pubkey::new_unique(),
        reserve_y: Pubkey::new_unique(),
    };
    match keys.into_layout_only_cached_state() {
        CachedPoolState::Meteora(MeteoraState {
            reserve_x_balance,
            reserve_y_balance,
            dlmm_bin_params_account_seeded,
            ..
        }) => {
            assert!(reserve_x_balance.is_none());
            assert!(reserve_y_balance.is_none());
            assert!(!dlmm_bin_params_account_seeded);
        }
        other => panic!("expected Meteora layout-only, got {other:?}"),
    }
}

fn pump_amm_master_row_with_accounts_and_reserves(pool: Pubkey) -> (Pubkey, PumpAmmState) {
    let accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let state = PumpAmmState {
        base_mint: Pubkey::new_unique(),
        quote_mint: Pubkey::new_unique(),
        pool_base_token_account: Pubkey::new_unique(),
        pool_quote_token_account: Pubkey::new_unique(),
        base_reserve: Some(1_111),
        quote_reserve: Some(2_222),
        pool_accounts: accounts.clone(),
        creator: Some(Pubkey::new_unique()),
    };
    (pool, state)
}

fn pump_amm_incoming_account_parse_empty_layout() -> PumpAmmState {
    PumpAmmState {
        base_mint: Pubkey::default(),
        quote_mint: Pubkey::default(),
        pool_base_token_account: Pubkey::default(),
        pool_quote_token_account: Pubkey::default(),
        base_reserve: None,
        quote_reserve: None,
        pool_accounts: vec![],
        creator: None,
    }
}

#[test]
fn merge_account_parse_preserves_pump_amm_accounts_and_reserves_after_pin() {
    let (pool, existing) = pump_amm_master_row_with_accounts_and_reserves(Pubkey::new_unique());
    let incoming = pump_amm_incoming_account_parse_empty_layout();

    let merged = merge_account_parse_preserves_existing(
        &CachedPoolState::PumpAmm(existing.clone()),
        CachedPoolState::PumpAmm(incoming),
        500,
    );

    match merged {
        CachedPoolState::PumpAmm(s) => {
            assert_eq!(s.pool_accounts.len(), 14);
            assert_eq!(s.pool_accounts, existing.pool_accounts);
            assert_eq!(s.base_reserve, Some(1_111));
            assert_eq!(s.quote_reserve, Some(2_222));
            assert_eq!(s.pool_base_token_account, existing.pool_base_token_account);
            assert_eq!(
                s.pool_quote_token_account,
                existing.pool_quote_token_account
            );
        }
        other => panic!("expected PumpAmm, got {other:?}"),
    }
    let _ = pool;
}

#[test]
fn live_pool_cache_upsert_account_parse_preserves_pump_amm_after_pin_promote() {
    let cache = LivePoolCache::new();
    let pool = Pubkey::new_unique();
    let (_, existing) = pump_amm_master_row_with_accounts_and_reserves(pool);

    cache.upsert(pool, CachedPoolState::PumpAmm(existing.clone()), 100);

    let incoming = pump_amm_incoming_account_parse_empty_layout();
    assert!(cache.upsert(pool, CachedPoolState::PumpAmm(incoming), 200));

    let got = cache.get(&pool).expect("MASTER row after account tick");
    match got {
        CachedPoolState::PumpAmm(s) => {
            assert_eq!(s.pool_accounts.len(), 14);
            assert_eq!(s.base_reserve, Some(1_111));
            assert_eq!(s.quote_reserve, Some(2_222));
        }
        other => panic!("expected PumpAmm, got {other:?}"),
    }
}

#[test]
fn live_pool_cache_remove_clears_master_row() {
    let cache = LivePoolCache::new();
    let pool = Pubkey::new_unique();
    let (_, state) = pump_amm_master_row_with_accounts_and_reserves(pool);
    cache.upsert(pool, CachedPoolState::PumpAmm(state), 1);
    assert!(cache.get(&pool).is_some());

    assert!(cache.remove(&pool));
    assert!(cache.get(&pool).is_none());
}

#[test]
fn pool_address_book_module_has_no_rpc_markers() {
    let Some(_root) = skip_if_no_sibling_iron_crab() else {
        return;
    };
    let source = fs::read_to_string(iron_crab_src("execution/pool_address_book.rs"))
        .expect("pool_address_book.rs lesbar");
    let forbidden = [
        ".rpc.",
        "get_account(",
        "getMultipleAccounts",
        "get_multiple_accounts",
    ];
    for needle in forbidden {
        assert!(
            !source.contains(needle),
            "pool_address_book.rs darf kein Hot-Path-RPC `{needle}` enthalten (I-7 / A.52)"
        );
    }
}
