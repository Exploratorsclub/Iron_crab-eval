//! Invarianten: Raydium Slippage (INVARIANTS.md A.9)
//!
//! Raydium::apply_slippage_min_out(amount_out, slippage_bps) = amount_out * (10_000 - slippage_bps) / 10_000
//! Randfälle: slippage_bps = 0 → unverändert; slippage_bps >= 10_000 → 0
//! Einheitliche Slippage-Semantik über DEXs (DoD §H Connector Contracts).

use ironcrab::solana::dex::raydium::Raydium;

#[test]
fn slippage_zero_bps_unchanged() {
    let amount_out: u64 = 50_000;
    let slippage_bps: u32 = 0;
    let min_out = Raydium::apply_slippage_min_out(amount_out, slippage_bps);
    assert_eq!(min_out, 50_000);
}

#[test]
fn slippage_one_percent_reduces() {
    let amount_out: u64 = 100_000;
    let slippage_bps: u32 = 100; // 1%
    let min_out = Raydium::apply_slippage_min_out(amount_out, slippage_bps);
    assert_eq!(min_out, 99_000);
}

#[test]
fn slippage_formula_general() {
    let amount_out: u64 = 100_000;
    let slippage_bps: u32 = 200; // 2%
    let min_out = Raydium::apply_slippage_min_out(amount_out, slippage_bps);
    let expected = (amount_out as u128 * (10_000 - slippage_bps) as u128 / 10_000) as u64;
    assert_eq!(min_out, expected);
}

#[test]
fn slippage_extreme_rounds_down() {
    let amount_out: u64 = 100;
    let slippage_bps: u32 = 9_999;
    let min_out = Raydium::apply_slippage_min_out(amount_out, slippage_bps);
    assert_eq!(min_out, 0);
}

#[test]
fn slippage_full_loss_zero() {
    let amount_out: u64 = 100;
    let slippage_bps: u32 = 10_000;
    let min_out = Raydium::apply_slippage_min_out(amount_out, slippage_bps);
    assert_eq!(min_out, 0);
}
