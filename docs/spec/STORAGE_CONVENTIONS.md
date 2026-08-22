# Storage Conventions (P0) – Replay, Decision Records, JetStream

**Stand:** 2026-08-22

Zwei Schichten, nicht verwechseln:

1. **JetStream = SSOT für Bot-Zustand** (I-24a): Pool-Cache, Wallet-Snapshots, TX-Confirms, Intents, ExecutionResults zum Replay nach Restart.
2. **JSONL = Forensik / Golden Replay:** append-only Dateien unter `trade_logs/`. Hot Path darf darauf nicht blockieren.

---

## 0) JetStream-Streams (Runtime)

Namen aus `src/nats/jetstream.rs` (Impl):

| Stream | Rolle |
|--------|--------|
| `POOL_CACHE` | market-data MASTER → EE/Arb SLAVE (`ironcrab.pool_cache.{pool}`, last-per-subject) |
| `WALLET_SNAPSHOT` | Wallet-Balances |
| `WALLET_TX_CONFIRM` | TX-Finality für Confirm-Wait (kein RPC-Fallback, I-7) |
| `TRADE_INTENTS` | persistente Intents (Startup-Race vs. Core NATS) |
| `EXECUTION_RESULTS` | persistente Results |
| `CONFIG_UPDATES` | Config-Reload |

Core NATS bleibt für hochfrequente MarketEvents (`ironcrab.v1.market_events`).

### 0.1 Positionen: KV vs. Writer

Zwei Ebenen, nicht verwechseln:

- **SSOT als Daten:** JetStream-KV-Bucket `POSITION_AUTHORITY` (`src/ipc/schema.rs`: `POSITION_AUTHORITY_KV_BUCKET`). Das ist der persistente Stand offener Positionen, den andere Prozesse nach Restart teilen.
- **Einziger Writer:** Binary `position-manager` (PA-6b). Es reduced `ExecutionResult` + `WalletBalanceSnapshot` und schreibt den Bucket. Execution-Engine und `momentum-bot` **lesen** ihn (Watch), sie schreiben ihn nicht.
- **Keine zweite Positions-SSOT:** EE-In-Process-Reducer ist Gate-Cache. Momentum-Overlay/Tracker ist Strategiezustand. LockManager sind Kapital-Locks. Wallet-Snapshots (`WALLET_SNAPSHOT`) sind Input, nicht die Positionsbuchhaltung.

Zwei Writer auf demselben Bucket = Split-Brain.

---

## 1) Grundregeln (JSONL)

- **Hot Path safe**: Trading-Pfade dürfen nicht auf DB/FS blockieren.
- **Append-only**: Write pattern ist append-only, keine In-Place Updates.
- **Schema-versioniert**: Jede Datei-/Record-Klasse hat `schema_version`.
- **Korrelation**: Alles lässt sich über IDs verknüpfen:
  - `event_id` (MarketEvent)
  - `intent_id` (TradeIntent)
  - `decision_id` (Decision Record)
  - `execution_id` (Execution result)

---

## 2) Log-Root & Layout

Root-Verzeichnis (Default):
- `IRONCRAB_LOG_DIR` falls gesetzt, sonst `trade_logs/`

Unterverzeichnisse:
- `trade_logs/market_events/`
- `trade_logs/intents/`
- `trade_logs/decisions/`
- `trade_logs/executions/`

---

## 3) Dateinamen (Rotation)

Rotation: **täglich** (UTC) + optional Größenlimit.

Namensschema:
- `market_events-YYYYMMDD.jsonl`
- `trade_intents-YYYYMMDD.jsonl`
- `decision_records-YYYYMMDD.jsonl`
- `execution_results-YYYYMMDD.jsonl`

Optional (Parquet für Analytics):
- `market_events-YYYYMMDD.parquet`
- `execution_results-YYYYMMDD.parquet`

---

## 4) Record-Header (Pflichtfelder)

Jeder JSONL-Record beginnt logisch mit:

- `schema_version` (u32)
- `ts_unix_ms` (u64)
- `component` (string) z. B. `market-data`, `momentum-bot`, `arb-strategy`, `execution-engine`, `position-manager`
- `build` (string) z. B. git SHA oder semver
- `run_id` (uuid/string) – Prozesslauf-ID

Zusätzlich pro Typ:

### 4.1 MarketEvents
- `event_id`
- `source` (geyser/rpc/ws)
- `slot` (wenn vorhanden)
- `kind` (pool_created, swap_observed, price_update, …)
- `payload` (normalisiert)

### 4.2 TradeIntents
- `intent_id`
- `source` (momentum/arb-strategy/execution-worker)
- `tier` (0/1)
- `deadline_slot` oder `ttl_ms`
- `required_capital` (units explizit)
- `resources` (mints/pools/accounts)
- `expected_value` / `roi_bps`

### 4.3 Decision Records
- `decision_id`
- `intent_id`
- `regime` (EARLY/ESTABLISHED/NA)
- `checks[]` (reason-coded pass/fail)
- `plan_hash`
- `simulate` { ok/err, logs_preview }
- `send` { bundle_id/signature }
- `confirm` { status, slot }

### 4.4 Execution Results
- `execution_id`
- `decision_id`
- `signature` / `bundle_id`
- `status` (sent/confirmed/failed)
- `fees` (lamports, tip, cu)
- `pnl` (gross/net, units)

---

## 5) Retention

Default:
- JSONL: 7–30 Tage (konfigurierbar)
- Parquet: optional länger

Regel:
- Rotation/Deletion läuft **asynchron** (nicht im Hot Path).

---

## 6) Minimaler Replay-„Bundle“

Ein Replay-Case besteht aus:
- JetStream-Stand oder Dump der relevanten Streams (mindestens `POOL_CACHE` / Intents / Results, je nach Case)
- `market_events-*.jsonl` (Input, soweit JSONL geschrieben wird)
- `trade_intents-*.jsonl`
- `decision_records-*.jsonl`
- `execution_results-*.jsonl`
- `config.toml` (genauer Snapshot)

Damit kann man:
- Entscheidungen reproduzieren
- Reject-Gründe vergleichen
- Regression Tests bauen
