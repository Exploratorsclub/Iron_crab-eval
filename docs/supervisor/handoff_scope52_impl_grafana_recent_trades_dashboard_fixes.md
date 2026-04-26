WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 52: Grafana Recent Trades Dashboard Fixes

## Task-Beschreibung

Fix three small UX issues in the Grafana "Recent Trades (Current Run)" panel.

User-observed issues:

1. Clicking `Token (Mint)` opens Solscan with a shortened mint like `5JhNNh6x...pump`, causing "address not found". `TX Hash` links work because they use the full hash.
2. `Time` should support a user-readable switch between relative age ("vor 25 Minuten") and UTC timestamp. The sort arrow should continue to mean ordering (last-to-first / first-to-last).
3. Column widths waste space in `Action`, `Amount`, `PnL (SOL)`, and `PnL %`, causing the final `Detail` column to be clipped.

Keep this scope limited to the dashboard/trades API UX. Do not touch trading logic.

## Relevante Invarianten / Constraints

### No Trading Behavior Change

This scope must not change execution, risk, routing, strategy, LockManager, position tracking, or liquidation behavior.

### No Deploy Without User Approval

Do not deploy, restart services, or run `deploy.sh`. Only PR changes are allowed.

### Data Integrity

The API already exposes both:

- shortened display field: `mint`, e.g. `5JhNNh6x...pump`
- full field: `mint_full`, e.g. `5JhNNh6x163Nvr7kZVM8EcbTok1byhVJenqtrjDapump`

The table may display a shortened mint, but links must use the full mint.

## Bestehendes Pattern / Relevante Code-Stellen

Primary files:

- `docs/grafana_multiprocess_dashboard.json`
- `scripts/trades_server.py` if needed for time display data.

Current API sample from production:

```json
{
  "timestamp_ms": 1777210247854,
  "time": "2026-04-26 15:30:47",
  "action": "SELL",
  "mint": "5JhNNh6x...pump",
  "tx_hash": "3KkyA9CHd1F2RpM6vzyDPJG3tTu835Tfx5zTHrXnRjkYJv8ds5SzNWjuY9mA3hbsN82unW2TLaoAzL52mmyug7Ht",
  "amount_tokens": 20849.038157,
  "value_sol": 0.003535347,
  "pnl_sol": -0.000214653,
  "pnl_pct": -5.72,
  "mint_full": "5JhNNh6x163Nvr7kZVM8EcbTok1byhVJenqtrjDapump",
  "reason": "MOMENTUM_EXIT",
  "reason_detail": "..."
}
```

Current Grafana panel:

- `docs/grafana_multiprocess_dashboard.json`
- Panel title: `Recent Trades (Current Run)`
- Target URL: `/trades?mode=run`
- Column `Token (Mint)` uses selector `mint`
- Link currently uses `https://solscan.io/token/${__value.text}` which resolves to shortened mint.
- `Time` uses selector `timestamp_ms`, type `timestamp_epoch`, override unit `dateTimeFromNow`.
- Existing widths: `Token (Mint)=120`, `TX Hash=100`, `Reason=200`, `Detail=300`; several numeric/action columns have no explicit narrow widths.

## Required Fixes

### 1. Solscan Token Link Uses Full Mint

Preferred solution:

- Keep display value as shortened `mint`.
- Add hidden or available field `mint_full` to the table data if Grafana data links can reference sibling fields.
- Change token data link URL to use full mint, not display text.

Possible Grafana data link patterns to evaluate:

- `${__data.fields.mint_full}`
- `${__data.fields["mint_full"]}`
- `${__data.fields.TokenFull}` if aliasing is required.

If Grafana/Infinity cannot reference hidden sibling fields reliably, change the `Token (Mint)` column selector to `mint_full` and use Grafana display/text transformation or a separate display field. Do not leave links using shortened text.

Acceptance:

- Display can remain shortened.
- Clicking token opens `https://solscan.io/token/<full mint>`.
- Clicking TX hash remains unchanged and works.

### 2. Time Display Toggle

Desired UX:

- Ability to view either relative age or UTC time.
- Sort arrow remains standard Grafana table sorting only.

Implementation guidance:

- First check if Grafana supports a clean native toggle via dashboard variable used in field unit/URL/query.
- If click-on-header toggle is not supported by Grafana table panels, do not hack around it with brittle behavior.
- Acceptable robust alternatives:
  1. Add a dashboard variable `time_mode` (`relative`, `utc`) and make `/trades?mode=run&time_mode=${time_mode}` return a `time_display` column.
  2. Or show two compact columns: `Age` (relative) and `UTC` (absolute), with narrow widths.

If using API support:

- Update `scripts/trades_server.py` to accept `time_mode=relative|utc` for `/trades`.
- Add fields like:
  - `time_utc`: ISO/UTC or `YYYY-MM-DD HH:MM:SS UTC`
  - `time_age` or `time_display`
- Keep `timestamp_ms` available for sorting.

Acceptance:

- User can switch/see relative and UTC without breaking table sort.
- The sort arrow continues to sort chronological order.
- Default should remain relative age unless a better Grafana-native UX is obvious.

### 3. Column Widths

Tune widths so `Detail` is less clipped.

Suggested starting widths:

- `Time` / `Age`: 100-120
- `Action`: 70
- `Token (Mint)`: 130
- `TX Hash`: 110
- `Amount`: 90
- `Value (SOL)`: 115
- `PnL (SOL)`: 95
- `PnL %`: 75
- `Reason`: 150-170
- `Detail`: remaining width, at least 450 if Grafana accepts fixed width

Acceptance:

- `Action`, `Amount`, `PnL (SOL)`, `PnL %` no longer waste large empty space.
- `Detail` is visibly less clipped in a 24-column full-width panel.

## Erlaubte Dateien

- `docs/grafana_multiprocess_dashboard.json`
- `scripts/trades_server.py` if needed for time mode / UTC fields.
- Optional docs note if the dashboard import process is documented nearby.

## Verboten

- No changes to `src/` trading logic.
- No deploy, no server restart.
- No Grafana datasource UID changes unless absolutely necessary.
- Do not remove existing columns unless replaced by equivalent/better UX.
- Do not break `/trades?mode=run` consumers.

## Erwartete Tests / Checks

Run at minimum:

```bash
python3 -m py_compile scripts/trades_server.py
python3 -m json.tool docs/grafana_multiprocess_dashboard.json >/dev/null
```

If adding query-param behavior, add a tiny local/script-level check or document manual verification:

- `/trades?mode=run&time_mode=relative`
- `/trades?mode=run&time_mode=utc`

Also inspect the dashboard JSON to confirm:

- Token link references full mint field.
- TX link still references full tx hash.
- Column widths are explicit and narrower where requested.

## PR Summary Requirements

In the PR, include:

- Exact Grafana data-link expression used for full mint.
- Whether time toggle is implemented as variable/query param or dual columns, and why.
- Mention that no trading behavior changed.
