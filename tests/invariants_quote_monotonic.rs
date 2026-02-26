//! Invariante: quote_monotonic
//!
//! Größeres amount_in führt zu mindestens gleichem amount_out bei quote_output_amount.

use ironcrab::execution::live_pool_cache::{CachedPoolState, PumpAmmState};
use ironcrab::execution::quote_calculator::quote_output_amount;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

fn setup_pump_amm_state() -> CachedPoolState {
    let base_mint = Pubkey::new_from_array([2u8; 32]);
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    CachedPoolState::PumpAmm(PumpAmmState {
        base_mint,
        quote_mint,
        pool_base_token_account: Pubkey::new_from_array([3u8; 32]),
        pool_quote_token_account: Pubkey::new_from_array([4u8; 32]),
        base_reserve: Some(1_000_000_000),
        quote_reserve: Some(100_000_000),
        pool_accounts: (0..14).map(|_| Pubkey::new_unique()).collect(),
        creator: None,
    })
}

#[test]
fn quote_monotonic() {
    let state = setup_pump_amm_state();
    let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();

    let out_1 = quote_output_amount(&state, 1_000_000, &quote_mint).unwrap();
    let out_2 = quote_output_amount(&state, 10_000_000, &quote_mint).unwrap();
    let out_3 = quote_output_amount(&state, 50_000_000, &quote_mint).unwrap();

    assert!(
        out_2 >= out_1,
        "größeres amount_in muss mindestens gleiches amount_out liefern"
    );
    assert!(
        out_3 >= out_2,
        "größeres amount_in muss mindestens gleiches amount_out liefern"
    );
}
