//! Invariante A.21: DEX Parser Inner-Instruction Fallback (INVARIANTS.md §A.21)
//!
//! parse_transaction_update() erkennt DEX-Trades auch wenn sie als Inner Instruction (CPI) ausgeführt werden.
//! Aggregator-Trades (Jupiter, etc.) rufen DEX-Programme als CPI auf.

use ironcrab::solana::dex_parser::{
    parse_transaction_update, DexType, ParsedDexEvent, PUMPFUN_AMM_PROGRAM, RAYDIUM_AMM_V4,
};
use ironcrab::solana::geyser_listener::{
    GeyserTransactionUpdate, InnerInstruction, TokenAmount, TokenBalance,
};
use solana_sdk::hash::hash;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

fn buy_discriminator() -> [u8; 8] {
    let out = hash(b"global:buy_exact_quote_in");
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&out.as_ref()[..8]);
    disc
}

fn sell_discriminator() -> [u8; 8] {
    let out = hash(b"global:sell");
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&out.as_ref()[..8]);
    disc
}

fn token_balance(account_index: u8, mint: &Pubkey, amount: u64, decimals: u8) -> TokenBalance {
    TokenBalance {
        account_index,
        mint: mint.to_string(),
        ui_token_amount: TokenAmount {
            ui_amount: Some(amount as f64 / 10f64.powi(decimals as i32)),
            decimals,
            amount: amount.to_string(),
        },
        program_id: Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string()),
    }
}

/// 23 Accounts für PumpSwap BUY
fn build_buy_accounts(trader: Pubkey) -> Vec<Pubkey> {
    vec![
        Pubkey::new_unique(),
        trader,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
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
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap(),
    ]
}

/// 21 Accounts für PumpSwap SELL
fn build_sell_accounts(trader: Pubkey) -> Vec<Pubkey> {
    vec![
        Pubkey::new_unique(),
        trader,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
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
        Pubkey::new_unique(),
        Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    ]
}

#[test]
fn cpi_trade_parsed_from_inner_instruction() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();
    let aggregator_program = Pubkey::new_unique();

    let mut buy_accounts = build_buy_accounts(trader);
    buy_accounts[3] = base_mint;

    let mut account_keys = vec![trader, aggregator_program, pumpfun_amm];
    account_keys.extend(buy_accounts.iter().cloned());

    let program_id_index = 2u8;
    let accounts_indices: Vec<u8> = (3..26).map(|i| i as u8).collect();

    let mut inner_data = Vec::with_capacity(24);
    inner_data.extend_from_slice(&buy_discriminator());
    inner_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    inner_data.extend_from_slice(&100_000u64.to_le_bytes());

    let inner = InnerInstruction {
        program_id_index,
        accounts: accounts_indices,
        data: inner_data,
    };

    let update = GeyserTransactionUpdate {
        signature: "cpi_buy_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts: vec![Pubkey::new_unique(); 5],
        instruction_data: vec![0u8; 8],
        inner_instructions: vec![inner],
        pre_token_balances: vec![token_balance(0, &base_mint, 0, 6)],
        post_token_balances: vec![token_balance(0, &base_mint, 50_000, 6)],
        pre_balances: vec![10_000_000, 0],
        post_balances: vec![9_000_000, 0],
        fee_lamports: 5000,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update)
        .expect("CPI BUY aus Inner Instruction muss geparst werden");

    match &parsed {
        ParsedDexEvent::Trade { dex, is_buy, .. } => {
            assert_eq!(*dex, DexType::PumpFunAmm);
            assert!(*is_buy);
        }
        _ => panic!("Erwartet Trade, erhalten: {:?}", parsed),
    }
}

