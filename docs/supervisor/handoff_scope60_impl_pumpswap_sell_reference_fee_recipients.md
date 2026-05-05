WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 60: PumpSwap Force-Refresh Must Propagate SELL Reference Fee Recipients

## Task-Beschreibung

Fix the current PumpSwap liquidation failure root cause:

- A kill-switch liquidation for mint `y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump` and PumpSwap pool `HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7` failed in simulation with `Custom(6023)`.
- The first attempt used base 21-account SELL layout and failed.
- The existing cold-path `EnsurePumpAmmPoolAccounts(force_refresh=true)` correctly asked `market-data` to refresh the pool.
- `market-data` found a successful same-pool SELL reference transaction and correctly detected the extended 24-account layout with `third_meta=AktftA98kSWAxn6kVSoqBXBELUArjKu2H9WmKB48ULFY`.
- The retry used 24 accounts, global SELL `fee_config=5PH...` and `fee_program=pfee...`, but still failed with `Custom(6023)`.

Root cause from runtime evidence:

`market-data` uses the same successful SELL reference transaction only to propagate the extended SELL layout, but it does not propagate the authoritative SELL protocol fee recipient accounts from that reference. It keeps the earlier market/global_config-derived fee recipient pair in `PumpAmmPoolStatic`, so `as_pool_accounts_v14()` publishes wrong v14 slots `[6]/[7]`; then `tx_builder` correctly preserves those wrong v14 slots as SELL metas `#9/#10`.

The fix must make `market-data` use the correct values it already observes from the successful same-pool SELL transaction.

## Runtime-Evidenz

Liquidation window:

```text
2026-04-30T13:07:55Z Starting liquidation job
wallet=Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM
mint=y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
balance_raw=16650263074
pool=HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7
```

First tx build before refresh:

```text
sell_extended=false
sell_ix_account_count=21
sim_error=UiTransactionError(InstructionError(1, Custom(6023)))
```

Market-data force-refresh:

```text
EnsurePumpAmmPoolAccounts received
base_mint=y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
pool_address_hint=Some("HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7")
force_refresh=true
```

Market-data currently derives wrong protocol fee pair from global_config/market parse:

```text
protocol_fee_recipient=62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV
protocol_fee_recipient_ta=94qWNrtmfn42h3ZjUZwWvK1MEo9uVmmrBPd2hpNjYDjb
```

Market-data also observes a successful same-pool SELL reference:

```text
signature=3TsEarZgfUg5BzfVzE7a7mdCyqTgsqXzmf3iCE9GnzzTFYP3SdzEWtGMzE9WXzAoWQgwaVnW3fVocqzZQP4j217A
layout=Extended { third_meta: AktftA98kSWAxn6kVSoqBXBELUArjKu2H9WmKB48ULFY }
termination_reason=layout_found
```

The correct PumpSwap inner SELL ix accounts from that successful reference are:

```text
#0  HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7
#1  3yKBP2HLVfBzTUBjWVxmXyvSxg5jbiFN8fdEsjxxvv4n
#2  ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw
#3  y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
#4  So11111111111111111111111111111111111111112
#5  9TKbwBYryzjg4hh1Lt43Z1r6ov4Ajo3k6KtyJm8UvyNE
#6  BcRUAP5mvjrsbB7Q49AatjixwtwwFuf5mcP9yjy9Huc8
#7  3T42nPotoc1nGNekbTYKMTaUZZu3ndA5a4we1HfWmxt7
#8  7pdfwGevvq2ybRUp945NA9ZcvkZD41fAu7sAN9SJfPYC
#9  AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY
#10 FGptqdxjahafaCzpZ1T6EDtCzYMv7Dyn5MgBLyB3VUFW
#11 TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb
#12 TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA
#13 11111111111111111111111111111111
#14 ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL
#15 GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR
#16 pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA
#17 6mh4SQ1i28dKKhEw52kMNiDbfNCRtsa867dcKXeWdA6N
#18 EL4Y4zsAbxuVFv1w6aMCY4FMqR6oG8fQ6g4rULvkC2eG
#19 5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx
#20 pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ
#21 CXfrfpXNoQ8Qj4Zf6MxTqoh3datTxgKFsQnt7MFv257z
#22 GXPFM2caqTtQYC2cJ5yJRi9VDkpsYZXzYdwYpGnLmtDL
#23 AktftA98kSWAxn6kVSoqBXBELUArjKu2H9WmKB48ULFY
```

