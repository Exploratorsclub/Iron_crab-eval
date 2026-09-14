# Pump AMM v14 Schicht C in MASTER Implementation Plan

> **For agentic workers:** Impl nur via Supervisor-Handoff / Cloud Agent auf `architecture-rebuild`. Kein lokaler Impl-Code durch den Supervisor.

**Goal:** Pump-`pool_accounts` (verifizierte 14er-Liste) überleben bis zum Pin im Adressbuch und danach als JetStream-Last-Message, damit MASTER nach Pin und nach MD-Restart Schicht C hat.

**Architecture:** A.52 bleibt: ungepinnt nur Buch, gepinnt nur MASTER, kein Subscribe-all. C ist TX-Pin-Payload, kein Quote-Feld. `DEFAULT_TTL_MS=120_000` bleibt für unvollständige Layouts; vollständige Pump-v14 werden nicht als „ungenutzter Discovery-Junk“ evictet. Vault-`BalanceUpdated` darf eine bekannte C-Liste nicht aus der JetStream-Last-Message streichen (Bug #33 zu Ende).

**Tech Stack:** Rust, `PoolAddressBook`, `LivePoolCache`, `md_sidefx_build_balance_updated_from_cache`, JetStream `PoolCacheUpdate`.

## Global Constraints

- Kein Hot-Path-RPC (I-7). Kein Cold-RPC-Seed in diesem PR (I-24d bleibt Request/Reply, ungenutzt hier).
- Kein Subscribe-all / kein TX-`TrackMint` (I-MD-5). Ungepinnt 0 Subs.
- I-MD-7 Cap: Buch-Eintraege zaehlen nicht als Subs. Cap 32768 bleibt; bei Over-Cap zuerst Eintraege **ohne** vollstaendige Pump-v14 evicten.
- Kein LastTradeMid. Quotes nur `ExecutableMarginal`. TX schreibt keine Reserves.
- Kein Quote-TTL-/Age-Pflaster (`MAX_PRICE_AGE_MS`, `arb_max_leg_age_slots`). `DEFAULT_TTL_MS` **nicht** von 120_000 auf einen groesseren Wert setzen.
- `create_arb_intent` 14er-Gate nicht lockern. Keine erfundene v14-Liste (I-12).
- Basis: GitHub `architecture-rebuild` @ `4bc28c2` (nicht lokaler stale Checkout).
- #444 Promote-before-Admit und #441 Buch bleiben.

---

## Dateien

| Datei | Rolle |
|-------|--------|
| `src/execution/pool_address_book.rs` | `evict_stale` / `evict_over_cap`: vollstaendige Pump-v14 halten; Unit-Tests |
| `src/bin/market_data.rs` | Promote Fill-Missing C (bestehendes `merge_tx_pool_accounts_into_existing`); Tests Pin nach „TTL-Zeit“ |
| `src/market_data/sidefx/handlers.rs` | `BalanceUpdated` / Cache-Heartbeat: `pool_accounts` aus MASTER anhaengen (FIX-26-Pattern) |
| `src/execution/live_pool_cache.rs` | nur wenn Helper fuer Metadata-Serialisierung noetig; kein Quote-Overwrite |
| `src/execution/pool_cache_sync.rs` | nur wenn Bootstrap-Test beweist, dass Last-Message ohne C eine vorhandene Zeile wipen wuerde — Preserve ist schon da; Fokus ist **Incoming mit C** |
| `src/metrics.rs` | optional Counter: Buch-Evict uebersprungen wegen Pump-v14; JetStream-BalanceUpdated-with-C |
| `docs/BUGS_FIXES.md` | kurz #33 Vault-Tick-Last-Message |

---

## Tasks

### 1. Adressbuch: vollstaendige Pump-v14 nicht TTL-evicten

`PoolAddressBook::evict_stale` (`pool_address_book.rs`):

- Eintrag `PoolLayoutKeys::PumpAmm { pool_accounts, .. }` mit `pool_accounts.len() >= 14` **behalten**, unabhaengig von `last_seen_ms`.
- Alle anderen DEX und Pump ohne v14 weiter nach `ttl_ms` (120 s) evicten.
- `DEFAULT_TTL_MS` Konstante unveraendert lassen.

`evict_over_cap`: wenn `len > cap`, zuerst LRU-Eintraege **ohne** vollstaendige Pump-v14 entfernen. Nur wenn danach immer noch `len > cap`, duerfen v14-Eintraege LRU-evictet werden (Cap bleibt hart).

`merge` / `insert_from_demote` rufen weiter `evict_stale` — ein frischer Orca-Merge darf eine 3 Minuten alte Pump-v14 **nicht** loeschen.

Unpin-Demote: C darf zurueck ins Buch (bestehendes `from_cached_layout_state` kopiert `pool_accounts`). Danach gilt dieselbe TTL-Ausnahme, bis erneut gepinnt wird.

### 2. Promote kopiert C fill-missing (Regressionsschutz)

`promote_address_book_to_master_if_needed` merget bereits `pool_accounts` wenn existing leer und incoming nicht. Tests, die das **beweisen** muessen (heute fehlt der Pump-C-Fall):

- Buch hat Pump-v14 + Vaults, MASTER leer, Pin → MASTER `pool_accounts.len()>=14`, Buch leer, Promote gezaehlt.
- MASTER hat Pump-Vaults und `pool_accounts=[]`, Buch hat v14 → Fill-Missing, Reserves/`as_of` unveraendert.
- Buch hat nur Vaults (kein v14) → Promote ohne erfundene Accounts; Account-Decode bleibt Fallback.
- Ungepinnt TX mit v14 → Buch, kein MASTER, kein Explicit-Zuwachs.

Kein zweites MASTER. Nach erfolgreichem Promote mit C: Buch-Eintrag weg (Pool ist gepinnt).

### 3. JetStream Last-Message traegt C sobald MASTER sie kennt (Bug #33 zu Ende)

Ist: Trade-FIX-33 schreibt C. `md_sidefx_build_balance_updated_from_cache` baut `BalanceUpdated` **ohne** `pool_accounts` (0 Treffer im Body). Vault-Ticks ueberschreiben die Last-Message. MD-Restart bootstrapped leeres C; Preserve (#28) hilft nicht, weil MASTER leer ist.

Soll: Helper analog FIX-26 (Account-Pfad ~`effective_pool_accounts` aus State, sonst `get_pump_amm_pool_accounts`):

- Wenn `CachedPoolState::PumpAmm` und `pool_accounts` nicht leer: Metadata-Key `"pool_accounts"` = comma-joined Pubkeys (bestehendes Wire-Format).
- Aufrufen aus `md_sidefx_build_balance_updated_from_cache` **und** jedem anderen `new_balance_updated`-Pfad fuer Pump AMM (Vault-Tick, Heartbeat), der JetStream enqueued.
- Bestehenden Trade-FIX-33-Publish nicht entfernen.
- Keine Quotes/Reserves aus TX. Keine v14 erfinden, wenn MASTER `[]` hat.

SLAVE `apply_pool_cache_update` Preserve fuer leere Incoming bleibt. Nach diesem Task ist Incoming nach Restart **nicht** mehr leer, wenn C je beobachtet wurde und danach nur Vault-Ticks kamen.

### 4. Hot-TX C-Write bleibt Pin-Seed (kein Subscribe-all)

Bereits: `apply_tx_pool_accounts_for_hot_pool` upsert + `set_pump_amm_pool_accounts_at_slot` wenn hot und `len>=14`; Handler ruft danach `md_sidefx_pin_seed_write_pump_layer_c`.

Nur haerten, wenn Tests eine Luecke zeigen:

- Hot + TX-v14 + MASTER-Zeile existiert (nach Promote) → C landet, auch wenn Pin-Seed-Metrik vorher 0 war.
- `MissNoCacheEntry`: Layout-only aus derselben TX sicherstellen, dann C setzen (Handoff 2026-08-23). Kein RPC.
- Non-hot: weiter nur Buch, kein MASTER-C.

Keine Aenderung an `create_arb_intent`. Optional Arb: `missing_accounts` auf WARN mit buy/sell pool+dex — nur wenn eine Zeile reicht, kein Refactor.

### 5. Tests (Impl-Unit)

`pool_address_book.rs`:

- Pump-v14 mergen, `now + 180_000`, `evict_stale` → Eintrag da; Pump ohne v14 / Orca → weg.
- Cap voll mit Junk + eine v14 → Over-Cap entfernt Junk zuerst.

`market_data.rs` (bestehende Address-Book-Testfixtures):

- Pin nach simulierter >120 s seit Buch-Write → MASTER hat v14 (Task 1+2).
- Ungepinnt bleibt ohne MASTER.

Handler / Cache-Update (wo heute FIX-26/33 Tests sitzen):

- MASTER hat v14, `md_sidefx_build_balance_updated_from_cache` → Metadata enthaelt dieselben 14 Keys.
- Apply der Nachricht auf leeren Cache → `pool_accounts.len()==14`.

Keine Eval-Testdateien in diesem PR.

### 6. Nicht in diesem PR

- Pump-TX-Parser Account-Count-Traps (Pattern #14).
- DLMM Bin-Walker / mid vs executable.
- Deploy. LastTradeMid. Slot-Sustain. Relatives Pairing. Subscribe-all. Quote-TTL. Hot-Path-RPC. `architecture-rebuild-next`. `DEFAULT_TTL_MS` hochsetzen. 14er-Gate lockern. Kanonisierung `protocol_fee_recipient` (#35).
