# Plan: EXEC_HOT innerhalb Latenz-SLO — Processable Set + Classify + Burst

**Status:** L2c-Spec in Arbeit (Imminent-Entry I-MD-9). Prod tip `530cb8f` (L2b) · Killswitch weiter User-ON  
**Ergänzt / löst Rest von:** `plan_realtime_slo_hard_split_20260723.md` (L1–L2b erledigt; L3/L4 noch offen)  
**Forensik-Scripts:** `docs/supervisor/scripts/latency_rootcause_forensics.py`, `..._forensics2.py`

---

## 1. Kurz-Verdict

L1/L2/L2b haben den **alten** Failure-Mode (gemeinsame Broadcast-FIFO + Enrich blockiert EXEC_HOT → Depth 10⁵, Lag ≥60 s) **beseitigt**.

Der **aktuelle** Failure-Mode ist ein anderer:

> Das **EXEC_HOT-processable Set** (explizit admitted ≈ **18.8k**, ≈ **260–280 EXEC_HOT-Updates/s**) erzeugt unter Tageslast und **Bursts** weiterhin Channel-Lag **über SLO**, obwohl Depth oft 0 und lagged=0 ist. Classify/Handler sind im Median mikrosekunden-schnell; der Schaden kommt aus **Burst-Rückstau** (Depth-Spikes) + **Classify-p99-Fat-Tail** (~22 ms) auf dem Recv-Pfad.

TX (~ms) und Momentum wall-lag (~ms) sind **nicht** der Blocker. Killswitch-OFF ist weiter **nicht** erlaubt, bis EXEC_HOT p50/p99 sustained &lt; 50 ms / 200 ms.

---

## 2. Was bereits greift (Beweis, dass L1–L2b wirken)

| Signal | L2 Soak (alt) | L2b jetzt | Lesart |
|--------|---------------|-----------|--------|
| Broadcast depth exec_hot | ~262k | oft **0**, 1h-Avg ~119, **Max 1h ~4800** | Isolation OK; Rest = Bursts |
| Lagged exec_hot | ~975k | **0** | Cap nicht mehr „vollgelaufen unsichtbar“ |
| Soft enrich shed drops | hoch | **~4.0M** | Enrich wird unter Druck verworfen |
| Hard shed tracker | — | steps ~1800 / groups ~18k | Tracker-Demote aktiv |
| Hard shed momentum / arb | — | **24 / 0** steps | Momentum nur leicht, Arb nie |
| Admit reject tracker / mom / arb | — | ~57k / ~13k / **0** | Re-admit-Suppress greift außer Arb |
| TX p99 | OK | ~19 ms | unverändert gesund |
| Mom wall-lag | früher Minuten | **~9 ms** | L3 nicht der aktuelle Blocker |

---

## 3. Woran es **jetzt genau** scheitert

### 3.1 Messdefinition (wichtig)

`market_data_account_channel_lag_ms{class=exec_hot}` = Zeit **Listener-`send` → market-data-`recv`** (Broadcast + Tokio-Scheduling), **nicht** Handler-Parse-Dauer (RUNBOOK).

Deshalb kann gelten: Handler p50 ~26 µs **und** Lag p50 ~115 ms gleichzeitig — Lag ist **Wartezeit vor Recv**, nicht „langsames Vault-Parsing“.

### 3.2 Live-Zahlen (Prod, 5m / 1h Fenster, tip `530cb8f`)

| Metrik | Wert | SLO / Ziel |
|--------|------|------------|
| EXEC_HOT ups/s (5m) | ~258 | — |
| Geyser account ups/s | ~643 | — |
| Drop ups/s | ~347 | Early-drop arbeitet |
| Enrich ups/s | ~16 | Soft-shed hält Enrich klein |
| Explicit admitted | ~18.8k | zu groß für Burst-SLO |
| Momentum active pins | ~2251 | großer EXEC_HOT-Treiber |
| Arb pinned pools | ~622 | noch nie hard-gesheddet |
| Enrichment registry pools | ~53k | darf EXEC nicht füttern (tut es nach Soft-shed weitgehend nicht) |
| Lag p50 / p90 / p99 (5m) | **~114 / ~234 / ~449 ms** | **&lt;50 / — / &lt;200** |
| Max Lag p99 (5m over 1h) | **~8.7 s** | Burst-Kollaps noch da |
| Depth now / avg1h / max1h | 0 / ~119 / **~4800** | Burst-Queue |
| Classify p50 / p90 / p99 | ~2.7 µs / ~8 µs / **~22 ms** | Fat-Tail serialisiert Recv |
| Handler p50 / p90 / p99 | ~26 µs / ~47 µs / ~13 ms | Median OK; Tail sekundär |
| High-enqueue p99 | ~50 µs | Worker-Enqueue nicht der Engpass |
| EXEC_HOT workers | 4 | nicht blind erhöhen |

