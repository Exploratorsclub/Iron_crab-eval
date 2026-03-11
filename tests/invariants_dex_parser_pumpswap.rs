//! Invariante A.20: DEX Parser PumpSwap BUY/SELL (INVARIANTS.md §A.20)
//!
//! parse_transaction_update() parst sowohl BUY (23 Accounts) als auch SELL (21 Accounts) korrekt.
//! KNOWN_BUG_PATTERNS #14; Guard-Check war != 23, jetzt < 21.

use ironcrab::solana::dex_parser::{
    parse_transaction_update, DexType, ParsedDexEvent, PUMPFUN_AMM_PROGRAM,
};
use ironcrab::solana::geyser_listener::{GeyserTransactionUpdate, TokenAmount, TokenBalance};
use solana_sdk::hash::hash;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Anchor discriminator für PumpSwap buy_exact_quote_in (Sha256 von 'global:buy_exact_quote_in', erste 8 Bytes)
fn buy_discriminator() -> [u8; 8] {
    let out = hash(b"global:buy_exact_quote_in");
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&out.as_ref()[..8]);
    disc
}

/// Anchor discriminator für PumpSwap sell (Sha256 von 'global:sell', erste 8 Bytes)
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

/// Erstellt 23 instruction_accounts für BUY (Pool-Market, User, GlobalConfig, BaseMint, QuoteMint, ...)
fn build_buy_accounts(trader: Pubkey) -> Vec<Pubkey> {
    let accounts = vec![
        Pubkey::new_unique(), // pool_market [0]
        trader,               // user [1]
        Pubkey::new_unique(), // global_config [2]
        Pubkey::new_unique(), // base_mint [3]
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(), // quote_mint [4]
        Pubkey::new_unique(), // user_base_ta [5]
        Pubkey::new_unique(), // user_quote_ta [6]
        Pubkey::new_unique(), // pool_base_vault [7]
        Pubkey::new_unique(), // pool_quote_vault [8]
        Pubkey::new_unique(), // protocol_fee_recipient [9]
        Pubkey::new_unique(), // protocol_fee_recipient_ta [10]
        Pubkey::new_unique(), // spl_token [11]
        Pubkey::new_unique(), // spl_token [12]
        Pubkey::new_unique(), // system [13]
        Pubkey::new_unique(), // ata_program [14]
        Pubkey::new_unique(), // event_authority [15]
        Pubkey::new_unique(), // global_volume_accumulator [16] - BUY only
        Pubkey::new_unique(), // coin_creator_vault_ata [17]
        Pubkey::new_unique(), // coin_creator_vault_authority [18]
        Pubkey::new_unique(), // user_volume [19] - BUY only
        Pubkey::new_unique(), // fee_config [20]
        Pubkey::new_unique(), // fee_program [21]
        Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap(), // program_id [22]
    ];
    assert_eq!(accounts.len(), 23);
    accounts
}

/// Erstellt 21 instruction_accounts für SELL (ohne volume accumulators)
fn build_sell_accounts(trader: Pubkey) -> Vec<Pubkey> {
    let accounts = vec![
        Pubkey::new_unique(), // pool_market [0]
        trader,               // user [1]
        Pubkey::new_unique(), // global_config [2]
        Pubkey::new_unique(), // base_mint [3]
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(), // quote_mint [4]
        Pubkey::new_unique(), // user_base_ta [5]
        Pubkey::new_unique(), // user_quote_ta [6]
        Pubkey::new_unique(), // pool_base_vault [7]
        Pubkey::new_unique(), // pool_quote_vault [8]
        Pubkey::new_unique(), // protocol_fee_recipient [9]
        Pubkey::new_unique(), // protocol_fee_recipient_ta [10]
        Pubkey::new_unique(), // spl_token [11]
        Pubkey::new_unique(), // spl_token [12]
        Pubkey::new_unique(), // system [13]
        Pubkey::new_unique(), // ata_program [14]
        Pubkey::new_unique(), // event_authority [15]
        Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap(), // program_id [16]
        Pubkey::new_unique(), // coin_creator_vault_ata [17]
        Pubkey::new_unique(), // coin_creator_vault_authority [18]
        Pubkey::new_unique(), // fee_config [19]
        Pubkey::new_unique(), // fee_program [20]
    ];
    assert_eq!(accounts.len(), 21);
    accounts
}

