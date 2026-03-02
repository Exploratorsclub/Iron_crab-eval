# Plan: Hot-Path RPC-Freiheit (Priorität 3, I-4, I-7)

**Zweck:** Eval-Tests für die Invariante, dass DEX-Connectors bei `allow_rpc_on_miss = false` (Hot Path) bei Cache-Miss **keinen** RPC aufrufen und stattdessen `None` bzw. `Err` zurückgeben.

**Quellen:** Tests_todo.md Priorität 3, ARCHITECTURE_AUDIT.md §3.2, INVARIANTS.md I-4/I-7

---

## 1. Invariante und Kontext

### Formalisierung

**Invariante:** DEX-Connectors mit `allow_rpc_on_miss = false` liefern bei Cache-Miss (reserves/pool_accounts nicht in LivePoolCache) **sofort** `Ok(None)` oder `Err(...)` — ohne RPC-Aufruf.

**Begründung (ARCHITECTURE_AUDIT):**
- Hot Path (Arb, Momentum-Buy/Sell) nutzt `allow_rpc_on_miss = false`
- RPC im Hot Path bricht Latenz-Ziel (<1s)
- P3 #11/#12: Raydium, RaydiumCpmm, MeteoraDlmm, PumpFunAmmDex implementieren `allow_rpc_on_miss`

**Test-Strategie:** Dummy-RPC-URL (`http://127.0.0.1:0`). Bei korrekter Implementierung wird vor dem RPC-Fallback abgebrochen → keine Netzwerk-Anfrage. Falls die Implementierung fälschlich RPC ausführen würde, käme Connection-Refused oder Timeout.

---

## 2. Betroffene DEX-Connectors

| DEX | Konstruktor | Cache-Miss-Verhalten | Öffentliche API |
|-----|-------------|----------------------|-----------------|
| PumpFunAmmDex | `new_with_cache(rpc, cache, false)` | `quote_exact_in` → `Ok(None)`, `pool_accounts_v1_for_base_mint` → `Ok(None)` | Dex::quote_exact_in, pool_accounts_v1_for_base_mint |
| Raydium | `new_with_live_cache(rpc, None, false)` | `fetch_and_update_reserves` → `Err("GEYSER-ONLY")` | fetch_and_update_reserves |
| RaydiumCpmm | `new_with_live_cache(rpc, None, false)` | `quote_exact_in` → `Err("GEYSER-ONLY")` (nach set_pool_from_accounts, aber ohne Vault-Balances) | Dex::quote_exact_in |
| MeteoraDlmm | `new_with_live_cache(rpc, None, false)` | `quote_exact_in` → `Err("GEYSER-ONLY")` (analog) | Dex::quote_exact_in |
| Orca | `new_with_cache(rpc, None, Some(lpc))` | `quote_exact_in` → `Ok(None)` oder 0-Quote bei Cache-Miss | Dex::quote_exact_in |

**Orca:** Orca hat kein `allow_rpc_on_miss`-Flag. Entscheidend ist `live_pool_cache`: Wenn **gesetzt**, nutzt Orca bei Vault-Cache-Miss **statische Reserves** (0,0) und macht **keinen RPC** (orca.rs:409–427). RPC erfolgt nur, wenn `live_pool_cache.is_none()` (Cold Path). Daher ist Orca ebenfalls Hot-Path-RPC-frei und gehört in den Plan.

---

## 3. Implementierungsschritte

### 3.1 Testdatei `tests/invariants_hot_path_no_rpc.rs`

**Struktur:**

```rust
//! Invariante: Hot-Path RPC-Freiheit (INVARIANTS.md A.12, I-4, I-7)
//!
//! DEX-Connectors mit allow_rpc_on_miss=false liefern bei Cache-Miss None/Err ohne RPC.
```

**Tests:**

| Test | DEX | Setup | Aufruf | Erwartung |
|------|-----|-------|--------|-----------|
| `pump_amm_quote_cache_miss_no_rpc` | PumpFunAmmDex | Leerer Cache, allow_rpc_on_miss=false | quote_exact_in(unknown_mint) | Ok(None) |
| `pump_amm_pool_accounts_cache_miss_no_rpc` | PumpFunAmmDex | Leerer Cache, allow_rpc_on_miss=false | pool_accounts_v1_for_base_mint(unknown) | Ok(None) |
| `raydium_vault_cache_miss_no_rpc` | Raydium | inject_cached_amm_state (Pool-Meta), kein LivePoolCache, allow_rpc_on_miss=false | fetch_and_update_reserves(pool) | Err mit "GEYSER-ONLY" |
| `raydium_cpmm_quote_cache_miss_no_rpc` | RaydiumCpmm | set_pool_from_accounts (Pool-Struktur), kein LivePoolCache, allow_rpc_on_miss=false | quote_exact_in(...) | Err mit "GEYSER-ONLY" |
| `meteora_dlmm_quote_cache_miss_no_rpc` | MeteoraDlmm | set_pool_from_accounts, kein LivePoolCache, allow_rpc_on_miss=false | quote_exact_in(...) | Err mit "GEYSER-ONLY" |
| `orca_quote_cache_miss_no_rpc` | Orca | live_pool_cache gesetzt (leer), insert_mock_pool, kein Orca-State im Cache | quote_exact_in(...) | Ok(None) oder Quote mit 0 (kein RPC) |

