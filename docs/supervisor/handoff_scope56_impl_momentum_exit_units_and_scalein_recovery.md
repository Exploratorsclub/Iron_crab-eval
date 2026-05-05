WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 56: Momentum Exit Units + Scale-in Recovery

## Task-Beschreibung

Fixe zwei produktive Regressionen aus dem Testlauf nach Scope 55:

1. `momentum-bot` berechnet die neue executable exit quote in einer anderen Einheit als `entry_price/current_price`. Dadurch werden echte Verluste als riesige Gewinne bewertet und STOP_LOSS greift nicht.
2. Confirmed Scale-in BUYs koennen in `execution-engine` bestaetigt sein, aber in `momentum-bot.positions` fehlen. Danach erzeugt Momentum Exit-Intents nur fuer die Probe-Menge; Execution sieht die volle Position und es bleiben Restpositionen offen.

Der Scope soll klein bleiben und kein PA-5/PositionAuthority-Big-Bang sein. Ziel ist ein robuster Zwischenfix bis Momentum spaeter die Exit-Menge aus PositionAuthority liest.

## Runtime-Evidenz

Server-Stand:

- Repo/Commit auf Server: `124fd040c8ddc727de5108a8b0a70b4b0b07463b` (`Scope 55: Momentum exit price validation against executable pool quote`)
- `momentum-bot` Metrics: `open_positions 0`
- `execution-engine` Metrics: `open_positions 2`, `position_authority_open_positions 2`, `position_authority_drift_lockmanager 0`

Betroffene Mints:

- `9mR7mmX55n1F56rKBBcfMrRzMoJjaZnSRs6fV1GRpump`
  - Probe BUY: `12_345_672_542` raw tokens for `0.00125 SOL`
  - Scale-in BUY: `37_843_472_159` raw tokens for `0.00375 SOL`
  - Execution total: `50_189_144_701`
  - SELL `int-a86b75ab-000006`: sold `12_345_672_542`, total_pos `50_189_144_701`
  - Dashboard reason: `TIME_EXIT`, detail reports `P&L: 90606.1%`
  - Real PnL from UI amounts: `entry_tps = 12_345.672542 / 0.00125 = 9_876_538.0336`; `sell_tps = 12_345.672542 / 0.000418528 = 29_497_841.3439`; `pnl = (entry/sell - 1) * 100 = -66.5%`

- `Hej4eDvHyTZ6ihcFwJ1Sn54hDSJ6VVvy3KLAXBwbpump`
  - Probe BUY: `44_054_058_278` raw tokens for `0.00125 SOL`
  - Scale-in BUY: `132_140_555_123` raw tokens for `0.00375 SOL`
  - Execution total: `176_194_613_401`
  - SELL `int-a86b75ab-000007`: sold `44_054_058_278`, total_pos `176_194_613_401`
  - Dashboard reason: `TRAILING_STOP`, detail reports impossible drawdown `-51723893939580.1% from ATH`

Critical logs:

```text
LockManager: accumulated token balance from confirmed BUY fill intent_id=int-a86b75ab-000001 mint=9mR7... fill_out_raw=37843472159 balance_before=12345672542 total_available=50189144701
LockManager: partial confirmed SELL ... intent_id=int-a86b75ab-000006 mint=9mR7... sold_raw=12345672542 total_pos=50189144701
LockManager: accumulated token balance from confirmed BUY fill intent_id=int-a86b75ab-000004 mint=Hej4... fill_out_raw=132140555123 balance_before=44054058278 total_available=176194613401
LockManager: partial confirmed SELL ... intent_id=int-a86b75ab-000007 mint=Hej4... sold_raw=44054058278 total_pos=176194613401
```

Momentum logs showed `BUY CONFIRMED - Opening position` / `Position opened` for the probe path, but no matching `Position scaled in` logs for the confirmed scale-in fills. This points to the existing-position orphaned BUY path being incomplete or pending cleanup/source mismatch removing the scale-in from Momentum's overlay.

## Relevante Invarianten (Volltext)

### I-7 Hot Path RPC-Freiheit

Momentum-/Arb-/normaler Execution-Hot-Path darf keine blockierenden RPC-Calls ausfuehren. Reconciliation-RPC nur Cold Path. Dieser Scope darf keine neuen RPC-Aufrufe in `process_exit_signals`, `check_for_exits`, `should_exit`, `generate_and_publish_exit_intent` oder anderen normalen Momentum-Trading-Pfaden einfuehren. Die executable quote muss weiter aus `LivePoolCache` / in-process Cache-Daten kommen.

### I-13 Position-Pool-Matching