#[test]
fn pumpswap_buy_23_accounts_parsed() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();

    let mut instruction_accounts = build_buy_accounts(trader);
    instruction_accounts[3] = base_mint;

    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&buy_discriminator());
    instruction_data.extend_from_slice(&1_000_000u64.to_le_bytes()); // amount_in
    instruction_data.extend_from_slice(&100_000u64.to_le_bytes()); // min_out

    let mut account_keys = vec![trader];
    account_keys.extend(instruction_accounts.iter().cloned());
    if !account_keys.contains(&pumpfun_amm) {
        account_keys.push(pumpfun_amm);
    }

    let update = GeyserTransactionUpdate {
        signature: "pumpswap_buy_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts,
        instruction_data,
        inner_instructions: vec![],
        pre_token_balances: vec![token_balance(1, &base_mint, 0, 6)],
        post_token_balances: vec![token_balance(1, &base_mint, 50_000, 6)],
        pre_balances: vec![10_000_000, 0],
        post_balances: vec![9_000_000, 0],
        fee_lamports: 5000,
        compute_units_consumed: None,
    };

    let parsed =
        parse_transaction_update(&update).expect("BUY mit 23 Accounts muss geparst werden");

    match &parsed {
        ParsedDexEvent::Trade {
            is_buy,
            dex,
            trader: t,
            ..
        } => {
            assert!(*is_buy, "BUY muss is_buy=true liefern");
            assert_eq!(*dex, DexType::PumpFunAmm);
            assert_eq!(*t, trader);
        }
        _ => panic!("Erwartet Trade, erhalten: {:?}", parsed),
    }
}

#[test]
fn pumpswap_sell_21_accounts_parsed() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();

    let mut instruction_accounts = build_sell_accounts(trader);
    instruction_accounts[3] = base_mint;

    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&sell_discriminator());
    instruction_data.extend_from_slice(&200_000u64.to_le_bytes()); // amount_in (tokens)
    instruction_data.extend_from_slice(&50_000u64.to_le_bytes()); // min_out

    let mut account_keys = vec![trader];
    account_keys.extend(instruction_accounts.iter().cloned());
    if !account_keys.contains(&pumpfun_amm) {
        account_keys.push(pumpfun_amm);
    }

    let update = GeyserTransactionUpdate {
        signature: "pumpswap_sell_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts,
        instruction_data,
        inner_instructions: vec![],
        pre_token_balances: vec![token_balance(1, &base_mint, 300_000, 6)],
        post_token_balances: vec![token_balance(1, &base_mint, 100_000, 6)],
        pre_balances: vec![5_000_000, 0],
        post_balances: vec![7_000_000, 0],
        fee_lamports: 5000,
        compute_units_consumed: None,
    };

    let parsed =
        parse_transaction_update(&update).expect("SELL mit 21 Accounts muss geparst werden");

    match &parsed {
        ParsedDexEvent::Trade { is_buy, dex, .. } => {
            assert!(!*is_buy, "SELL muss is_buy=false liefern");
            assert_eq!(*dex, DexType::PumpFunAmm);
        }
        _ => panic!("Erwartet Trade, erhalten: {:?}", parsed),
    }
}

#[test]
fn pumpswap_insufficient_accounts_rejected() {
    let trader = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();

    let instruction_accounts: Vec<Pubkey> = (0..20).map(|_| Pubkey::new_unique()).collect();
    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&buy_discriminator());
    instruction_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    instruction_data.extend_from_slice(&100_000u64.to_le_bytes());

    let mut account_keys = vec![trader];
    account_keys.extend(instruction_accounts.iter().cloned());
    account_keys.push(pumpfun_amm);

    let update = GeyserTransactionUpdate {
        signature: "pumpswap_insufficient_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts,
        instruction_data,
        inner_instructions: vec![],
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
        "<21 Accounts muss None liefern, erhalten: {:?}",
        parsed
    );
}

#[test]
fn pumpswap_unknown_discriminator_rejected() {
    let trader = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();
    let instruction_accounts = build_buy_accounts(trader);

    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&[0u8; 8]); // falscher Discriminator
    instruction_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    instruction_data.extend_from_slice(&100_000u64.to_le_bytes());

    let mut account_keys = vec![trader];
    account_keys.extend(instruction_accounts.iter().cloned());
    account_keys.push(pumpfun_amm);

    let update = GeyserTransactionUpdate {
        signature: "pumpswap_unknown_disc_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts,
        instruction_data,
        inner_instructions: vec![],
        pre_token_balances: vec![],
        post_token_balances: vec![],
        pre_balances: vec![],
        post_balances: vec![],
        fee_lamports: 0,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update);
    let is_trade = matches!(parsed, Some(ParsedDexEvent::Trade { .. }));
    assert!(
        !is_trade,
        "Falscher Discriminator darf keinen Trade liefern, erhalten: {:?}",
        parsed
    );
}

