# IronCrab Handoff – `fix-momentum-ath-drawdown-calc`

## Regel-Verweis (Pflicht, zuerst)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

## Task-Beschreibung

Fix falscher ATH-basierter Drawdown-Berechnung im Momentum-Bot: Trailing Stop zeigt -1986% from ATH bei realen -2.4%, obwohl PnL korrekt (-1.2%). Ursache ist falsche Umrechnung von tokens_per_sol zu SOL_per_token in drawdown_from_ath_pct().

## Relevante Invarianten (VOLLTEXT)

drawdown_from_ath_pct() nutzt denselben Preis wie pnl_pct() (tokens_per_sol)
highest_price tracktet minimalen tokens_per_sol (billigster Preis = ATH)
Formel: (entry/current - 1)*100 für PnL, (current/highest - 1)*100 für Drawdown
Kein RPC-Hot-Path, keine externen Preise

## OpenBrain-relevante Treffer

(aus User-Prompt / Bug-Pattern)
- `architectural_decision` (Aehnlichkeit ~0.0985): IronCrab BUGS_FIXES FIX-01 bis FIX-19: FIX-01 Revert e341c04b. FIX-02 Multi-DEX Liquidation. FIX-44 6005-Retry mark_pumpfun_complete. FIX-05 Geyser subscribe_with_request. FIX-06 bonding_curve_exit_threshold. FIX-08 LockManager total_sol. FIX-10 WsolManager LockManager.available_wsol. FIX-11 WsolManager RPC-Fallback entfernt. FIX-12 Einzelner JetStream Consumer. FIX-13 RPC Creator Liquidation. FIX-14 Ghost Positions zero-balance Overrides. FIX-15 quote_mint dynamisch. FIX-16 Initial WalletBal... (title=BUGS_FIXES FIX-01 bis FIX-19, tags=['fix', 'bugs'])
- `architectural_decision` (Aehnlichkeit ~0.0843): IronCrab Komponenten: market-data Geyser ingest MarketEvents WalletBalanceUpdates. momentum-bot EARLY ESTABLISHED Regime TradeIntents. arb-strategy Multi-Pool Arbitrage TradeIntents. execution-engine LivePoolCache QuoteCalculator WsolManager CrossDexHandler Capital Locks Simulate-gate. control-plane REST API Config Kill-Switch. trades-server Grafana Datasource. Ports: 9801 9802 9803 9804 8080 9899. (title=Komponenten und Ports, tags=['components', 'ports'])
- `architectural_decision` (Aehnlichkeit ~0.06): IronCrab DoD Stop Rule: Neue Funktionalität nicht fertig ohne Decision Record vollständig simulate-gated reason-coded rejects. Kein reales Kapital ohne diese drei. DoD TODO Option C: Wallet Tracking ohne RPC-Scanning. market-data TX-Inferenz für ATA Lifecycle. Keine periodischen Wallet-RPC-Scans. Erkennung neu erstellter geschlossener ATAs via Geyser-Resubscribe. (title=DoD Stop Rule Option C, tags=['dod', 'process'])
- `failure_pattern` (Aehnlichkeit ~0.0532): RPC-Calls im Trading-Flow (category=rpc, fix_strategy=Geyser LivePoolCache, related_modules=['dex_connector', 'market_data'])
- `architectural_decision` (Aehnlichkeit ~0.0525): Cold-Path-Recovery-Paritaet fuer PumpFun Bonding Curve ist jetzt implementiert; sell_all/manual Cold Path zaehlt explizit mit, Hot Path bleibt out of scope. (title=PumpFun Cold-Path Recovery gemergt, context=Nach PumpSwap wurde PumpFun Bonding Curve als naechster DEX-Slice fuer Cold-Path-Recovery umgesetzt und gemergt., consequences=Naechster sinnvoller Schritt ist ein Eval-Vertrag fuer PumpFun-Cold-Path-Recovery-Semantik. Weitere DEXe bleiben separate kleine Scopes., tags=['pumpfun', 'bonding_curve', 'cold_path', 'recovery', 'request_reply', 'market_data', 'merged'])
- `architectural_decision` (Aehnlichkeit ~0.0523): PumpSwap Async-Healing Scope 4 (Alert Rules) ist gemergt. Auf den bereits vorhandenen Healing-Countern wurden kleine Grafana-Alert-Regeln fuer no-NATS, async publish fail, Trigger-Spike und hohe Cooldown-Suppression eingefuehrt. Der Scope blieb rein beobachtend und aenderte keinerlei Runtime- oder Healing-Logik. Ein erster PR-Stand enthielt eine out-of-scope package-lock-Aenderung; diese wurde per Supervisor-Follow-up entfernt, danach lief CI und finaler Bugbot-Gate gruen durch. (title=PumpSwap Async-Healing Scope 4 Alerts gemergt, context=Nach Scope 1-3 und dem Eval-Vertrag wurde der naechste kleine operative Folgeschritt als Alerting statt Dashboard umgesetzt und als PR #36 gemergt., consequences=Der Healing-Pfad ist jetzt nicht nur funktional, beobachtbar und eval-seitig abgesichert, sondern auch operativ ueber Alerts abgedeckt. Weitere Folgeschritte sind optional und eher Feintuning/Operator-UX statt Kernlogik., tags=['pump_amm', 'async_healing', 'scope4', 'alerts', 'grafana', 'bugbot', 'merged'])
- `failure_pattern` (Aehnlichkeit ~0.0513): Successful market-data refresh result is not actually consumed by the structural retry path; execution-engine logs still show pool_accounts_source=intent_resources and unchanged sell_ix_accounts_csv after status=Ok. (category=request-reply, fix_strategy=Narrow fix must make the bounded cold-path retry consume the fresh market-data/JetStream result for the rebuilt PumpSwap sell, especially the observed Extended layout third_meta, without introducing hot-path RPC or local truth in execution-engine., related_modules=['execution_engine', 'tx_builder', 'market_data', 'pumpfun_amm'])
- `architectural_decision` (Aehnlichkeit ~0.051): Merge PR #16: Iron_crab-eval now contains a final green Raydium AMM v4 EnsureRaydiumAmmPoolState Request/Reply E2E contract. CI and final Bugbot both passed before merge. The next smallest remaining Request/Reply eval gap is Orca Whirlpool only: add a narrow on-wire blackbox contract for EnsureOrcaWhirlpoolPoolState in the shared request_reply_e2e_contract harness, without broadening into Meteora, PumpFun one-retry semantics, or generic multi-DEX abstractions. (title=Raydium eval contract merged; Orca next, context=PR #16 merged after green CI and final Bugbot without issues, consequences=Next small scope is Orca Whirlpool Request/Reply eval only; keep scope limited to request_reply_e2e_contract.rs plus a short spec bullet., tags=['eval', 'request_reply', 'raydium_amm', 'orca', 'scope24', 'i24d', 'merge'])

(aus Task und Invarianten)
- `failure_pattern` (Aehnlichkeit ~0.0608): Bug #27: PumpSwap AMM Liquidation scheitert nach Restart wegen degenerate Cache Reserves. Nach Deploy werden PumpSwap AMM Pools mit (0,0) Reserves per Geyser entdeckt. Vault-Balance-Updates kommen asynchron — wenn nur ein Vault-Update vor der Liquidation ankommt, hat der Cache z.B. (691T tokens, 0 SOL). In pumpfun_amm.rs quote_exact_in() liefert der Cache Some((691T, 0)) als Cache-HIT, amount_out=0, Code gibt Ok(None) zurueck und erreicht den RPC-Fallback NICHT. Fix: (a) pumpfun_amm.rs: Bei d... (category=unknown)
- `architectural_decision` (Aehnlichkeit ~0.059): IronCrab WsolManager AccountJanitor: WsolManager event-driven NATS wallet_balance. Wrap wenn WSOL < min_wsol. Unwrap wenn WSOL > max_wsol. Kein Wrap bei KillSwitch. FIX-16 Initial WalletBalanceUpdate. AccountJanitor Close Empty ATA Merge Dust Swap Dust. WSOL von tradeable positions ausschließen FIX-36. Cold Path. (title=WSOL Manager Account Janitor, tags=['wsol', 'janitor', 'cold_path'])
- `architectural_decision` (Aehnlichkeit ~0.0561): Cold Path Recovery-Requests an market-data muessen force refresh via RPC ausloesen; Hot Path darf nicht blockierend darauf warten. (title=PumpSwap Recovery-Semantik: force refresh nur im Cold Path, context=PumpSwap SELLs koennen trotz vorhandener Cache-Daten strukturell fehlschlagen; cache-first Recovery liefert sonst denselben stale State erneut., consequences=execution-engine bleibt ohne lokale Discovery; Liquidation darf synchron auf autoritativen Refresh warten; regulaere Sells nur asynchroner Refresh-Trigger; Warn-Logs/Metriken fuer RPC-Recovery verpflichtend., tags=['pump_amm', 'recovery', 'cold_path', 'hot_path', 'market-data', 'execution-engine', 'i24d', 'rpc', 'force_refresh'])
- `failure_pattern` (Aehnlichkeit ~0.0472): Static SELL=21 assumption despite official cashback remaining_accounts and observed successful 24-account sells (category=pump_amm_sell_layout, fix_strategy=Dynamic sell layout based on authoritative cached feature flags and deterministic extra-account derivation, related_modules=['src/solana/dex/pumpfun_amm.rs', 'src/bin/market_data.rs', 'src/execution/pool_cache_sync.rs', 'src/execution/tx_builder.rs'])
- `architectural_decision` (Aehnlichkeit ~0.0446): Extend the existing bounded wallet bootstrap verification to Raydium CPMM only, reusing the explicit readiness path from PR #48. (title=Next scope after Raydium CPMM readiness, context=Post-merge decision after Scope 9 / PR #48, consequences=Keeps rollout incremental, preserves hot-path RPC-free architecture, and avoids a broad multi-DEX bootstrap orchestrator., tags=['readiness', 'raydium_cpmm', 'bootstrap', 'wallet_relevant', 'bounded_rpc', 'scope10'])
- `architectural_decision` (Aehnlichkeit ~0.0443): IronCrab Eval Liquidation 6005-Retry: Nach BondingCurveComplete (6005) mark_pumpfun_complete_for_mint. is_pumpfun_complete_for_mint Some(true). Liquidation Phase 2 überspringt PumpFun, nutzt PumpSwap AMM. find_pump_amm_pool_by_base_mint, get_pump_amm_pool_accounts_by_base_mint. golden_replay_liquidation_6005_retry prüft Replay-Determinismus. (title=Eval Liquidation 6005-Retry A.13, tags=['eval', 'liquidation', '6005', 'pumpfun', 'pumpswap'])

## Bestehendes Pattern

KNOWN_BUG_PATTERNS.md Pattern #3 (Invertierte PnL-Formel) ist relevant; hier aber spezifisch für ATH-basierten Drawdown statt aktuellem PnL.

## Erlaubte Dateien

- `src/momentum_bot/types.rs`
- `src/momentum_bot/position.rs`
- `src/trade_intent/types.rs`
- `docs/BUGS_FIXES.md`

## Verboten

Keine Änderungen an RPC-Calls, Market-Data-Geyser, ExecutionEngine oder TradeIntent-Build. Nur Position-Preis-Logik und Drawdown-PnL-Formeln.

## Pruef-Befehle

```text
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