Preis-Updates und exit-relevante Quotes duerfen nur fuer den Pool der Position verwendet werden. Multi-Pool-Tokens duerfen nicht durch einen anderen Pool falsch bewertet werden. Fuer Momentum gilt: Wenn eine Position `position.pool` gesetzt hat, darf `current_price`/executable quote nur aus diesem Pool kommen. Falls keine pool-korrekte Cache-Quote verfuegbar ist, darf ein preisbasierter Exit nicht durch eine Quote aus einem anderen Pool validiert werden.

### I-14 tokens_per_sol-Konvention

Intern verwendet Momentum `tokens_per_sol`. LOWER `tokens_per_sol` bedeutet: der Token ist wertvoller. PnL ist `pnl_pct = (entry_tokens_per_sol / current_tokens_per_sol - 1) * 100`.

Konsequenzen:

- `current_tokens_per_sol` steigt -> Token wird billiger -> negativer PnL / Verlust.
- `current_tokens_per_sol` sinkt -> Token wird teurer -> positiver PnL / Gewinn.
- `highest_price`/ATH fuer diese Konvention ist der niedrigste beobachtete `tokens_per_sol`.
- Alle Werte, die in `tokens_per_sol::pnl_pct` oder `tokens_per_sol::drawdown_from_ath_pct` fliessen, muessen dieselbe Einheit haben.

In diesem Scope ist besonders wichtig: `entry_price` wird aus UI token amount / UI SOL amount gebildet. Eine executable quote aus raw token amount / raw lamports ist um Faktor `10^(SOL_DECIMALS - token_decimals)` falsch skaliert und darf nicht direkt mit `entry_price` verglichen werden.

### I-9 Simulation-Gate

Keine Transaktion darf ohne erfolgreiche Simulation gesendet werden. Dieser Scope darf die Execution-Simulation, `process_intent` oder das Send-Gate nicht umgehen oder abschwaechen.

### I-12 Decision Record / Intent-Ablehnung

Ein Intent darf nicht still ohne nachvollziehbaren Decision Record / Log verworfen werden. Wenn ein Exit wegen fehlender pool-korrekter Quote suppressed wird oder ein orphaned BUY nicht angewendet werden kann, muss das klar geloggt werden.

### PA-Rollout-Ziel: Momentum ist nur Strategie-Overlay

Langfristig wird `momentum-bot.positions` nicht die dauerhafte Positions-SOT sein. Bis PA-5 darf Momentum aber seine Exit-Overlay-Position nicht vorzeitig verlieren oder Restpositionen ignorieren. Nach einem partial SELL darf Momentum eine Position nicht als vollstaendig erledigt behandeln, solange Execution/Authority noch Restbestand sehen oder die verkaufte Menge kleiner als die bekannte Position ist.

## Bestehende Patterns

### Pattern A: Korrekte PnL-Formel und Einheit

`src/execution/tokens_per_sol.rs`:

```rust
pub fn pnl_pct(entry_price: f64, current_price: f64) -> f64 {
    if entry_price <= 0.0 || current_price <= 0.0 {
        return 0.0;
    }
    ((entry_price / current_price) - 1.0) * 100.0
}

pub fn drawdown_from_ath_pct(highest_price: f64, current_price: f64) -> f64 {
    if highest_price <= 0.0 || current_price <= 0.0 {
        return 0.0;
    }
    ((current_price / highest_price) - 1.0) * 100.0
}
```

`src/bin/momentum_bot.rs` entry price path uses UI amounts:

```rust
let sol_ui_for_price = result
    .fill_in
    .as_ref()
    .map(|a| a.as_f64())
    .unwrap_or(sol_invested_raw as f64 / 1_000_000_000.0)
    .max(0.0);
let tok_ui = fill_out.as_f64().max(0.0);
let entry_price = if sol_ui_for_price > 0.0 {
    tok_ui / sol_ui_for_price
} else {
    1.0
};
```

Therefore `MomentumContext::executable_exit_quote` must return UI token / UI SOL too. Do not compare raw/raw tps with UI/UI tps.

Expected formula:

```rust
let token_ui = pos.token_amount as f64 / 10f64.powi(pos.token_decimals as i32);
let sol_ui = sol_out as f64 / 1_000_000_000.0;
let tps = token_ui / sol_ui;
```

Use an existing constants/helper if one exists; otherwise keep it local and test it.

### Pattern B: Scale-in updates existing Momentum position

`src/bin/momentum_bot.rs::open_position` already has the correct existing-position update behavior:

```rust
if let Some(pos) = positions.get_mut(p.mint) {
    pos.token_amount = pos.token_amount.saturating_add(p.token_amount);
    pos.add_investment(p.sol_invested);
    pos.exit_generated = false;
    pos.exit_generated_at = None;
    info!(
        mint = %p.mint,
        additional_sol = p.sol_invested,
        additional_tokens_raw = p.token_amount,
        total_sol = pos.sol_invested,
        total_tokens_raw = pos.token_amount,
        "Position scaled in"
    );
}
```

