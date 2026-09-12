//! Invariante A.48 / A.1: DLMM constant-price bin walk — nicht Mini-CPMM xy=k pro Bin.
//!
//! - Preis: `P = (1 + bin_step/10000)^bin_id` in Q64.64
//! - X→Y: `floor(amount_in_after_fee * P / 2^64)`, Cap `bin.amount_y`, Walk −1
//! - Y→X: `floor(amount_in_after_fee * 2^64 / P)`, Cap `bin.amount_x`, Walk +1
//! - Einseitige Bins: leere Output-Seite überspringen, nicht ganze Bin droppen
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Blackbox API (`quote_exact_in`, `dlmm_*_from_bins`, `meteora_bin_walker`) + Source-Grep-Gate.

use ironcrab::arbitrage::pool_quote::{
    dlmm_token_output_from_bins, quote_exact_in, DlmmBinArrays, QuoteKind, QuotePoolInput,
    QuoteVaultInput, NATIVE_SOL_MINT,
};
use ironcrab::ipc::BinData;
use ironcrab::solana::dex::meteora_bin_walker::{dlmm_fee_bps, walker_from_bins};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn sample_pool(address: &str, token_mint: &str) -> QuotePoolInput {
    QuotePoolInput {
        pool_address: address.to_string(),
        dex: "meteora_dlmm".to_string(),
        token_mint: token_mint.to_string(),
        trade_price_buy: None,
        trade_price_sell: None,
        trade_updated_at: Instant::now(),
        has_reserve_data: true,
        token_decimals: 6,
    }
}

fn dlmm_vault(
    token_amount: u64,
    sol_amount: u64,
    active_id: i32,
    bin_step: u16,
    token_mint: &str,
) -> QuoteVaultInput {
    QuoteVaultInput {
        reserve_base: token_amount,
        reserve_quote: sol_amount,
        update_slot: 1,
        updated_at: Instant::now(),
        active_id: Some(active_id),
        bin_step: Some(bin_step),
        dlmm_sol_is_x: false,
        dlmm_token_x_mint: Some(token_mint.to_string()),
    }
}

fn single_active_bin(active_id: i32, amount_x: u64, amount_y: u64) -> DlmmBinArrays {
    let array_index = active_id as i64 / 70;
    let offset = (active_id - array_index as i32 * 70) as u8;
    let mut bins: DlmmBinArrays = HashMap::new();
    bins.insert(
        array_index,
        vec![BinData {
            offset,
            amount_x,
            amount_y,
        }],
    );
    bins
}

fn flat_bins_from_arrays(bins: &DlmmBinArrays) -> Vec<(i32, u64, u64)> {
    let mut flat = Vec::new();
    for (array_idx, entries) in bins {
        for bin in entries {
            let bin_id = (*array_idx * 70 + bin.offset as i64) as i32;
            flat.push((bin_id, bin.amount_x, bin.amount_y));
        }
    }
    flat.sort_by_key(|(id, _, _)| *id);
    flat
}

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn iron_crab_walker_rs() -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("solana")
        .join("dex")
        .join("meteora_bin_walker.rs")
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let path = iron_crab_walker_rs();
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder meteora_bin_walker.rs nicht lesbar unter {:?}",
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

/// A.48: Einseitiger aktiver Bin (amount_x=0, amount_y>0) — früher Mini-CPMM skippte amount_x==0 || amount_y==0.
#[test]
fn dlmm_one_sided_active_bin_y_liquidity_quotes_sol_to_token() {
    let active_id = 0i32;
    let bin_step = 100u16;
    let token_mint = "TokenMint11111111111111111111111111111111";
    let pool = sample_pool("dlmm_one_sided", token_mint);
    // SOL = X, Token = Y; nur Y-Liquidität (Output-Seite für X→Y) im aktiven Bin.
    let amount_y = 100_000_000_000u64;
    let token_reserve = 1_000_000_000_000u64;
    let sol_reserve = 1_000_000_000_000u64;
    let vault = QuoteVaultInput {
        reserve_base: token_reserve,
        reserve_quote: sol_reserve,
        update_slot: 1,
        updated_at: Instant::now(),
        active_id: Some(active_id),
        bin_step: Some(bin_step),
        dlmm_sol_is_x: true,
        dlmm_token_x_mint: Some(NATIVE_SOL_MINT.to_string()),
    };
    let bins = single_active_bin(active_id, 0, amount_y);

    let amount_in = 1_000_000u64;
    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        Some(&bins),
        NATIVE_SOL_MINT,
        token_mint,
        amount_in,
    )
    .expect("SOL→Token auf einseitigem Y-Output-Bin muss ExecutableMarginal liefern");

    assert_eq!(quote.kind, QuoteKind::ExecutableMarginal);
    assert!(
        quote.amount_out > 0,
        "einseitiger Bin (amount_x=0, amount_y>0) mit SOL=X darf nicht wie Mini-CPMM komplett skippen"
    );

    let walker_out = dlmm_token_output_from_bins(active_id, bin_step, amount_in, &bins, true)
        .expect("dlmm_token_output_from_bins muss einseitigen Bin quoten");
    assert_eq!(
        quote.amount_out, walker_out,
        "quote_exact_in und dlmm_token_output_from_bins müssen übereinstimmen"
    );
}