**Lag-Histogramm (Rate 5m):** nur ~16/s unter 10 ms; Masse zwischen **50–250 ms**; fast nichts über 500 ms *im aktuellen 5m-Fenster* — aber 1h-Max-p99 zeigt, dass **Burst-Fenster** weiter in Sekunden laufen.

### 3.3 Kausalgraph

```text
Geyser Account (~640/s)
    │
    ▼
Broadcast (alle Updates)
    │
    ├─ EXEC_HOT recv: classify ALL → enqueue nur ExecHot (~260/s)
    └─ ENRICH recv:  classify ALL → soft-drain/shed (~16/s admitted)

Classify:
  p50 ~3µs  → OK
  p99 ~22ms → membership/hot/vault lookups unter Contention
             → Recv-Task stockt → Broadcast depth spike (bis ~4800)
             → channel_lag p50/p99 über SLO

Processable set:
  ~18.8k admitted + ~2.2k mom pins + ~622 arb
  → EXEC_HOT update rate bleibt dauerhaft hoch
  → kein Headroom für Bursts
  → Shed-Controller reagiert primär auf Depth-Tier,
    nicht auf Lag-SLO; Arb-Tier nie erreicht obwohl p99>200ms
```

### 3.4 Was „benötigte Daten“ vs. „dürfen warten“ bedeutet

| Klasse | Inhalt | Latenz-Vertrag |
|--------|--------|----------------|
| **Must-hot** | Open-Position Vault/Bin, Wallet, Arb **selected** quote_ready/executable | p50&lt;50 ms, p99&lt;200 ms — **niemals** shed |
| **Imminent-hot** | Momentum nur nach Filter-Pass bis Intent/Fill (`ProbeBuyPending` / HotSet-Wait) | kurzlebig; klein; Pin erst spät |
| **Best-effort** | Discovery/Validation **ohne** Pin, Tracker, Enrichment, Arb warmable | TX/P2; shed OK |

**Korrekter Betrieb** heißt: Must-hot immer frisch; Imminent-hot nur für echte Entry-Kandidaten; Discovery ohne Massen-Pins.

Heute: Fast alle Momentum-Tracker pinnen schon bei `get_or_create_tracker` → ~2251 pins / ~18.8k admitted — **zu groß**.

---

### 3.5 Momentum Imminent-Entry-Vertrag (User 2026-07-25, verbindlich)

**Strategie-Ziel:** Nicht First-Buy neuer Tokens, sondern Tokens mit **starken Bewegungen** handeln und einen Teil der Bewegung mitnehmen. Extra Entry-Latenz durch späten Pin ist akzeptabel, **wenn** Exit/Quote-Frische hält.

**Bewusster Tradeoff:** etwas mehr **Entry-Latenz** gegen zuverlässige **Exit-/Quote-Latenz** (EXEC_HOT SLO).

#### Soll-Ablauf

```text
Discovery / Validation (Filter)
  │  — kein Geyser-Pin (nur TX/P2 Events)
  ▼
Initialer Filter-Pass (grün)
  │  — jetzt erst active_pools Pin (imminent)
  ▼
WaitHotSet (neu / explizit)
  │  — warte auf Explicit-Sub + Vault/Reserves fresh genug
  │  — Filter laufen WEITER auf neuen Trades/Events
  │  — bei Filter rot → Unpin + zurück Validation/Rejected (kein Intent)
  ▼
Pre-Intent Revalidate (PFLICHT)
  │  — Filter noch einmal grün? Sonst kein Intent
  │  — Hot-Set/Reserves noch fresh? Sonst weiter warten / Timeout
  ▼
ProbeBuy Intent → Execution
  │
  ▼
Open Position → pin_reason position (Must-hot)
```

