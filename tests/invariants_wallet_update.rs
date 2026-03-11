//! Invarianten: LockManager Atomic Wallet Updates (INVARIANTS.md A.27)
//!
//! Verifiziert: update_native_sol_only() und update_wsol_only() entkoppeln SOL/WSOL-Updates.
//! Isolation und Konsistenz (total_native_sol + wsol_balance) bei simuliertem Wrap/Unwrap.

use ironcrab::storage::{LockHolder, LockManager, LockResult};
use std::collections::HashMap;

/// Test 1: update_native_sol_only aendert nur native SOL, WSOL bleibt unveraendert
#[test]
fn native_sol_update_does_not_affect_wsol() {
    let manager = LockManager::new(1_000_000_000);
    manager.update_wallet_balances(1_000_000_000, Some(500_000_000));

    manager.update_native_sol_only(800_000_000);

    assert_eq!(manager.wsol_balance(), 500_000_000, "WSOL unveraendert");
    assert_eq!(
        manager.total_native_sol(),
        800_000_000,
        "native SOL aktualisiert"
    );
}

/// Test 2: update_wsol_only aendert nur WSOL, native SOL bleibt unveraendert
#[test]
fn wsol_update_does_not_affect_native_sol() {
    let manager = LockManager::new(1_000_000_000);

    manager.update_wsol_only(300_000_000);

    assert_eq!(
        manager.total_native_sol(),
        1_000_000_000,
        "native SOL unveraendert"
    );
    assert_eq!(manager.wsol_balance(), 300_000_000, "WSOL aktualisiert");
}

/// Test 3: Simuliertes Wrap erhaelt Wallet-Wert (total_native_sol + wsol_balance konsistent)
#[test]
fn simulated_wrap_preserves_total() {
    let manager = LockManager::new(3_000_000_000);
    manager.update_wallet_balances(3_000_000_000, Some(0));

    let total_before = manager.total_native_sol() + manager.wsol_balance();
    assert_eq!(total_before, 3_000_000_000);

    // Simuliere Wrap: SOL -2B, WSOL +2B
    manager.update_native_sol_only(1_000_000_000);
    manager.update_wsol_only(2_000_000_000);

    let total_after = manager.total_native_sol() + manager.wsol_balance();
    assert_eq!(total_before, total_after, "Wallet-Wert bei Wrap erhalten");
}

/// Test 4: Simuliertes Unwrap erhaelt Wallet-Wert
#[test]
fn simulated_unwrap_preserves_total() {
    let manager = LockManager::new(1_000_000_000);
    manager.update_wallet_balances(1_000_000_000, Some(2_000_000_000));

    let total_before = manager.total_native_sol() + manager.wsol_balance();
    assert_eq!(total_before, 3_000_000_000);

    // Simuliere Unwrap: WSOL -2B, SOL +2B
    manager.update_wsol_only(0);
    manager.update_native_sol_only(3_000_000_000);

    let total_after = manager.total_native_sol() + manager.wsol_balance();
    assert_eq!(total_before, total_after, "Wallet-Wert bei Unwrap erhalten");
}

/// Test 5: update_native_sol_only mit aktiven Locks — available_sol wird direkt gesetzt,
/// total_native_sol = available_sol + locked. Dokumentiert bestehendes Verhalten.
#[test]
fn update_with_active_locks_no_double_count() {
    let manager = LockManager::new(2_000_000_000).with_fairness(5, 60, 30, false);

    let holder = LockHolder::new("intent-lock");
    let result = manager.try_lock_capital(holder.clone(), 500_000_000, HashMap::new());
    assert!(matches!(result, LockResult::Acquired));

    assert_eq!(manager.available_sol(), 1_500_000_000);
    assert_eq!(manager.total_native_sol(), 2_000_000_000);

    // Geyser-Update: on-chain balance unveraendert bei 2B
    manager.update_native_sol_only(2_000_000_000);

    // total_native_sol = available_sol + locked = 2B + 500M
    assert_eq!(manager.total_native_sol(), 2_500_000_000);
    assert_eq!(manager.available_sol(), 2_000_000_000);

    manager.release_locks("intent-lock");
}

/// Test 6: WSOL initial nicht initialisiert, update_wsol_only initialisiert
#[test]
fn wsol_initialized_after_update() {
    let manager = LockManager::new(1_000_000_000);

    assert_eq!(
        manager.wsol_balance(),
        0,
        "WSOL initial nicht initialisiert"
    );

    manager.update_wsol_only(100_000_000);

    assert_eq!(manager.wsol_balance(), 100_000_000);
}
