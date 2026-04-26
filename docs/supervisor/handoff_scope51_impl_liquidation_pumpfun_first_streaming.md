WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Handoff Scope 51: Liquidation PumpFun-First Routing und Streaming Execution

## Task-Beschreibung

Fix the liquidation latency regression observed on 2026-04-26.

Current behavior:

1. `run_liquidation_job` performs an RPC owner scan and correctly finds all non-zero wallet token accounts. Keep this.
2. It then iterates over the full inventory and prepares all liquidation intents before sending the first SELL.
3. For each token, the current routing tries PumpSwap/multi-pool first, even for known PumpFun bonding-curve tokens.
4. PumpSwap discovery is often slow or fails. Because the loop waits per token, the first SELL can be delayed by many minutes.

Production evidence:

- KillSwitch at `00:52:59`.
- RPC owner scan found `non_zero_positions=11`.
- Liquidation prepared intents sequentially from `00:53:43` until `01:01:08`.
- First liquidation SELL was only sent after all intents were prepared, confirmed at `01:01:21`.
- Most per-token delays were PumpSwap request/reply discovery or timeout/error before falling back to PumpFun.

Required behavior:

- Keep the initial RPC owner scan. It is correct and required to capture all wallet tokens.
- For known active PumpFun bonding-curve tokens, try PumpFun direct SELL first.
- Only fall back to PumpSwap/multi-pool if PumpFun is not usable or simulation/build indicates migration/completion, especially `6005` / BondingCurveComplete.
- If all data for an intent is available, submit/process that intent immediately. Do not wait for discovery/preparation of unrelated tokens.
- Discovery, when needed, must remain sequential/bounded. Do not launch many parallel RPC discoveries.
- Parallel or overlapping sends are allowed if the intent is fully prepared and normal execution locks/simulation gates apply.
- Correct the misleading log text `pump_amm=timeout (10s)` to reflect the actual 45s timeout. Do not reduce `DISCOVERY_REQUEST_TIMEOUT_SECS` or the 45s PumpSwap quote timeout in this scope.

## Relevante Invarianten (Volltext)

### I-5 Cold Path

COLD PATH includes Liquidation, Manual Actions and Bootstrap. RPC is allowed in the cold path. Safety and correctness are more important than raw speed. Authoritative on-chain state may be loaded here.

For this scope: the initial `getTokenAccountsByOwner` owner scan remains mandatory and correct.

### I-7 Hot Path RPC-Freiheit

Hot Path (normal Momentum/Arb trading flow) must not perform blocking RPC calls. This scope touches only KillSwitch liquidation cold path. Do not introduce RPC into regular Momentum sells.

### I-9 Simulation-Gate

Transactions must not be sent without successful simulation. This scope must not bypass simulation for PumpFun or PumpSwap fallback.

### I-12 Decision Record

No intent may be silently dropped. If PumpFun direct SELL fails and fallback is attempted, the decision/failure path must remain observable. If an intent cannot be built, the logs must include the mint and reason.

### A.38 / I-24d Cold-Path Discovery nur per Request/Reply

If execution-engine needs missing pool accounts in Cold Path, it may send a correlated request to `market-data` and wait bounded for the authoritative response. Execution-engine must not locally discover missing pool accounts or write local replacement truth into the SLAVE cache. Discovery, MASTER write and JetStream publication remain owned by `market-data`.

### A.29 Liquidation Vollstaendigkeit

Liquidation must detect all non-zero tokens in the wallet, build a correct SELL intent for each and must not fail because cache data is missing when a Cold Path RPC/recovery path is allowed.

## Bestehendes Pattern / Relevante Code-Stellen

Primary file:

- `src/bin/execution_engine.rs`

Relevant current code:

- `run_liquidation_job(...)`
  - performs RPC owner scan for SPL + Token-2022 accounts
  - seeds LockManager with RPC balances
  - builds `liquidation_intents: Vec<TradeIntent>`
  - iterates inventory and currently tries multi-pool first
  - after preparation, processes all intents

- PumpSwap path in liquidation:
  - `tokio::time::timeout(Duration::from_secs(45), pump_amm.quote_exact_in(...))`
  - stale log text currently emits `pump_amm=timeout (10s)` although the timeout is 45s
  - if cache has ready PumpAmm quote/accounts, it should use cache and **not** discover
  - if quote exists but pool_accounts are missing, it requests `EnsurePumpAmmPoolAccounts` with `pool_id` as hint (fast path)
  - if quote cache miss returns `Ok(None)`, it requests discovery with `pool_hint` from cache if available; without hint market-data may fall back to slow `getProgramAccounts`

