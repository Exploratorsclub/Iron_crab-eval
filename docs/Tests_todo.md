# Tests TODO – noch offene Eval-Tests

**Stand:** 2026-08-22  
**Zweck:** Lebendes Backlog. Erledigte Einträge sind unten kompakt; nicht erneut implementieren.

**Quellen:** `docs/spec/INVARIANTS.md`, `EVAL_TEST_CANDIDATES.md` (historisch), Impl `ARCHITECTURE_AUDIT.md` (nicht umschreiben).

**Hinweis (PumpFun IDL):** A.25 nutzt `track_volume: OptionBool` (Instruction-Daten 25 Byte; Hot Path `track_volume=false`).

---

## Offene Punkte

### 1. DEX-übergreifende Recovery-Verifikation (Eval-Gap)

Cold-Path force-refresh ist im Impl für die bekannten DEX-Pfade gemergt. **Eval-/On-Wire-Vertrag** fehlt noch für **Raydium CPMM** und **Meteora CPMM**. PumpSwap und PumpFun Bonding sind abgedeckt.

Richtung: pro DEX ein enger Blackbox-Slice, kein All-at-once-Test.

### 2. L2c — Momentum Imminent-Entry (I-MD-9 / A.50) — OFFEN

**Spec:** `docs/spec/MOMENTUM_ACTIVE_POOLS.md`, `INVARIANTS.md` A.50  
**Gewünscht:** Pin-Publish nicht allein bei Discovery; Probe/Scale-In nur nach Hot-Set + Revalidate; `WaitHotSet` Unpin; Open-Position `pin_reason: position` bleibt must-hot; Metriken `filter_pass_hot_fresh` / `wait_hot_set_*` / `intent_path`.

**Zieldatei:** `tests/invariants_momentum_imminent_entry.rs` (existiert noch nicht).

**Prüf-Befehle:** Eval-Workflow „Rust“ (`fmt`, `check`/`build`, `clippy -p ironcrab-eval` ohne `--all-targets`). Volle Suite: Impl Eval Level 5.

### 3. A.48 Arb Quote Contract (Material-Slot / Fingerprint)

Datei `tests/invariants_arb_quote_contract.rs` existiert. Spec-Text A.48 (Material-Slot, Heartbeat darf `as_of_slot` nicht fälschen, Idle-Buy+Live-Sell nicht `passed_gates`) bei Spec-Änderungen gegen diese Tests halten; fehlende Fälle hier nachtragen.

### 4. Optional / niedrig

- Control-Plane `ConfigUpdate` Schema-Konsistenz (DoD §I) — bisher ausgelassen.
- Neue Scopes nur aus Runtime-Evidenz oder ungetesteter DEX-Recovery, nicht aus dem geschlossenen PumpSwap/PumpFun-Rollout.

---

## Erledigt (nicht erneut aufsetzen)

| Thema | Datei(en) |
|-------|-----------|
| I-13 Pool-Matching | `invariants_pool_matching.rs` |
| Liquidation 6005-Retry | `invariants_liquidation_flow.rs`, golden replay |
| I-4 / I-7 Hot-Path kein RPC | `invariants_hot_path_no_rpc.rs` |
| Router hops2 / best quote | `invariants_router_slippage.rs` |
| Arbitrage Engine | `invariants_arbitrage_engine.rs` |
| Orca / PumpFun IX | `invariants_orca_ix.rs`, `invariants_pumpfun_ix.rs` |
| Compute-Budget | `invariants_compute_budget.rs` |
| I-14 tokens_per_sol | `invariants_tokens_per_sol.rs` |
| DEX Parser PumpSwap / CPI | `invariants_dex_parser_pumpswap.rs`, `invariants_dex_parser_cpi.rs` |
| PumpFun Cashback / Market Order | `invariants_pumpfun_cashback.rs`, `invariants_pumpfun_market_order.rs` |
| PumpSwap Recovery + A.43/A.44 | gemergt (Eval-Vertrag) |
| Trailing Session High | `invariants_trailing_session_high.rs` |

Migrationsplan für **neue** Tests: Invariante in Spec → Blackbox in eval → Impl-Regression behalten oder ersetzen → CI wie CONTRIBUTING.md.

---

## Querbezüge

- **INVARIANTS.md** – verbindliche Spec
- **DEFINITION_OF_DONE.md** – historischer Umbau (Banner lesen)
- **EVAL_TEST_CANDIDATES.md** / **ARCHITECTURE_AUDIT.md** – nicht umschreiben

*ironcrab-eval*
