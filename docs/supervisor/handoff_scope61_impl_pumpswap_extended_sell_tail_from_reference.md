WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und .cursor/rules/ironcrab-core.mdc BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss statt die Aenderung durchzufuehren.

# Scope 61: PumpSwap Extended SELL Must Use Full Reference Tail Metas

## Task-Beschreibung

Scope 60 fixed propagation of PumpSwap SELL protocol fee recipient metas `#9/#10`, but production liquidation still fails in simulation with `Custom(6023)`.

New evidence shows the remaining mismatch is the PumpSwap extended SELL tail:

- `market-data` now correctly observes a successful same-pool SELL reference and propagates `#9/#10`.
- The retry tx now has correct `#9/#10` for the latest reference.
- The retry still derives extended tail metas `#21/#22` locally and sets wrong flags/order/owners.
- A successful same-pool SELL reference shows the tail is not the current builder-derived `user_volume_wsol_ata, user_volume_accumulator, third_meta` layout.

Fix goal: propagate and use the full observed PumpSwap extended SELL tail metas `#21/#22/#23` from the same-pool successful SELL reference, not just the last `third_meta`.

## Runtime-Evidenz

Server is on merged PR #104:

```text
4ae1bcc fix(pumpswap): propagate SELL reference protocol fee recipients on force_refresh (#104)
```

Failed liquidation:

```text
2026-04-30T16:11:36Z KillSwitch liquidate_positions=true
wallet=Ase7z1mRLps2cTNQnRHpLyQL4Q5FHwonjmZnYCTuUDZM
mint=y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
balance_raw=16650263074
pool=HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7
intent_id=liquidation-59cf8e1b-354c-4aba-be1a-84cc7855d415
```

After force-refresh, market-data observed same-pool reference:

```text
signature=3FZXNFR8raYQ524i2aH2eiXLkAPdrEVPxqhxcJJy6kvrinFCrGkUUn5ZVGggQn6BMvfetDAibJrp3ugcNkR4LuL9
layout=Extended { third_meta: HjQjngTDqoHE6aaGhUqfz9aQ7WZcBRjy5xB8PScLSr8i }
protocol_fee_recipient_source="sell_reference_ix"
sell_reference_protocol_fee_recipient=7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX
sell_reference_protocol_fee_recipient_ta=X5QPJcpph4mBAJDzc4hRziFftSbcygV59kRb2Fu6Je1
```

The retry tx used those corrected `#9/#10`, but still failed:

```text
sell_extended=true
sell_ix_account_count=24
sell_ix_accounts_csv=...,7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX,X5QPJcpph4mBAJDzc4hRziFftSbcygV59kRb2Fu6Je1,...,AJiyruMFVWBvjRLmqqm1aCgbGWHrfQWA8zP3GjGtPooR,JBLBpHmSwamPTbv7LD9zTih7BpEXhLcsxwp9qbqvmzY7,HjQjngTDqoHE6aaGhUqfz9aQ7WZcBRjy5xB8PScLSr8i
error=UiTransactionError(InstructionError(1, Custom(6023)))
```

Full simulation logs show failure still occurs immediately after pfee `GetFees`:

```text
Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA invoke [1]
Program log: Instruction: Sell
Program pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ invoke [2]
Program log: Instruction: GetFees
Program pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ consumed 46
error=UiTransactionError(InstructionError(1, Custom(6023)))
```

Successful same-pool reference `3FZX...` top-level PumpSwap SELL accounts:

```text
#0  HS9UsHpMZLYzzbLwWXJfHzsRd8HmuzMLcutHwVKGt1P7
#1  Hz2QZ6UTB5az77KBoewZxv2eGP8uTVCRwRUSPRM9kQvX
#2  ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw
#3  y7bgE68ZWVodvVmMUWQhShAnjVTmJVGpdnC1wYspump
#4  So11111111111111111111111111111111111111112
#5  795tmYP6gvdj3CD7VK7ubyPcbmyFhrMTXtfQQXFYYMW3
#6  BoyhqmVvHcvGS98AkDrLKdZ14pMAHxhrVXWswiZPcw5N
#7  3T42nPotoc1nGNekbTYKMTaUZZu3ndA5a4we1HfWmxt7
#8  7pdfwGevvq2ybRUp945NA9ZcvkZD41fAu7sAN9SJfPYC
#9  7hTckgnGnLQR6sdH7YkqFTAA7VwTfYFaZ6EhEsU3saCX
#10 X5QPJcpph4mBAJDzc4hRziFftSbcygV59kRb2Fu6Je1
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
#22 5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD
#23 HjQjngTDqoHE6aaGhUqfz9aQ7WZcBRjy5xB8PScLSr8i
```

