//! Invarianten: DEX Connector (INVARIANTS.md §1.3)
//!
//! Verifiziert mathematische und verhaltensbezogene Invarianten für DEX-Connectors.
//! PumpSwap AMM: Quote-Monotonie, Price-Impact, Unknown Pair, Zero Input, Build IX,
//! PumpSwap `build_swap_ix_from_pool_accounts` Fee-Metas (protocol_fee_recipient / TA).

use ironcrab::execution::live_pool_cache::{CachedPoolState, LivePoolCache, PumpAmmState};
use ironcrab::solana::dex::pumpfun_amm::PumpFunAmmDex;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_pubkey::Pubkey as SplPubkey;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account_client::address::get_associated_token_address;
use std::str::FromStr;
use std::sync::Arc;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
const PUMPFUN_AMM_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";
/// Swap-IX Account-Metas für protocol_fee_recipient / protocol_fee_recipient_ta (öffentliche PumpFunAmmDex-Doku).
const SWAP_IX_PROTOCOL_FEE_RECIPIENT_META: usize = 9;
const SWAP_IX_PROTOCOL_FEE_RECIPIENT_TA_META: usize = 10;

fn pool_accounts_v1_pumpswap_layout(
    base_mint: Pubkey,
    quote_mint: Pubkey,
    protocol_fee_recipient: Pubkey,
    protocol_fee_recipient_ta: Pubkey,
) -> Vec<Pubkey> {
    vec![
        Pubkey::new_unique(),      // [0] pool_market
        Pubkey::new_unique(),      // [1] global_config
        base_mint,                 // [2] base_mint
        quote_mint,                // [3] quote_mint
        Pubkey::new_unique(),      // [4] pool_base_vault
        Pubkey::new_unique(),      // [5] pool_quote_vault
        protocol_fee_recipient,    // [6]
        protocol_fee_recipient_ta, // [7]
        Pubkey::new_unique(),      // [8] event_authority
        Pubkey::new_unique(),      // [9] coin_creator_vault_ata
        Pubkey::new_unique(),      // [10] coin_creator_vault_authority
        Pubkey::new_unique(),      // [11] global_volume_accumulator
        Pubkey::new_unique(),      // [12] fee_config
        Pubkey::new_unique(),      // [13] fee_program
    ]
}

fn spl_pubkey_from_sdk(p: &Pubkey) -> SplPubkey {
    SplPubkey::new_from_array(p.to_bytes())
}

fn sdk_pubkey_from_spl(p: &SplPubkey) -> Pubkey {
    Pubkey::new_from_array(p.to_bytes())
}

fn pump_swap_swap_ix(ixs: &[Instruction]) -> &Instruction {
    let program = Pubkey::from_str(PUMPFUN_AMM_PROGRAM_ID).unwrap();
    ixs.iter()
        .find(|ix| ix.program_id == program && !ix.data.is_empty())
        .expect("expected non-empty PumpSwap AMM swap instruction")
}

fn make_pump_amm_cache_with_reserves(
    pool_market: Pubkey,
    base_mint: Pubkey,
    base_reserve: u64,
    quote_reserve: u64,
) -> Arc<LivePoolCache> {
    let cache = LivePoolCache::new();
    cache.upsert(
        pool_market,
        CachedPoolState::PumpAmm(PumpAmmState {
            base_mint,
            quote_mint: Pubkey::from_str(WSOL_MINT).unwrap(),
            pool_base_token_account: Pubkey::new_unique(),
            pool_quote_token_account: Pubkey::new_unique(),
            base_reserve: Some(base_reserve),
            quote_reserve: Some(quote_reserve),
            pool_accounts: vec![],
            creator: None,
        }),
        100,
    );
    Arc::new(cache)
}

fn make_empty_cache() -> Arc<LivePoolCache> {
    Arc::new(LivePoolCache::new())
}

