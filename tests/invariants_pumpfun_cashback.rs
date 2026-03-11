//! Invarianten A.22–A.24: PumpFun Cashback-Upgrade (6024 Fix)
//!
//! Post-Cashback-Upgrade (Feb 2026): bonding_curve_v2 PDA, cashback_enabled in SELL,
//! BondingCurveState::parse() liest cashback_enabled aus Byte 82.

use ironcrab::solana::dex::pumpfun::{BondingCurveState, PumpFunDex, PUMPFUN_PROGRAM_ID};
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// PumpFun BondingCurveState Discriminator (8 bytes)
const BONDING_CURVE_DISCRIMINATOR: [u8; 8] = [0x17, 0xb7, 0xf8, 0x37, 0x60, 0xd8, 0xac, 0x60];

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

/// A.22: build_buy_ix() liefert genau 17 Accounts, bonding_curve_v2 an Index 16.
#[test]
fn buy_ix_has_17_accounts() {
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
        ix.accounts.len(),
        17,
        "BUY must have 17 accounts (bonding_curve_v2 at 16)"
    );

    let expected_bonding_curve_v2 = derive_bonding_curve_v2(&token_mint);
    let last = &ix.accounts[16];
    assert_eq!(
        last.pubkey, expected_bonding_curve_v2,
        "Index 16 must be bonding_curve_v2 PDA"
    );
    assert!(!last.is_signer, "bonding_curve_v2 must not be signer");
    assert!(!last.is_writable, "bonding_curve_v2 must not be writable");
}

/// A.23: build_sell_ix(cashback=false) liefert 15 Accounts, bonding_curve_v2 an Index 14.
#[test]
fn sell_ix_non_cashback_has_15_accounts() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            500_000,
            false, // cashback_enabled
        )
        .expect("build_sell_ix");

    assert_eq!(
        ix.accounts.len(),
        15,
        "SELL non-cashback must have 15 accounts (bonding_curve_v2 at 14)"
    );

    let expected_bonding_curve_v2 = derive_bonding_curve_v2(&token_mint);
    let last = &ix.accounts[14];
    assert_eq!(
        last.pubkey, expected_bonding_curve_v2,
        "Index 14 must be bonding_curve_v2 PDA"
    );
    assert!(!last.is_writable, "bonding_curve_v2 must not be writable");
}

/// A.23: build_sell_ix(cashback=true) liefert 16 Accounts, user_volume_accumulator an 14, bonding_curve_v2 an 15.
#[test]
fn sell_ix_cashback_has_16_accounts() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");

    let ix = dex
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            500_000,
            true, // cashback_enabled
        )
        .expect("build_sell_ix");

    assert_eq!(
        ix.accounts.len(),
        16,
        "SELL cashback must have 16 accounts (user_volume_accumulator at 14, bonding_curve_v2 at 15)"
    );

    let expected_bonding_curve_v2 = derive_bonding_curve_v2(&token_mint);
    let last = &ix.accounts[15];
    assert_eq!(
        last.pubkey, expected_bonding_curve_v2,
        "Index 15 must be bonding_curve_v2 PDA"
    );
    assert!(!last.is_writable, "bonding_curve_v2 must not be writable");

    let user_volume_acc = &ix.accounts[14];
    assert!(
        user_volume_acc.is_writable,
        "Index 14 (user_volume_accumulator) must be writable"
    );
}