On-chain account inspection:

```text
#21 CXfr... missing account, readonly in transaction
#22 5Yx...  owner_program=pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ, readonly in transaction
#23 Hj...   owner_program=Tokenkeg..., WSOL ATA owner=5Yx..., writable in transaction
```

Current builder-generated tail for our wallet:

```text
#21 AJiyruMFVWBvjRLmqqm1aCgbGWHrfQWA8zP3GjGtPooR  (WSOL ATA, writable)
#22 JBLBpHmSwamPTbv7LD9zTih7BpEXhLcsxwp9qbqvmzY7  (pAMM-owned PDA, writable)
#23 HjQjngTDqoHE6aaGhUqfz9aQ7WZcBRjy5xB8PScLSr8i  (readonly)
```

This proves the current assumption in `pump_amm_sell_cashback_first_two_metas()` is wrong for this pool/program version.

## Relevante Invarianten (Volltext)

### I-4 / I-7 Hot Path RPC-Freiheit

HOT PATH (Discovery, Buy, Sell, Monitoring): GEYSER-ONLY. Keine blockierenden RPC-Calls. Latenz-Ziel unter 1s Discovery bis TX on-chain. Nie RPC in Hot Paths ohne explizite Freigabe. Dieser Scope darf keine neuen blockierenden RPC-Calls im normalen Momentum-/Arb-Hot-Path einfuehren.

### I-5 / I-6 Cold Path Correctness

COLD PATH (Liquidation, Manual Actions, Bootstrap): RPC erlaubt. Safety und correctness vor Speed. Nie RPC aus Cold Paths entfernen um zu "optimieren", wenn dadurch safety-kritische Flows brechen. Der betroffene Pfad ist `EnsurePumpAmmPoolAccounts(force_refresh=true)` nach Liquidation-Simulation-Fail.

### I-9 Simulation-Gate

Wenn Simulation fehlschlaegt, nie senden. Kein Bypass fuer `Custom(6023)`. Der Fix muss die Account-Metas korrigieren, nicht das Simulationsergebnis umgehen.

### I-24d Cold-Path Discovery nur per Request/Reply

execution-engine darf fehlende oder fehlerhafte PumpSwap `pool_accounts` weder selbst discovern noch lokal in den SLAVE Cache schreiben. Discovery, MASTER-Write und JetStream-Publikation bleiben bei market-data. Dieser Fix muss in `market-data` / `pumpfun_amm` Autoritaet liegen, nicht in lokaler execution-engine-Heilung.

### PumpSwap Extended SELL Contract

Fuer extended PumpSwap SELL muss der Builder die vom autoritativen Market-Data-/Reference-TX-Pfad gelieferten Account-Metas in der dokumentierten Reihenfolge und mit den korrekten AccountMeta-Flags verwenden. Nicht nur `third_meta` ist relevant: die komplette Tail-Form muss stimmen.

## Bestehendes Pattern / Code-Kontext

Current wrong code path:

```rust
let (user_vol_wsol_ata, user_vol) =
    Self::pump_amm_sell_cashback_first_two_metas(user, quote_mint, quote_tp);
metas.push(AccountMeta::new(user_vol_wsol_ata, false)); // #21 writable
metas.push(AccountMeta::new(user_vol, false));          // #22 writable
metas.push(AccountMeta::new_readonly(third, false));    // #23 readonly
```

This produced:

```text
#21 AJiy... writable WSOL ATA
#22 JBL... writable pAMM PDA
#23 Hj... readonly
```

But successful reference requires:

```text
#21 CXfr... readonly
#22 5Yx... readonly pfee-owned account
#23 Hj... writable WSOL ATA owned by #22
```

Expected fix pattern:

- Extend the Scope 60 observation struct to carry the full extended SELL tail accounts, not just `third_meta`.
- Preserve tail accounts from same-pool successful SELL reference:
  - tail0 / ix #21 = `CXfr...`
  - tail1 / ix #22 = `5Yx...`
  - tail2 / ix #23 = `Hj...`
- Update LivePoolCache / v14-adjacent metadata so tx_builder can access the full tail.
- Update `build_swap_ix_from_pool_accounts()` and cache-based `build_swap_ix()` so when authoritative extended tail is available, it appends:
  - #21 readonly observed tail0
  - #22 readonly observed tail1
  - #23 writable observed tail2