#[tokio::test]
async fn contract_pump_amm_quote_monotonic() {
    let base_mint = Pubkey::new_unique();
    let pool_market = Pubkey::new_unique();
    let cache = make_pump_amm_cache_with_reserves(
        pool_market,
        base_mint,
        1_000_000_000_000,
        50_000_000_000,
    );
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);

    let base_mint_str = base_mint.to_string();

    let out1 = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 100_000)
        .await
        .expect("quote ok")
        .map(|q| q.amount_out);
    let out2 = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 1_000_000)
        .await
        .expect("quote ok")
        .map(|q| q.amount_out);
    let out3 = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 10_000_000)
        .await
        .expect("quote ok")
        .map(|q| q.amount_out);

    assert!(out1.is_some() && out1.unwrap() > 0);
    assert!(out2.is_some() && out2.unwrap() > 0);
    assert!(out3.is_some() && out3.unwrap() > 0);
    assert!(out1.unwrap() <= out2.unwrap());
    assert!(out2.unwrap() <= out3.unwrap());
}

#[tokio::test]
async fn contract_pump_amm_price_impact_non_decreasing() {
    let base_mint = Pubkey::new_unique();
    let pool_market = Pubkey::new_unique();
    let cache = make_pump_amm_cache_with_reserves(
        pool_market,
        base_mint,
        1_000_000_000_000,
        50_000_000_000,
    );
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);

    let base_mint_str = base_mint.to_string();

    let impact1 = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 1_000_000)
        .await
        .expect("quote ok")
        .map(|q| q.price_impact_bps);
    let impact2 = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 10_000_000)
        .await
        .expect("quote ok")
        .map(|q| q.price_impact_bps);
    let impact3 = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 100_000_000)
        .await
        .expect("quote ok")
        .map(|q| q.price_impact_bps);

    assert!(impact1.is_some());
    assert!(impact2.is_some());
    assert!(impact3.is_some());
    assert!(impact1.unwrap() <= impact2.unwrap());
    assert!(impact2.unwrap() <= impact3.unwrap());
}

#[tokio::test]
async fn contract_pump_amm_unknown_pair_returns_none() {
    let base_mint = Pubkey::new_unique();
    let cache = make_empty_cache();
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);

    let base_mint_str = base_mint.to_string();
    let result = dex
        .quote_exact_in(WSOL_MINT, &base_mint_str, 1_000_000)
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn contract_pump_amm_zero_input() {
    let base_mint = Pubkey::new_unique();
    let pool_market = Pubkey::new_unique();
    let cache = make_pump_amm_cache_with_reserves(
        pool_market,
        base_mint,
        1_000_000_000_000,
        50_000_000_000,
    );
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let dex = PumpFunAmmDex::new_with_cache(rpc, cache, false);

    let base_mint_str = base_mint.to_string();
    let result = dex.quote_exact_in(WSOL_MINT, &base_mint_str, 0).await;

    assert!(result.is_ok());
    let quote_opt = result.unwrap();
    assert!(
        quote_opt.is_none() || quote_opt.as_ref().unwrap().amount_out == 0,
        "zero input must yield None or amount_out == 0"
    );
}

#[test]
fn contract_pump_amm_build_ix_valid_accounts() {
    let wsol = Pubkey::from_str(WSOL_MINT).unwrap();
    let base_mint = Pubkey::new_unique();
    let base_mint_str = base_mint.to_string();
    let user = Pubkey::new_unique();

    let pool_accounts: Vec<Pubkey> = vec![
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        base_mint,
        wsol,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ];
    assert_eq!(pool_accounts.len(), 14);

    let result = PumpFunAmmDex::build_swap_ix_from_pool_accounts(
        WSOL_MINT,
        &base_mint_str,
        1_000_000_000,
        100_000,
        user,
        &pool_accounts,
        None,
    );

    assert!(result.is_ok());
    let ixs = result.unwrap();
    assert!(!ixs.is_empty());
    assert_eq!(
        ixs[0].program_id,
        Pubkey::from_str(PUMPFUN_AMM_PROGRAM_ID).unwrap()
    );
    assert!(!ixs[0].data.is_empty());
}

