WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 57: Recovered PumpFun Residuals Must Not Route To PumpSwap Without Complete Evidence

## Task-Beschreibung

Fixe eine produktive Routing-Regression nach Scope 56: Eine Restposition eines aktiven PumpFun-Bonding-Curve-Tokens wurde nach Recovery/Restart als `pump_amm`-Position rekonstruiert und danach ueber PumpSwap verkauft. Der Token war aber weiterhin auf PumpFun Bonding Curve; beide BUYs und der erfolgreiche Probe-SELL liefen ueber PumpFun BC.

Der Fix soll verhindern, dass Recovery/Exit-Routing fuer aktive PumpFun-Restpositionen einfach den "neuesten" Pool nimmt und dadurch auf PumpSwap AMM wechselt. PumpSwap darf nur gewaehlt werden, wenn harte Complete/Migration-Evidence vorliegt.

## Runtime-Evidenz

Server-Stand:

- Commit: `2040a8419c53f5d6febd33534fee04856a0304b8` (`Scope 56`)
- Rest-Mint: `y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump`
- Rest-ATA: `52GrRS6s8DJ43W4KAaWmqUwmnpMFq2ZbiaqdN15eHjU6`
- Restmenge: `16_650.263074` Token-2022 (`raw=16650263074`)

On-chain Pfade:

- Probe BUY `2DCBd9ar...`:
  - Program `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
  - Log `Instruction: BuyExactSolIn`
  - PumpFun Bonding Curve

- Scale-in BUY `Ac3ojzUG...`:
  - Program `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
  - Log `Instruction: BuyExactSolIn`
  - PumpFun Bonding Curve

