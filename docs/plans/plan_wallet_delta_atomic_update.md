# Plan: Wallet-Delta Fix — Atomare SOL/WSOL-Updates

**Status:** Impl + Eval abgeschlossen. Alle Tests bestanden. Bereit fuer Deployment.
**Erstellt:** 2026-03-04
**Bug-Pattern:** KNOWN_BUG_PATTERNS #23

## Problem

Die Prometheus-Metrik `wallet_total_sol_lamports` (= `total_native_sol() + wsol_balance()`) zeigt falsche Werte nach WSOL Wrap/Unwrap-Operationen. Ursache: Die Event-Handler fuer NATIVE_SOL und WSOL aktualisieren jeweils beide Werte nicht-atomar, was zu Cross-Kontamination fuehrt.

### Konkretes Fehlverhalten

1. **Non-atomic Updates**: SOL/WSOL Geyser-Events kommen als separate Messages. Zwischen den Events ist die Metrik temporaer falsch.
2. **Cross-Kontamination im WSOL-Handler**: Liest `total_native_sol()` (inkl. Locks) und schreibt es als `available_sol` zurueck. Bei aktiven Capital-Locks: Doppelzaehlung.
3. **SOL-Handler ignoriert WSOL=0**: `if wsol > 0 { Some(wsol) } else { None }` — nach Unwrap (WSOL=0) wird der Nullwert nicht uebernommen.
4. **Grafana-Query**: `offset 24h` nimmt Einzelpunkt, der transient falsch sein kann.

## Fix-Beschreibung

### 1. `src/storage/locks.rs` — Zwei separate Update-Methoden

Neue oeffentliche Methoden:

```rust
pub fn update_native_sol_only(&self, sol_lamports: u64) {
    *self.available_sol.write() = sol_lamports;
}

pub fn update_wsol_only(&self, wsol_lamports: u64) {
    *self.available_wsol.write() = wsol_lamports;
    self.wsol_initialized.store(true, Ordering::Relaxed);
}
```

`update_wallet_balances()` bleibt bestehen (fuer Bootstrap), wird aber nicht mehr aus den Event-Handlern gerufen.

### 2. `src/bin/execution_engine.rs` — Event-Handler entkoppeln

**NATIVE_SOL Handler** (ca. Zeile 5780-5788):
```rust
if mint == "NATIVE_SOL" {
    ctx.lock_manager.update_native_sol_only(*balance_raw);
    if let Some(ref tx) = ctx.wsol_balance_tx {
        let wsol = ctx.lock_manager.wsol_balance();
        let _ = tx.try_send((*balance_raw, Some(wsol)));
    }
}
```

**WSOL Handler** (ca. Zeile 5789-5795):
```rust
} else if mint == WSOL_MINT || mint == SOL_MINT {
    ctx.lock_manager.update_wsol_only(*balance_raw);
    if let Some(ref tx) = ctx.wsol_balance_tx {
        let sol = ctx.lock_manager.total_native_sol();
        let _ = tx.try_send((sol, Some(*balance_raw)));
    }
}
```

### 3. `docs/grafana_multiprocess_dashboard.json` — Query glaetten

Aendere die 24h Wallet Delta Query von:
```
(wallet_total_sol_lamports{...} - wallet_total_sol_lamports{...} offset 24h) / 1e9
```
zu:
```
(avg_over_time(wallet_total_sol_lamports{job="execution-engine"}[5m]) - avg_over_time(wallet_total_sol_lamports{job="execution-engine"}[5m] offset 24h)) / 1e9
```

## Neue Invariante

**A.27 LockManager Atomic Wallet Updates**
- `update_native_sol_only()` aendert nur native SOL, WSOL bleibt unveraendert
- `update_wsol_only()` aendert nur WSOL, native SOL bleibt unveraendert
- Nach simuliertem Wrap (SOL -X, WSOL +X) ist `total_native_sol() + wsol_balance()` konsistent

## Betroffene Dateien

| Datei | Aenderung |
|---|---|
| `src/storage/locks.rs` | +2 Methoden: `update_native_sol_only`, `update_wsol_only` |
| `src/bin/execution_engine.rs` | Event-Handler auf neue Methoden umstellen (~4 Zeilen je Handler) |
| `docs/grafana_multiprocess_dashboard.json` | 24h Delta Query mit avg_over_time |