#[test]
fn contract_pump_amm_build_ix_preserves_observed_protocol_fee_metas() {
    let wsol = Pubkey::from_str(WSOL_MINT).unwrap();
    let base_mint = Pubkey::new_unique();
    let base_mint_str = base_mint.to_string();
    let user = Pubkey::new_unique();
    let protocol_fee_recipient = Pubkey::new_unique();
    let protocol_fee_recipient_ta = Pubkey::new_unique();

    let pool_accounts = pool_accounts_v1_pumpswap_layout(
        base_mint,
        wsol,
        protocol_fee_recipient,
        protocol_fee_recipient_ta,
    );

    let result = PumpFunAmmDex::build_swap_ix_from_pool_accounts(
        &base_mint_str,
        WSOL_MINT,
        1_000_000_000,
        100_000,
        user,
        &pool_accounts,
        None,
    );

    assert!(result.is_ok(), "expected Ok: {:?}", result);
    let ixs = result.unwrap();
    let swap_ix = pump_swap_swap_ix(&ixs);
    assert_eq!(
        swap_ix.accounts[SWAP_IX_PROTOCOL_FEE_RECIPIENT_META].pubkey,
        protocol_fee_recipient
    );
    assert_eq!(
        swap_ix.accounts[SWAP_IX_PROTOCOL_FEE_RECIPIENT_TA_META].pubkey,
        protocol_fee_recipient_ta
    );
}

#[test]
fn contract_pump_amm_build_ix_derives_fee_ta_from_observed_recipient_when_ta_missing() {
    let wsol = Pubkey::from_str(WSOL_MINT).unwrap();
    let base_mint = Pubkey::new_unique();
    let base_mint_str = base_mint.to_string();
    let user = Pubkey::new_unique();
    let protocol_fee_recipient = Pubkey::new_unique();
    let expected_fee_ta = sdk_pubkey_from_spl(&get_associated_token_address(
        &spl_pubkey_from_sdk(&protocol_fee_recipient),
        &spl_pubkey_from_sdk(&wsol),
    ));

    let mut pool_accounts =
        pool_accounts_v1_pumpswap_layout(base_mint, wsol, protocol_fee_recipient, expected_fee_ta);
    pool_accounts[7] = Pubkey::default();

    let result = PumpFunAmmDex::build_swap_ix_from_pool_accounts(
        &base_mint_str,
        WSOL_MINT,
        1_000_000_000,
        100_000,
        user,
        &pool_accounts,
        None,
    );

    assert!(result.is_ok(), "expected Ok: {:?}", result);
    let ixs = result.unwrap();
    let swap_ix = pump_swap_swap_ix(&ixs);
    assert_eq!(
        swap_ix.accounts[SWAP_IX_PROTOCOL_FEE_RECIPIENT_META].pubkey,
        protocol_fee_recipient
    );
    assert_eq!(
        swap_ix.accounts[SWAP_IX_PROTOCOL_FEE_RECIPIENT_TA_META].pubkey,
        expected_fee_ta
    );
}

#[test]
fn contract_pump_amm_build_ix_errors_when_protocol_fee_pubkeys_missing() {
    let wsol = Pubkey::from_str(WSOL_MINT).unwrap();
    let base_mint = Pubkey::new_unique();
    let base_mint_str = base_mint.to_string();
    let user = Pubkey::new_unique();

    let mut pool_accounts =
        pool_accounts_v1_pumpswap_layout(base_mint, wsol, Pubkey::default(), Pubkey::default());
    pool_accounts[6] = Pubkey::default();
    pool_accounts[7] = Pubkey::default();

    let result = PumpFunAmmDex::build_swap_ix_from_pool_accounts(
        &base_mint_str,
        WSOL_MINT,
        1_000_000_000,
        100_000,
        user,
        &pool_accounts,
        None,
    );

    let err = result.expect_err("expected Err when protocol_fee_recipient and fee TA are default");
    let msg = err.to_string();
    assert!(
        msg.contains("protocol_fee_recipient") && msg.contains("protocol_fee_recipient_ta"),
        "error should name both fee accounts; got: {msg}"
    );
    assert!(
        msg.contains("missing") || msg.contains("observed fee"),
        "error should indicate missing/unobserved fee accounts, not a generic build failure; got: {msg}"
    );
}