#### Harte Regeln

1. **Kein Pin** bei `get_or_create_tracker` / reiner Discovery.  
2. **Pin erst** nach initialem Filter-Pass (Übergang in Imminent / kurz vor `ProbeBuyPending`).  
3. **Kein Blind-Intent:** Intent nur wenn **(a)** Hot-Set/Reserves fresh genug **und** **(b)** Filter **unmittelbar vor** Intent-Emit noch grün.  
4. Während `WaitHotSet` müssen dieselben Pre-Entry-Gates weiter laufen (Trades, LP, Dev-Sell, Velocity, …).  
5. Timeout auf `WaitHotSet` (konfigurierbar): bei Timeout → Unpin, kein Intent, zurück WAIT/Reject mit reason (z.B. `WAIT_HOT_SET` / timeout).  
6. Open-Position-Pins und Wallet: **nie** shed.  
7. Spec-Änderung nötig: `MOMENTUM_ACTIVE_POOLS.md` Publisher-Regel (1) „New tracker → pin“ entfällt zugunsten dieses Vertrags.

#### Was absichtlich nicht gilt

- Pin aller Validation-Tracker „nur damit Vaults warm sind“.  
- Intent feuern sobald Hot-Set da ist, **ohne** erneute Filterprüfung.

---

## 4. Warum weitere Symptom-Fixes scheitern würden

| Maßnahme | Warum verboten / nutzlos |
|----------|---------------------------|
| Worker 4→8 / Broadcast-Cap↑ | Bursts werden größer unsichtbar; CPU-Fight; Regression kehrt zurück |
| Trailing-% lockern | Maskiert stale Marks |
| Hot-Path RPC | I-7; skaliert nicht |
| Nur Threshold-Tuning ohne Lag-Feedback | Depth oft 0 bei p50&gt;100 ms — Depth-only Controller ist **blind** für den aktuellen Mode |
| Arb C1b Coverage vor Frische | mehr Pins → mehr Lag |

---

## 5. Zielbild (verbindlich)

1. **EXEC_HOT Lag** sustained (15m + 1h, keine Burst-p99&gt;1 s): p50&lt;50 ms, p99&lt;200 ms  
2. **Depth** exec_hot: p50≈0, p99&lt;50, max-Spikes &lt;500 (coalesce)  
3. **Must-hot** Pins bleiben subscribed und werden nicht evictet  
4. **Conditionally-hot** automatisch so klein, dass (1)–(2) halten  
5. **Classify** p99 &lt; 1 ms (Ziel), p50 unverändert µs  
6. Danach erst: Killswitch-OFF-Empfehlung + L3 nur falls Mom-position regressiert + L4 24h Soak  

---

## 6. Ansätze (Trade-offs) — Empfehlung: A→B→C

### Ansatz A — Imminent-Entry Pins + Lag-closed-loop (**empfohlen, P0**)

Zwei gekoppelte Teile (klein schneidbar):

**A1 — Momentum Pin-Lifecycle (strategisch, Scope L2c-Mom / Spec zuerst)**  
§3.5 umsetzen: spät pinnen, `WaitHotSet`, Filter weiter + Pre-Intent-Revalidate. Das ist der strukturelle Shrink der ~2251 Früh-Pins.

**A2 — Lag-closed-loop Shed (Scope L2c-MD)**  
Falls Rest-Last (Tracker/Arb) Lag noch bricht: Shed nach Lag p50/p99 + Depth; Tracker → Arb warmable; Must-hot/Imminent während ProbeBuy immun; nie Wallet/OpenPos.

**Pro:** trifft Root Cause „zu früh / zu breit gepinnt“; passt zu Momentum-Ziel (Bewegung mitnehmen, nicht First-Buy). **Contra:** Entry etwas später; ohne Pre-Intent-Revalidate wäre es gefährlich — daher Regel 3 Pflicht.

### Ansatz B — Classify Hot-Path (**P0, Scope L2d**, parallel/nach L2c)

