//! Invarianten A.25–A.26: PumpFun Market Order BUY (buy_exact_sol_in)
//!
//! Momentum BUY nutzt buy_exact_sol_in statt global:buy für Market Orders.
//! Discriminator [56, 252, 116, 8, 158, 223, 205, 95]. 17 Accounts identisch zu build_buy_ix.

use ironcrab::solana::dex::pumpfun::{PumpFunDex, PUMPFUN_PROGRAM_ID};
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// buy_exact_sol_in Discriminator (8 bytes)
const BUY_EXACT_SOL_IN_DISCRIMINATOR: [u8; 8] = [56, 252, 116, 8, 158, 223, 205, 95];

/// global:buy Discriminator (8 bytes)
const GLOBAL_BUY_DISCRIMINATOR: [u8; 8] = [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea];

fn setup_dex() -> PumpFunDex {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:8899"));
    let mut dex = PumpFunDex::new(rpc, None).expect("PumpFunDex::new");
    let wallet = Pubkey::from_str("Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM").expect("wallet");
    dex.set_user_authority(wallet);
    dex
}

fn derive_bonding_curve_v2(token_mint: &Pubkey) -> Pubkey {
    let program_id = Pubkey::from_str(PUMPFUN_PROGRAM_ID).expect("pumpfun program id");
    let (pda, _) =
        Pubkey::find_program_address(&[b"bonding-curve-v2", token_mint.as_ref()], &program_id);
    pda
}

/// A.25: market_order_buy_has_17_accounts — build_buy_exact_sol_ix liefert 17 Accounts, 24 Bytes Data.
#[test]
fn market_order_buy_has_17_accounts() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_buy_exact_sol_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            1,
        )
        .expect("build_buy_exact_sol_ix");

    assert_eq!(
        ix.accounts.len(),
        17,
        "buy_exact_sol_in must have 17 accounts (A.25)"
    );
    assert_eq!(
        ix.data.len(),
        24,
        "Data must be 24 bytes (8 disc + 8 sol + 8 min_tokens)"
    );
}

/// A.25: market_order_buy_correct_discriminator — Discriminator ist buy_exact_sol_in, nicht global:buy.
#[test]
fn market_order_buy_correct_discriminator() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_buy_exact_sol_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            1,
        )
        .expect("build_buy_exact_sol_ix");

    assert_eq!(
        ix.data[0..8],
        BUY_EXACT_SOL_IN_DISCRIMINATOR,
        "Discriminator must be buy_exact_sol_in"
    );
    assert_ne!(
        ix.data[0..8],
        GLOBAL_BUY_DISCRIMINATOR,
        "Must NOT use global:buy discriminator"
    );
}

/// A.25: market_order_buy_data_serialization — sol_amount und min_tokens_out korrekt serialisiert.
#[test]
fn market_order_buy_data_serialization() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_buy_exact_sol_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            5_000_000,
            42,
        )
        .expect("build_buy_exact_sol_ix");

    let sol_amount = u64::from_le_bytes(ix.data[8..16].try_into().unwrap());
    let min_tokens_out = u64::from_le_bytes(ix.data[16..24].try_into().unwrap());

    assert_eq!(
        sol_amount, 5_000_000,
        "sol_amount must be serialized correctly"
    );
    assert_eq!(
        min_tokens_out, 42,
        "min_tokens_out must be serialized correctly"
    );
}

/// A.26: market_order_buy_bonding_curve_v2_last — bonding_curve_v2 ist letztes Account, !signer, !writable.
#[test]
fn market_order_buy_bonding_curve_v2_last() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_buy_exact_sol_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            1,
        )
        .expect("build_buy_exact_sol_ix");

    let expected_bonding_curve_v2 = derive_bonding_curve_v2(&token_mint);
    let last = ix.accounts.last().unwrap();

    assert_eq!(
        last.pubkey, expected_bonding_curve_v2,
        "Last account must be bonding_curve_v2 PDA"
    );
    assert!(!last.is_signer, "bonding_curve_v2 must not be signer");
    assert!(!last.is_writable, "bonding_curve_v2 must not be writable");
}

/// A.26: market_order_buy_same_accounts_as_regular_buy — Account-Layout identisch zu build_buy_ix.
#[test]
fn market_order_buy_same_accounts_as_regular_buy() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let market_ix = dex
        .build_buy_exact_sol_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            1,
        )
        .expect("build_buy_exact_sol_ix");

    let regular_ix = dex
        .build_buy_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            2_000_000,
        )
        .expect("build_buy_ix");

    assert_eq!(
        market_ix.accounts.len(),
        regular_ix.accounts.len(),
        "Account count must match"
    );

    for (i, (m, r)) in market_ix
        .accounts
        .iter()
        .zip(regular_ix.accounts.iter())
        .enumerate()
    {
        assert_eq!(m.pubkey, r.pubkey, "Account {} pubkey must match", i);
        assert_eq!(
            m.is_signer, r.is_signer,
            "Account {} is_signer must match",
            i
        );
        assert_eq!(
            m.is_writable, r.is_writable,
            "Account {} is_writable must match",
            i
        );
    }

    assert_eq!(
        market_ix.program_id, regular_ix.program_id,
        "program_id must match"
    );
}

/// regular_buy_uses_different_discriminator — build_buy_ix nutzt global:buy, nicht buy_exact_sol_in.
#[test]
fn regular_buy_uses_different_discriminator() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_buy_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            2_000_000,
        )
        .expect("build_buy_ix");

    assert_eq!(
        ix.data[0..8],
        GLOBAL_BUY_DISCRIMINATOR,
        "build_buy_ix must use global:buy discriminator"
    );
    assert_ne!(
        ix.data[0..8],
        BUY_EXACT_SOL_IN_DISCRIMINATOR,
        "build_buy_ix must NOT use buy_exact_sol_in discriminator"
    );
}
