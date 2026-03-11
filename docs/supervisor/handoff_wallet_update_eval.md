# Handoff: Eval-Tests fuer A.27 LockManager Atomic Wallet Updates

## Kontext

Neue Methoden `update_native_sol_only()` und `update_wsol_only()` in LockManager entkoppeln SOL/WSOL-Updates. Eval-Tests muessen die Isolation und Konsistenz verifizieren.

## Aufgabe

Erstelle `tests/invariants_wallet_update.rs` mit folgenden Blackbox-Tests:

### Test 1: `native_sol_update_does_not_affect_wsol`
1. Erstelle LockManager mit initial_sol = 1_000_000_000
2. Setze WSOL via `update_wallet_balances(1_000_000_000, Some(500_000_000))`
3. Rufe `update_native_sol_only(800_000_000)` auf
4. Assert: `wsol_balance() == 500_000_000` (unveraendert)
5. Assert: `total_native_sol() == 800_000_000` (aktualisiert)

### Test 2: `wsol_update_does_not_affect_native_sol`
1. Erstelle LockManager mit initial_sol = 1_000_000_000
2. Rufe `update_wsol_only(300_000_000)` auf
3. Assert: `total_native_sol() == 1_000_000_000` (unveraendert)
4. Assert: `wsol_balance() == 300_000_000` (aktualisiert)

### Test 3: `simulated_wrap_preserves_total`
1. Erstelle LockManager mit initial_sol = 3_000_000_000
2. Setze WSOL=0 via `update_wallet_balances(3_000_000_000, Some(0))`
3. Berechne total_before = `total_native_sol() + wsol_balance()` = 3_000_000_000
4. Simuliere Wrap: `update_native_sol_only(1_000_000_000)` dann `update_wsol_only(2_000_000_000)`
5. Berechne total_after = `total_native_sol() + wsol_balance()`
6. Assert: `total_before == total_after` (Wallet-Wert erhalten)

### Test 4: `simulated_unwrap_preserves_total`
1. Erstelle LockManager mit initial_sol = 1_000_000_000
2. Setze WSOL=2_000_000_000 via `update_wallet_balances(1_000_000_000, Some(2_000_000_000))`
3. Berechne total_before = `total_native_sol() + wsol_balance()` = 3_000_000_000
4. Simuliere Unwrap: `update_wsol_only(0)` dann `update_native_sol_only(3_000_000_000)`
5. Berechne total_after = `total_native_sol() + wsol_balance()`
6. Assert: `total_before == total_after`

### Test 5: `update_with_active_locks_no_double_count`
1. Erstelle LockManager mit initial_sol = 2_000_000_000
2. Lock 500_000_000 via `try_lock_capital`
3. Assert: `available_sol() == 1_500_000_000`, `total_native_sol() == 2_000_000_000`
4. Rufe `update_native_sol_only(2_000_000_000)` auf (simulating Geyser update — on-chain balance unchanged)
5. Assert: `total_native_sol() == 2_500_000_000` (available_sol=2_000_000_000 + locked=500_000_000)
   ODER: dokumentiere das erwartete Verhalten — available_sol wird direkt gesetzt, locks addieren sich dazu.
   WICHTIG: Dies ist das bestehende Verhalten von total_native_sol(). Der Test dokumentiert es, damit zukuenftige Fixes es beruecksichtigen koennen.

### Test 6: `wsol_initialized_after_update`
1. Erstelle LockManager mit initial_sol = 1_000_000_000
2. Assert: WSOL ist initial nicht initialisiert (wsol_balance() == 0)
3. Rufe `update_wsol_only(100_000_000)` auf
4. Assert: `wsol_balance() == 100_000_000`

## Imports

```rust
use ironcrab::storage::{LockHolder, LockManager, LockResult};
use std::collections::HashMap;
```

## Hinweise

- Alle Tests muessen Blackbox sein — nur oeffentliche API nutzen
- `with_fairness(5, 60, 30, false)` fuer Tests mit Capital-Locks verwenden (wie in invariants_lock_manager.rs)
- Nach Erstellung: `cargo fmt`, `cargo clippy -p ironcrab-eval --all-targets -- -D warnings`
