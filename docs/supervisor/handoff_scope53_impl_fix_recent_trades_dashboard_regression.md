WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 53: Fix Recent Trades Dashboard Regression

## Task-Beschreibung

Scope 52 was deployed but the UX is not what was requested.

Regression introduced:

- A visible `Sort (ms)` column was added. This was never requested and must be removed.
- A visible `mint_full` column was added. This was never requested and must be removed.
- The user wanted the existing `Token (Mint)` column to show the complete mint address and link to Solscan, similar to the `TX Hash` column.
- The user wanted the existing `Time` column to be switchable between relative time and UTC, not an extra sorting column.

Fix the dashboard/API UX while keeping the scope strictly limited to dashboard/trades API files.

## Required Behavior

### 1. Token (Mint)

There must be exactly one visible token column:

- Header: `Token (Mint)`
- Display value: full mint address, e.g. `5JhNNh6x163Nvr7kZVM8EcbTok1byhVJenqtrjDapump`
- Link: `https://solscan.io/token/<full mint>`

Do not show a separate `mint_full` column.

Implementation guidance:

- Prefer changing the dashboard table column selector from `mint` to `mint_full`, while keeping text `Token (Mint)`.
- Then the Solscan URL can safely use `${__value.text}` because the visible value is full mint.
- Remove hidden `mint_full` field/column and any dependency on `${__data.fields.mint_full}` unless absolutely necessary.

### 2. Time

There must be exactly one visible time column:

- Header: `Time`
- It can show relative age by default.
- User must be able to switch the displayed value to UTC timestamp.
- No visible `Sort (ms)` column.

Implementation guidance:

- Keep `timestamp_ms` only as a backend/internal field if needed, but do not expose it as a visible table column.
- If using `time_display`, the visible column should be named `Time`.
- The dashboard variable `time_mode=relative|utc` from Scope 52 is acceptable.
- `/trades?mode=run&time_mode=${time_mode}` is acceptable.
- If Grafana cannot sort the string `Time` column chronologically, do not add a visible sort column. Use a hidden field/transformation only if it stays invisible in the final table. If reliable hidden sort is not possible, accept Grafana's default table sorting behavior and document the limitation in the PR summary.

### 3. Remove Unwanted Columns

Remove from the visible table:

- `Sort (ms)`
- `mint_full`

The user explicitly does not want these columns.

### 4. Widths

Keep the column width improvements:

- `Action`, `Amount`, `PnL (SOL)`, `PnL %` should stay narrow.
- `Detail` should get more width.

Adjust widths after removing the two unwanted columns.

## Erlaubte Dateien

- `docs/grafana_multiprocess_dashboard.json`
- `scripts/trades_server.py` only if needed for the Time mode data.

## Verboten

- No changes to trading logic.
- No changes to `src/`.
- No deploy, no server restart.
- Do not add new visible helper columns.
- Do not remove existing semantic columns like `Reason` or `Detail`.

## Checks

Run:

```bash
python3 -m py_compile scripts/trades_server.py
python3 scripts/trades_server.py --self-check
python3 -m json.tool docs/grafana_multiprocess_dashboard.json >/dev/null
```

Manual JSON checks:

- Panel `Recent Trades (Current Run)` has no visible `Sort (ms)` column.
- Panel has no visible `mint_full` column.
- `Token (Mint)` selector is full mint or otherwise displays full mint.
- Token link uses full mint.
- `Time` is the only visible time column and is controlled by `time_mode`.

## PR Summary Requirements

Explain:

- how `Token (Mint)` now shows and links the full mint
- how `Time` toggles relative/UTC
- that `Sort (ms)` and `mint_full` visible columns were removed
- that no trading behavior changed
