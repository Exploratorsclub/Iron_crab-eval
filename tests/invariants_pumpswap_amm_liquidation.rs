//! PumpSwap AMM Cold Path State-Integritaet (A.32, A.33, I-24d)
//!
//! A.32: Degenerate Cache-Reserves duerfen nicht als gueltiger Truth dominieren.
//! Null-/Partial-Reserves muessen als Fehler erkannt werden; Cold Path mit
//! degenerate Cache und RPC unreachable liefert Err, nicht Ok(None).
//!
//! A.33: PoolDiscovered ohne pool_accounts darf vorhandene pool_accounts nicht
//! ueberschreiben. Kein Downgrade von gutem State auf leeren State (Bug #28).
//!
//! I-24d: Cold-Path Discovery nur per Request/Reply ueber market-data.
//! pool_accounts duerfen nur via autoritativem PoolCacheUpdate in den SLAVE Cache;
//! keine lokale Engine-Write als Truth-Quelle. Recovery via autoritativen Update-Pfad.

use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::execution::pool_cache_sync::apply_pool_cache_update;
use ironcrab::execution::quote_calculator::quote_output_amount;
use ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType, RecordHeader};
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn wsol_mint() -> Pubkey {
    Pubkey::from_str(WSOL_MINT).unwrap()
}

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

/// A.32e: Cold Path (allow_rpc=true) mit degenerate Cache-Reserves und RPC unreachable
/// muss Err liefern, NICHT Ok(None). Degenerierter State darf nicht still als
/// "kein Quote" durchgehen – entweder RPC-Fallback oder klarer Failure.
#[tokio::test]
async fn pumpamm_cold_path_degenerate_reserves_yields_err_not_ok_none() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: wsol_mint(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(691_000_000_000_000),
            quote_reserve: Some(0),
            pool_accounts: vec![],
            creator: None,
        }),
        100,
    );

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, true);

    let result = dex
        .quote_exact_in(WSOL_MINT, &base_mint.to_string(), 1_000_000)
        .await;

    assert!(
        result.is_err(),
        "A.32: Cold Path mit degenerate Reserves und RPC unreachable muss Err liefern, nicht Ok(None)"
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

// --- I-24d: Cold-Path Discovery nur per Request/Reply ueber market-data ---
//
// Verhaltensorientierte Tests am beobachtbaren Vertrag:
// a) fehlende pool_accounts fuehren nicht zu lokaler Engine-Truth-Heilung (inkl. Cache-Postcondition)
// b) autoritativer PoolCacheUpdate macht den Zustand verfuegbar
// c) nach autoritativem Update kann der naechste Versuch sinnvoll fortfahren
// d) not_found (Cache-Miss ohne RPC) und externer Fehler (RPC unreachable) getrennt abgesichert

/// I-24d (a): Pool mit leeren pool_accounts – Cold Path liefert Failure, keine lokale Heilung.
/// Beobachtbarer Vertrag: Rueckgabewert kein Erfolg; Cache-Postcondition: SLAVE Cache unveraendert.
#[tokio::test]
async fn i24d_missing_pool_accounts_no_local_healing() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: wsol_mint(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(1_000_000_000),
            quote_reserve: Some(100_000_000),
            pool_accounts: vec![],
            creator: None,
        }),
        100,
    );

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache.clone(), true);

    let result = dex.pool_accounts_v1_for_base_mint(base_mint).await;

    let got_data = result
        .as_ref()
        .ok()
        .and_then(|o| o.as_ref())
        .map(|v| !v.is_empty());
    assert!(
        !got_data.unwrap_or(false),
        "I-24d: Fehlende pool_accounts duerfen nicht zu Some mit Daten fuehren (keine lokale Heilung)"
    );

    let after = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        after.is_none_or(|v| v.is_empty()),
        "I-24d: Cache-Postcondition – SLAVE Cache darf nicht lokal mit pool_accounts befuellt worden sein"
    );
}

// --- A.33: PoolDiscovered darf pool_accounts nicht ueberschreiben ---
//
// Wenn brauchbare pool_accounts bereits vorhanden sind, duerfen spaetere schwaechere
// Events ohne Accounts diese nicht loeschen. Bug #28: PoolDiscovered upsert loeschte
// pool_accounts, Liquidation scheiterte mit err_discovery.

