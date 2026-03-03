//! Invariante: PumpFun build_swap_ix Instruction-Gültigkeit (INVARIANTS.md A.3, DoD §H)
//!
//! PumpFunDex::build_swap_ix_async_with_slippage liefert 2 IXs (ATA + swap),
//! program_id = pump.fun, user an Index 6 signer+writable.

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
