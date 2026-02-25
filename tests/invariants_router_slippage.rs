//! Invariante: Router Slippage (INVARIANTS.md §1.5)
//!
//! Router::cumulative_min_out wendet Slippage auf das letzte amount_out an.

use ironcrab::solana::dex::router::Router;
use ironcrab::solana::dex::Quote;

#[test]
fn cumulative_min_out_applies_slippage_on_final_amount() {
    let quotes = vec![
        Quote {
            amount_out: 50_000,
            price_impact_bps: 10,
            route: vec!["P1".into()],
            fee_bps: 30,
            in_reserve: 1_000_000,
            out_reserve: 2_000_000,
            input_mint: "A".into(),
            output_mint: "B".into(),
            tick_spacing: None,
        },
        Quote {
            amount_out: 100_000,
            price_impact_bps: 15,
            route: vec!["P2".into()],
            fee_bps: 30,
            in_reserve: 2_000_000,
            out_reserve: 3_000_000,
            input_mint: "B".into(),
            output_mint: "C".into(),
            tick_spacing: None,
        },
    ];
    let min_out = Router::cumulative_min_out(&quotes, 100);
    assert_eq!(min_out, 99_000);
}
