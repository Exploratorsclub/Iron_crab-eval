//! Unit-Tests: is_6005_bonding_curve_complete (ARCHITECTURE_AUDIT A.4 6005-Retry)
//!
//! Verifiziert die Erkennung von BondingCurveComplete (6005) in verschiedenen Fehlerstring-Formaten.

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
    assert!(is_6005_bonding_curve_complete(
        &"InstructionError(1, Custom(6005))"
    ));
}

#[test]
fn detects_custom_6005() {
    assert!(is_6005_bonding_curve_complete(&"Custom(6005)"));
}

#[test]
fn detects_simulation_failed_with_6005() {
    assert!(is_6005_bonding_curve_complete(
        &"Simulation failed: InstructionError(1, Custom(6005))"
    ));
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
fn rejects_empty_string() {
    assert!(!is_6005_bonding_curve_complete(&""));
}
