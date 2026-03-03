//! Invariante: Compute-Budget-Estimator (INVARIANTS.md A.18)
//!
//! estimate_single_swap(notional) liefert compute_unit_limit in vernünftigem Bereich
//! und compute_unit_price_micro_lamports >= 1.

use ironcrab::solana::compute_budget_estimator::estimate_single_swap;

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
