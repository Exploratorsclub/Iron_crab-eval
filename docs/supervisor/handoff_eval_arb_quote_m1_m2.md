# Handoff Eval: Arb Quote Contract Tests (E-ARB-1 + E-ARB-2)

WICHTIG: Lies und befolge die STOP-CHECK Regeln in AGENTS.md und `.cursor/rules/eval-test-authority.mdc` BEVOR du eine Datei aenderst. Wenn eine geplante Aenderung gegen eine Regel verstoesst, STOPPE sofort und melde den Verstoss.

## Task

Zwei Eval-PRs oder **ein** PR mit zwei Testmodulen (Supervisor bevorzugt **ein PR nach M2 Impl-Merge**):

### E-ARB-1 (nach M1 Impl gemergt)

Datei: `tests/invariants_arb_quote_contract.rs`

Tests gegen oeffentliche `ironcrab::arbitrage::pool_quote` API:

1. `quote_kind_pairing_rejects_cross_kind` — ExecutableMarginal vs LastTradeMid nicht pairable
2. `quote_monotonicity_pump_amm` — groesseres amount_in → >= amount_out (A.1 analog)
3. `executable_marginal_preferred_over_stale_trade` — wenn beide da, kind ExecutableMarginal
4. `dlmm_quote_requires_bins` — ohne bins → None oder LastTradeMid only

Bump `Cargo.toml` git-dep auf Impl-Commit mit M1.

### E-ARB-2 (nach M2 Impl gemergt)

Erweiterung gleiche Datei oder `tests/invariants_arb_two_hop_v2.rs`:

1. `two_hop_v2_no_legacy_mid_spread_path_when_enabled` — Source grep: v2 enabled → kein `comparable_price` fuer Opportunity-Entscheid (structural test wie andere phase grep tests)
2. `round_trip_profit_formula` — Fixture Pools → positive profit mock
3. Dokumentiere A.48 in Test-Kommentaren

## Invariante die getestet wird (VOLLTEXT A.48)

1. **QuoteKind-Pairing:** Cross-DEX 2-hop Round-Trip vergleicht nur Pools mit gleichem `QuoteKind`.
2. **Round-Trip-Screening:** 2-hop v2 Profit aus SOL→Token→SOL, nicht Mid-Spread Reserve vs Trade.
3. **Freshness:** PoolQuote.fresh folgt Quote-TTL.
4. **Unified Quoter:** pool_quote Modul exportiert aus `ironcrab::arbitrage`.

## Zieldatei

- `tests/invariants_arb_quote_contract.rs` (neu)
- Optional `tests/invariants_arb_two_hop_v2.rs`
- `docs/spec/INVARIANTS.md` — A.48 Test-Datei-Verweis aktualisieren falls noetig

## Pruef-Befehle

```bash
cargo fmt -p ironcrab-eval -- --check
cargo check
cargo build
cargo clippy -p ironcrab-eval
cargo test invariants_arb_quote
```

Vor Merge koordinierter Aenderung: volle `cargo test` mit sibling Iron_crab Checkout.

## Verboten

- Keine Aenderungen an `Iron_crab/src/` (Eval-Repo only)
- Keine Tests die RPC erfordern
