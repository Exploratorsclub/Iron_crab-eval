# Handoff: ARCHITECTURE_AUDIT – Offene Verstöße abarbeiten

**Erstellt:** 2026-03-04 | **Quelle:** ARCHITECTURE_AUDIT.md §4.2, §3.4, §5 BUG A

---

## 1. Übersicht

Drei offene bzw. unvollständige Verstöße aus dem Architektur-Audit:

| # | Verstoß | Priorität | Datei/Modul |
|---|---------|-----------|-------------|
| 1 | PumpSwap AMM quote_mint hardcodiert | Gering | dex_parser.rs ~1115 |
| 2 | BUG A: Liquidation überspringt Token (Edge Cases) | Mittel | execution_engine.rs |
| 3 | wallet.rs / wsol_manager.rs RPC-Verstöße | Prüfen | wallet.rs, wsol_manager.rs |

---

## 2. Verstoß 1: PumpSwap quote_mint (SSOT)

**ARCHITECTURE_AUDIT §4.2:** PumpSwap AMM quote_mint hardcodiert (`dex_parser.rs:952`) — ⚠️ Potenziell bei non-SOL-PumpSwap-Pools; Risiko gering.

**Aktueller Code (dex_parser.rs ~1115):**
```rust
// instruction_accounts[4] = quote_mint (WSOL)
let quote_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
```

**Problem:** `quote_mint` wird hardcodiert statt aus `instruction_accounts[4]` gelesen. Bei non-SOL-PumpSwap-Pairs (z.B. USDC) wäre das falsch.

**Fix:** `quote_mint` aus `update.instruction_accounts.get(4).copied()` lesen; Fallback auf `SOL_MINT_PUBKEY` wenn nicht vorhanden (Rückwärtskompatibilität).

**Referenz:** Meteora/Raydium CPMM nutzen bereits `extract_quote_mint` bzw. vault-mint-basierte Ableitung (BUG H behoben). Gleiches Muster anwenden.

---

## 3. Verstoß 2: BUG A – Liquidation Edge Cases

**ARCHITECTURE_AUDIT §5:** Token werden übersprungen bei `min_out_sol.is_none()`, fehlendem Creator im Cache, `pool_accounts_v1_for_base_mint()` → None.

**Status:** TEILWEISE BEHOBEN — Multi-Pool zuerst, PumpFun Fallback, 6005-Retry implementiert.

**Offen:** Defensive Checks für Edge Cases:
- Logging wenn Token übersprungen wird (mit Grund)
- Ggf. Retry-Logik wenn Creator erst später im Cache erscheint

**KNOWN_BUG_PATTERNS §18:** Liquidation: Stale Data vs. RPC — Multi-Pool-Reihenfolge, RPC-Fallback für Creator.

---

## 4. Verstoß 3: wallet.rs / wsol_manager.rs

**ARCHITECTURE_AUDIT §3.4:**
- wallet.rs: `get_balance()`, `get_account(mint)`, `get_account(&ata)` — VERSTOSS wenn im Hot Path
- wsol_manager.rs: `get_balance()`, `get_token_account_balance()` — VERSTOSS

**Analyse erforderlich:**
1. **Caller-Analyse:** Wer ruft `Wallet::sol_balance()`, `token_program_for_mint()`, `build_ata_ix()` auf?
2. **wsol_manager:** Nutzt er Geyser/JetStream für Balances oder RPC? (Aktuell: AtomicU64 aus WalletBalanceSnapshot — event-driven)
3. **Hot vs. Cold Path:** Alle Aufrufer von wallet/wsol_manager klassifizieren.

**Falls nur Cold Path:** Als „AKZEPTIERT (by design)“ dokumentieren (wie BUG E cleanup_wallet).
**Falls Hot Path:** Geyser-Alternative implementieren.

---

## 5. Invarianten (INVARIANTS.md)

- I-4 / I-7: Kein RPC im Hot Path
- I-16: Geyser/LivePoolCache autoritativ im Hot Path

---

## 6. Erlaubte Dateien (Impl Agent)

- `Iron_crab/src/solana/dex_parser.rs`
- `Iron_crab/src/bin/execution_engine.rs` (nur Liquidation-relevante Teile)
- `Iron_crab/src/wallet.rs` (nur Analyse, ggf. Kommentare)
- `Iron_crab/docs/ARCHITECTURE_AUDIT.md` (Status-Update)

---

## 7. Verboten

- Keine Änderungen an Eval-Tests (Iron_crab-eval)
- Keine RPC-Entfernung aus Cold Path (I-6)
