# Plan: Positions-Counter-Drift und Liquidation-Failure Fix

**Status:** Impl + Eval abgeschlossen. Alle Tests bestanden. Bereit fuer Deployment.
**Datum:** 2026-03-04
**Invarianten:** A.28 (Open Positions Counter), A.29 (Liquidation Vollstaendigkeit)
**Betroffene Repos:** Iron_crab (Impl), Iron_crab-eval (Tests)

---

## Problem A: Open-Positions-Counter driftet (3 statt 5)

**Root Cause:** `open_positions` (AtomicUsize) wird ueber zwei unabhaengige Pfade modifiziert:
1. Execution Result Handler (Zeile 8062-8118): BUY fetch_add(1), SELL fetch_sub(1)
2. Geyser WalletBalanceSnapshot Handler (Zeile 5801-5832): Balance-Transitions

Race Conditions fuehren zu nicht-deterministischem Drift.

**Fix:** `open_positions` als abgeleiteten Wert aus `LockManager.count_non_zero_token_balances()`.
- Entferne separaten AtomicUsize Counter
- Entferne alle fetch_add/fetch_sub Aufrufe
- `get_open_positions()` leitet aus LockManager ab
- OPEN_POSITIONS_GAUGE periodisch aktualisiert

## Problem B: Kill-Switch-Liquidation scheitert

**Root Cause (wahrscheinlich):**
1. `cashback_enabled` defaulted auf `false` (pool_cache_sync.rs:227, pumpfun.rs unwrap_or(false))
2. `run_liquidation_job().await` blockiert Main-Loop (execution_engine.rs:5646)

**Fixes:**
- B1: RPC-Fallback fuer cashback_enabled in build_swap_ix_async_with_slippage
- B2: Liquidation als tokio::spawn statt .await
- B3: Retry fuer fehlgeschlagene Token

## Tests

- A.28: invariants_open_positions.rs (4 Tests)
- A.29: invariants_liquidation_flow.rs erweitert (5 Tests)
