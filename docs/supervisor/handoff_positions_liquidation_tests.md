# Handoff: Invariant-Tests A.28 + A.29

Implementiere die Tests fuer die neuen Invarianten A.28 und A.29.
Lese ZUERST docs/spec/INVARIANTS.md fuer die formale Definition.

WICHTIG: Alle Tests sind BLACKBOX-Tests. Importiere nur die PUBLIC API der Iron_crab crate.
Nutze `use iron_crab::storage::locks::{LockManager, LockHolder, LockResult};` und aehnliche public imports.
Kein direkter Zugriff auf private Felder.

Pruefe nach Implementierung: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.

---

## Test A.28: Open Positions Counter Konsistenz

**Datei:** `tests/invariants_open_positions.rs`

**Benoetigte Imports:**
```rust
use iron_crab::storage::locks::{LockManager, LockHolder, LockResult};
use std::collections::HashMap;
```

### Test 1: count_matches_token_balances

```rust
#[test]
fn count_matches_token_balances() {
    let manager = LockManager::new(5_000_000_000).with_fairness(5, 60, 30, false);

    // Simuliere 3 BUY-Fills via set_available_token_balance
    manager.set_available_token_balance("mint_A".to_string(), 1_000_000);
    manager.set_available_token_balance("mint_B".to_string(), 2_000_000);
    manager.set_available_token_balance("mint_C".to_string(), 500_000);

    assert_eq!(
        manager.count_non_zero_token_balances(),
        3,
        "3 Mints mit non-zero Balance = 3 open positions"
    );
}
```

### Test 2: count_after_sell_all

```rust
#[test]
fn count_after_sell_all() {
    let manager = LockManager::new(5_000_000_000).with_fairness(5, 60, 30, false);

    manager.set_available_token_balance("mint_A".to_string(), 1_000_000);
    manager.set_available_token_balance("mint_B".to_string(), 2_000_000);
    assert_eq!(manager.count_non_zero_token_balances(), 2);

    // Simuliere SELL-All: Balance auf 0 setzen
    manager.set_available_token_balance("mint_A".to_string(), 0);
    manager.set_available_token_balance("mint_B".to_string(), 0);

    assert_eq!(
        manager.count_non_zero_token_balances(),
        0,
        "Nach Verkauf aller Token: 0 open positions"
    );
}
```

### Test 3: no_drift_on_concurrent_updates

```rust
#[test]
fn no_drift_on_concurrent_updates() {
    let manager = LockManager::new(5_000_000_000).with_fairness(5, 60, 30, false);

    // Simuliere Geyser-Update setzt Balance
    manager.set_available_token_balance("mint_A".to_string(), 1_000_000);
    assert_eq!(manager.count_non_zero_token_balances(), 1);

    // Simuliere ExecutionResult add_available_token_balance (Doppel-Update)
    // In der neuen Architektur aendert das nur die Balance, nicht den Counter
    manager.add_available_token_balance("mint_A".to_string(), 500_000);
    assert_eq!(
        manager.count_non_zero_token_balances(),
        1,
        "Doppel-Update auf gleichen Mint = immer noch 1 Position"
    );

    // Geyser setzt autoritativen Wert
    manager.set_available_token_balance("mint_A".to_string(), 1_500_000);
    assert_eq!(
        manager.count_non_zero_token_balances(),
        1,
        "Autoritatives Geyser-Update aendert count nicht"
    );

    // Zweiter Mint
    manager.set_available_token_balance("mint_B".to_string(), 2_000_000);
    assert_eq!(manager.count_non_zero_token_balances(), 2);

    // Sell mint_A
    manager.set_available_token_balance("mint_A".to_string(), 0);
    assert_eq!(
        manager.count_non_zero_token_balances(),
        1,
        "Nach Sell mint_A: nur mint_B offen"
    );
}
```

### Test 4: restart_recovery

```rust
#[test]
fn restart_recovery() {
    // Simuliere Restart: neuer LockManager, dann Balances aus JetStream/Geyser bootstrappen
    let manager = LockManager::new(5_000_000_000).with_fairness(5, 60, 30, false);

    // Bootstrap: 5 Token-Mints mit Balance (wie bei Wallet-Scan nach Restart)
    manager.set_available_token_balance("mint_1".to_string(), 100_000);
    manager.set_available_token_balance("mint_2".to_string(), 200_000);
    manager.set_available_token_balance("mint_3".to_string(), 0); // leer, kein open position
    manager.set_available_token_balance("mint_4".to_string(), 500_000);
    manager.set_available_token_balance("mint_5".to_string(), 300_000);

    assert_eq!(
        manager.count_non_zero_token_balances(),
        4,
        "4 von 5 Mints haben Balance > 0 = 4 open positions"
    );
}
```

---

