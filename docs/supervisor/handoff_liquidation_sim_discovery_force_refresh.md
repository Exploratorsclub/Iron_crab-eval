# Handoff: Liquidation Sim-Failure → market-data Discovery (force_refresh)

## Regel-Verweis (Pflicht)

**WICHTIG:** Lies und befolge die STOP-CHECK Regeln in `AGENTS.md` und `.cursor/rules/ironcrab-core.mdc` BEVOR du eine Datei änderst. Wenn eine geplante Änderung gegen eine Regel verstößt, STOPPE sofort und melde den Verstoß.

---

## Supervisor-Hinweis (Kontext)

Der Supervisor hatte fälschlich lokal Code in `Iron_crab` geändert (Verstoß gegen Workspace-Regel). Lokaler Stand **`src/bin/execution_engine.rs`** wurde per `git checkout -- src/bin/execution_engine.rs` zurückgesetzt — **dieser Impl-Scope ist NICHT durch einen lokalen Commit abgesichert**; Änderungen ausschließlich über **Cloud-Agent-PR** gegen `architecture-rebuild`.

---

## Aufgabe

Bei **`is_liquidation_sell`** (wie in `process_intent`: Sell + `purpose=liquidation` **oder** `kill_switch=true`) soll bei einem **jeden Simulationsfehler** (nicht nur „structural error family“ wie 6013/6023/Overflow) vor dem Aufgeben ein **Cold-Path-Recovery** erfolgen:

1. **`pump_amm`:** Bereits vorhandene `EnsurePumpAmmPoolAccounts`-Recovery nutzt oft `force_refresh=true` über `request_discovery_and_wait(..., true)` — der **Auslöser** ist aber aktuell an `is_pump_amm_structural_sim_error` gekoppelt ([`execution_engine.rs` Sim-Loop vor `emit_rejected`/final break](https://github.com/Exploratorsclub/Iron_crab/tree/architecture-rebuild)). **Änderungsziel:** Für **Liquidations-SELL** soll dieselbe Retry-Schleife (`continue` nach JetStream-/Cache-Wait) bei **allen** PumpSwap-Sim-Fehlern laufen (`6004`, Slippage, …), solange noch kein erfolgreicher Recovery-Versuch lief (**ein** Retry weiterhin über Flags wie `pump_amm_recovery_attempted` abbilden).

2. **`pumpfun` (Bonding) / `orca`:** Analog die Bedingungen lockern: Statt nur `is_*_structural_sim_error(..)` gilt für **Liquidation** **jeder** fehlgeschlagene Sim (gleiche Retry-Ökonomie: ein zusätzlicher Durchgang pro DEX wie heute).

3. **Nicht betroffen / unverändert lassen wo sinnvoll:** `sell_all` ohne Liquidation/Kill-Switch — dort **weiterhin** structural-only, um unnötige RPCs zu vermeiden (wie bisher dokumentiert bei Cold-Path-Hot-Path-Trennung).

4. Keine neuen RPC-Calls im normalen Trading-Hot-Path; Recovery bleibt hinter bestehenden **`is_cold_path_recovery_sell`** / Liquidations-Gates.

---

## Relevante Invarianten (Volltext, kurz)

- **I-7 / Hot-Path RPC:** Keine zusätzlichen blockierenden RPCs im Discovery/Buy/Sell-Hot-Path; Recovery nur in bestehendem Sim-Fail-/Cold-Path-Block.
- **I-24d Cold Path:** Discovery über market-data **`ControlRequest`** + JetStream-SSOT; Pattern wie bestehende `Ensure*`-Recovery-Stücke einhalten.
- **I-9 Simulate-gated:** Weiter simulate-gated senden.

---

## Bestehendes Pattern (Orientierung)

- Sim-Schleife in `execution_engine.rs` (~Zeilen 9358–9690): PumpSwap Structural + `wait_for_*`/`request_discovery_and_wait` mit **`true`** dort, wo bereits force_refresh gedacht ist.

---

## Erlaubte Dateien

- Primär `src/bin/execution_engine.rs` (Hilfsfunktion `[inline]` optional; Bedingungen an `is_liquidation_sell` wie im Scope bereits berechnet).
- Gegebenenfalls Unit-Tests in `execution_engine` **mod execution_engine_tests** falls vorhandenes Muster für Cold-Path/Gating.

---

## Verboten

- Keine Änderungen in `Iron_crab-eval/tests/` (dieser Scope ist Impl-only).
- Kein Hot-Path-RPC in neue Codepfade ohne Cold-Path-Kontext.

---

## Prüf-Befehle (vor PR)

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --features test_helpers --quiet   # wenn im Repo für EE üblich
```

---

## Cloud-Agent Start (Impl)

```powershell
$handoff = Get-Content -Raw "Iron_crab-eval/docs/supervisor/handoff_liquidation_sim_discovery_force_refresh.md"
$prompt = "$handoff`nImplementiere beschriebene Änderungen. Branch: architecture-rebuild. PR automatisch erstellen.`nPflicht: AGENTS.md + ironcrab-core STOP-CHECK."
$body = @{ prompt = @{ text = $prompt }; source = @{ repository = "https://github.com/Exploratorsclub/Iron_crab"; ref = "architecture-rebuild" }; target = @{ autoCreatePr = $true; branchName = "cursor/liquidation-sim-discovery-force-refresh-all" } } | ConvertTo-Json -Depth 6
$cred = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("$env:CURSOR_API_KEY`:"))
Invoke-RestMethod -Uri "https://api.cursor.com/v0/agents" -Method Post -Headers @{Authorization="Basic $cred"; "Content-Type"="application/json"} -Body $body
```

(Ohne `$env:CURSOR_API_KEY` zuerst in den Cursor Settings einen Key setzen.)

---

## Nach Impl-PR (Supervisor-Pflicht)

1. CI + Invarianten-Review (Hot-Path-RPC, Simulation-Gate, I-24).
2. Nach mergebarem Stand: Bugbot auf dem PR wie in `supervisor-agent.mdc` (Poll, kein zweiter Trigger auf gleichem Stand ohne Ausgang ohne User-Erlaubnis).
