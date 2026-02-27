//! Invarianten: Router Slippage & Best Quote (INVARIANTS.md A.5)
//!
//! - cumulative_min_out: Slippage auf letztes amount_out
//! - Multi-Hop-Plan: build_best_hops2_plan_exact_in liefert min_out = expected_out * (10_000 - slippage_bps) / 10_000
//! - Best Quote Selection: best_quote_exact_in liefert den Quote mit höchstem amount_out unter allen DEXs

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

    orca0.insert_mock_pool(
        mint_a,
        mint_b,
        500_000_000_000u128,
        1_000_000_000_000u128,
        30,
    );
    orca1.insert_mock_pool(
        mint_b,
        mint_c,
        1_000_000_000_000u128,
        1_500_000_000_000u128,
        30,
    );

    let auth = Pubkey::new_unique();
    for dex in [&orca0, &orca1] {
        dex.set_user_authority(auth);
        dex.set_user_token_account(mint_a, Pubkey::new_unique());
        dex.set_user_token_account(mint_b, Pubkey::new_unique());
        dex.set_user_token_account(mint_c, Pubkey::new_unique());
    }

    let router = Router::new(vec![
        orca0.clone() as Arc<dyn Dex>,
        orca1.clone() as Arc<dyn Dex>,
    ]);

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

    let expected_min =
        (plan.expected_out as u128 * (10_000 - slippage_bps) as u128 / 10_000) as u64;
    assert_eq!(
        plan.min_out, expected_min,
        "min_out must apply slippage to final output"
    );
}

/// INVARIANTS.md A.5: Best Quote Selection – Router wählt den Quote mit höchstem amount_out.
#[tokio::test]
async fn best_quote_selects_highest_amount_out() {
    let rpc = Arc::new(SolanaRpc::new("http://127.0.0.1:0"));

    let orca0 = Arc::new(Orca::new(rpc.clone()));
    let orca1 = Arc::new(Orca::new(rpc.clone()));

    let mint_a = Pubkey::new_from_array([5u8; 32]);
    let mint_b = Pubkey::new_from_array([6u8; 32]);

    // Orca0: 1:2 ratio → niedrigeres amount_out
    orca0.insert_mock_pool(mint_a, mint_b, 1_000_000_000u128, 2_000_000_000u128, 30);
    // Orca1: 1:6 ratio → höheres amount_out (besseres Verhältnis)
    orca1.insert_mock_pool(mint_a, mint_b, 1_000_000_000u128, 6_000_000_000u128, 30);

    let router = Router::new(vec![
        orca0.clone() as Arc<dyn Dex>,
        orca1.clone() as Arc<dyn Dex>,
    ]);

    let amount_in: u64 = 10_000;

    let best = router
        .best_quote_exact_in(&mint_a.to_string(), &mint_b.to_string(), amount_in)
        .await
        .expect("best_quote_exact_in should not fail");

    let best = best.expect("expected a quote from at least one DEX");

    // Orca1 (dex_index 1) hat bessere Reserves → höheres amount_out
    assert_eq!(
        best.dex_index, 1,
        "Router must select the DEX with highest amount_out"
    );
    assert!(
        best.quote.amount_out > 0,
        "selected quote must have non-zero amount_out"
    );

    // Sicherstellen: amount_out von Orca1 ist höher als von Orca0
    let q0 = orca0
        .quote_exact_in(&mint_a.to_string(), &mint_b.to_string(), amount_in)
        .await
        .ok()
        .flatten();
    let q1 = orca1
        .quote_exact_in(&mint_a.to_string(), &mint_b.to_string(), amount_in)
        .await
        .ok()
        .flatten();
    let out0 = q0.map(|q| q.amount_out).unwrap_or(0);
    let out1 = q1.map(|q| q.amount_out).unwrap_or(0);
    assert!(
        out1 > out0,
        "Orca1 must yield higher amount_out than Orca0 for invariant test"
    );
    assert_eq!(
        best.quote.amount_out, out1,
        "Router must return the quote with highest amount_out"
    );
}