- Do not derive #21/#22 locally for this observed-tail path.
- If no authoritative full tail is available, fail clearly or keep existing fallback only where proven safe; do not label partial tail as ready.

## Relevant Known Bug Patterns

### Known Bug #20: DEX Swap Instruction Account Order

Simulation failed / Custom error often means wrong DEX account order. Fix against real Mainnet reference TXs, not assumptions.

### Known Bug #34: Cold-Path Recovery cache-first answered stale PumpSwap pool_accounts

Cold-path recovery after structural sim fail must be true force-refresh via market-data. It must not reuse the same bad partial pool_accounts.

### Known Bug #35: PumpSwap protocol_fee_recipient global kanonisiert statt reale Werte zu bewahren

Same principle: do not invent PumpSwap account metas. Preserve observed values from successful same-pool reference.

### Known Bug #36: Cache-Hit ist nicht automatisch ready

Partial extended state (`third_meta` only) is not really SELL-ready. Readiness must require the full required tail metadata.

## Erlaubte Dateien

Prefer:

- `src/solana/dex/pumpfun_amm.rs`
- `src/execution/live_pool_cache.rs`
- `src/bin/market_data.rs`
- focused tests in same modules

Allowed if necessary:

- `src/execution/tx_builder.rs`
- IPC metadata structs if full tail needs persisted propagation; if public shape changes, update serde tests.

Avoid:

- broad `execution_engine.rs` changes
- momentum strategy logic

## Verboten

- Kein Simulation-Bypass.
- Kein Hot-Path-RPC.
- Keine execution-engine-local cache healing.
- Kein Revert von Scope 60 `#9/#10` propagation.
- Kein Revert von Scope 59 global SELL `fee_config` / `fee_program`.
- Kein Hardcoding nur fuer `HS9Us...`; use generic observed reference tail propagation.
- Kein Deploy.

## Konkrete Anforderungen

### 1. Propagate full observed extended SELL tail

When force-refresh observes a same-pool successful 24-account SELL reference, preserve all three tail accounts:

```text
sell_tail_0 = account #21
sell_tail_1 = account #22
sell_tail_2 = account #23
```

Do not reduce this to only `third_meta`.

### 2. Use observed tail with correct AccountMeta flags

For the observed-tail path, final SELL ix must append:

```text
#21 AccountMeta::new_readonly(observed_tail_0, false)
#22 AccountMeta::new_readonly(observed_tail_1, false)
#23 AccountMeta::new(observed_tail_2, false)
```

This matches the successful reference transaction flags:

```text
#21 CXfr... readonly
#22 5Yx... readonly
#23 Hj... writable
```

### 3. Tighten readiness

`sell_requires_cashback_remaining=true` / extended readiness must not mean "only third_meta exists". For force-refresh / JetStream ready state, extended SELL should be considered ready only when the full tail is present.

### 4. Diagnostics

Log final tail source and values:

```text
sell_extended_tail_source=sell_reference_ix
sell_extended_tail_0=...
sell_extended_tail_1=...
sell_extended_tail_2=...
sell_ix_meta_21=...
sell_ix_meta_22=...
sell_ix_meta_23=...
```

### 5. Tests

Add tests that fail on current behavior:

1. Production-reference builder regression:
   - v14 `[6]/[7] = 7hTck... / X5QP...`
   - full observed tail `#21/#22/#23 = CXfr... / 5Yx... / Hj...`
   - final SELL has 24 metas:
     - #9/#10 = `7hTck... / X5QP...`
     - #19/#20 = `5PH... / pfee...`
     - #21/#22/#23 exactly `CXfr... / 5Yx... / Hj...`
     - #21/#22 readonly, #23 writable

2. Observation propagation test:
   - parsing reference SELL ix captures all three tail accounts, not just `third_meta`.

3. Readiness test:
   - extended=true with only old `third_meta` but missing tail0/tail1 is not treated as fully ready for the observed-tail path.

## Pruef-Befehle

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

PR must also pass Impl CI including Eval Level 5.

## Supervisor-Review-Fokus

- Full tail `#21/#22/#23` is propagated from same-pool SELL reference.
- Final SELL account flags match successful reference: #21 readonly, #22 readonly, #23 writable.
- #9/#10 Scope 60 behavior remains intact.
- #19/#20 Scope 59 global fee_config/fee_program remains intact.
- No hot-path RPC, no simulation bypass, no execution-engine local healing.
