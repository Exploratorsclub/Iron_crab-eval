//! Invariante: Liquidation 6005-Retry Komponenten (INVARIANTS.md A.13, A.29)
//!
//! Verifiziert mark_pumpfun_complete_for_mint und find_pump_amm_pool_by_base_mint.
//! Der vollständige run_liquidation_job-Flow (6005 → Retry mit pump_amm) wird durch
//! golden_replay_liquidation_6005_retry getestet.
//!
//! A.29: Liquidation Vollständigkeit – build_sell_ix Account-Layout, count mit Locks.
//!
//! PumpFun Bonding Curve Cold-Path (A.31, A.39): Stale Cache darf nicht blind dominieren.
//! cashback_enabled muss im Cold Path per RPC verifiziert werden; falsches Layout
//! (15 Accounts bei cashback=true) führt zu Custom(6024) Overflow (Bug #25).

use ironcrab::execution::live_pool_cache::{
    create_shared_cache, CachedPoolState, PumpAmmState, PumpFunState, SharedLivePoolCache,
};
use ironcrab::solana::dex::pumpfun::PumpFunDex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

fn wsol_mint() -> Pubkey {
    Pubkey::from_str(WSOL_MINT).unwrap()
}

fn setup_pumpfun_dex() -> PumpFunDex {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:8899"));
    let mut dex = PumpFunDex::new(rpc, None).expect("PumpFunDex::new");
    let wallet = Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet");
    dex.set_user_authority(wallet);
    dex
}

/// Cache mit PumpFun-State (complete=false) für mark_pumpfun_complete-Tests
fn setup_cache_with_pumpfun() -> (SharedLivePoolCache, Pubkey, Pubkey) {
    let cache = create_shared_cache();
    let token_mint = Pubkey::new_from_array([2u8; 32]);
    let bonding_curve = Pubkey::new_from_array([3u8; 32]);
    let associated_bonding_curve = Pubkey::new_from_array([4u8; 32]);
    let creator = Pubkey::new_from_array([5u8; 32]);

    let state = CachedPoolState::PumpFun(PumpFunState {
        token_mint,
        bonding_curve,
        associated_bonding_curve,
        virtual_sol_reserves: 100_000_000,
        virtual_token_reserves: 1_000_000_000,
        real_sol_reserves: 100_000_000,
        real_token_reserves: 1_000_000_000,
        complete: false,
        creator,
        cashback_enabled: false,
    });

    cache.upsert(bonding_curve, state, 0);
    (cache, bonding_curve, token_mint)
}

/// Cache mit PumpAmm-State für find_pump_amm_pool_by_base_mint-Tests
fn setup_cache_with_pump_amm() -> (SharedLivePoolCache, Pubkey, Pubkey) {
    let cache = create_shared_cache();
    let pool_market = Pubkey::new_from_array([1u8; 32]);
    let base_mint = Pubkey::new_from_array([2u8; 32]);
    let quote_mint = wsol_mint();

    let state = CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_from_array([3u8; 32]),
        pool_quote_token_account: Pubkey::new_from_array([4u8; 32]),
        base_reserve: Some(1_000_000_000),
        quote_reserve: Some(100_000_000),
        pool_accounts: (0..14).map(|_| Pubkey::new_unique()).collect(),
        creator: None,
    });

    cache.upsert(pool_market, state, 0);
    (cache, pool_market, base_mint)
}

#[test]
fn mark_pumpfun_complete_sets_is_complete_true() {
    let (cache, _bonding_curve, token_mint) = setup_cache_with_pumpfun();
    assert_eq!(cache.is_pumpfun_complete_for_mint(&token_mint), Some(false));

    cache.mark_pumpfun_complete_for_mint(&token_mint);
    assert_eq!(
        cache.is_pumpfun_complete_for_mint(&token_mint),
        Some(true),
        "Nach mark_pumpfun_complete_for_mint muss is_pumpfun_complete_for_mint Some(true) liefern"
    );
}

#[test]
fn mark_pumpfun_complete_returns_true_when_found() {
    let (cache, _bonding_curve, token_mint) = setup_cache_with_pumpfun();
    let result = cache.mark_pumpfun_complete_for_mint(&token_mint);
    assert!(
        result,
        "mark_pumpfun_complete_for_mint muss true zurückgeben wenn Mint gefunden"
    );
}

#[test]
fn mark_pumpfun_complete_returns_false_for_unknown_mint() {
    let cache: SharedLivePoolCache = create_shared_cache();
    let unknown_mint = Pubkey::new_unique();
    let result = cache.mark_pumpfun_complete_for_mint(&unknown_mint);
    assert!(
        !result,
        "mark_pumpfun_complete_for_mint muss false zurückgeben bei unbekanntem Mint"
    );
}

#[test]
fn find_pump_amm_pool_returns_pool_when_cached() {
    let (cache, pool_market, base_mint) = setup_cache_with_pump_amm();
    let result = cache.find_pump_amm_pool_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "find_pump_amm_pool_by_base_mint muss Some liefern bei Cache-Hit"
    );
    assert_eq!(result.unwrap(), pool_market);
}

