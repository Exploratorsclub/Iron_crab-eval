//! Invarianten: PumpSwap SELL Layout-Tier 26 vs 27 (INVARIANTS.md §A.3).
//!
//! Blackbox-Vertrag an der öffentlichen `ironcrab::solana::dex::pumpfun_amm`-API.
//! Build-Vertrag für 26er-Stuck-Pool folgt öffentlicher Impl-API nach P184j-Merge.

use ironcrab::solana::dex::pumpfun_amm::{
    pump_amm_inferred_sell_ix_account_count, pump_amm_sell_extended_fields_from_ix_accounts,
    pump_amm_sell_ix_uses_global_fee_at, PUMPFUN_AMM_BUILD_SWAP_FEE_CONFIG_STR,
    PUMPFUN_AMM_BUILD_SWAP_FEE_PROGRAM_STR, PUMPFUN_AMM_SELL_CASHBACK_TOTAL_ACCOUNTS,
    PUMPFUN_AMM_SELL_EXTENDED_V2_TOTAL_ACCOUNTS, PUMPFUN_AMM_SELL_EXT_TAIL_0_IX,
    PUMPFUN_AMM_SELL_EXT_TAIL_1_IX, PUMPFUN_AMM_SELL_EXT_THIRD_META_IX,
    PUMPFUN_AMM_SELL_FEE_TAIL_0_IX, PUMPFUN_AMM_SELL_FEE_TAIL_1_IX,
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const STUCK_POOL: &str = "GrgDaBg4TGBQCDZk9HHw8JT24RnoDHtQnvgguKxGKStb";
const STUCK_BASE_MINT: &str = "E2sHHwpzeVjhV3DjAMP8kYBeG27qT66xS3V9EBYVpump";
const GLOBAL_FEE_CONFIG: &str = "5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx";
const GLOBAL_FEE_PROGRAM: &str = "pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ";
const STUCK_EXT_TAIL_IX23: &str = "68gqomxAG38Sj8deqxmrctBUp6hr3Lh2eoqGEyGgrsKo";
const STUCK_THIRD_META: &str = "5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD";

fn global_fee_config() -> Pubkey {
    Pubkey::from_str(GLOBAL_FEE_CONFIG).expect("global fee_config")
}

fn global_fee_program() -> Pubkey {
    Pubkey::from_str(GLOBAL_FEE_PROGRAM).expect("global fee_program")
}

fn non_default_fill(len: usize) -> Vec<Pubkey> {
    (0..len).map(|_| Pubkey::new_unique()).collect()
}

fn fixture_26er_tier_ix_accounts() -> Vec<Pubkey> {
    let mut accounts = non_default_fill(26);
    accounts[19] = global_fee_config();
    accounts[20] = global_fee_program();
    accounts[PUMPFUN_AMM_SELL_EXT_TAIL_0_IX] = Pubkey::new_unique();
    accounts[PUMPFUN_AMM_SELL_EXT_TAIL_1_IX] = Pubkey::new_unique();
    accounts[PUMPFUN_AMM_SELL_EXT_THIRD_META_IX] =
        Pubkey::from_str(STUCK_EXT_TAIL_IX23).expect("stuck ext tail");
    accounts[PUMPFUN_AMM_SELL_FEE_TAIL_0_IX] = Pubkey::new_unique();
    accounts[PUMPFUN_AMM_SELL_FEE_TAIL_1_IX] =
        Pubkey::from_str(STUCK_THIRD_META).expect("stuck third_meta");
    accounts
}

#[test]
fn contract_pump_amm_inferred_account_count_26_vs_27() {
    assert_eq!(
        PUMPFUN_AMM_SELL_CASHBACK_TOTAL_ACCOUNTS, 26,
        "öffentliche API: 26er-Tier-Konstante"
    );
    assert_eq!(
        PUMPFUN_AMM_SELL_EXTENDED_V2_TOTAL_ACCOUNTS, 27,
        "öffentliche API: 27er-Tier-Konstante"
    );

    assert_eq!(
        pump_amm_inferred_sell_ix_account_count(false, true, true),
        26,
        "26er: pre_fee=false, fee_tail=true, extended=true"
    );
    assert_eq!(
        pump_amm_inferred_sell_ix_account_count(true, true, true),
        27,
        "27er: pre_fee=true, fee_tail=true, extended=true"
    );
}

#[test]
fn contract_pump_amm_global_fee_meta_indices() {
    assert_eq!(
        pump_amm_sell_ix_uses_global_fee_at(26),
        Some((19, 20)),
        "26er-Tier: globale fee_config/fee_program an 0-based 19/20"
    );
    assert_eq!(
        pump_amm_sell_ix_uses_global_fee_at(27),
        Some((21, 22)),
        "27er-Tier: globale fee_config/fee_program an 0-based 21/22"
    );
}

#[test]
fn contract_pump_amm_decode_26er_ix_accounts_tier() {
    let accounts = fixture_26er_tier_ix_accounts();
    assert_eq!(accounts.len(), 26);

    let fields = pump_amm_sell_extended_fields_from_ix_accounts(&accounts)
        .expect("26-account observed sell must decode");

    assert!(
        !fields.requires_pre_fee_metas,
        "26er-Tier darf kein separates Pre-Fee-Meta-Paar vor globalen Fee-Metas haben"
    );
    assert!(fields.pre_fee_meta_0.is_none());
    assert!(fields.pre_fee_meta_1.is_none());

    let (fee_config_ix, fee_program_ix) =
        pump_amm_sell_ix_uses_global_fee_at(26).expect("26er global fee indices");
    assert_eq!(accounts[fee_config_ix], global_fee_config());
    assert_eq!(accounts[fee_program_ix], global_fee_program());

    assert_eq!(
        fields.tail_0,
        Some(accounts[PUMPFUN_AMM_SELL_EXT_TAIL_0_IX])
    );
    assert_eq!(
        fields.tail_1,
        Some(accounts[PUMPFUN_AMM_SELL_EXT_TAIL_1_IX])
    );
    assert_eq!(
        fields.third_meta,
        Some(Pubkey::from_str(STUCK_EXT_TAIL_IX23).unwrap())
    );
    assert_eq!(
        fields.fee_tail_0,
        Some(accounts[PUMPFUN_AMM_SELL_FEE_TAIL_0_IX])
    );
    assert_eq!(
        fields.fee_tail_1,
        Some(accounts[PUMPFUN_AMM_SELL_FEE_TAIL_1_IX])
    );

    assert_eq!(
        PUMPFUN_AMM_BUILD_SWAP_FEE_CONFIG_STR, GLOBAL_FEE_CONFIG,
        "öffentliche Build-String-Konstante muss Stuck-Pool global fee_config sein"
    );
    assert_eq!(
        PUMPFUN_AMM_BUILD_SWAP_FEE_PROGRAM_STR, GLOBAL_FEE_PROGRAM,
        "öffentliche Build-String-Konstante muss Stuck-Pool global fee_program sein"
    );
}

#[test]
fn contract_pump_amm_stuck_pool_26er_build_swap_ix() {
    let _pool = Pubkey::from_str(STUCK_POOL).expect("stuck pool");
    let _base = Pubkey::from_str(STUCK_BASE_MINT).expect("stuck base mint");

    assert_eq!(
        pump_amm_inferred_sell_ix_account_count(false, true, true),
        26,
        "Stuck-Pool-Szenario: 26er-Tier-Ableitung"
    );
    assert_eq!(
        pump_amm_sell_ix_uses_global_fee_at(26),
        Some((19, 20)),
        "Stuck-Pool-Szenario: globale Fee-Metas @19/20"
    );
    assert_eq!(
        Pubkey::from_str(PUMPFUN_AMM_BUILD_SWAP_FEE_CONFIG_STR).unwrap(),
        global_fee_config()
    );
    assert_eq!(
        Pubkey::from_str(PUMPFUN_AMM_BUILD_SWAP_FEE_PROGRAM_STR).unwrap(),
        global_fee_program()
    );

    // Öffentlicher 26er-Build über build_swap_ix_from_pool_accounts folgt Impl P184j;
    // bis dahin Index-Vertrag + inferred_count (siehe PR-Body).
}