#[test]
fn pumpswap_pool_accounts_have_14_elements() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();

    let mut instruction_accounts = build_buy_accounts(trader);
    instruction_accounts[3] = base_mint;

    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&buy_discriminator());
    instruction_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    instruction_data.extend_from_slice(&100_000u64.to_le_bytes());

    let mut account_keys = vec![trader];
    account_keys.extend(instruction_accounts.iter().cloned());
    account_keys.push(pumpfun_amm);

    let update = GeyserTransactionUpdate {
        signature: "pumpswap_pool_accts_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts,
        instruction_data,
        inner_instructions: vec![],
        pre_token_balances: vec![token_balance(1, &base_mint, 0, 6)],
        post_token_balances: vec![token_balance(1, &base_mint, 50_000, 6)],
        pre_balances: vec![10_000_000, 0],
        post_balances: vec![9_000_000, 0],
        fee_lamports: 0,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update).expect("BUY muss geparst werden");
    if let ParsedDexEvent::Trade {
        pool_accounts: Some(pa),
        ..
    } = parsed
    {
        assert_eq!(pa.len(), 14, "pool_accounts muss 14 Elemente haben");
    } else {
        panic!("Erwartet Trade mit pool_accounts");
    }

    let mut sell_accounts = build_sell_accounts(trader);
    sell_accounts[3] = base_mint;
    let mut instruction_data_sell = Vec::with_capacity(24);
    instruction_data_sell.extend_from_slice(&sell_discriminator());
    instruction_data_sell.extend_from_slice(&200_000u64.to_le_bytes());
    instruction_data_sell.extend_from_slice(&50_000u64.to_le_bytes());

    let mut account_keys_sell = vec![trader];
    account_keys_sell.extend(sell_accounts.iter().cloned());
    account_keys_sell.push(pumpfun_amm);

    let update_sell = GeyserTransactionUpdate {
        signature: "pumpswap_sell_pool_accts_sig".to_string(),
        slot: 1,
        account_keys: account_keys_sell,
        instruction_accounts: sell_accounts,
        instruction_data: instruction_data_sell,
        inner_instructions: vec![],
        pre_token_balances: vec![token_balance(1, &base_mint, 300_000, 6)],
        post_token_balances: vec![token_balance(1, &base_mint, 100_000, 6)],
        pre_balances: vec![5_000_000, 0],
        post_balances: vec![7_000_000, 0],
        fee_lamports: 0,
        compute_units_consumed: None,
    };

    let parsed_sell = parse_transaction_update(&update_sell).expect("SELL muss geparst werden");
    if let ParsedDexEvent::Trade {
        pool_accounts: Some(pa),
        ..
    } = parsed_sell
    {
        assert_eq!(pa.len(), 14, "SELL pool_accounts muss 14 Elemente haben");
    } else {
        panic!("Erwartet Trade mit pool_accounts");
    }
}

#[test]
fn pumpswap_buy_uses_correct_trader() {
    let trader = Pubkey::new_unique();
    let base_mint = Pubkey::new_unique();
    let pumpfun_amm = Pubkey::from_str(PUMPFUN_AMM_PROGRAM).unwrap();

    let mut instruction_accounts = build_buy_accounts(trader);
    instruction_accounts[3] = base_mint;

    let mut instruction_data = Vec::with_capacity(24);
    instruction_data.extend_from_slice(&buy_discriminator());
    instruction_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    instruction_data.extend_from_slice(&100_000u64.to_le_bytes());

    let mut account_keys = vec![trader];
    account_keys.extend(instruction_accounts.iter().cloned());
    account_keys.push(pumpfun_amm);

    let update = GeyserTransactionUpdate {
        signature: "pumpswap_trader_sig".to_string(),
        slot: 1,
        account_keys,
        instruction_accounts,
        instruction_data,
        inner_instructions: vec![],
        pre_token_balances: vec![token_balance(1, &base_mint, 0, 6)],
        post_token_balances: vec![token_balance(1, &base_mint, 50_000, 6)],
        pre_balances: vec![10_000_000, 0],
        post_balances: vec![9_000_000, 0],
        fee_lamports: 0,
        compute_units_consumed: None,
    };

    let parsed = parse_transaction_update(&update).expect("BUY muss geparst werden");
    match &parsed {
        ParsedDexEvent::Trade { trader: t, .. } => {
            assert_eq!(*t, trader, "trader muss account_keys[0] sein");
        }
        _ => panic!("Erwartet Trade"),
    }
}
