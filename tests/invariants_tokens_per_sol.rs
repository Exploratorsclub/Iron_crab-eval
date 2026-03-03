//! Invariante: tokens_per_sol (INVARIANTS.md A.19, I-14)
//!
//! LOWER tps = token wertvoller. pnl_pct = (entry/current - 1)*100.
//! highest_price = niedrigster tps (bester Preis für Holder).

use ironcrab::execution::tokens_per_sol::{drawdown_from_ath_pct, pnl_pct, updated_highest_price};

#[test]
fn pnl_pct_zero_when_equal() {
    assert!((pnl_pct(100.0, 100.0) - 0.0).abs() < 1e-10);
}

#[test]
fn pnl_pct_positive_when_token_expensive() {
    // entry=200, current=100 -> (200/100 - 1)*100 = +100%
    assert!((pnl_pct(200.0, 100.0) - 100.0).abs() < 1e-10);
}

#[test]
fn pnl_pct_negative_when_token_cheap() {
    // entry=100, current=200 -> (100/200 - 1)*100 = -50%
    assert!((pnl_pct(100.0, 200.0) - (-50.0)).abs() < 1e-10);
}

#[test]
fn highest_price_tracks_lowest_tps() {
    // Lower tps = better. min(150, 120) = 120
    assert!((updated_highest_price(150.0, 120.0) - 120.0).abs() < 1e-10);
}

#[test]
fn highest_price_unchanged_if_higher() {
    // Current highest 100 is better than new 150
    assert!((updated_highest_price(100.0, 150.0) - 100.0).abs() < 1e-10);
}

#[test]
fn drawdown_zero_at_ath() {
    assert!((drawdown_from_ath_pct(100.0, 100.0) - 0.0).abs() < 1e-10);
}

#[test]
fn drawdown_positive_when_worse() {
    // current=120, highest=100 -> (120/100 - 1)*100 = +20%
    assert!((drawdown_from_ath_pct(100.0, 120.0) - 20.0).abs() < 1e-10);
}

#[test]
fn edge_case_zero_prices_pnl() {
    assert!((pnl_pct(0.0, 100.0) - 0.0).abs() < 1e-10);
    assert!((pnl_pct(100.0, 0.0) - 0.0).abs() < 1e-10);
}

#[test]
fn edge_case_zero_updated_highest() {
    assert!((updated_highest_price(0.0, 100.0) - 0.0).abs() < 1e-10);
    assert!((updated_highest_price(100.0, 0.0) - 0.0).abs() < 1e-10);
}

#[test]
fn edge_case_zero_drawdown() {
    assert!((drawdown_from_ath_pct(0.0, 100.0) - 0.0).abs() < 1e-10);
    assert!((drawdown_from_ath_pct(100.0, 0.0) - 0.0).abs() < 1e-10);
}
