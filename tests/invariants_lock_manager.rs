//! Invarianten: LockManager
//!
//! Verifiziert: total_locked + available konsistent, kein doppelter Lock pro Intent.

use ironcrab::storage::{LockHolder, LockManager, LockResult};
use std::collections::HashMap;

/// Invariante: total_locked + available = initial (SOL-Erhaltung über Lock/Release)
#[test]
fn lock_manager_total_conserved() {
    let initial_sol = 1_000_000_000u64; // 1 SOL
    let manager = LockManager::new(initial_sol).with_fairness(5, 60, 30, false);

    // Vor jeder Operation: total_native_sol = available + locked
    assert_eq!(manager.total_native_sol(), initial_sol);
    assert_eq!(manager.available_sol(), initial_sol);

    let holder = LockHolder::new("intent-1");
    let result = manager.try_lock_capital(holder.clone(), 500_000_000, HashMap::new());
    assert!(matches!(result, LockResult::Acquired));

    // Nach Lock: available reduziert, total bleibt gleich
    assert_eq!(manager.available_sol(), 500_000_000);
    assert_eq!(manager.total_native_sol(), initial_sol);

    manager.release_locks("intent-1");

    // Nach Release: wieder initial
    assert_eq!(manager.available_sol(), initial_sol);
    assert_eq!(manager.total_native_sol(), initial_sol);
}

/// Invariante: Gleicher Intent-ID nicht doppelt gelockt (Capital Lock)
#[test]
fn no_double_lock_same_intent() {
    let manager = LockManager::new(1_000_000_000).with_fairness(5, 60, 30, false);

    let holder = LockHolder::new("intent-same");
    let r1 = manager.try_lock_capital(holder.clone(), 200_000_000, HashMap::new());
    assert!(matches!(r1, LockResult::Acquired));

    // Zweiter Versuch mit gleicher intent_id → Conflict
    let r2 = manager.try_lock_capital(holder, 100_000_000, HashMap::new());
    assert!(matches!(r2, LockResult::Conflict { .. }));

    manager.release_locks("intent-same");
}
