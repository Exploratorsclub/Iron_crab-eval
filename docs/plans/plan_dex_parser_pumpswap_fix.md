# Plan: PumpSwap dex_parser Bug-Fixes

**Datum:** 2026-03-11
**Supervisor:** IronCrab Supervisor
**Status:** Entscheidung getroffen — Option A für Bug 2. Delegation aktiv.

---

## Kontext

Der Momentum-Bot generiert fast keine Trade-Intents (1 in 1+ Stunde statt mehrere pro Minute).
Diagnose zeigt zwei Probleme im `dex_parser`:

1. **Bug 1 (Kritisch):** Guard-Check blockiert PumpSwap SELL-Transaktionen
2. **Bug 2 (Architektur):** Aggregator-routed Trades (Jupiter etc.) werden komplett verpasst

### Evidenz

- 1000 NATS-Events analysiert: nur 19 Trade-Events (1.9%), davon 1 pump_amm
- 142 TransactionDetected Events (unparsed Trades)
- Velocity-Filter rejected 86.6% aller Evaluierungen (11.9M von 13.8M)
- 3561 Tokens tracked, aber nur 1 passierte alle Filter

---

## Bug 1: Guard-Check `instruction_accounts.len() != 23`

### Problem

`src/solana/dex_parser.rs` Zeile 1087:
```rust
if update.instruction_accounts.len() != 23 {
    return None;
}
```

PumpSwap BUY: 23 Accounts → passiert ✓
PumpSwap SELL: 21 Accounts → wird verworfen ✗

### Historie

- `ee4c938f` (3. Jan): Führte PumpSwap-Parser mit `!= 23` ein (nur BUY-TXs beobachtet)
- `049290d8` (19. Jan): Fixte Account-Mapping für BUY/SELL korrekt, vergaß Guard-Check
- Bekanntes Muster: KNOWN_BUG_PATTERNS.md #14 (Account-Count)

### Fix

Zeile 1087 ändern:
```rust
if update.instruction_accounts.len() < 21 {
    return None;
}
```

### Impl-Handoff

**Ziel-Datei:** `Iron_crab/src/solana/dex_parser.rs`
**Änderung:** Eine Zeile — Guard-Check von `!= 23` auf `< 21`
**INVARIANTS.md prüfen:** Kein Impact auf bestehende Invarianten
**KNOWN_BUG_PATTERNS.md:** Pattern #14 aktualisieren mit diesem spezifischen Fix

### Eval-Test-Handoff

**Neue Invariante: A.20 DEX Parser PumpSwap Account-Count**
**Ziel-Datei:** `Iron_crab-eval/tests/invariants_dex_parser_pumpswap.rs`

Tests:
1. `pumpswap_buy_23_accounts_parsed` — BUY TX mit 23 Accounts → ParsedDexEvent::Trade mit is_buy=true
2. `pumpswap_sell_21_accounts_parsed` — SELL TX mit 21 Accounts → ParsedDexEvent::Trade mit is_buy=false
3. `pumpswap_insufficient_accounts_rejected` — TX mit <21 Accounts → None
4. `pumpswap_buy_sol_amount_correct` — BUY: sol_amount aus native balance change
5. `pumpswap_sell_sol_amount_correct` — SELL: sol_amount aus native balance change
6. `pumpswap_pool_accounts_14_elements` — pool_accounts Vec hat immer 14 Elemente
7. `pumpswap_discriminator_mismatch_rejected` — Falscher Discriminator → None

**Blackbox-Ansatz:** Tests konstruieren synthetische `GeyserTransactionUpdate` Structs
(Analog zu existierendem `test_pumpfun_amm_create_pool_parsing` in dex_parser.rs)

---

## Bug 2: Aggregator-CPI Trades (Architektur-Entscheidung nötig)

### Problem

Der Parser nutzt nur `instruction_data` / `instruction_accounts` vom Top-Level Instruction.
PumpSwap-Trades via Jupiter/Raydium-Aggregator erscheinen als CPI (Inner Instructions).
Das `inner_instructions` Feld in `GeyserTransactionUpdate` wird nicht ausgewertet.

### Datenstruktur (bereits vorhanden)

```rust
pub struct GeyserTransactionUpdate {
    // ... top-level ...
    pub instruction_accounts: Vec<Pubkey>,
    pub instruction_data: Vec<u8>,
    // CPI data (NICHT GENUTZT für Trade-Parsing):
    pub inner_instructions: Vec<InnerInstruction>,
    // ...
}

pub struct InnerInstruction {
    pub program_id_index: u8,     // Index in account_keys
    pub accounts: Vec<u8>,         // Indices in account_keys (NICHT Pubkeys!)
    pub data: Vec<u8>,
}
```

### Optionen

→ **Entscheidung durch User erforderlich**

---

### Option A: Inner-Instruction Fallback

**Beschreibung:**
Wenn Top-Level Parsing fehlschlägt (returns None), iteriere `inner_instructions`.
Für jede Inner Instruction: resolve `program_id` aus `account_keys[program_id_index]`.
Wenn bekannter DEX → resolve Account-Indices zu Pubkeys → parse wie Top-Level.

