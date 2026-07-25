# Handoff — Impl: L2c-Mom Imminent-Entry Pins (I-MD-9)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

## Task-Beschreibung

**Basis:** `architecture-rebuild` Tip nach L2b (`530cb8f` oder neuer).  
**Spec (Source of Truth):** Eval `docs/spec/MOMENTUM_ACTIVE_POOLS.md` (Imminent-Entry) + `INVARIANTS.md` **A.50 / I-MD-9**.  
**Plan:** Eval `docs/plans/plan_realtime_slo_processable_set_20260725.md` §3.5.

**Warum:** Prod pinnt Momentum heute schon bei `get_or_create_tracker` (~2251 active pins) → EXEC_HOT ~280 ups/s, Lag über SLO. Discovery/Filter brauchen **keine** Vault-Pins (TX/MarketEvents). Pins sind für Imminent Entry + Open Position.

**Strategie-Ziel:** Nicht First-Buy; starke Bewegungen mitnehmen. Bewusst Entry-Latenz gegen Exit-/Quote-Frische.

### Soll-Verhalten (normativ)

```text
Discovery / Validation — KEIN active_pools Pin
  → initialer Filter-Pass grün
  → Pin pin_reason=tracker + State WaitHotSet (neu oder äquivalent)
  → Filter laufen WEITER auf neuen Events
  → bei Filter rot → Unpin (removed filter_failed/rejected), kein Intent
  → Hot-Set/Reserves fresh genug?
  → Pre-Intent Revalidate: Filter NOCHMAL grün? sonst kein Intent
  → erst dann ProbeBuy / ScaleIn Intent
  → open_position → pin_reason=position
```

### PFLICHT

1. **Entfernen / ersetzen:** Publish `active` mit `pin_reason: tracker` allein wegen `get_or_create_tracker` / Discovery.  
2. **Einführen:** Pin erst nach initialem Pre-Entry-Filter-Pass; explizites Warten auf Hot-Set/Vault-Frische.  
3. **Kein Blind-Intent:** Intent nur wenn (a) Reserves/Hot-Set fresh **und** (b) Filter unmittelbar vor Emit grün.  
4. **Timeout** auf WaitHotSet (Config, sinnvoller Default); bei Timeout → `removed` `hot_set_timeout`, kein Intent.  
5. Wire: `removed.reason` um `hot_set_timeout` / `filter_failed` erweitern falls Enum heute closed.  
6. Reconcile (~30s) darf Discovery-only Trackers **nicht** re-pinnen.  
7. Open-Position / Wallet Pins unverändert Must-hot.  
8. Kein Hot-Path-RPC (I-7). Kein blindes Worker/Cap-Inflate.

### Abnahme

- Unit/Integr.: kein Pin-Publish auf Tracker-Create.  
- Intent-Pfad: Revalidate+Freshness required.  
- Filter-rot / Timeout während Wait → Unpin, kein Intent.  
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.  
- Prod-Erwartung nach Deploy (separat): `momentum_active_pool_pins` ≪ 2251 im Steady State ohne Open Positions.

## Relevante Invarianten (Volltext)

**I-MD-9 (A.50):** Momentum darf Geyser-Pins (`ironcrab.v1.momentum.active_pools`) **nicht** allein wegen Discovery / `get_or_create_tracker` setzen. Pin (`pin_reason: tracker`) erst nach **initialem Pre-Entry-Filter-Pass** (Imminent / `WaitHotSet`). ProbeBuy-/ScaleIn-Intent nur wenn **(a)** Hot-Set / Vault-Reserves fresh genug **und** **(b)** Pre-Entry-Filter **unmittelbar vor** Intent-Emit erneut grün sind — **kein Blind-Intent**. Während `WaitHotSet` laufen dieselben Pre-Entry-Gates weiter; bei Filter-rot oder Timeout → Unpin, kein Intent. `pin_reason: position` (Open Position) und Wallet sind Must-hot.

**I-7:** Kein Hot-Path-RPC.

**I-12:** Intent mit Decision Record; Ablehnung/Timeout beobachtbar (Reason Codes).

**I-4 / Geyser-First:** Pin-Pfad ohne RPC; Cache-Miss → warten/reconcile, nicht RPC-seed.

## Bestehendes Pattern

- Heute (zu ersetzen): `MOMENTUM_ACTIVE_POOLS` Publisher-Regel alt „New tracker → pin“ — in Spec **withdrawn**.  
- Pin-Transport Phase 2b: NATS → md-track-worker (`ApplyMomentumActivePools`), nicht md-state.  
- Pre-Entry-Gates: Momentum v2 Spec Soft/Hard gates auf `Trade` / `LiquidityRemoved` / mint info — wiederverwenden für WaitHotSet + Pre-Intent Revalidate.  
- Freshness: bestehende LivePoolCache / reserve-age / quote-ready Checks nutzen; kein neuer RPC.

## Erlaubte Dateien

- `src/bin/momentum_bot.rs` und Momentum-Module die `active_pools` publishen / TrackerState  
- `src/nats` / IPC Types für `removed.reason` falls nötig  
- Unit-Tests neben bestehenden Momentum-Pin-Tests  
- Optional Metriken: wait_hot_set duration, timeout, filter_failed_during_wait, pins_by_reason

## Verboten

- Pin bei Discovery / get_or_create_tracker  
- Intent feuern sobald Hot-Set da ist **ohne** Filter-Revalidate  
- Hot-Path RPC, Cap/Worker blind erhöhen, Trailing-% Tuning  
- MD Shed-Controller-Umbau (das ist **L2c-MD**, separater Scope)  
- Änderungen an Eval-Tests in diesem PR (Eval folgt nach Merge)

## Prüf-Befehle

```text
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Commit-Message-Vorschlag: `feat(momentum): imminent-entry pins after filter pass (L2c I-MD-9)`