/// A.33: PoolDiscovered ohne pool_accounts ueberschreibt vorhandene pool_accounts nicht.
#[test]
fn a33_pool_discovered_without_accounts_preserves_existing() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_address = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    let pool_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let accounts_str: Vec<String> = pool_accounts.iter().map(|p| p.to_string()).collect();

    let update_with_accounts = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 1_000_000_000,
        quote_reserve: 100_000_000,
        pool_address: pool_address.to_string(),
        metadata: Some({
            let mut m = HashMap::new();
            m.insert("pool_accounts".to_string(), accounts_str.join(","));
            m
        }),
        geyser_slot: 100,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &update_with_accounts);

    let before = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        before.as_ref().is_some_and(|v| v.len() == 14),
        "A.33: Nach erstem Update muessen pool_accounts vorhanden sein"
    );

    let update_without_accounts = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 1_100_000_000,
        quote_reserve: 110_000_000,
        pool_address: pool_address.to_string(),
        metadata: None,
        geyser_slot: 200,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &update_without_accounts);

    let after = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        after.is_some(),
        "A.33: pool_accounts duerfen durch schwaecheres PoolDiscovered ohne Accounts nicht geloescht werden"
    );
    let accounts = after.unwrap();
    assert_eq!(accounts.len(), 14);
    assert_eq!(accounts, pool_accounts);
}

/// I-24d (b): Autoritativer PoolCacheUpdate macht pool_accounts verfuegbar.
/// Beobachtbarer Vertrag: apply_pool_cache_update (von market-data) liefert den Zustand.
#[test]
fn i24d_authoritative_update_makes_state_available() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_address = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    let pool_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let accounts_str: Vec<String> = pool_accounts.iter().map(|p| p.to_string()).collect();

    let update = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 1_000_000_000,
        quote_reserve: 100_000_000,
        pool_address: pool_address.to_string(),
        metadata: Some({
            let mut m = HashMap::new();
            m.insert("pool_accounts".to_string(), accounts_str.join(","));
            m
        }),
        geyser_slot: 100,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &update);

    let result = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "I-24d: Autoritativer PoolCacheUpdate muss pool_accounts verfuegbar machen"
    );
    let accounts = result.unwrap();
    assert_eq!(accounts.len(), 14);
    assert_eq!(accounts, pool_accounts);
}

/// I-24d (c): Nach autoritativem Update kann der naechste Versuch fortfahren.
/// Beobachtbarer Vertrag: DEX liefert pool_accounts nach autoritativem Update.
#[tokio::test]
async fn i24d_after_authoritative_update_retry_can_proceed() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    let pool_accounts: Vec<Pubkey> = (0..14).map(|_| Pubkey::new_unique()).collect();
    let update = PoolCacheUpdate {
        header: RecordHeader::new("market-data", "v0.1", "run-eval"),
        dex: "pump_amm".to_string(),
        base_mint: base_mint.to_string(),
        quote_mint: WSOL_MINT.to_string(),
        base_reserve: 1_000_000_000,
        quote_reserve: 100_000_000,
        pool_address: pool_market.to_string(),
        metadata: Some({
            let mut m = HashMap::new();
            m.insert(
                "pool_accounts".to_string(),
                pool_accounts
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            m
        }),
        geyser_slot: 100,
        liquidity_lamports: None,
        update_type: PoolCacheUpdateType::PoolDiscovered,
    };
    apply_pool_cache_update(&cache, &update);

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, true);

    let result = dex.pool_accounts_v1_for_base_mint(base_mint).await;

    assert!(
        result.is_ok(),
        "I-24d: Nach autoritativem Update muss pool_accounts abrufbar sein"
    );
    let accounts = result.unwrap();
    assert!(
        accounts.as_ref().is_some_and(|v| v.len() == 14),
        "I-24d: PumpAmm erwartet 14 pool_accounts; Retry kann fortfahren wenn verfuegbar"
    );
}

/// I-24d (d) not_found: Unbekannter base_mint, leerer Cache, kein RPC.
/// Sauber getrennt von external_failure: allow_rpc_on_miss=false, nur Cache-Lookup.
#[tokio::test]
async fn i24d_not_found_clear_failure() {
    let cache = Arc::new(LivePoolCache::new());
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);

    let unknown_mint = Pubkey::new_unique();
    let result = dex.pool_accounts_v1_for_base_mint(unknown_mint).await;

    assert!(
        result.is_ok(),
        "not_found-Pfad: API liefert Ok (kein Panic)"
    );
    assert!(
        result.unwrap().is_none(),
        "I-24d: not_found (Cache-Miss) muss Ok(None) liefern"
    );
}

/// I-24d (d) external failure: Pool mit leeren pool_accounts, Cold Path, RPC unreachable.
/// Modelliert Timeout/Unavailable – klarer Fehlervertrag (Err). Getrennt von not_found (Ok(None)).
#[tokio::test]
async fn i24d_external_failure_clear_failure() {
    let cache = Arc::new(LivePoolCache::new());
    let pool_market = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();

    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: wsol_mint(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(1_000_000_000),
            quote_reserve: Some(100_000_000),
            pool_accounts: vec![],
            creator: None,
        }),
        100,
    );

    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, true);

    let result = dex.pool_accounts_v1_for_base_mint(base_mint).await;

    assert!(
        result.is_err(),
        "I-24d: externer Fehler (RPC unreachable) muss Err liefern, nicht Ok(None)"
    );
}