**RPC-URL:** `http://127.0.0.1:0` (nicht erreichbar). Kein Timeout nötig – bei korrekter Implementierung erfolgt sofortiger Return.

### 3.2 Setup-Details pro DEX

**PumpFunAmmDex:**
- `LivePoolCache::new()` (leer)
- `PumpFunAmmDex::new_with_cache(rpc, cache, false)`
- `quote_exact_in(WSOL, unknown_base_mint, amount)` → Ok(None)
- `pool_accounts_v1_for_base_mint(unknown_base_mint)` → Ok(None)

**Raydium:**
- `Raydium::new_with_live_cache(rpc, None, false)`
- `inject_cached_amm_state(pool_addr, base_mint, quote_mint, base_vault, quote_vault, decimals, market_id)` – Pool-Struktur ohne Vault-Balances im Cache
- `fetch_and_update_reserves(&pool_addr)` → Err, Message enthält "GEYSER-ONLY" oder "vault reserves not in LivePoolCache"

**RaydiumCpmm:**
- `RaydiumCpmm::new_with_live_cache(rpc, None, false)`
- `set_pool_from_accounts(pool_addr, [pool, token_0, token_1, vault_0, vault_1])`
- `quote_exact_in(token_0, token_1, amount)` → Err mit "GEYSER-ONLY"
- Referenz: Iron_crab `raydium_cpmm.rs` Zeilen 701–732

**MeteoraDlmm:**
- `MeteoraDlmm::new_with_live_cache(rpc, None, false)`
- `set_pool_from_accounts(pool_addr, [pool, token_x, token_y, reserve_x, reserve_y])`
- `quote_exact_in(token_x, token_y, amount)` → Err mit "GEYSER-ONLY"
- Referenz: Iron_crab `meteora_dlmm.rs` Zeilen 957–989

**Orca:**
- `Orca::new_with_cache(rpc, None, Some(Arc::new(LivePoolCache::new())))` — leerer LivePoolCache
- `insert_mock_pool(pool_id, token_a, token_b, vault_a, vault_b, fee_rate)` — Pool-Struktur ohne Vault-Balances im LivePoolCache
- `quote_exact_in(token_a, token_b, amount)` → Bei Cache-Miss nutzt Orca (0,0) als Reserves → Ok(None) oder Quote mit amount_out=0, **kein RPC**

### 3.3 Spec-Ergänzung `docs/spec/INVARIANTS.md`

Neuer Abschnitt nach A.11:

```markdown
### A.12 Hot-Path RPC-Freiheit (I-4, I-7)
- **Datei:** `tests/invariants_hot_path_no_rpc.rs`
- **Invariante:** DEX-Connectors liefern bei Cache-Miss None/Err ohne RPC (Hot Path).
- **Getestet:** PumpFunAmmDex (quote, pool_accounts), Raydium, RaydiumCpmm, MeteoraDlmm (allow_rpc_on_miss=false). Orca (live_pool_cache gesetzt → bei Vault-Miss statische Reserves, kein RPC).
- **Kontext:** Hot Path (Arb, Momentum) darf keine blockierenden RPC-Calls ausführen.
```

### 3.4 `docs/Tests_todo.md` aktualisieren

- Priorität 3 als „in Arbeit“ bzw. nach Umsetzung „erledigt“ markieren
- Implementierungs-Checkliste: Test #3 auf „erledigt“ setzen
- Tabelle „Offene Invarianten“: I-4/I-7 als „Eval-getestet“ eintragen

---

## 4. Ablauf (Reihenfolge)

1. **Schritt 1:** `tests/invariants_hot_path_no_rpc.rs` mit 5 Tests anlegen
2. **Schritt 2:** `docs/spec/INVARIANTS.md` um A.12 ergänzen
3. **Schritt 3:** `docs/Tests_todo.md` aktualisieren
4. **CI:** `cargo fmt`, `cargo check`, `cargo clippy -p ironcrab-eval --all-targets -- -D warnings`, `cargo test`

---

## 5. Referenzen (Iron_crab)

- `src/solana/dex/pumpfun_amm.rs`: `allow_rpc_on_miss`, Zeilen 208–214, 2250–2256, Tests 2770–2780
- `src/solana/dex/raydium.rs`: Zeilen 1324–1330, Test 1829–1859
- `src/solana/dex/raydium_cpmm.rs`: Zeilen 227–233, Test 701–732
- `src/solana/dex/meteora_dlmm.rs`: Zeilen 229–235, Test 957–989

---

*Erstellt: Test Authority, ironcrab-eval*
