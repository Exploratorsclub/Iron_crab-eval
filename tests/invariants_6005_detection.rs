//! Invariante: 6005 BondingCurveComplete-Erkennung (INVARIANTS.md §A.8)
//!
//! is_6005_bonding_curve_complete erkennt BondingCurveComplete (6005) in Fehlermeldungen.
//! Voraussetzung für 6005-Retry in Liquidation (PumpFun → PumpSwap AMM).

use ironcrab::execution::error_detection::is_6005_bonding_curve_complete;

#[test]
fn detects_6005_decimal() {
    assert!(is_6005_bonding_curve_complete(&"6005"));
}

#[test]
fn detects_6005_hex() {
    assert!(is_6005_bonding_curve_complete(&"0x1775"));
}

#[test]
fn detects_instruction_error_custom_6005() {
    assert!(is_6005_bonding_curve_complete(&"InstructionError(1, Custom(6005))"));
}

#[test]
fn detects_custom_6005() {
    assert!(is_6005_bonding_curve_complete(&"Custom(6005)"));
}

#[test]
fn detects_6005_in_anyhow() {
    let err = anyhow::Error::msg("Sim failed: Custom(6005)");
    assert!(is_6005_bonding_curve_complete(&err));
}

#[test]
fn rejects_custom_6023() {
    assert!(!is_6005_bonding_curve_complete(&"Custom(6023)"));
}

#[test]
fn rejects_other_simulation_failure() {
    assert!(!is_6005_bonding_curve_complete(&"Simulation failed: other"));
}

#[test]
fn rejects_empty() {
    assert!(!is_6005_bonding_curve_complete(&""));
}

#[test]
fn rejects_unrelated_error() {
    assert!(!is_6005_bonding_curve_complete(&"Connection refused"));
}