**Änderungen:**
- `parse_dex_transaction()` erweitern mit Fallback-Loop
- Neue Hilfsfunktion `resolve_inner_instruction_accounts()` 
- Jeder DEX-Parser bekommt die aufgelösten Pubkeys (identisches Interface)

**Vorteile:**
- Fängt ALLE CPI-Trades ab (Jupiter, Raydium Aggregator, beliebige Programme)
- Keine externe Abhängigkeit, nutzt vorhandene Daten
- Einmaliger Aufwand, funktioniert für alle zukünftigen Aggregatoren

**Nachteile:**
- Mittlere Komplexität: Inner-Instruction Account-Indices müssen in Pubkeys aufgelöst werden
- Potenzielle Duplikate: Wenn eine TX sowohl Top-Level als auch Inner Match hat
- Performance: Mehr Parsing pro TX (aber nur im Fallback-Pfad)

**Aufwand:** ~2-3 Tage Impl + Tests

---

### Option B: Account-State-basierte Trade-Erkennung

**Beschreibung:**
Statt TX-Instructions zu parsen: Pool-Vault-Accounts über Geyser Account-Updates monitoren.
Balance-Änderungen in Pool-Vaults = Trade. Richtung aus SOL-Vault-Delta ableitbar.

**Änderungen:**
- Neue Logik in `market_data.rs` Account-Update-Handler
- Pool-Vault-Tracking pro bekanntem Pool
- Trade-Event-Generierung aus State-Diff

**Vorteile:**
- Funktioniert unabhängig vom Aufruf-Pfad (direkt, CPI, beliebiger Aggregator)
- Kein Instruction-Parsing nötig

**Nachteile:**
- Kein `trader`-Feld ableitbar → `unique_buyers` kann nicht berechnet werden → **bricht Momentum-Filter**
- Kann individuelle Trades nicht trennen wenn mehrere Swaps im selben Slot
- Höhere Komplexität für State-Tracking
- **INKOMPATIBEL mit aktuellem Momentum-Bot Design** (braucht trader für unique_buyers)

**Aufwand:** ~5-7 Tage, erfordert Momentum-Bot Redesign

---

### Option C: Hybrid — Bekannte Aggregatoren parsen

**Beschreibung:**
Parser für Top-3 Aggregatoren hinzufügen (Jupiter V6, Raydium Route, etc.).
Aus deren Instruction-Format die inneren DEX-Swaps extrahieren.

**Änderungen:**
- Neue Parser-Funktionen pro Aggregator
- Aggregator-Program-IDs zur Subscription hinzufügen
- Inner-Instruction-Extraktion pro Aggregator-Format

**Vorteile:**
- Gezielt, überschaubare Komplexität pro Aggregator
- Bekannte Aggregator-Formate sind stabil dokumentiert

**Nachteile:**
- Wartungsaufwand: Jeder neue Aggregator braucht eigenen Parser
- Bricht bei Aggregator-Updates (neue Instruction-Formate)
- Deckt nur bekannte Aggregatoren ab (nicht Custom-Bots)

**Aufwand:** ~3-5 Tage initial, laufende Wartung

---

### Empfehlung (Supervisor)

**Option A (Inner-Instruction Fallback)** ist die robusteste Lösung:
- Nutzt bereits vorhandene Daten (`inner_instructions` ist im Struct)
- Einmaliger Aufwand, kein Wartungs-Overhead für neue Aggregatoren
- Kompatibel mit bestehendem Momentum-Bot Design (trader-Feld bleibt)
- Option B ist inkompatibel (kein trader → bricht unique_buyers)
- Option C hat laufenden Wartungsaufwand

**Entscheidung (2026-03-11):** Option A gewählt. Inner-Instruction Fallback wird implementiert.

---

## Deployment-Plan (nach Entscheidung)

### Phase 1: Bug 1 Fix (sofort)
1. Impl Agent: Guard-Check fixen + KNOWN_BUG_PATTERNS.md updaten
2. Test Authority: Eval-Tests für PumpSwap BUY/SELL Parsing
3. Build + Deploy auf Server
4. Verifizierung: NATS-Event-Sampling (pump_amm Trade-Count)

### Phase 2: Bug 2 Fix (nach Entscheidung)
1. Impl Agent: Gewählte Option implementieren
2. Test Authority: Eval-Tests für CPI-Trade-Parsing
3. Build + Deploy
4. Verifizierung: TransactionDetected-Count sinkt, Trade-Count steigt

---

## Querbezüge

- **KNOWN_BUG_PATTERNS.md** #14: PumpFun/PumpSwap pool_accounts Account-Count
- **INVARIANTS.md** B.2: Hot Path (I-4) — dex_parser ist Teil des Hot Path
- **Open Brain:** failure_pattern gespeichert (ID: 940a95b0)
- **EVAL_TEST_CANDIDATES.md** Z.102: dex_parser war als "Parser-Interna" markiert — wird jetzt als Invariante aufgenommen
