WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 54: Recent Trades Width/Precision Tweaks

## Task-Beschreibung

Small follow-up to Scope 53 for the Grafana `Recent Trades (Current Run)` table.

User feedback after importing the fixed dashboard:

1. `Token (Mint)` link works now, but the column is too wide. It should be the same width as `TX Hash`.
2. `Value (SOL)` was made smaller even though that was not requested. Keep its current width, but reduce displayed decimals from 12 to 9, because values do not need more than 9 decimal places.
3. Do not reintroduce visible helper columns like `Sort (ms)` or `mint_full`.
4. User would like the time-mode control closer to `Recent Trades`, ideally between the Engine metrics section and the `Recent Trades` table.
5. After shrinking `Token (Mint)`, use the freed horizontal space so the table content shifts left and `Detail` shows more without horizontal scrolling.

## Existing Time Mode UX

The dashboard already has a Grafana variable:

- Variable name: `time_mode`
- Label: `Trades: Zeit (Recent Trades)`
- Values: `relative`, `utc`
- The table target uses `/trades?mode=run&time_mode=${time_mode}`
- The visible `Time` column uses `time_display`

Do not change this behavior in this scope unless required by a validation issue.

Important Grafana constraint:

- Dashboard variables normally render in Grafana's top variable bar, not between panels.
- If moving the actual variable control between panels is not supported by Grafana dashboard JSON, do not fake it with brittle behavior.
- Preferred robust compromise: add a small text/markdown info panel directly above `Recent Trades` (between Engine section and Recent Trades) explaining: `Time Mode: use the "Trades: Zeit (Recent Trades)" dropdown at the top to switch Relativ/UTC. Table arrow only controls sort order.`
- If Grafana supports a native variable-placement mechanism in the dashboard JSON version used here, use it; otherwise document in PR summary that Grafana variables are global/top-bar controls and the scope adds local guidance near the table.

## Required Changes

### Token (Mint) Width

Set `Token (Mint)` column width equal to the `TX Hash` column width.

Current intended behavior:

- `Token (Mint)` selector should remain `mint_full`.
- Link should remain `https://solscan.io/token/${__value.text}`.
- Display must remain full mint address.

### Value (SOL) Decimals

Set `Value (SOL)` decimals to `9`.

Keep its current width from the existing dashboard after Scope 53. Do not widen or shrink it unless absolutely necessary.

### Horizontal Space / Detail Visibility

After making `Token (Mint)` as narrow as `TX Hash`, ensure the table width allocation benefits `Detail`:

- Keep narrow columns explicit (`Action`, `Amount`, `PnL (SOL)`, `PnL %`, `Token (Mint)`, `TX Hash`).
- Keep or increase `Detail` width so more detail text is visible without horizontal scrolling.
- Do not add new visible columns.
- If Grafana still horizontally scrolls because full mint addresses are long, prefer keeping `Token (Mint)` width fixed and allowing cell truncation with link, rather than widening it again.

### Preserve Existing Fixes

Ensure these remain true:

- No visible `Sort (ms)` column.
- No visible separate `mint_full` column.
- `Time` remains the only visible time column.
- `Token (Mint)` opens Solscan with the full mint.
- `TX Hash` link still works.

## Erlaubte Dateien

- `docs/grafana_multiprocess_dashboard.json`

Avoid touching `scripts/trades_server.py` unless a validation issue requires it.

## Verboten

- No trading logic.
- No `src/` changes.
- No deploy, no server restart.
- Do not add helper columns.
- Do not change the time-mode API unless necessary.

## Checks

Run:

```bash
python3 -m json.tool docs/grafana_multiprocess_dashboard.json >/dev/null
```

Manual JSON checks:

- `Token (Mint)` width equals `TX Hash` width.
- `Value (SOL)` decimals = `9`.
- `Token (Mint)` selector = `mint_full`.
- `Token (Mint)` Solscan URL = `https://solscan.io/token/${__value.text}`.
- No visible `Sort (ms)` or separate `mint_full` columns.
- A small local guidance panel exists near `Recent Trades` if the actual variable cannot be moved there.
- `Detail` has the widest configured table width and benefits from the freed space.

## PR Summary Requirements

Mention:

- Token and TX columns now use equal width.
- `Value (SOL)` decimals reduced to 9.
- Whether the actual `time_mode` variable could be moved; if not, mention the local guidance panel and Grafana's top-bar variable constraint.
- Detail visibility improved without adding visible helper columns.
- No trading behavior changed.
