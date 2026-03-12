//! Invariante: PumpFun build_swap_ix Instruction-Gültigkeit (INVARIANTS.md A.3, DoD §H)
//!
//! PumpFunDex::build_swap_ix_async_with_slippage liefert 2 IXs (ATA + swap),
//! program_id = pump.fun, user an Index 6 signer+writable.
//! tx_builder::build_tx_plan unterstützt PumpFun SELL rein per Derivation.

use ironcrab::execution::tx_builder;
use ironcrab::ipc::{
    ExplicitAmount, IntentOrigin, IntentTier, TradeIntent, TradeResources, TradeSide, TradingRegime,
};
use ironcrab::solana::dex::pumpfun::PumpFunDex;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

#[tokio::test]
async fn pumpfun_build_buy_ix_valid_accounts() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));
    let mut dex = PumpFunDex::new(rpc, None).expect("PumpFunDex::new");

    let wallet =
        Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet pubkey");
    dex.set_user_authority(wallet);

    let creator =
        Pubkey::from_str("2tFqgkJX6kqz8q6o9tFv3oJ9nQx7n1m3fHk2m8f3oKpZ").expect("creator pubkey");
    let token_mint = "9xQeWvG816bUx9EPfKJb9N9dKz5wW7Yy2hBzXv4mQ4kG";

    let ixs = dex
        .build_swap_ix_async_with_slippage(
            "So11111111111111111111111111111111111111112",
            token_mint,
            1_000_000,
            123_456,
            Some(creator),
            500,
            None,
            false,
            false, // allow_rpc_fallback: Hot-Path-Test, keine RPC-Calls
        )
        .await
        .expect("build_swap_ix_async_with_slippage");

    assert_eq!(ixs.len(), 2, "expected ATA creation + pump.fun instruction");

    let ix = &ixs[1];
    assert_eq!(
        ix.program_id,
        Pubkey::from_str(PUMPFUN_PROGRAM_ID).expect("pumpfun program id"),
        "program_id must be pump.fun"
    );

    let user_meta = ix.accounts.get(6).expect("user meta index 6");
    assert_eq!(user_meta.pubkey, wallet);
    assert!(user_meta.is_signer);
    assert!(user_meta.is_writable);

    assert!(
        ix.accounts
            .iter()
            .any(|m| m.is_writable && m.pubkey != wallet),
        "expected at least one writable non-user account"
    );
    assert!(!ix.data.is_empty(), "instruction data must not be empty");
}

/// TxBuilder unterstützt PumpFun SELL rein per Derivation (kein RPC).
/// Mit metadata.creator und metadata.min_out_raw liefert build_tx_plan 2 IXs (ATA + pump.fun).
#[tokio::test]
async fn tx_builder_supports_pumpfun_sell() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));

    let wallet =
        Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet pubkey");

    let creator = "2tFqgkJX6kqz8q6o9tFv3oJ9nQx7n1m3fHk2m8f3oKpZ";
    let token_mint = "9xQeWvG816bUx9EPfKJb9N9dKz5wW7Yy2hBzXv4mQ4kG";
    let sol_mint = "So11111111111111111111111111111111111111112";

    let mut intent = TradeIntent::new(
        "test",
        "test",
        "test",
        "intent-sell-derivation".to_string(),
        "test",
        IntentTier::Tier1,
        IntentOrigin::StrategyA,
        ExplicitAmount::new(1_000_000, 6),
        TradeResources {
            input_mint: token_mint.to_string(),
            output_mint: sol_mint.to_string(),
            pools: vec!["pumpfun".to_string()],
            accounts: vec![],
            token_program: None,
        },
        0,
        500,
        TradeSide::Sell,
        TradingRegime::NotApplicable,
    );

    intent
        .metadata
        .insert("creator".to_string(), creator.to_string());
    intent
        .metadata
        .insert("min_out_raw".to_string(), "1".to_string());

    let plan = match tx_builder::build_tx_plan(&intent, wallet, Arc::clone(&rpc), None, None, false)
        .await
    {
        tx_builder::TxPlanOutcome::Planned(p) => p,
        tx_builder::TxPlanOutcome::Unsupported(u) => {
            panic!(
                "unexpected unsupported plan: {:?} - {}",
                u.reason, u.details
            )
        }
    };

    assert_eq!(
        plan.instructions.len(),
        2,
        "expected ATA creation + pump.fun instruction"
    );

    let ix = &plan.instructions[1];
    assert_eq!(
        ix.program_id,
        Pubkey::from_str(PUMPFUN_PROGRAM_ID).expect("pumpfun program id"),
        "program_id must be pump.fun"
    );

    let user_meta = ix.accounts.get(6).expect("user meta index 6");
    assert_eq!(user_meta.pubkey, wallet);
    assert!(user_meta.is_signer);
    assert!(user_meta.is_writable);

    assert!(!ix.data.is_empty(), "instruction data must not be empty");
}
