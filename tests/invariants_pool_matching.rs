//! Invariante: Pool-Matching (INVARIANTS.md A.11, I-13, FIX-38)
//!
//! Position-Preis-Updates nur anwenden, wenn source_pool == position.pool.
//! Verhindert falsche PnL und TAKE_PROFIT bei Multi-Pool-Tokens.

use ironcrab::execution::position_utils::should_apply_position_price_update;

/// Invariante: source_pool == position.pool → Update anwenden
#[test]
fn pool_matching_same_pool_applies() {
    assert!(should_apply_position_price_update("PoolA", Some("PoolA")));
}

/// Invariante: source_pool != position.pool → Update übersprungen
#[test]
fn pool_matching_different_pool_skips() {
    assert!(!should_apply_position_price_update("PoolA", Some("PoolB")));
}

/// Invariante: source_pool None (Legacy) → Update anwenden
#[test]
fn pool_matching_none_source_applies() {
    assert!(should_apply_position_price_update("PoolA", None));
}

/// Invariante: position_pool leer → Update anwenden
#[test]
fn pool_matching_empty_position_applies() {
    assert!(should_apply_position_price_update("", Some("PoolA")));
}

/// Invariante: beide leer → Update anwenden
#[test]
fn pool_matching_both_empty_applies() {
    assert!(should_apply_position_price_update("", Some("")));
}