- Successful Probe SELL `czkPKW1...`:
  - Program `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
  - Log `Instruction: Sell`
  - PumpFun Bonding Curve
  - Wallet balance moved from `22_542.499189` to `16_650.263074`

Later logs after restart/recovery:

```text
STOP_LOSS trigger ... mint=y7bgE68... position_pool=HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7 entry_price=1.0 ...
EXIT SIGNAL DETECTED ... pool=HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7 ... token_amount=16650263074
No cached trade ratio - using fallback for emergency exit ... pool=HS9UsHp... dex=pump_amm
Generated EXIT TradeIntent ... pool=HS9UsHp... dex=pump_amm
```

Execution then fails:

```text
Sell routing path ... sell_routing=multi_pool
tx_plan: capped intent min_out with fresh cache quote ...
Running simulation
PumpSwap regular SELL: simulation structural error ... Custom(6023)
```

Important correction: the previous successful sell was **not** PumpSwap. It was PumpFun BC. The failure happens because the residual position was later routed as PumpSwap.

## Relevante Invarianten (Volltext)

### I-7 Hot Path RPC-Freiheit

Momentum-/Arb-/normaler Execution-Hot-Path darf keine blockierenden RPC-Calls ausfuehren. Reconciliation-RPC nur Cold Path. Dieser scope darf keine neuen Hot-Path-RPCs in Momentum exit checks, normaler Trade-Intent-Erzeugung oder Execution regular path einfuehren. Falls Cold-Path-Recovery/Bootstrap RPC nutzt, muss das bounded und klar cold-path bleiben.

### I-9 Simulation-Gate

Keine Transaktion darf ohne erfolgreiche Simulation gesendet werden. Dieser Scope darf weder Simulation umgehen noch bekannte `6023`/`6005` Fehler als sendbar behandeln.

### I-13 Position-Pool-Matching

Positionen duerfen nicht anhand eines fremden Pools bewertet oder verkauft werden. Eine Position, die ueber PumpFun Bonding Curve gekauft und nicht nachweislich migriert wurde, darf nicht durch eine spaetere PumpSwap-Pool-Beobachtung auf PumpSwap umgebogen werden. Pool-Wechsel braucht harte Evidence.

### I-14 tokens_per_sol-Konvention

Alle PnL-/Preiswerte bleiben `tokens_per_sol`: niedriger = Token wertvoller; `pnl_pct = (entry/current - 1) * 100`. Dieser Scope soll diese Formel nicht veraendern.

### I-12 Decision Record / nicht still verwerfen

Wenn Recovery keinen sicheren Pool bestimmen kann oder PumpSwap wegen fehlender Complete-Evidence nicht gewaehlt wird, muss das sichtbar geloggt werden. Kein stilles Umhaengen auf "best available" bei PumpFun-Restpositionen.

## Bestehende Patterns und relevante Bugs

### Pattern 1: `select_reconcile_pool` ist aktuell gefaehrlich

Aktueller Code in `src/bin/momentum_bot.rs`:

```rust
fn select_reconcile_pool(&self, mint: &str) -> Option<(String, String, Option<f64>)> {
    let pools = self.mint_pools.read();
    let pool_list = pools.get(mint)?;
    let best = pool_list.iter().max_by_key(|p| p.last_trade_slot)?;
    Some((
        best.pool_address.clone(),
        best.dex.clone(),
        best.last_trade_ratio,
    ))
}
```

Das nimmt schlicht den neuesten Pool. Fuer `y7...` hat das offenbar `HS9... / pump_amm` gewaehlt, obwohl die reale Position noch PumpFun BC war.

### Pattern 2: `find_best_sell_pool` darf "best available fallback" nicht fuer aktive PumpFun erzwingen

Aktueller Code hat Phase 3:

```rust
let usable = if preferred.is_empty() {
    warn!("All pools excluded ... using best-available fallback");
    &valid
} else {
    &preferred
};
```

Fuer PumpFun-Restpositionen ist dieser Fallback gefaehrlich, wenn er einen PumpSwap-Pool waehlt, obwohl kein `6005`/Complete vorliegt.

### Pattern 3: PumpFun complete evidence existiert bereits

Execution kennt `6005`:

- `is_6005_bonding_curve_complete`
- `mark_pumpfun_complete_for_mint`
- Kommentare: `6005-Retry: Bei BondingCurveComplete (PumpFun) Retry mit PumpSwap AMM`

Das korrekte Verhalten ist: PumpSwap erst nach harter Complete-Evidence.

### Known Bug Patterns

- #3 `tokens_per_sol` nicht anfassen.
- #13 TAKE_PROFIT/PnL nur evidenzbasiert anfassen.
- #14 PumpFun/PumpSwap Account-Formate nicht vermischen.
- #21 PumpFun Cashback-Upgrade: PumpFun BC-Sell muss Account-Set inkl. Token-2022/Cashback korrekt verwenden.
- #22 Kein Simulation-Bypass.
- #34/#35 PumpSwap Recovery/Account-Canonicalization: nicht als Root Cause fuer diesen Scope behandeln, ausser fuer Tests/Logs. Der Kern hier ist falsches Routing zu PumpSwap, nicht PumpSwap-Builder.
- #36 Cache-Hit ist nicht automatisch ready.

## Erlaubte Dateien

Preferiert:

- `src/bin/momentum_bot.rs`

Erlaubt falls notwendig fuer schmale Helper/Tests:

- `src/bin/execution_engine.rs`
- `src/execution/live_pool_cache.rs`
- `src/solana/dex/pumpfun.rs`

Nicht anfassen ohne STOP-CHECK-Begruendung:

- PumpSwap account builder internals, sofern nicht durch Tests eindeutig noetig.
- Simulation/send gate.
- Public IPC struct shape.

## Verboten

- Kein Hot-Path-RPC.
- Kein Simulation-Bypass.
- Kein pauschales "PumpSwap retry" fuer aktive PumpFun-Tokens.
- Kein neuer dauerhafter Positions-SOT in Momentum.
- Kein Umschreiben des PA-Plans.
- Kein Deploy.

## Konkrete Anforderungen

### 1. Recovery-Pool-Auswahl fuer PumpFun-Restpositionen korrigieren

`select_reconcile_pool` / `build_reconciled_position` soll fuer einen Mint nicht blind `max(last_trade_slot)` nehmen, wenn PumpFun BC-Pool bekannt ist und keine harte Complete-Evidence existiert.

Gewuenschtes Verhalten:

- Wenn fuer den Mint ein PumpFun-BC-Pool mit brauchbaren Daten bekannt ist und `bonding_curve_complete != Some(true)`, diesen fuer Reconciliation bevorzugen.
- Wenn LivePoolCache sicher sagt `pumpfun complete == false`, PumpFun ebenfalls bevorzugen.
- Wenn harte Complete-Evidence existiert (`6005`, `bonding_curve_complete == Some(true)`, authoritative complete flag), darf PumpSwap gewaehlt werden.
- Wenn nur PumpSwap bekannt ist, aber PumpFun-Position nicht ausgeschlossen werden kann, nicht still auf PumpSwap umhaengen. Entweder klar loggen und Position nicht fuer PumpSwap-Exit rekonstruieren, oder eine sichere PumpFun-Route aus JSONL/KV/ExecutionHistory verwenden.

### 2. Existing-position Exit-Routing darf Original-Pool nicht wegoptimieren

`generate_and_publish_exit_intent` ruft `find_best_sell_pool` auch fuer bestehende Positionen auf. Fuer eine Position mit `original_dex == "pumpfun"` und ohne Complete-Evidence muss `find_best_sell_pool` den Original-PumpFun-Pool bevorzugen und darf nicht wegen "best available" auf `pump_amm` wechseln.

Akzeptanz:

- Original `pumpfun` + no complete evidence -> Sell intent bleibt `dex=pumpfun`.
- Original `pumpfun` + `6005`/complete evidence -> PumpSwap fallback/route erlaubt.
- PumpSwap-Pools fuer denselben Mint duerfen nicht automatisch gewinnen, nur weil quote/last_trade_slot neuer ist.

### 3. Tests

Fuege fokussierte Unit-Tests hinzu:

1. `select_reconcile_pool_prefers_active_pumpfun_over_newer_pump_amm`
   - mint hat PumpFun-Pool mit `bonding_curve_complete=None` oder `Some(false)`
   - mint hat neueren PumpSwap-Pool
   - Reconcile waehlt PumpFun.

2. `find_best_sell_pool_keeps_original_pumpfun_when_not_complete`
   - Original-Pool ist PumpFun.
   - PumpSwap hat bessere quote/neueres slot.
   - Kein Complete-Beleg.
   - Result bleibt PumpFun.

3. `find_best_sell_pool_allows_pump_amm_after_pumpfun_complete`
   - PumpFun-Pool `bonding_curve_complete=Some(true)` oder equivalent complete evidence.
   - PumpSwap darf gewaehlt werden.

4. Regression fuer `y7`-Klasse:
   - PumpFun BUY/Sell history / recovered residual with PumpFun route.
   - PumpSwap pool `HS9...` exists and is newer.
   - Exit intent does not use `pump_amm` unless complete evidence is set.

### 4. Logging

Add structured logs for:

- When PumpFun is preferred over PumpSwap during recovery because no complete evidence exists.
- When PumpSwap is allowed because complete evidence exists.
- When fallback would have chosen PumpSwap but is blocked due to active PumpFun position.

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR muss ausserdem Impl CI inkl. Eval Level 5 gruen haben.

## Supervisor-Review-Fokus

- Kein neuer RPC im normalen Momentum exit path.
- Keine Aenderung an `tokens_per_sol`-Formel.
- Kein Simulation bypass.
- Tests decken den aktiven PumpFun + neuer PumpSwap-Pool Fall ab.
- PumpSwap darf nur nach `6005`/complete evidence gewaehlt werden.
