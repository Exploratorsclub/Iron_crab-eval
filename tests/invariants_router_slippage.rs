//! Invarianten: Router Slippage (INVARIANTS.md A.5)
//!
//! - cumulative_min_out: Slippage auf letztes amount_out
//! - Multi-Hop-Plan: build_best_hops2_plan_exact_in liefert min_out = expected_out * (10_000 - slippage_bps) / 10_000

use ironcrab::solana::dex::orca::Orca;
use ironcrab::solana::dex::router::Router;
use ironcrab::solana::dex::Dex;
use ironcrab::solana::dex::Quote;
use ironcrab::solana::rpc::SolanaRpc;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

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

/// INVARIANTS.md A.5: Multi-Hop-Plan min_out = expected_out * (10_000 - slippage_bps) / 10_000.
#[tokio::test]
async fn multi_hop_plan_min_out_applies_slippage_on_final_output() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));

    let orca0 = Arc::new(Orca::new(rpc.clone()));
    let orca1 = Arc::new(Orca::new(rpc.clone()));

    let mint_a = Pubkey::new_from_array([1u8; 32]);
    let mint_b = Pubkey::new_from_array([2u8; 32]);
    let mint_c = Pubkey::new_from_array([3u8; 32]);

    orca0.insert_mock_pool(mint_a, mint_b, 500_000_000_000u128, 1_000_000_000_000u128, 30);
    orca1.insert_mock_pool(mint_b, mint_c, 1_000_000_000_000u128, 1_500_000_000_000u128, 30);

    let auth = Pubkey::new_unique();
    for dex in [&orca0, &orca1] {
        dex.set_user_authority(auth);
        dex.set_user_token_account(mint_a, Pubkey::new_unique());
        dex.set_user_token_account(mint_b, Pubkey::new_unique());
        dex.set_user_token_account(mint_c, Pubkey::new_unique());
    }

    let router = Router::new(vec![orca0.clone() as Arc<dyn Dex>, orca1.clone() as Arc<dyn Dex>]);

    let amount_in: u64 = 50_000;
    let slippage_bps: u32 = 200; // 2%

    let plan = router
        .build_best_hops2_plan_exact_in(
            &mint_a.to_string(),
            &mint_c.to_string(),
            amount_in,
            slippage_bps,
        )
        .await
        .expect("build_best_hops2_plan_exact_in should not fail");

    let plan = plan.expect("expected a multi-hop plan for A->B->C");

    let expected_min = (plan.expected_out as u128 * (10_000 - slippage_bps) as u128 / 10_000) as u64;
    assert_eq!(
        plan.min_out, expected_min,
        "min_out must apply slippage to final output"
    );
}
