# Warum Pump AMM v14 nicht im MASTER ist (obwohl sie schon mal da waren)

**Datum:** 2026-09-15  
**SHA MD prod:** `795fe5b` (#444 promote-before-admit), seit 22:04:05 CEST 2026-09-14  
**SHA GitHub `architecture-rebuild`:** `4bc28c2` (#445 rustls; src unverändert ggü. #444)  
**Arb/EE:** unverändert seit 02:25 (`6f09b84`)

## Kurz

Sie fehlen nicht, weil der Parser sie nie gesehen hat. Sie fehlen im **MASTER**, weil Schicht C nach A.52 **nicht** in der Quote-SSOT lebt, solange der Pool ungepinnt ist — und weil der einzige persistente Kanal (JetStream Last-Message) sie danach wieder löscht.

Quotes (Vault-Reserves) und Intents (`DexPoolAccounts` / 14er-Liste) sind zwei Rohre. Prod beweist das: Orca→Pump-Opps mit ~9–10 % Spread, `missing_accounts` fast 1:1 zu den Pump-Beinen, Pin-Seed Pump AMM nur **13–15 Writes / ~3 h**, `write_miss=0`.

## Was „schon mal da“ bedeutet

Drei verschiedene Speicher, die Leute alle „Cache“ nennen:

| Speicher | Pump v14? | Überlebt MD-Restart? | Überlebt 120 s ohne Trade? |
|----------|-----------|----------------------|----------------------------|
| NATS Core `DexPoolAccounts` | ja, live | nein | nein (kein Persist) |
| Adressbuch (ungepinnt) | ja, wenn TX `len>=14` | **nein (RAM)** | **nein (`DEFAULT_TTL_MS=120_000`)** |
| MASTER `LivePoolCache` | nur nach Pin + C-Write / Promote | **nein (RAM)**; Snapshot restored Explicit-Keys, nicht C | ja, bis Unpin/Wipe |
| JetStream `POOL_CACHE` Last-Message | nur wenn Metadata `pool_accounts` in der **letzten** Nachricht steht | ja | ja |

Vor A.52 (#441) schrieb TX Layout inkl. C in **dieselbe** `LivePoolCache`-Map. Das ist die Erinnerung „die waren im MASTER“. Nach der Trennung Buch vs MASTER ist ungepinntes C **absichtlich** nicht mehr MASTER.

Arb-Metrik `arb_pool_accounts_backfill_total{source=live_cache}=2510` ist **Lifetime seit 02:25** — also inkl. Stunden vor dem MD-Restart 22:04. Nach dem Restart ist das kein Nachweis, dass C **jetzt** in MASTER sitzt. Aktuell `missing_accounts` (Lifetime 153, +92 seit #444).

## Ist-Pfad (GitHub `architecture-rebuild` @ `4bc28c2`)

1. `md_sidefx_process_pump_amm_trade` ruft immer `apply_tx_pool_accounts_for_hot_pool`.
2. **Ungepinnt:** `cache_state_from_tx_pool_accounts` setzt `pool_accounts = accounts.to_vec()` wenn `len>=14`, dann `book.merge`. **Kein** MASTER. `md_sidefx_pin_seed_write_pump_layer_c` returnt sofort `if !is_hot`.
3. **Gepinnt:** Fill-Missing in MASTER + `set_pump_amm_pool_accounts_at_slot`, danach nochmal Pin-Seed-C. Metrik `pump_amm` written zählt nur diesen Hot-Pfad.
4. Trade publiziert FIX-33 `PoolCacheUpdate` mit `metadata["pool_accounts"]` auf JetStream — **wenn** `len>=14` und NATS an.
5. Danach schreiben Vault-Ticks `BalanceUpdated` über `md_sidefx_build_balance_updated_from_cache`. Diese Funktion hat **0× `pool_accounts`**. Last-Message pro Pool = Reserves **ohne** C.
6. Account-Pfad hat FIX-26 (C an `PoolDiscovered` anhängen, falls MASTER sie hat). Ein späterer Vault-Tick macht das wieder zunichte.
7. `PoolAddressBook::evict_stale`: **alle** Einträge, inkl. vollständiger Pump-v14, nach 120 s ohne `merge`.

Account-Decode darf v14 nicht erfinden (I-4 / I-12). Promote kopiert nur, was noch im Buch liegt. Pin nach TTL oder nach Restart → MASTER mit Vaults (Rohr B), C leer → Quotes ja, Intents nein.

## Prod-Kette nach #444 (MD-Restart 22:04)

1. RAM-Buch weg, RAM-MASTER weg.
2. JetStream-Bootstrap: letzte Nachricht typisch `BalanceUpdated` ohne C → MASTER/SLAVE starten ohne 14er-Liste (KNOWN_BUG_PATTERNS **#33**, verschärft durch Vault-Ticks).
3. #444 Promote-before-Admit füllt Vault-Keys aus dem Buch — **wenn** das Buch sie noch hat. C ist da oft schon TTL-tot.
4. Re-Seed nur bei **hot** Pump-TX mit `len>=14`: 15 Writes, Completeness ~5 complete / ~100 missing_vault.
5. Dominant Opp-Mint `Ce2gx9KG…pump`, Kante orca→pump_amm. Spread ~930–990 bps. Hard stop bleibt fehlendes Pump-C, nicht die Sim.

Der eine Intent der Nacht (`arb-74389790-000000`) war **orca→DLMM**, brauchte kein Pump-v14, und starb an `post_sim_pnl` / `FEE_UNPROFITABLE`. Das ist ein anderer Scope.

## Was der Scope nicht ist

- Parser-Account-Count (`<21` vs 23/26/27) — Follow-up, nicht dieser PR.
- DLMM mid vs executable.
- Age/TTL der **Quotes** anheben (`MAX_PRICE_AGE_MS`, `arb_max_leg_age_slots`, `DEFAULT_TTL_MS` als Zahl).
- Hot-Path-RPC, Subscribe-all, LastTradeMid, 14er-Gate in `create_arb_intent` lockern.