The orphaned BUY recovery currently checks `already_has_position` and does nothing if the position exists. That is wrong for a scale-in ExecutionResult whose pending intent was cleaned up or otherwise not found. Existing-position orphaned BUY recovery must call the same `open_position(OpenPositionParams { ... })` path or an equivalent amount-aware helper so the scale-in fill is applied once.

### Pattern C: Execution is already amount-aware for partial SELL

`execution-engine` correctly observed:

```text
sold_raw < total_pos -> partial confirmed SELL
```

Do not change this into a full close. The problem is Momentum's overlay and quote units, not Execution's partial-sell detection.

## Erlaubte Dateien

Prefer a focused implementation in:

- `src/bin/momentum_bot.rs`

Allowed if needed for small shared tests/helpers:

- `src/execution/tokens_per_sol.rs`
- `src/execution/mod.rs`

Do not edit unrelated DEX connectors or execution send/simulation code unless you hit a STOP-CHECK and explain why.

## Verboten

- Keine neuen RPC-Calls im Momentum-Hot-Path.
- Keine Quote aus einem anderen Pool verwenden, um einen Exit fuer `position.pool` zu validieren.
- Keine Aenderung, die `execution-engine` Simulation-Gate oder Tx-Send-Gate lockert.
- Keine pauschale Full-Close-Annahme nach einem confirmed SELL, wenn `sold_raw < total_pos`.
- Keine neue dauerhafte Positions-SOT in Momentum bauen. Das ist ein Zwischenfix bis PA-5.
- Keine Fallback-Logik, die raw/raw und UI/UI `tokens_per_sol` mischt.
- Kein Deploy.

## Konkrete Anforderungen

### 1. Fix executable quote units

`MomentumContext::executable_exit_quote` muss `ExitExecutableQuote.tokens_per_sol` in derselben Einheit liefern wie `PositionTracker.entry_price/current_price`.

Add tests that prove:

- `entry_tps = 9_876_538.0336`, `sell_raw_tokens=12_345_672_542`, `sol_out_lamports=418_528`, `token_decimals=6` => executable tps about `29_497_841.3439`, PnL about `-66.5%`, not `+90_606%`.
- Lower executable tps than entry remains profit.
- Higher executable tps than entry triggers/permits STOP_LOSS according to config.

### 2. Fix orphaned existing-position BUY recovery

When `ExecutionResult` is a confirmed BUY and there is no pending intent, but `token_mint` and `fill_out` exist:

- If Momentum has no position: keep the current orphaned create-position recovery behavior.
- If Momentum already has a position: apply the confirmed BUY as an additional fill to the existing position using the same amount-aware path as scale-in, including:
  - add `fill_out.raw` to `pos.token_amount`
  - update weighted entry via `add_investment`
  - reset `exit_generated` / `exit_generated_at`
  - preserve/upgrade `token_program`, decimals, creator using existing rules
  - log clearly that an orphaned existing-position BUY was applied

It must be idempotent enough for normal JetStream replay/pending behavior. If there is already an idempotency key or ExecutionResult tracking mechanism, use it. If not, do not invent a large durable store in this scope; add a narrow in-memory guard only if needed and explain in comments/tests.

Add tests that prove:

- Probe position exists with `12_345_672_542`; orphaned confirmed scale-in with `37_843_472_159` updates total to `50_189_144_701`.
- Exit intent generation after that uses the full tracked total, not probe-only.
- Confirmed partial SELL does not make Momentum forget a residual position if there is still tracked balance. If current code closes on any SELL confirm, make it amount-aware in Momentum as well.

### 3. Preserve TIME_EXIT behavior

TIME_EXIT must remain a backstop, but it must report PnL using correctly-scaled pool quote when available. A real hard-stop loss should not wait until TIME_EXIT just because stale `current_price` showed fake profit.

## Pruef-Befehle

Run at minimum:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Also ensure Impl CI's Eval Level 5 passes on PR.

## Supervisor-Review-Fokus nach PR

- Search PR diff for any new `rpc`, `get_account`, `getProgramAccounts`, `getTokenAccountsByOwner`, `RpcClient` in Momentum hot path.
- Verify `executable_exit_quote` unit conversion handles Token-2022 decimals and 6-decimal pump tokens.
- Verify tests cover the production numbers from `9mR7...`.
- Verify no full-close assumption is introduced for partial sells.
- Verify final PR remains small enough; if it starts touching PA architecture broadly, stop and split.
