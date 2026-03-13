# Plan: PumpSwap Pool Discovery & Bootstrap Fix (Bug #31, #32, #33)

## Kontext

### Performance-Messung (Server, 2026-03-13)
| RPC Call | Ergebnis | Dauer |
|----------|----------|-------|
| `getAccountInfo(pool)` | 1 Account, 301 Bytes | **0.9ms** |
| `getProgramAccounts(PumpSwap, memcmp_filter)` | 1 Match aus 6.2M Accounts | **25.4s** |
| `getProgramAccounts(PumpSwap, dataSlice_only)` | 6,241,723 Accounts | **56.4s** |
| `getTokenAccountsByOwner(coin_creator)` | -32010 REJECTED | sofort |

### Root Causes
1. **`try_parse_pool_static_from_market_account_inner`** nutzt `getTokenAccountsByOwner` als Fallback fuer coin_creator/protocol_fee_recipient ATAs, die nicht on-chain existieren → scheitert an Validator Secondary Index Whitelist (-32010)
2. **JetStream Bootstrap-Luecke:** `pool_accounts` werden bei Trade-Parsing in den MASTER LivePoolCache gesetzt und per `DexPoolAccounts` auf NATS pub/sub emittiert (nicht persistent). Ein separates `PoolCacheUpdate` auf JetStream wird NUR bei Geyser-Account-Updates publiziert. Fuer inaktive Pools nach dem letzten Trade → kein neues PoolCacheUpdate → JetStream hat alte Nachricht ohne pool_accounts → Bootstrap nach Restart hat leere pool_accounts.
3. **Liquidation-Timeout 10s ist zu kurz:** `getProgramAccounts` braucht ~26s als letzter Fallback.

---

## Fix A: `try_parse_pool_static_from_market_account_inner` — ATA-Only, kein getTokenAccountsByOwner (Bug #32)

**Datei:** `Iron_crab/src/solana/dex/pumpfun_amm.rs`

**Problem:** Die Closure `find_authority_with_existing_token_account` (Zeile 553-590) hat einen "Slow path" (Zeile 576-587) der `find_any_token_account_for_owner_and_mint` aufruft → `getTokenAccountsByOwner` → scheitert mit -32010.

**Fix:** Den "Slow path" (Zeile 576-587) entfernen bzw. durch ATA-Derivation ersetzen:
- Wenn die ATA fuer einen Kandidaten nicht on-chain existiert (getAccountInfo returns None), trotzdem die abgeleitete ATA-Adresse zurueckgeben
- PumpSwap erstellt die ATA waehrend des Swaps automatisch (CreateIdempotent)
- Fuer WSOL (quote_mint) ist das Token-Program immer SPL Token, also ist die ATA eindeutig ableitbar

**Aenderung:**
```rust
// VORHER (Zeile 553-590):
let find_authority_with_existing_token_account = |candidates: Vec<Pubkey>, mint: Pubkey| async move {
    for cand in candidates {
        // 1) Fast path: ATA exists.
        for tp in [token_program, token_2022_program] {
            let ata = Self::derive_ata_with_program(cand, mint, tp);
            // ... getAccountInfo check ...
            if ata_mint == mint && ata_token_owner == cand {
                return Ok(Some((cand, ata)));
            }
        }
        // 2) Slow path: getTokenAccountsByOwner ← PROBLEM
        if let Some(ta) = self.find_any_token_account_for_owner_and_mint(...).await? {
            return Ok(Some((cand, ta)));
        }
    }
    Ok(None)
};

// NACHHER:
let find_authority_with_token_account = |candidates: Vec<Pubkey>, mint: Pubkey| async move {
    for cand in candidates {
        // 1) Fast path: ATA exists on-chain → return it
        for tp in [token_program, token_2022_program] {
            let ata = Self::derive_ata_with_program(cand, mint, tp);
            if let Some((ata_owner, ata_exec, ata_data)) =
                self.rpc_get_account_owner_executable_and_data(ata).await?
            {
                if !ata_exec && (ata_owner == token_program || ata_owner == token_2022_program) {
                    if let Some((ata_mint, ata_token_owner)) =
                        Self::parse_spl_token_account_mint_and_owner(&ata_data)
                    {
                        if ata_mint == mint && ata_token_owner == cand {
                            return Ok(Some((cand, ata)));
                        }
                    }
                }
            }
        }
        // 2) ATA does not exist on-chain → derive and use anyway.
        //    PumpSwap creates it via CreateIdempotent during the swap.
        //    For WSOL (quote_mint), always use SPL Token.
        //    NO getTokenAccountsByOwner fallback (incompatible with restricted secondary indexes).
        if mint == expected_quote_mint {
            let ata = Self::derive_ata_with_program(cand, mint, token_program);
            return Ok(Some((cand, ata)));
        }
    }
    Ok(None)
};
```

**Test (Unit):** `test_find_authority_derives_ata_without_rpc_fallback` — Verify that when ATA doesn't exist on-chain, the derived ATA address is returned (no getTokenAccountsByOwner call).

