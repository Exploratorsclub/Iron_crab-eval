//! Cross-DEX Cold-Path Reserve-Fallback (Raydium, RaydiumCpmm, MeteoraDlmm, Orca Whirlpool)
//!
//! Invariante: Bekannter Pool + fehlende Live-Reserves im Cold Path =
//! autoritativer RPC-Fallback oder klarer Failure.
//!
//! Wenn fuer einen bereits bekannten Pool die fuer Quote/Ausfuehrung benoetigten
//! Reserve-/Vault-Daten im LivePoolCache fehlen, darf der Cold Path den Fall
//! nicht still wie einen harmlosen Cache-Miss behandeln. Er muss entweder den
//! autoritativen Reserve-State per RPC nachladen oder einen klaren Fehler liefern.
//! Nicht erlaubt: stilles Ok(None) oder verdeckter lokaler Ersatz-Truth.
//!
//! Hot Path bleibt GEYSER-ONLY (A.12); dieser Test prueft die Cold-Path-Gegenseite.

use ironcrab::execution::live_pool_cache::{
    LivePoolCache, OrcaWhirlpoolState, SharedLivePoolCache,
};
use ironcrab::solana::dex::meteora_dlmm::MeteoraDlmm;
use ironcrab::solana::dex::orca::Orca;
use ironcrab::solana::dex::raydium::Raydium;
use ironcrab::solana::dex::raydium_cpmm::RaydiumCpmm;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

const DUMMY_RPC: &str = "http://127.0.0.1:0";
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// Raydium AMM: Bekannter Pool, fehlende Vault-Reserves, Cold Path (allow_rpc=true),
/// RPC unreachable → klarer Fehler (Err), NICHT stilles Ok(None).
#[tokio::test]
async fn raydium_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err() {
    let rpc = Arc::new(SolanaRpc::new(DUMMY_RPC));
    let raydium = Raydium::new_with_live_cache(rpc, None, true);

    let pool_addr = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::new_unique();
    let base_vault = Pubkey::new_unique();
    let quote_vault = Pubkey::new_unique();
    let market_id = Pubkey::new_unique();

    // Fehlende Vault-Reserves: coin_reserve/pc_reserve None spiegeln keinen brauchbaren Quote-State.
    raydium.inject_cached_amm_state(
        pool_addr,
        base_mint,
        quote_mint,
        base_vault,
        quote_vault,
        9,
        6,
        None,
        None,
        market_id,
        None,
        None,
        None,
    );

    let result = raydium.fetch_and_update_reserves(&pool_addr).await;

    assert!(
        result.is_err(),
        "Cold Path: bekannter Pool + fehlende Reserves + RPC unreachable muss Err liefern, nicht stilles Ok"
    );
}

/// Raydium CPMM: Bekannter Pool, fehlende Vault-Reserves, Cold Path (allow_rpc=true),
/// RPC unreachable → klarer Fehler (Err), NICHT stilles Ok(None).
#[tokio::test]
async fn raydium_cpmm_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err() {
    let rpc = Arc::new(SolanaRpc::new(DUMMY_RPC));
    let cpmm = RaydiumCpmm::new_with_live_cache(rpc, None, true);

    let pool_addr = Pubkey::new_unique();
    let token_0 = Pubkey::new_unique();
    let token_1 = Pubkey::new_unique();
    let vault_0 = Pubkey::new_unique();
    let vault_1 = Pubkey::new_unique();

    cpmm.set_pool_from_accounts(
        &pool_addr.to_string(),
        &[
            pool_addr.to_string(),
            token_0.to_string(),
            token_1.to_string(),
            vault_0.to_string(),
            vault_1.to_string(),
        ],
    )
    .expect("set_pool_from_accounts");

    let result = cpmm
        .quote_exact_in(&token_0.to_string(), &token_1.to_string(), 1_000_000)
        .await;

    assert!(
        result.is_err(),
        "Cold Path: bekannter Pool + fehlende Reserves + RPC unreachable muss Err liefern, nicht Ok(None)"
    );
}

/// Meteora DLMM: Bekannter Pool, fehlende Reserve-Daten, Cold Path (allow_rpc=true),
/// RPC unreachable → klarer Fehler (Err), NICHT stilles Ok(None).
#[tokio::test]
async fn meteora_dlmm_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err() {
    let rpc = Arc::new(SolanaRpc::new(DUMMY_RPC));
    let meteora = MeteoraDlmm::new_with_live_cache(rpc, None, true);

    let pool_addr = Pubkey::new_unique();
    let token_x = Pubkey::new_unique();
    let token_y = Pubkey::new_unique();
    let reserve_x = Pubkey::new_unique();
    let reserve_y = Pubkey::new_unique();

    meteora
        .set_pool_from_accounts(
            &pool_addr.to_string(),
            &[
                pool_addr.to_string(),
                token_x.to_string(),
                token_y.to_string(),
                reserve_x.to_string(),
                reserve_y.to_string(),
            ],
        )
        .expect("set_pool_from_accounts");

    let result = meteora
        .quote_exact_in(&token_x.to_string(), &token_y.to_string(), 1_000_000)
        .await;

    assert!(
        result.is_err(),
        "Cold Path: bekannter Pool + fehlende Reserves + RPC unreachable muss Err liefern, nicht Ok(None)"
    );
}

/// Orca Whirlpool: Bekannter Pool (LivePoolCache-Eintrag), fehlende Vault-Reserve-Balances,
/// Cold Path (`new_with_cache_ext` mit allow_rpc_on_miss=true), RPC unreachable → Err,
/// NICHT stilles Ok(None).
#[tokio::test]
async fn orca_whirlpool_cold_path_known_pool_missing_reserves_rpc_unreachable_yields_err() {
    let rpc = Arc::new(SolanaRpc::new(DUMMY_RPC));
    let live_pool_cache: SharedLivePoolCache = Arc::new(LivePoolCache::new());
    let orca = Orca::new_with_cache_ext(rpc, None, Some(live_pool_cache), true);

    let pool_addr = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let quote_mint = Pubkey::from_str(WSOL_MINT).expect("wsol mint");
    let vault_a = Pubkey::new_unique();
    let vault_b = Pubkey::new_unique();

    // Bekannter Pool im SLAVE-Cache, aber keine brauchbaren Vault-Balances (None = fehlend).
    let cached = OrcaWhirlpoolState {
        token_mint_a: base_mint,
        token_mint_b: quote_mint,
        token_vault_a: vault_a,
        token_vault_b: vault_b,
        tick_current_index: 0,
        sqrt_price: 1u128 << 64,
        liquidity: 1_000_000u128,
        fee_rate: 30,
        protocol_fee_rate: 0,
        tick_spacing: 64,
        vault_a_balance: None,
        vault_b_balance: None,
        token_a_program: None,
        token_b_program: None,
    };
    orca.inject_cached_orca_state(&pool_addr, &cached)
        .expect("inject_cached_orca_state");

    let result = orca
        .quote_exact_in(&base_mint.to_string(), &quote_mint.to_string(), 1_000_000)
        .await;

    assert!(
        result.is_err(),
        "Cold Path: bekannter Orca-Pool + fehlende Vault-Reserves + RPC unreachable muss Err liefern, nicht stilles Ok(None)"
    );
}
