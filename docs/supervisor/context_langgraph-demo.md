# IronCrab Handoff – `langgraph-demo`

## Regel-Verweis (Pflicht, zuerst)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

## Task-Beschreibung

Impl: kleine, isolierte Aenderung laut Scope (Platzhalter fuer LangGraph-Demo).

## Relevante Invarianten (VOLLTEXT)

I-7: Kein synchroner RPC im Hot Path.
I-9: Transaktionen nur nach erfolgreicher Simulation.

## OpenBrain-relevante Treffer

- `architectural_decision` (Aehnlichkeit ~0.076): PumpSwap Async-Healing Scope 3 (Metrics/Observability) ist gemergt. Der bereits vorhandene Hot-Path-Healing-Pfad fuer regulaere momentum-bot PumpSwap-SELLs ist jetzt ueber bestehende Runtime-Metriken sichtbar: Trigger ausserhalb Cooldown, Cooldown-suppressed Trigger, async publish ok, async publish fail/drop und no-NATS skip. Die Aenderung ist rein beobachtend; Trigger-, Cooldown- und Cold-Path-Semantik bleiben unveraendert. Der begleitende Eval-Vertrag fuer die Recovery-Semantik wurde ebenfa... (title=PumpSwap Async-Healing Scope 3 und Eval-Vertrag gemergt, context=Nach den bereits gemergten Scope-1/2-Aenderungen wurden PR #34 (Iron_crab) und PR #13 (Iron_crab-eval) vom Supervisor reviewed, durch CI und finalen Bugbot-Gate gebracht und anschliessend gemergt., consequences=Der Recovery-Pfad ist jetzt sowohl implementiert als auch beobachtbar und eval-seitig vertraglich abgesichert. Weitere Folgescopes sollten operativ klein bleiben und eher Dashboard-/Alerting-/Operator-Doku betreffen als neue Healing-Logik., tags=['pump_amm', 'async_healing', 'scope3', 'metrics', 'observability', 'eval', 'bugbot', 'merged'])
- `architectural_decision` (Aehnlichkeit ~0.0646): Merge PR #52: Meteora CPMM now has an explicit DexPoolReadiness path in market-data -> JetStream -> SLAVE, and mint-level ready-gating counts Meteora CPMM only via explicit Ready. The next incremental scope should reuse this path in bounded wallet bootstrap verification for Meteora CPMM only, without broadening to Meteora DLMM or Orca. (title=Meteora CPMM explicit readiness merged, context=Merge von PR #52 nach gruener CI und finalem Bugbot ohne Issues, consequences=Next small scope is bounded wallet bootstrap verification for Meteora CPMM only, reusing the new explicit ready path and preserving hot-path RPC-free architecture., tags=['readiness', 'meteora_cpmm', 'merge', 'scope13', 'bootstrap_next'])
- `failure_pattern` (Aehnlichkeit ~0.0601): Confirmed partial SELL path assumes full close in execution-engine/market-data state while Momentum/position lifecycle allows scale-in residuals; max_open_positions relies on Momentum state instead of authoritative wallet/execution holdings. (category=positions_exits, fix_strategy=Treat confirmed SELLs as amount-aware: subtract sold_amount from LockManager and only untrack ATA/clear position when full wallet balance is sold or account actually closed. Momentum exit intents after scale-in must sell total held amount and keep residual positions counted until on-chain zero. Max-open gate should use authoritative execution/wallet open-position count or reconciled position state, not only transient strategy positions., related_modules=['momentum_bot', 'execution_engine', 'market_data', 'LockManager', 'max_open_positions'])
- `failure_pattern` (Aehnlichkeit ~0.0537): Disk voll (/var/solana 98%), Ledger 2.8TB trotz limit-ledger-size, validator.log 365GB durch Restart-Loop, Permission denied nach mkdir als root (category=infrastructure, fix_strategy=Sauberer Neustart: Ledger + Accounts + Log löschen, Berechtigungen auf sol:sol setzen, Validator neu starten (Snapshot-Download), related_modules=['agave-validator', 'server-infrastructure', 'ironcrab-prod'])
- `architectural_decision` (Aehnlichkeit ~0.0535): After merging Iron_crab-eval PR #20, PumpFun Bonding Curve cold-path recovery eval is green. The next smallest remaining eval gap is Orca cold-path reserve fallback: known Orca pool plus missing/unusable live reserves in cold path with RPC unreachable must yield Err, not silent Ok(None). The implementation-side Orca API is now observable via the public cold-path constructor with explicit RPC-on-miss control, so the next scope should be a narrow eval-only extension of the cross-dex cold-path r... (title=Scope 27 merged; Orca cold-path eval next, context=PR #20 merged after green CI and final Bugbot without new issues, consequences=Next small scope is a Test Authority slice extending the cold-path reserve-fallback eval contract to Orca only; avoid broad multi-DEX refactors or new impl work unless eval reveals a real gap., tags=['eval', 'orca', 'cold_path', 'reserve_fallback', 'scope28', 'post_scope27', 'incremental_rollout'])
- `architectural_decision` (Aehnlichkeit ~0.0532): IronCrab Bug Quick-Check: Sieht das wie bekannte Muster? Hot oder Cold Path? Pool/Quote/Fill von richtiger Quelle? State exit_generated pending position korrekt zurückgesetzt? DEX-Namen Decimals Units konsistent? Root Cause sicher? Kein Fix ohne Evidenz. (title=Bug Quick-Check, tags=['checklist', 'bug'])

## Erlaubte Dateien

- `src/lib.rs`

## Verboten

Keine Aenderungen ausserhalb der erlaubten Pfade; keine Secrets committen.

## Pruef-Befehle

```text
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