/// A.48: Kleines Exact-In liefert nicht xy=k auf denselben Reserves (constant-price ≠ CPMM).
#[test]
fn dlmm_small_exact_in_differs_from_reserve_cpmm() {
    let active_id = 5i32;
    let bin_step = 100u16;
    let token_mint = "TokenMint22222222222222222222222222222222";
    let pool = sample_pool("dlmm_not_cpmm", token_mint);
    let token_amount = 10_000_000_000u64;
    let sol_amount = 10_000_000_000u64;
    let vault = dlmm_vault(token_amount, sol_amount, active_id, bin_step, token_mint);
    let bins = single_active_bin(active_id, token_amount, sol_amount);

    let amount_in = 50_000u64;
    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        Some(&bins),
        NATIVE_SOL_MINT,
        token_mint,
        amount_in,
    )
    .expect("DLMM quote mit aktivem Bin und Vault-Reserves");

    let fee_bps = dlmm_fee_bps(bin_step);
    let after_fee = amount_in as u128 * (10_000 - fee_bps as u128) / 10_000;
    let ri = sol_amount as u128;
    let ro = token_amount as u128;
    let cpmm_approx = ((after_fee * ro) / (ri + after_fee)) as u64;

    assert_ne!(
        quote.amount_out, cpmm_approx,
        "DLMM constant-price Walk darf nicht Reserve-CPMM xy=k approximieren (A.48)"
    );
    assert!(quote.amount_out > 0);

    let flat = flat_bins_from_arrays(&bins);
    let walker = walker_from_bins(active_id, bin_step, &flat);
    let (walker_out, _, _) = walker
        .quote_y_to_x(amount_in, fee_bps)
        .expect("Walker Y→X (SOL in)");
    assert_eq!(
        quote.amount_out, walker_out,
        "quote_exact_in muss Walker-Integer-Math treffen (nicht f64 bin_id_to_price)"
    );
}

/// A.1: Monotonie — größeres amount_in → amount_out nicht kleiner.
#[test]
fn dlmm_quote_monotonicity_larger_in_not_smaller_out() {
    let active_id = 5i32;
    let bin_step = 100u16;
    let token_mint = "TokenMint33333333333333333333333333333333";
    let pool = sample_pool("dlmm_monotone", token_mint);
    let token_amount = 10_000_000_000u64;
    let sol_amount = 10_000_000_000u64;
    let vault = dlmm_vault(token_amount, sol_amount, active_id, bin_step, token_mint);
    let bins = single_active_bin(active_id, token_amount, sol_amount);

    let small_in = 100_000u64;
    let large_in = 500_000u64;

    let small_quote = quote_exact_in(
        &pool,
        Some(&vault),
        Some(&bins),
        NATIVE_SOL_MINT,
        token_mint,
        small_in,
    )
    .expect("kleines Probe-In");
    let large_quote = quote_exact_in(
        &pool,
        Some(&vault),
        Some(&bins),
        NATIVE_SOL_MINT,
        token_mint,
        large_in,
    )
    .expect("groesseres Probe-In");

    assert!(
        large_quote.amount_out >= small_quote.amount_out,
        "A.1: groesseres amount_in ({large_in}) muss mindestens gleiches amount_out liefern \
         (small_out={}, large_out={})",
        small_quote.amount_out,
        large_quote.amount_out
    );
    assert!(small_quote.amount_out > 0);
}

/// A.48: Production-Walker darf kein per-bin xy=k / constant_product fuer Fill nutzen.
#[test]
fn dlmm_walker_production_no_per_bin_cpmm_grep() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let path = iron_crab_walker_rs();
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let prod = production_source(&source);

    assert!(
        prod.contains("get_price_from_id") && prod.contains("get_amount_out"),
        "DLMM Fill muss Q64.64 get_price_from_id / get_amount_out nutzen"
    );
    assert!(
        !prod.contains("constant_product"),
        "meteora_bin_walker Production darf kein constant_product Fill nutzen (A.48)"
    );

    let forbidden_xy_k = ["amount_x * amount_y", "amount_y * amount_x", "x * y == k"];
    for pattern in forbidden_xy_k {
        assert!(
            !prod.contains(pattern),
            "meteora_bin_walker Production darf kein xy=k Fill-Pattern '{pattern}' enthalten"
        );
    }
}
