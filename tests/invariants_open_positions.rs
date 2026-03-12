//! Invariante A.28: Open Positions Counter Konsistenz (INVARIANTS.md)
//!
//! Single Source of Truth: count_non_zero_token_balances() = Anzahl non-zero Einträge
//! in available_tokens. Verhindert Ghost Positions (KNOWN_BUG_PATTERNS #5).

use ironcrab::storage::LockManager;

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