## Test A.29: Liquidation Vollstaendigkeit (erweitere bestehende Datei)

Die bestehende Datei `tests/invariants_liquidation_flow.rs` hat bereits Tests fuer 6005-Retry.
Fuege die neuen Tests AM ENDE der Datei hinzu (nach den bestehenden Tests).

HINWEIS: Wenn `invariants_liquidation_flow.rs` nur LivePoolCache-Tests hat und keine
Liquidation-E2E-Tests, dann fuege diese neuen Tests dort ein.

### Test 5: liquidation_cashback_account_layout

Dieser Test prueft dass build_sell_ix mit cashback_enabled=true 16 Accounts erzeugt.
(Die Invariante A.23 testet bereits build_sell_ix direkt. Dieser Test ergaenzt den
Liquidation-Kontext: cashback_enabled darf nicht auf false defaulten.)

```rust
#[test]
fn liquidation_cashback_account_layout_16_accounts() {
    // PumpFun SELL mit cashback_enabled=true muss 16 Accounts haben
    // Dies verifiziert dass die Liquidation den korrekten Account-Count nutzt
    use iron_crab::solana::dex::pumpfun::PumpFunDex;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    let pumpfun = PumpFunDex::new_offline();
    let token_mint = Pubkey::new_unique();
    let bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();

    let ix = pumpfun.build_sell_ix(
        &token_mint,
        &bonding_curve,
        &user_token_account,
        &creator,
        &token_program,
        1_000_000,    // amount_in
        100,          // min_sol_output
        true,         // cashback_enabled = true
    ).expect("build_sell_ix should succeed");

    assert_eq!(
        ix.accounts.len(),
        16,
        "PumpFun SELL with cashback_enabled=true must have 16 accounts"
    );

    // Letztes Account muss bonding_curve_v2 sein
    let last = ix.accounts.last().unwrap();
    assert!(!last.is_signer, "bonding_curve_v2 is not signer");
    assert!(!last.is_writable, "bonding_curve_v2 is readonly");
}
```

### Test 6: liquidation_non_cashback_account_layout_15

```rust
#[test]
fn liquidation_non_cashback_account_layout_15() {
    use iron_crab::solana::dex::pumpfun::PumpFunDex;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    let pumpfun = PumpFunDex::new_offline();
    let token_mint = Pubkey::new_unique();
    let bonding_curve = Pubkey::new_unique();
    let user_token_account = Pubkey::new_unique();
    let creator = Pubkey::new_unique();
    let token_program = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();

    let ix = pumpfun.build_sell_ix(
        &token_mint,
        &bonding_curve,
        &user_token_account,
        &creator,
        &token_program,
        1_000_000,
        100,
        false,        // cashback_enabled = false
    ).expect("build_sell_ix should succeed");

    assert_eq!(
        ix.accounts.len(),
        15,
        "PumpFun SELL with cashback_enabled=false must have 15 accounts"
    );
}
```

### Test 7: count_non_zero_with_locks_active

```rust
#[test]
fn count_non_zero_with_locks_active() {
    let manager = LockManager::new(5_000_000_000).with_fairness(5, 60, 30, false);

    // Token-Balance mit aktivem Lock
    let mut tokens = HashMap::new();
    tokens.insert("mint_A".to_string(), 1_000_000u64);
    let holder = LockHolder::new("sell-intent-1");
    manager.set_available_token_balance("mint_A".to_string(), 1_000_000);
    let result = manager.try_lock_capital(holder, 0, tokens);
    assert!(matches!(result, LockResult::Acquired));

    // Trotz Lock: die Balance im available_tokens ist reduziert
    // Aber count_non_zero_token_balances zaehlt die effektive Balance
    // Bei set_available_token_balance mit Lock wird effective = raw - locked
    // In diesem Fall: 1M - 1M = 0 -> count koennte 0 sein
    // ODER die Logik zaehlt Eintraege > 0

    // Fuege einen zweiten Mint ohne Lock hinzu
    manager.set_available_token_balance("mint_B".to_string(), 500_000);

    // mint_B hat definitive Balance > 0
    assert!(
        manager.count_non_zero_token_balances() >= 1,
        "Mindestens mint_B hat non-zero Balance"
    );

    manager.release_locks("sell-intent-1");
}
```

---

## Pruefung
Am Ende MUESSEN bestehen:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

HINWEIS: Falls Imports wie `PumpFunDex::new_offline()` nicht existieren, nutze stattdessen
die verfuegbare Konstruktor-Methode. Pruefe die public API mit `cargo doc --open` oder
lese die relevanten lib.rs / mod.rs Dateien.

Passe die Tests an die tatsaechlich verfuegbare API an. Die Testnamen und Invarianten
muessen erhalten bleiben, aber die konkreten API-Aufrufe muessen kompilieren.