/// A.22/A.23: bonding_curve_v2 ist immer das letzte Account (BUY + SELL).
#[test]
fn bonding_curve_v2_always_last_account() {
    let dex = setup_dex();
    let token_mint = Pubkey::new_unique();
    let (bonding_curve, _) = PumpFunDex::derive_bonding_curve_static(&token_mint);
    let associated_bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).expect("token program");
    let expected_bonding_curve_v2 = derive_bonding_curve_v2(&token_mint);

    // BUY
    let buy_ix = dex
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
        buy_ix.accounts.last().unwrap().pubkey,
        expected_bonding_curve_v2,
        "BUY: last account must be bonding_curve_v2"
    );

    // SELL non-cashback
    let sell_nc_ix = dex
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            500_000,
            false,
        )
        .expect("build_sell_ix");
    assert_eq!(
        sell_nc_ix.accounts.last().unwrap().pubkey,
        expected_bonding_curve_v2,
        "SELL non-cashback: last account must be bonding_curve_v2"
    );

    // SELL cashback
    let sell_cb_ix = dex
        .build_sell_ix(
            &token_mint,
            &bonding_curve,
            &associated_bonding_curve,
            &user_token_account,
            &creator,
            &token_program,
            1_000_000,
            500_000,
            true,
        )
        .expect("build_sell_ix");
    assert_eq!(
        sell_cb_ix.accounts.last().unwrap().pubkey,
        expected_bonding_curve_v2,
        "SELL cashback: last account must be bonding_curve_v2"
    );
}

/// A.24: BondingCurveState::parse() liest cashback_enabled=true aus Byte 82.
#[test]
fn bonding_curve_state_parse_cashback_enabled_true() {
    let creator = Pubkey::new_unique();
    let mut data = vec![0u8; 151];
    data[0..8].copy_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data[8..16].copy_from_slice(&1_073_000_000_000_000u64.to_le_bytes());
    data[16..24].copy_from_slice(&30_000_000_000u64.to_le_bytes());
    data[24..32].copy_from_slice(&793_100_000_000_000u64.to_le_bytes());
    data[32..40].copy_from_slice(&0u64.to_le_bytes());
    data[40..48].copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
    data[48] = 0u8; // complete
    data[49..81].copy_from_slice(&creator.to_bytes());
    data[81] = 0u8; // reserved
    data[82] = 1u8; // cashback_enabled = TRUE

    let state = BondingCurveState::parse(&data).expect("parse");
    assert!(
        state.cashback_enabled,
        "cashback_enabled must be true when data[82]==1"
    );
}

/// A.24: BondingCurveState::parse() liest cashback_enabled=false aus Byte 82.
#[test]
fn bonding_curve_state_parse_cashback_enabled_false() {
    let creator = Pubkey::new_unique();
    let mut data = vec![0u8; 151];
    data[0..8].copy_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data[8..16].copy_from_slice(&1_073_000_000_000_000u64.to_le_bytes());
    data[16..24].copy_from_slice(&30_000_000_000u64.to_le_bytes());
    data[24..32].copy_from_slice(&793_100_000_000_000u64.to_le_bytes());
    data[32..40].copy_from_slice(&0u64.to_le_bytes());
    data[40..48].copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
    data[48] = 0u8;
    data[49..81].copy_from_slice(&creator.to_bytes());
    data[81] = 0u8;
    data[82] = 0u8; // cashback_enabled = FALSE

    let state = BondingCurveState::parse(&data).expect("parse");
    assert!(
        !state.cashback_enabled,
        "cashback_enabled must be false when data[82]==0"
    );
}

/// A.24: Altes Layout (81 Bytes) → cashback_enabled = false (default).
#[test]
fn bonding_curve_state_parse_old_layout_no_cashback() {
    let creator = Pubkey::new_unique();
    let mut data = vec![0u8; 81];
    data[0..8].copy_from_slice(&BONDING_CURVE_DISCRIMINATOR);
    data[8..16].copy_from_slice(&1_073_000_000_000_000u64.to_le_bytes());
    data[16..24].copy_from_slice(&30_000_000_000u64.to_le_bytes());
    data[24..32].copy_from_slice(&793_100_000_000_000u64.to_le_bytes());
    data[32..40].copy_from_slice(&0u64.to_le_bytes());
    data[40..48].copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
    data[48] = 0u8;
    data[49..81].copy_from_slice(&creator.to_bytes());

    let state = BondingCurveState::parse(&data).expect("parse");
    assert!(
        !state.cashback_enabled,
        "81-byte layout (no byte 82) must default cashback_enabled to false"
    );
}