- `src/solana/dex/pumpfun_amm.rs`
  - `pool_accounts_v1_for_base_mint_with_hint(...)`
  - with pool hint: uses single `getAccount` fast path
  - without hint: may fall back to slow `discover_pool_static` / `getProgramAccounts`

Known bug patterns:

- #18 Liquidation: stale data vs RPC
- #26 `run_liquidation_job` must not block main loop; keep spawned job pattern
- #27/#31/#33/#34/#36 PumpSwap cache/readiness/discovery pitfalls
- New runtime pattern: liquidation latency scales linearly with optional PumpSwap discovery because all intents are prepared before first send.

## Erlaubte Dateien

- `src/bin/execution_engine.rs`
- Narrow tests in the existing execution-engine test module
- Documentation/log text only where directly related

Touch other files only if absolutely necessary and explain why in the PR.

## Verboten

- No deploy, no `deploy.sh`, no server/systemd restart.
- Do not change timeout durations from 45s in this scope.
- Do not parallelize RPC discovery across all tokens.
- Do not introduce hot-path RPC.
- Do not bypass simulation.
- Do not change PumpSwap parser/layout logic.
- Do not implement PositionAuthority in this scope.
- Do not change dashboard queries.

## Erwartete Implementierungsrichtung

### A. Classify route order per token

For each wallet token from the owner scan, decide route order:

1. If token has known PumpFun bonding-curve route and is not known completed/migrated:
   - Build PumpFun liquidation intent first.
   - Process/send it as soon as it is prepared.
   - If simulation/build returns BondingCurveComplete / `6005` or a known migration signal, then fall back to PumpSwap/multi-pool.

2. If token is known completed/migrated or PumpFun route is unavailable:
   - Use existing multi-pool routing.

3. If state is unknown:
   - Use existing safe multi-pool routing, but keep discovery sequential and bounded.

Use existing cache/metadata helpers where possible. Do not guess migration from string names.

### B. Stream execution instead of prepare-all-then-send

Do not collect all liquidation intents and only then process them.

Acceptable approaches:

- Simple safe approach: for each token, prepare one intent, immediately call `process_intent`, then continue to the next token.
- Better bounded approach: discovery remains sequential, but once an intent is prepared, spawn/process it immediately with a small bounded in-flight send limit. Wait for all spawned send tasks before reporting `Liquidation job completed`.

The key invariant: ready tokens should not wait behind unrelated token discovery.

### C. Preserve cache-first behavior

Answer to expected review question:

- PumpSwap should use cache when quote and ready pool_accounts/reserves are available.
- Request/reply discovery should happen only when cache is missing/not ready for the needed route.
- If a PumpFun direct route is usable, do not spend 45s discovering PumpSwap first.

### D. Fix log text

Change misleading log text:

- From: `pump_amm=timeout (10s)`
- To: `pump_amm=timeout (45s)`

Do not change the timeout value.

## Erwartete Tests

Add or update focused tests proving:

1. Known active PumpFun token route order prefers PumpFun before PumpSwap.
2. PumpFun 6005 / BondingCurveComplete triggers PumpSwap/multi-pool fallback.
3. A prepared liquidation intent is processed before unrelated later token discovery completes.
4. PumpSwap discovery is not launched when cache already has usable PumpSwap quote + ready pool_accounts.
5. PumpSwap discovery remains sequential/bounded when multiple tokens need it.
6. Log/quote_attempt text reflects `45s`, not `10s`.

If some of these are difficult to express as unit tests because current code is not factored, introduce small pure helper functions for route-order and timeout-text decisions, then test those helpers.

## Pruef-Befehle

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

If CI provides Eval Level 5, ensure it passes before merge.

## Runtime Acceptance Criteria

In a future liquidation with many active PumpFun bonding-curve tokens:

- owner scan still captures all non-zero tokens
- first SELL is submitted shortly after the first valid PumpFun intent is prepared
- PumpSwap discovery does not run before PumpFun for known active bonding-curve tokens
- no `pump_amm=timeout (10s)` log remains
- if PumpFun returns 6005/completed, fallback route remains available
- no deploy unless user explicitly approves