Therefore, for this pool the authoritative SELL fee recipient pair is:

```text
protocol_fee_recipient    = AVmoTthdrX6tKt4nDjco2D775W2YK3sDhxPcMmzUAmTY
protocol_fee_recipient_ta = FGptqdxjahafaCzpZ1T6EDtCzYMv7Dyn5MgBLyB3VUFW
```

The retry built by current code used:

```text
sell_extended=true
sell_ix_account_count=24
sell_cashback_third_meta=Some(AktftA98kSWAxn6kVSoqBXBELUArjKu2H9WmKB48ULFY)
SELL #9/#10 still from v14[6]/[7] = 62qc2... / 94qW...
simulation failed: UiTransactionError(InstructionError(1, Custom(6023)))
```

The wallet token account was also verified on-chain:

```text
token_account=52GrRS6s8DJ43W4KAaWmqUwmnpMFq2ZbiaqdN15eHjU6
program=Token-2022
amount=16650263074
owner=Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM
```

This rules out the main alternative hypothesis "wrong user base ATA / missing balance".

## Relevante Invarianten (Volltext)

### I-4 / I-7 Hot Path RPC-Freiheit

HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. Nie RPC in Hot Paths ohne explizite Freigabe. Dieser Scope darf keine neuen blockierenden RPC-Calls im normalen Momentum-/Arb-Hot-Path einfuehren.

### I-5 / I-6 Cold Path Correctness

COLD PATH (Liquidation, Manual Actions, Bootstrap): RPC erlaubt. Safety und correctness vor Speed. Nie RPC aus Cold Paths entfernen um zu "optimieren", wenn dadurch safety-kritische Flows brechen. Der betroffene Pfad ist `EnsurePumpAmmPoolAccounts(force_refresh=true)` nach Liquidation-Simulation-Fail.

### I-9 Simulation-Gate

Wenn Simulation fehlschlaegt, nie senden. Kein Bypass fuer `Custom(6023)`, keine "known transient" Send-Exception. Der Fix muss die Account-Metas korrigieren, nicht das Simulationsergebnis umgehen.

### I-12 Decision Record / Forensik

Jeder Intent braucht ein Decision Record und klare Failure-Diagnostik. Keine stille Ablehnung. Die vorhandenen Scope44-Diagnostics sollen mindestens erhalten bleiben und gerne um `sell_reference_protocol_fee_recipient` / `sell_reference_protocol_fee_recipient_ta` erweitert werden.

### I-24d Cold-Path Discovery nur per Request/Reply

execution-engine darf fehlende oder fehlerhafte PumpSwap `pool_accounts` weder selbst discovern noch lokal in den SLAVE Cache schreiben. Discovery, MASTER-Write und JetStream-Publikation bleiben bei market-data. Dieser Fix muss in `market-data` / `pumpfun_amm` Autoritaet liegen, nicht in einer lokalen execution-engine-Heilung.

### PumpSwap Build IX Fee-Metas Contract (A.3)

Fuer `pool_accounts` der Laenge 14 bezieht sich der Vertrag auf die Account-Metas der erzeugten PumpSwap-Swap-Instruction: Meta #9 `protocol_fee_recipient`, Meta #10 `protocol_fee_recipient_ta`, bezogen auf `pool_accounts[6]` / `pool_accounts[7]`. Sind `[6]` und `[7]` beobachtete nicht-default Pubkeys, muessen die entsprechenden Account-Metas genau diese Werte widerspiegeln. Kein stiller Ersatz durch einen globalen Mainnet-Recipient. Sind beide default/leer, muss der Build klar fehlschlagen.