- Membership/hot/vault/bin Checks: echte O(1) lock-freie Snapshots (kein Fat-Tail durch Lock/HashMap-Resize)  
- Doppel-Classify auf zwei Broadcast-Subs vermeiden: Klasse einmal am Ingress taggen **oder** gefilterte Kanäle  
- Metrik: classify p99 &lt; 1 ms Gate

**Pro:** reduziert Burst-Erzeugung bei gleichem Set. **Contra:** allein ohne Set-Shrink reicht bei 280/s + Pins oft nicht.

### Ansatz C — EXEC_HOT Latest-Wins Coalesce unter Burst (**P1, Scope L2e**)

- Bei depth↑ oder Lag-Alarm: pro Pubkey nur neuestes Update in HIGH-Queue behalten  
- Must-hot ebenfalls latest-wins (korrekt für Vault-Reserves), nie „drop forever“

**Pro:** koppelt Max-Depth. **Contra:** Cosmetik ohne A/B; mit A/B der Burst-Absorber.

### Nicht gewählt jetzt

- Separater Geyser-Filter nur Must-hot (größerer Architektur-Schnitt) — später L5 falls A–C nicht reichen  
- L3 Mom position drain — Mom wall bereits OK; nachholen bei Regression  

---

## 7. Konkrete Umsetzungs-Reihenfolge (kleine PRs)

| Scope | Repo | Inhalt | Abnahme |
|-------|------|--------|---------|
| **L2c-Spec** | Iron_crab-eval | `MOMENTUM_ACTIVE_POOLS.md` + ggf. kurzer INVARIANTS-Absatz: Imminent-Entry §3.5 | Spec review User |
| **L2c-Mom** | Iron_crab | Pin erst nach Filter-Pass; `WaitHotSet`; Filter weiter; Pre-Intent-Revalidate; kein Blind-Intent; Timeout | Unit/Integr.: kein Pin in Discovery; Intent nur nach revalidate+fresh; Mom pins ≪ 2251 |
| **L2c-MD** | Iron_crab | Lag-closed-loop Shed Rest (Tracker/Arb); Must-hot + ProbeBuy imminent immun | 1h Soak: EXEC_HOT p50&lt;50, p99&lt;200 |
| **L2d** | Iron_crab | Classify Fat-Tail | classify p99&lt;1 ms |
| **L2e** | Iron_crab | Latest-wins coalesce unter Burst | depth max ≪ 4800 |
| **Eval** | Iron_crab-eval | Invarianten zu §3.5 + Must-hot | Eval CI + Impl Level 5 |
| **L4** | Ops | 24h Soak + Killswitch-OFF-Gate | DoD Hard-Split §2 |

### Erlaubte Dateien (Richtung für Impl-Handoffs)

- `src/market_data/track/explicit_admission.rs`, `admission_wiring.rs`, Shed-Controller (L2b-Dateien)  
- `src/market_data/ingest/account_filter.rs` + IngestHost Snapshot-Pfade (L2d)  
- Account recv / HIGH enqueue Pfad (L2e coalesce)  
- Metriken + RUNBOOK-Absatz  

### Verboten

- `Iron_crab/src` Änderungen durch Supervisor selbst  
- Worker/Cap blind erhöhen, Trailing-Tuning, Hot-Path RPC, Cap-Blähung ohne Shed  
- Evict Open-Position / Wallet  

### Prüf-Befehle (Impl)

`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, danach Impl CI inkl. Eval Level 5.

---

## 8. Ops bis Code live

- Killswitch **anlassen** bis L2c (mind.) Soak grün  
- Kein Deploy ohne User-Freigabe  
- Monitoring: Lag p50/p99, depth max, shed_tier, admit rejects, classify p99, admitted, mom pins, arb pins  

---

## 9. Entscheidung / nächster Schritt

**User-Richtung (2026-07-25):** Imminent-Entry §3.5 bestätigt — Entry-Latenz gegen Exit-Frische; Filter während Wait + Revalidate vor Intent; kein Blind-Fire.

**Nächste Freigabe:** L2c-Spec (Eval-Docs) lokal/PR, dann Impl-Handoff **L2c-Mom** (Composer 2), danach L2c-MD falls Lag noch rot.