#[test]
fn top_level_takes_priority_over_inner() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let _pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();
    let raydium = Pubkey::from_str(RAYDIUM_AMM_V4).unwrap();

    let mut buy_accounts = build_buy_accounts(trader);
    buy_accounts[3] = base_mint;

    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&buy_discriminator());
    instruction_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    instruction_data.extend_from_slice(&100_000u64.to_le_bytes());

    let mut account_keys = vec![trader];
    account_keys.extend(buy_accounts.iter().cloned());
    account_keys.push(raydium);

    let inner_data_raydium = vec![9u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let raydium_inner = InnerInstruction {
        program_id_index: (account_keys.len() - 1) as u8,
        accounts: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        data: inner_data_raydium,
    };

    let update = GeyserTransactionUpdate {
        signature: "top_level_priority_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts: buy_accounts,
        instruction_data,
        inner_instructions: vec![raydium_inner],
        pre_token_balances: vec![token_balance(1, &base_mint, 0, 6)],
        post_token_balances: vec![token_balance(1, &base_mint, 50_000, 6)],
        pre_balances: vec![10_000_000, 0],
        post_balances: vec![9_000_000, 0],
        fee_lamports: 0,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update).expect("Top-Level PumpSwap muss geparst werden");

    match &parsed {
        ParsedDexEvent::Trade { dex, .. } => {
            assert_eq!(
                *dex,
                DexType::PumpFunAmm,
                "Top-Level PumpFunAmm hat Priorität"
            );
        }
        _ => panic!("Erwartet Trade, erhalten: {:?}", parsed),
    }
}

#[test]
fn cpi_sell_parsed_from_inner_instruction() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();
    let aggregator_program = Pubkey::new_unique();

    let mut sell_accounts = build_sell_accounts(trader);
    sell_accounts[3] = base_mint;

    let mut account_keys = vec![trader, aggregator_program, pumpfun_amm];
    account_keys.extend(sell_accounts.iter().cloned());

    let program_id_index = 2u8;
    let accounts_indices: Vec<u8> = (3..24).map(|i| i as u8).collect();

    let mut inner_data = Vec::with_capacity(24);
    inner_data.extend_from_slice(&sell_discriminator());
    inner_data.extend_from_slice(&200_000u64.to_le_bytes());
    inner_data.extend_from_slice(&50_000u64.to_le_bytes());

    let inner = InnerInstruction {
        program_id_index,
        accounts: accounts_indices,
        data: inner_data,
    };

    let update = GeyserTransactionUpdate {
        signature: "cpi_sell_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts: vec![Pubkey::new_unique(); 5],
        instruction_data: vec![0u8; 8],
        inner_instructions: vec![inner],
        pre_token_balances: vec![token_balance(0, &base_mint, 300_000, 6)],
        post_token_balances: vec![token_balance(0, &base_mint, 100_000, 6)],
        pre_balances: vec![5_000_000, 0],
        post_balances: vec![7_000_000, 0],
        fee_lamports: 5000,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update)
        .expect("CPI SELL aus Inner Instruction muss geparst werden");

    match &parsed {
        ParsedDexEvent::Trade { dex, is_buy, .. } => {
            assert_eq!(*dex, DexType::PumpFunAmm);
            assert!(!*is_buy);
        }
        _ => panic!("Erwartet Trade, erhalten: {:?}", parsed),
    }
}

#[test]
fn no_matching_inner_instruction_returns_none() {
    let trader = Pubkey::new_unique();
    let unknown_program = Pubkey::new_unique();

    let account_keys = vec![trader, unknown_program];

    let inner = InnerInstruction {
        program_id_index: 1,
        accounts: vec![0],
        data: vec![0u8; 8],
    };

    let update = GeyserTransactionUpdate {
        signature: "no_match_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts: vec![Pubkey::new_unique()],
        instruction_data: vec![0u8; 8],
        inner_instructions: vec![inner],
        pre_token_balances: vec![],
        post_token_balances: vec![],
        pre_balances: vec![],
        post_balances: vec![],
        fee_lamports: 0,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update);
    assert!(
        parsed.is_none(),
        "Keine bekannten DEX-Programme in Innern → None, erhalten: {:?}",
        parsed
    );
}
