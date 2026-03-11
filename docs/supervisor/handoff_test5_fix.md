# Handoff: Test 5 Assertions korrigieren

## Problem

Test 5 (`update_with_active_locks_no_double_count`) in `tests/invariants_wallet_update.rs` asserted bisher Double-Counting als korrektes Verhalten. Die Assertions muessen an das korrigierte `update_native_sol_only()` angepasst werden, das jetzt aktive Locks beim Setzen von `available_sol` abzieht.

## Fix

In `tests/invariants_wallet_update.rs`, den Test `update_with_active_locks_no_double_count` ersetzen.

**VORHER (Zeilen 74-95):**
```rust
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
```

**NACHHER:**
```rust
/// Test 5: update_native_sol_only mit aktiven Locks — subtrahiert Locks vom On-Chain-Wert.
/// Verifiziert: Kein Double-Counting. total_native_sol() == On-Chain-Wert nach Update.
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

    // available_sol = on_chain - locked = 2B - 500M = 1.5B (unveraendert)
    assert_eq!(manager.available_sol(), 1_500_000_000, "available = on_chain - locked");
    // total_native_sol = available + locked = 1.5B + 500M = 2B (korrekt, kein Double-Count)
    assert_eq!(manager.total_native_sol(), 2_000_000_000, "total == on-chain, kein Double-Count");

    manager.release_locks("intent-lock");
}
```

## Pruefung

- Nach dem Fix: `cargo fmt`, `cargo clippy -p ironcrab-eval --all-targets -- -D warnings`, `cargo test`
