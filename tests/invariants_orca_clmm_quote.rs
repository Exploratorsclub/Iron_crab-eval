//! Invariante A.54: Orca Arb-V2 ExecutableMarginal = Whirlpool tick walk, nicht Vault-x·y=k.
//!
//! - `orca_quote_exact_in` nutzt gecachte Tick-Arrays + `sqrt_price` / `liquidity` / `tick_current_index` / `fee_rate`
//! - Unvollständiges 3-Array-Fenster in Swap-Richtung → `None` (kein k-Fallback)
//! - `supports_cpmm` darf `"orca"` nicht enthalten
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Blackbox API (`orca_quote_exact_in`, `orca_tick_array` parse/build) + dokumentierter Source-Grep-Gate.

use ironcrab::solana::dex::orca_tick_array::{
    build_tick_array_account_bytes, parse_tick_array, swap_direction_tick_array_starts,
    TICK_ARRAY_SIZE,
};
use ironcrab::solana::dex::orca_tick_walker::{
    orca_quote_exact_in, OrcaTickArrays, OrcaWhirlpoolQuoteInput,
};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const FEE_RATE_MUL: u128 = 1_000_000;

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn iron_crab_pool_quote_rs() -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("arbitrage")
        .join("pool_quote.rs")
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let path = iron_crab_pool_quote_rs();
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder pool_quote.rs nicht lesbar unter {:?}",
            iron_crab_root()
        );
        return None;
    }
    Some(iron_crab_root())
}

fn production_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod ") {
        return &source[..idx];
    }
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source section")
}

fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needle = format!("fn {fn_name}(");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("expected fn {fn_name}( in pool_quote.rs"));
    let brace_start = source[start..]
        .find('{')
        .map(|i| start + i)
        .expect("expected opening brace for fn block");
    let mut depth = 0usize;
    let mut end = brace_start;
    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = brace_start + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(end > brace_start, "unclosed fn block for {fn_name}");
    source[start..end].to_string()
}

/// Vault-CPMM xy=k nur als Vergleichszahl im Test (kein `pool_quote::cpmm_amount_out`).
fn local_vault_k_amount_out(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    fee_rate: u16,
) -> u64 {
    let after_fee =
        (amount_in as u128).saturating_mul(FEE_RATE_MUL - fee_rate as u128) / FEE_RATE_MUL;
    if after_fee == 0 || reserve_in == 0 {
        return 0;
    }
    let num = after_fee * reserve_out as u128;
    let den = reserve_in as u128 + after_fee;
    (num / den) as u64
}

fn insert_three_swap_arrays(
    pool: &OrcaWhirlpoolQuoteInput,
    tick_now: i32,
    a_to_b: bool,
    tick_updates: &[(i32, bool, i128)],
) -> OrcaTickArrays {
    let (s0, s1, s2) = swap_direction_tick_array_starts(tick_now, pool.tick_spacing as i32, a_to_b);
    let mut map: OrcaTickArrays = HashMap::new();
    for start in [s0, s1, s2] {
        let bytes =
            build_tick_array_account_bytes(start, pool.pool, pool.tick_spacing, tick_updates);
        let parsed = parse_tick_array(&bytes).expect("parse tick array");
        assert_eq!(parsed.ticks.len(), TICK_ARRAY_SIZE as usize);
        map.insert(start, parsed);
    }
    map
}

/// Whirlpool Q64.64 sqrt price at tick 0 (price = 1.0).
fn sqrt_price_tick_zero() -> u128 {
    1u128 << 64
}

/// A.54: Tick-Walk-Output liegt unter Vault-k bei grossen Reserve-Fixturen (Walk > 0).
#[test]
fn orca_tick_walk_output_below_vault_k_with_concentrated_liquidity() {
    let pool_pk = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let tick = 0i32;
    let fee_rate = 3000u16; // Hundertstel eines bps → 0,30 %
    let liquidity = 800_000_000_000u128;
    let pool = OrcaWhirlpoolQuoteInput {
        pool: pool_pk,
        token_mint_a: mint_a,
        token_mint_b: mint_b,
        sqrt_price: sqrt_price_tick_zero(),
        liquidity,
        tick_current_index: tick,
        tick_spacing: 64,
        fee_rate,
    };

    let init_tick = tick - pool.tick_spacing as i32;
    let ticks =
        insert_three_swap_arrays(&pool, tick, true, &[(init_tick, true, liquidity as i128)]);

    let vault_reserve_a = 10_000_000_000_000u64;
    let vault_reserve_b = 10_000_000_000_000u64;
    let amount_in = 25_000_000u64;

    let walk = orca_quote_exact_in(
        &pool,
        &ticks,
        &mint_a.to_string(),
        &mint_b.to_string(),
        amount_in,
    )
    .expect("vollstaendiges Tick-Fenster + konzentrierte Liquiditaet muss Walk liefern");

    let k_out = local_vault_k_amount_out(amount_in, vault_reserve_a, vault_reserve_b, fee_rate);

    assert!(walk > 0, "Walk muss positiv sein");
    assert!(
        walk < k_out,
        "A.54: tick walk ({walk}) muss unter Vault-k ({k_out}) liegen, nicht cpmm_amount_out-Fallback"
    );
}

/// A.54: Leere oder unvollstaendige Tick-Map → `None`.
#[test]
fn orca_quote_none_when_tick_map_incomplete() {
    let pool_pk = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let tick = 128i32;
    let pool = OrcaWhirlpoolQuoteInput {
        pool: pool_pk,
        token_mint_a: mint_a,
        token_mint_b: mint_b,
        sqrt_price: sqrt_price_tick_zero(),
        liquidity: 1_000_000_000_000,
        tick_current_index: tick,
        tick_spacing: 64,
        fee_rate: 300,
    };
    let mint_a_s = mint_a.to_string();
    let mint_b_s = mint_b.to_string();
    let amount_in = 1_000_000u64;

    let empty: OrcaTickArrays = HashMap::new();
    assert!(
        orca_quote_exact_in(&pool, &empty, &mint_a_s, &mint_b_s, amount_in).is_none(),
        "leere OrcaTickArrays muss None liefern"
    );

    let (s0, _s1, _s2) = swap_direction_tick_array_starts(tick, pool.tick_spacing as i32, true);
    let bytes = build_tick_array_account_bytes(s0, pool.pool, pool.tick_spacing, &[]);
    let parsed = parse_tick_array(&bytes).expect("parse");
    let mut partial: OrcaTickArrays = HashMap::new();
    partial.insert(s0, parsed);
    assert!(
        orca_quote_exact_in(&pool, &partial, &mint_a_s, &mint_b_s, amount_in).is_none(),
        "nur ein Array im 3-Array-Swap-Fenster muss None liefern"
    );
}

/// A.54: `supports_cpmm` Match-Arm darf `"orca"` nicht enthalten.
#[test]
fn orca_not_in_supports_cpmm_grep() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let path = iron_crab_pool_quote_rs();
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let prod = production_source(&source);
    let body = extract_fn_block(prod, "supports_cpmm");

    assert!(
        !body.contains("\"orca\""),
        "supports_cpmm darf orca nicht als Vault-CPMM behandeln (A.54); fn block: {body}"
    );
}
