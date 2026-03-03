//! Invariante: Compute-Budget-Estimator (INVARIANTS.md A.18)
//!
//! estimate_single_swap(notional) liefert compute_unit_limit in vernünftigem Bereich
//! und compute_unit_price_micro_lamports >= 1.
//! Bei großem Notional wird der CU-Preis mit large_notional_multiplier multipliziert.

use ironcrab::solana::compute_budget_estimator::{
    estimate_from_instructions, estimate_single_swap, EstimatorConfig,
};
use solana_sdk::instruction::Instruction;

/// A.18: estimate_single_swap liefert gültigen ComputeEstimate.
#[test]
fn single_swap_estimate_in_range() {
    let est = estimate_single_swap(500_000_000);
    assert!(
        est.compute_unit_limit >= 80_000 && est.compute_unit_limit <= 400_000,
        "compute_unit_limit must be in [80_000, 400_000]"
    );
    assert_eq!(
        est.compute_unit_price_micro_lamports, 1,
        "default price must be 1"
    );
}

/// A.18 (ergänzt): Bei großem Notional steigt der CU-Preis (large_notional_multiplier).
#[test]
fn large_notional_increases_price() {
    let cfg = EstimatorConfig::default();
    let dummy_ix = Instruction {
        program_id: solana_sdk::pubkey::Pubkey::new_unique(),
        accounts: vec![],
        data: vec![],
    };
    let est = estimate_from_instructions(
        &[dummy_ix.clone(), dummy_ix],
        1,
        cfg.large_notional_threshold,
        cfg,
    );
    assert!(
        est.compute_unit_price_micro_lamports
            >= cfg.default_cu_price_micro_lamports * cfg.large_notional_multiplier,
        "large notional must apply price multiplier"
    );
}
