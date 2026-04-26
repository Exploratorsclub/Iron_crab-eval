# Plan: PositionAuthority als echte Positions-SOT

**Status:** Entwurf fuer Supervisor-Rollout / noch nicht implementiert  
**Datum:** 2026-04-26  
**Betroffene Repos:** `Iron_crab` (Impl), optional spaeter `Iron_crab-eval` (Blackbox-Vertraege)  
**Motivation:** Wiederkehrende Positions-Drift-Probleme durch mehrere lokale Wahrheiten (`momentum-bot.positions`, `execution-engine` LockManager, `market-data` ATA-Tracking / WalletSnapshots).

---

## Architekturziel

Langfristig soll ein eigener `position-manager` die **dauerhafte Positionswahrheit** besitzen.

Rollen:

- `market-data`: Chain-/Geyser-Daten, WalletBalanceSnapshots, ATA-/Mint-Beobachtung.
- `execution-engine`: validieren, locken, simulieren, signieren, senden. Der LockManager bleibt nur fuer In-Flight-Reservations und Konfliktvermeidung.
- `momentum-bot`: Strategie-Overlay fuer Exit-Entscheidungen (PnL, Stop-Loss, Trailing, Hold-Time), aber keine dauerhafte Positionswahrheit.
- `position-manager`: einzige dauerhafte SOT fuer offene Wallet-Positionen, Lots, Balances, Status und Reconciliation.

Wichtig: Diese Migration soll nicht als Big Bang erfolgen. Jeder Scope muss eigenstaendig testbar sein und darf Hot-Path-RPC-Freiheit nicht verletzen.

---

## Invarianten / Architekturregeln

### I-24a JetStream = SSOT fuer Bot-Zustand

Wallet-Balances, Positionen, Pool-Cache und Config gehoeren in JetStream/persistente Streams. Konsumenten bootstrappen daraus und holen Live-Updates von dort.

### I-1 / I-2 Role Separation

Nur `execution-engine` laedt Keys und signiert/sendet. `market-data`, `momentum-bot`, `arb-strategy`, `control-plane` und spaeter `position-manager` bleiben keyless.

### I-7 Hot Path RPC-Freiheit

Momentum-/Arb-/normaler Execution-Hot-Path darf keine blockierenden RPC-Calls ausfuehren. Reconciliation-RPC nur Cold Path.

### I-20/I-21 Locks

LockManager bleibt fuer Capital-/Resource-Locks und In-Flight-Reservations zustaendig. Er ist nicht die dauerhafte Positions-SOT.

---

## Zielmodell PositionAuthority

### Events

Der `position-manager` konsumiert und persistiert ein append-only Event-Log:

- `BuyConfirmed`
- `SellConfirmed`
- `WalletBalanceSnapshot`
- `WalletSnapshotComplete`
- `AtaClosed` / `BalanceZero`
- `ReserveRequested`
- `ReserveReleased`
- `ReconciliationCorrection`

### State

Abgeleiteter State pro Mint/ATA:

- `mint`
- `ata`
- `token_program`
- `decimals`
- `balance_raw`
- `lots` / BUY-Fills
- `sold_raw_total`
- `reserved_raw`
- `status = Open | Closing | Closed | ReconcileNeeded`
- `last_wallet_snapshot_slot`
- `last_execution_id`
- `last_update_source`

### Read API / Snapshot

Mindestens:

- `GetOpenPositions`
- `GetPosition(mint)`
- `GetTradableBalance(mint)` = `balance_raw - reserved_raw`
- `SubscribePositionUpdates`
- `OpenPositionsSnapshot` fuer Dashboard/Prometheus

---

## Rollout-Schnitt

### Scope PA-1: Read-only PositionAuthority im Impl-Repo

Ziel:

- Neues Modul oder Prozessgeruest, keyless.
- Konsumiert `ExecutionResult` und `WalletBalanceSnapshot`.
- Baut eigenen PositionState, beeinflusst aber noch keine Trading-Entscheidung.
- Vergleicht optional gegen `LockManager.get_open_positions()` und loggt Drift.

Erlaubte Dateien:

- neues `src/position_authority/*` oder kleiner `src/bin/position_manager.rs`
- NATS/JetStream Wiring nur lesend
- Tests fuer Event-Reduce-Logik

DoD:

- Unit-Tests fuer probe+scale-in, partial sell, full sell, wallet zero snapshot.
- Kein Hot-Path-RPC.
- Keine Aenderung an Execution/Momentum-Entscheidungen.

### Scope PA-2: Dashboard/OpenPositions aus PositionAuthority lesen

Ziel:

- Dashboard/OpenPositions-Snapshot aus PositionAuthority statt LockManager/Momentum ableiten.
- Keine Trading-Entscheidungen betroffen.

DoD:

- Metrics zeigen `open_positions_authority`.
- Alte Metrik optional parallel fuer Drift-Vergleich.

### Scope PA-3: Execution BUY-Gate gegen PositionAuthority

Ziel:

- `max_open_positions` nutzt PositionAuthority-Snapshot.
- Fallback auf LockManager nur wenn Authority nicht verfuegbar ist; Fallback muss im DecisionRecord sichtbar sein.

DoD:

- BUY mit Authority count >= max wird rejected.
- Kein RPC im Gate.
- DecisionRecord enthaelt `authority_current`, `lockmanager_current`, `strategy_current`, `effective_current`.

### Scope PA-4: Execution SELL-Preflight / Reservations

Ziel:

- LockManager reserviert gegen `PositionAuthority.GetTradableBalance`.
- LockManager bleibt lokal fuer In-Flight-Konflikte, aber nicht fuer dauerhafte Position.

DoD:

- SELL kann nur bis `tradable_balance` reservieren.
- Confirmed SELL erzeugt `SellConfirmed`; Authority reduziert Balance.
- Failed/Rejected gibt Reservation frei.

### Scope PA-5: Momentum auf read-only PositionView umstellen

Ziel:

- `momentum-bot.positions` wird Strategie-Overlay, nicht Positions-SOT.
- Exit-Intent-Amount kommt aus PositionAuthority.
- Momentum schliesst Overlay erst, wenn Authority `Closed` meldet.

DoD:

- Probe+scale-in Exit verkauft gesamte Authority-Balance.
- Partial SELL laesst Overlay aktiv und retry-faehig.
- Max hold/SL/TP arbeiten gegen Authority-Positionen.

### Scope PA-6: Alte doppelte Stores abbauen

Ziel:

- Entferne/entwerte dauerhafte Positionswahrheit aus Momentum-KV und LockManager.
- Behalte Recovery-Backfills nur als Migrationspfad.

DoD:

- Kein produktiver Pfad nutzt Momentum-KV oder LockManager als dauerhafte Positions-SOT.
- Drift-Metriken bleiben fuer eine Uebergangsphase.

---

## Eval-/Teststrategie

### Impl-Tests

- Event reducer: BUY + scale-in + partial SELL -> Rest bleibt open.
- Full SELL / wallet zero -> Closed.
- WalletSnapshot mit Balance > EventState -> ReconciliationCorrection.
- Reservation verhindert Over-Sell.

### Eval-Tests

Schmale Blackbox-Vertraege:

1. Publish BUY/SELL ExecutionResults + WalletSnapshots -> `OpenPositionsSnapshot` korrekt.
2. Partial SELL darf Position nicht schliessen.
3. Max-open gate nutzt Authority count.
4. Momentum exit amount folgt Authority balance.

### Runtime-Diagnostik

Neue Metriken:

- `position_authority_open_positions`
- `position_authority_drift_lockmanager`
- `position_authority_drift_momentum`
- `position_authority_reconciliation_corrections_total`
- `position_authority_stale_snapshot_age_ms`

---

## Nicht-Ziele

- Kein neuer Hot-Path-RPC.
- Kein Simulations-Bypass.
- Kein Deployment ohne explizite User-Freigabe.
- Kein Big-Bang-Umbau aller Komponenten.
- Kein Entfernen des LockManagers; er bleibt fuer Reservations erforderlich.

---

## Supervisor-Hinweise

Vor jedem Handoff:

1. `KNOWN_BUG_PATTERNS.md` einbeziehen: #5, #7, #15, #16, #18, #24.
2. Open Brain nach `positions`, `partial sell`, `SIM_INSUFFICIENT_BALANCE`, `max_open_positions` durchsuchen.
3. Handoff muss explizit sagen: `execution-engine` darf keinen lokalen dauerhaften Truth-Pfad neu erfinden.
4. Jeder Scope braucht CI + Eval Level 5 + finalen Bugbot vor Merge.
5. Nach Merge nicht deployen ohne ausdrueckliche User-Freigabe.
