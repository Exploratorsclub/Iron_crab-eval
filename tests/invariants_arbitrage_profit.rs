//! Invariante: Arbitrage Profit Filter (INVARIANTS.md §1.6)
//!
//! compute_net_profit filtert basierend auf min_profit_bps und est_tx_cost.

use ironcrab::solana::arbitrage::compute_net_profit;

#[test]
fn profit_filter_accepts_and_rejects() {
    let amount_in = 1_000_000u64;
    let final_out = 1_030_000u64;
    let net = compute_net_profit(amount_in, final_out, 50, 1_000).expect("should pass");
    assert!(net > 28_000 && net <= 29_000, "net within expected window");

    assert!(compute_net_profit(amount_in, 1_002_000u64, 50, 0).is_none());
    assert!(compute_net_profit(amount_in, 1_005_000u64, 10, 5_000).is_none());
}