### PumpSwap Extended SELL Contract (A.3)

Fuer SELL `base -> WSOL` mit 14er `pool_accounts`, `sell_requires_cashback_remaining=true` und `sell_cashback_third_meta=Some(third)` muss `build_swap_ix_from_pool_accounts` erfolgreich sein, genau eine PumpSwap-Swap-Instruction liefern und auf dem Extended-SELL-Pfad 24 Account-Metas exponieren; das letzte Meta muss exakt `third` sein.

## Bestehendes Pattern / Code-Kontext

Current code already has the building blocks:

- `PumpAmmPoolStatic::as_pool_accounts_v14()` maps `protocol_fee_recipient` / `_ta` to v14 `[6]/[7]`.
- `build_swap_ix_from_pool_accounts()` then preserves v14 `[6]/[7]` as final SELL #9/#10.
- `pump_amm_pool_static_from_parsed_swap_ix()` already knows how to parse a SELL ix:
  - `protocol_fee_recipient = parse_pk(9)`
  - `protocol_fee_recipient_ta = parse_pk(10)`
  - `sell_requires_cashback_remaining = acc_accounts.len() == 24`
  - `sell_cashback_third_meta = parse_pk(23)` for extended SELL.
- `pump_amm_sell_layout_observation_from_parsed_swap_ix()` currently throws away the parsed `PumpAmmPoolStatic` and returns only `(pool_market, base_mint, layout)`.
- `wrap_pool_accounts_v14_with_diagnostic()` applies only `sell_requires_cashback_remaining` and `sell_cashback_third_meta` to the local `pool`, then publishes `pool.as_pool_accounts_v14()`; it does not update `protocol_fee_recipient/_ta` from the successful SELL reference.

Expected fix pattern:

- In the force-refresh SELL-layout observation path, carry the full authoritative SELL observation, or at least `(layout, protocol_fee_recipient, protocol_fee_recipient_ta, reference_signature)`.
- When a same-pool, same-base successful SELL reference is found, update `pool.protocol_fee_recipient` and `pool.protocol_fee_recipient_ta` from SELL ix #9/#10 before `as_pool_accounts_v14()`.
- Preserve global SELL `fee_config=5PH...` and `fee_program=pfee...` behavior from Scope 59. Do not regress to using v14 `[12]` as SELL fee_config.

## Relevant Known Bug Patterns

### Known Bug #20: DEX Swap Instruction Account Order

Simulation failed / Custom error often means wrong DEX account order. Fix against real Mainnet reference TXs, not assumptions.

### Known Bug #34: Cold-Path Recovery cache-first answered stale PumpSwap pool_accounts

Cold-path recovery after structural sim fail must be true force-refresh via market-data. It must not reuse the same bad partial pool_accounts.

### Known Bug #35: PumpSwap protocol_fee_recipient global kanonisiert statt reale Werte zu bewahren

Do not globally canonicalize `protocol_fee_recipient` / `_ta`. Preserve real observed PumpSwap values from Geyser/RPC/reference tx. Only true global fields like SELL `fee_config` / `fee_program` may be canonicalized.

### Known Bug #36: Cache-Hit ist nicht automatisch ready

Partial state from pool-account parse is not necessarily full SELL-ready truth. For PumpSwap, readiness must include DEX-specific required SELL metadata and authoritative account fields.

### Memory note

There is an older memory saying validator tx-history was unavailable and global_config offset was used as a fallback. That was a pragmatic fallback, but this incident proves that when a same-pool successful SELL reference is available, it is more authoritative for SELL metas #9/#10 than the global_config-offset fallback.

## Erlaubte Dateien

Prefer:

- `src/solana/dex/pumpfun_amm.rs`
- `src/bin/market_data.rs`
- `src/execution/live_pool_cache.rs`
- focused unit tests in the same modules

Allowed if needed:

- `src/execution/tx_builder.rs` for diagnostics or tests only; do not change the core SELL fee_config behavior unless strictly necessary.

Avoid:

- broad changes in `src/bin/execution_engine.rs`
- any changes to momentum strategy logic
- public IPC wire-shape changes unless unavoidable and covered by serde tests

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Keine execution-engine-local cache healing for PumpSwap pool_accounts.
- Kein Revert von Extended 24-account SELL support.
- Kein Revert von Scope 59 global SELL fee_config / fee_program semantics.
- Kein global hardcoded `protocol_fee_recipient` / `_ta`.
- Kein Deploy.

## Konkrete Anforderungen

### 1. Propagate authoritative SELL reference fee recipients

For `force_refresh=true`, if the bounded tx-history / parsed successful swap observation finds a same-pool/same-base PumpSwap SELL ix, use that observation to update the `PumpAmmPoolStatic` before publishing v14.

Required mapping:

- parsed SELL ix account #9 -> `PumpAmmPoolStatic.protocol_fee_recipient` -> v14 `[6]`
- parsed SELL ix account #10 -> `PumpAmmPoolStatic.protocol_fee_recipient_ta` -> v14 `[7]`
- parsed SELL ix account #23 for 24-account SELL -> `sell_cashback_third_meta`

The existing market/global_config parse may still be a fallback when no reference SELL observation is available. But it must not override a successful same-pool SELL reference.

### 2. Keep the current tx_builder contract

`build_swap_ix_from_pool_accounts()` should continue to:

- preserve v14 `[6]/[7]` as final SELL metas #9/#10;
- use global SELL `fee_config=5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx`;
- use SELL `fee_program=pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ`;
- include 24 accounts when `sell_requires_cashback_remaining=true`;
- keep `third_meta` as the last account for extended SELL.

### 3. Improve diagnostics

Add or preserve logs that make the final source obvious:

- `protocol_fee_recipient_source=sell_reference_ix` or equivalent when applied;
- `reference_swap_signature`;
- final v14 `[6]/[7]`;
- final sell ix #9/#10;
- final sell ix account count and `third_meta`.

This is important for supervisor review after deploy.

### 4. Tests

Add focused tests that fail on current behavior:

1. A unit test around the force-refresh wrapping/observation flow:
   - start from a `PumpAmmPoolStatic` whose market/global_config-derived `protocol_fee_recipient/_ta` are wrong (`62qc2...` / `94qW...` or dummy values);
   - provide a parsed same-pool successful SELL reference with `#9/#10 = AVmo... / FGpt...` and 24 accounts with `third_meta=Aktft...`;
   - assert resulting v14 `[6]/[7]` equals `AVmo... / FGpt...`;
   - assert extended flag and `third_meta` are preserved.

2. A builder-level regression:
   - given v14 `[6]/[7] = AVmo... / FGpt...`, `build_swap_ix_from_pool_accounts(..., sell_requires_cashback_remaining=true, third=Aktft...)` must produce SELL metas #9/#10 equal to those values, #19/#20 equal to global `5PH...`/`pfee...`, 24 accounts total, #23 equal `Aktft...`.

3. A negative/fallback test if practical:
   - if no same-pool SELL observation exists, existing fallback behavior remains clear and does not invent default fee recipient values.

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR must also pass Impl CI including Eval Level 5.

## Supervisor-Review-Fokus

- Does the force-refresh reference SELL observation update v14 `[6]/[7]`?
- Are final SELL #9/#10 sourced from observed same-pool SELL reference when available?
- Are SELL #19/#20 still global FeeConfig/FeeProgram?
- Does the fix stay in market-data/pumpfun_amm authority, not execution-engine local healing?
- No Hot-Path RPC and no simulation bypass.
- Tests cover the exact production pool/reference account mismatch.