**Test (Eval):** Invariante A.37: `try_parse_pool_static_from_market_account_inner` darf NICHT `getTokenAccountsByOwner` aufrufen.

---

## Fix B: JetStream pool_accounts Persistenz (Bug #33)

**Datei:** `Iron_crab/src/bin/market_data.rs`

**Problem:** Bei Trade-Parsing werden `pool_accounts` im MASTER Cache gesetzt und per `DexPoolAccounts` auf NATS pub/sub emittiert (nicht persistent). Wenn kein weiteres Geyser-Account-Update fuer den Pool kommt, hat JetStream keine Nachricht mit pool_accounts.

**Fix:** Nach `set_pump_amm_pool_accounts()` bei Trade-Parsing (Zeile 3413-3418) ZUSAETZLICH ein `PoolCacheUpdate` mit pool_accounts in metadata auf JetStream publizieren. Konkret:
1. Nach Zeile 3418 (`ctx.live_pool_cache.set_pump_amm_pool_accounts(...)`)
2. Ein `PoolCacheUpdate` mit `update_type: "PoolDiscovered"`, `dex: "pump_amm"` und `metadata: {"pool_accounts": "..."}` auf den JetStream POOL_CACHE Subject publizieren
3. Gleiche Logik fuer `create_pool` Pfad (Zeile 3317-3318)

**Test (Integration):** Nach Trade-Parsing muss ein `PoolCacheUpdate` mit nicht-leeren `pool_accounts` in JetStream vorhanden sein. Bootstrap nach Restart muss diese pool_accounts korrekt lesen.

**Test (Eval):** Invariante A.38: Wenn market-data pool_accounts per Trade-Parsing erhaelt, MUSS ein PoolCacheUpdate mit pool_accounts in JetStream publiziert werden.

---

## Fix C: Liquidation-Timeout erhoehen (Bug #31 Workaround)

**Datei:** `Iron_crab/src/bin/execution_engine.rs`

**Problem:** Hardcoded `Duration::from_secs(10)` an Zeilen 1645 und 2020 ist zu kurz fuer `getProgramAccounts` Fallback (~26s).

**Fix:** Timeout auf `Duration::from_secs(45)` erhoehen. Langfristig konfigurierbar machen (config.toml).

**Aenderung:** Zeile 1645 und 2020: `Duration::from_secs(10)` → `Duration::from_secs(45)`

**Test (Unit):** Verify that the timeout constant is >= 30s.

---

## Fix D: EE Startup — proaktives pool_accounts Seeding (Enhancement)

**Datei:** `Iron_crab/src/bin/execution_engine.rs` (nach Bootstrap)

**Problem:** Nach JetStream Bootstrap haben PumpSwap Pools oft leere pool_accounts. Erst wenn ein Trade beobachtet wird, werden sie befuellt. Fuer inaktive Pools passiert das nie.

**Fix:** Nach `bootstrap_pool_cache_from_jetstream()`, alle PumpSwap Pools im Cache durchgehen, die keine pool_accounts haben. Fuer jeden:
1. Pool-Adresse ist bekannt (im Cache)
2. `getAccountInfo(pool_address)` aufrufen (schnell, <1ms)
3. Pool-Daten parsen und PumpAmmPoolStatic ableiten (nutzt Fix A)
4. pool_accounts in den SLAVE Cache schreiben

Das ist ein einmaliger Cold-Start Aufwand (~1ms pro Pool × Anzahl Pools ohne pool_accounts).

**Test (Integration):** Nach Bootstrap muessen alle PumpSwap Pools im Cache gueltige pool_accounts haben.

---

## Reihenfolge

1. **Fix A** (blockierend) — Eliminiert `getTokenAccountsByOwner` Abhaengigkeit
2. **Fix C** (schnell, Safety-Net) — Timeout erhoehen
3. **Fix B** (JetStream Persistenz) — Nachhaltige Loesung fuer Bootstrap
4. **Fix D** (Startup Seeding) — Belt-and-Suspenders fuer Cold Start

## Betroffene Dateien

| Fix | Datei | Bereich |
|-----|-------|---------|
| A | `src/solana/dex/pumpfun_amm.rs` | `try_parse_pool_static_from_market_account_inner`, Zeile 550-590 |
| B | `src/bin/market_data.rs` | Trade-Parsing PumpSwap, Zeile 3413-3418 und 3317-3318 |
| C | `src/bin/execution_engine.rs` | Liquidation Quote Timeout, Zeile 1645 und 2020 |
| D | `src/bin/execution_engine.rs` | Nach Bootstrap, neuer Abschnitt |

## Neue Invarianten

| ID | Beschreibung |
|----|-------------|
| A.37 | `try_parse_pool_static_from_market_account_inner` darf NICHT `getTokenAccountsByOwner` aufrufen |
| A.38 | Bei Trade-Parsing mit pool_accounts MUSS ein PoolCacheUpdate auf JetStream publiziert werden |
| A.39 | Liquidation-Quote-Timeout muss >= 30s sein |
| A.40 | Nach Startup-Bootstrap muessen alle PumpSwap Pools im Cache gueltige pool_accounts haben |