#[test]
fn get_pump_amm_pool_accounts_by_base_mint_returns_accounts() {
    let (cache, _pool_market, base_mint) = setup_cache_with_pump_amm();
    let result = cache.get_pump_amm_pool_accounts_by_base_mint(&base_mint);
    assert!(
        result.is_some(),
        "get_pump_amm_pool_accounts_by_base_mint muss Some liefern bei Cache-Hit"
    );
    let accounts = result.unwrap();
    assert_eq!(accounts.len(), 14);
}

// --- A.29: Liquidation Vollständigkeit ---

/// PumpFun SELL mit cashback_enabled=true muss 16 Accounts haben.
/// Verifiziert dass die Liquidation den korrekten Account-Count nutzt.
#[test]
fn liquidation_cashback_account_layout_16_accounts() {
    let pumpfun = setup_pumpfun_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).unwrap();

    let ix = pumpfun
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000, // amount_in
            100,       // min_sol_output
            true,      // cashback_enabled = true
        )
        .expect("build_sell_ix should succeed");

    assert_eq!(
        ix.accounts.len(),
        16,
        "PumpFun SELL with cashback_enabled=true must have 16 accounts"
    );

    // Letztes Account muss bonding_curve_v2 sein
    let last = ix.accounts.last().unwrap();
    assert!(!last.is_signer, "bonding_curve_v2 is not signer");
    assert!(!last.is_writable, "bonding_curve_v2 is readonly");
}

#[test]
fn liquidation_non_cashback_account_layout_15() {
    let pumpfun = setup_pumpfun_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).unwrap();

    let ix = pumpfun
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            100,
            false, // cashback_enabled = false
        )
        .expect("build_sell_ix should succeed");

    assert_eq!(
        ix.accounts.len(),
        15,
        "PumpFun SELL with cashback_enabled=false must have 15 accounts"
    );
}

// --- PumpFun Bonding Curve Cold-Path (A.31, A.39) ---
//
// Stale Cache darf nicht blind dominieren. Wenn Cache cashback_enabled=false hat,
// darf der Cold Path NICHT still erfolgreich ein 15-Account-Layout liefern, wenn
// on-chain cashback_enabled=true wäre. Entweder: RPC-Verifikation → korrektes Layout,
// oder: klarer Failure (Err).

/// Cache mit PumpFun-State (cashback_enabled=false) – simuliert stale JetStream-Cache.
fn setup_cache_with_pumpfun_stale_cashback() -> (SharedLivePoolCache, Pubkey, Pubkey) {
    let cache = create_shared_cache();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_from_array([4u8; 32]);
    let creator = Pubkey::new_from_array([5u8; 32]);

    let state = CachedPoolState::PumpFun(PumpFunState {
        token_mint,
        bonding_curve,
        associated_bonding_curve,
        virtual_sol_reserves: 100_000_000,
        virtual_token_reserves: 1_000_000_000,
        real_sol_reserves: 100_000_000,
        real_token_reserves: 1_000_000_000,
        complete: true,
        creator,
        cashback_enabled: false, // Stale: JetStream hatte kein cashback_enabled im Metadata
    });

    cache.upsert(bonding_curve, state, 0);
    (cache, bonding_curve, token_mint)
}

/// A.31/A.39: Stale Cache (cashback_enabled=false) + Cold Path (allow_rpc_fallback=true)
/// + RPC unreachable → klarer Failure (Err), NICHT stilles Ok mit falschem 15-Account-Layout.
/// Beweist: Cache-HIT mit cashback_enabled=false wird im Cold Path nicht blind vertraut.
#[tokio::test]
async fn pumpfun_cold_path_stale_cache_rpc_unreachable_clear_failure() {
    let (cache, _bonding_curve, token_mint) = setup_cache_with_pumpfun_stale_cashback();
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let mut dex = PumpFunDex::new(rpc, Some(cache)).expect("PumpFunDex::new");
    let wallet = Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet");
    dex.set_user_authority(wallet);

    let result = dex
        .build_swap_ix_async_with_slippage(
            &token_mint.to_string(),
            WSOL_MINT,
            1_000_000,
            100,
            None,
            500,
            None,
            false,
            true, // allow_rpc_fallback: Cold Path
        )
        .await;

    assert!(
        result.is_err(),
        "A.31/A.39: Cold Path mit stale Cache (cashback=false) und RPC unreachable \
         muss Err liefern, nicht stilles Ok mit falschem 15-Account-Layout"
    );
}

/// A.39: Aktive PumpFun Bonding Curve + korrektes cashback_enabled → erweitertes Layout.
/// build_sell_ix(cashback_enabled=true) liefert 16 Accounts (user_volume_accumulator + bonding_curve_v2).
/// Verhindert Bug #25: falsches Layout bei cashback → Custom(6024) Overflow.
#[test]
fn pumpfun_cold_path_cashback_true_produces_extended_layout() {
    let pumpfun = setup_pumpfun_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).unwrap();

    let ix = pumpfun
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            100,
            true, // cashback_enabled = true → erweitertes Layout
        )
        .expect("build_sell_ix should succeed");

    assert_eq!(
        ix.accounts.len(),
        16,
        "A.39: PumpFun Cold Path SELL mit cashback_enabled=true muss 16 Accounts haben \
         (user_volume_accumulator + bonding_curve_v2), nicht 15"
    );

    let last = ix.accounts.last().unwrap();
    assert!(!last.is_signer, "bonding_curve_v2 is not signer");
    assert!(!last.is_writable, "bonding_curve_v2 is readonly");
}
