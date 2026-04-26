# Handoff: Eval — PumpSwap `build_swap_ix_from_pool_accounts` (Scope 46 / Cashback-SELL)

## Koordination mit Impl

- **Impl-PR:** [Iron_crab #78](https://github.com/Exploratorsclub/Iron_crab/pull/78) (Branch `cursor/scope46-pumpswap-dynamic-sell-layout-retry1` → `architecture-rebuild`).
- **`ironcrab-eval` hängt ab von:** `ironcrab = { git = "…", branch = "architecture-rebuild" }`.
- **Reihenfolge:** Eval-Änderungen bauen gegen die **neue** öffentliche API. Praktisch: PR #78 zuerst nach `architecture-rebuild` mergen, **dann** diesen Eval-PR öffnen bzw. CI erneut laufen lassen. (Ohne gemergtes Impl bleibt `cargo build`/`cargo test` für Eval mit 9-arg-Aufrufen rot.)

## Ziel

Nach dem Cashback-/Extended-SELL-Update in `pumpfun_amm.rs` hat `PumpFunAmmDex::build_swap_ix_from_pool_accounts` **zwei zusätzliche Parameter** (nach `base_token_program`). Die bestehenden Eval-Verträge in `invariants_dex_connector.rs` rufen noch mit **7** Argumenten auf → **E0061**. Tests an die neue Signatur anpassen und das **erweiterte 24-Account-SELL-Layout** zusätzlich absichern (nicht nur `false, None`).

## Neue Signatur (Referenz: gemergter Impl-Stand)

```rust
pub fn build_swap_ix_from_pool_accounts(
    input_mint: &str,
    output_mint: &str,
    amount_in: u64,
    min_out: u64,
    user: Pubkey,
    pool_accounts: &[Pubkey],
    base_token_program: Option<Pubkey>,
    sell_requires_cashback_remaining: bool,
    sell_cashback_third_meta: Option<Pubkey>,
) -> Result<Vec<Instruction>>
```

**Semantik (SELL):**

- `sell_requires_cashback_remaining == false`: klassisches SELL mit **21** Metas (wie bisher).
- `sell_requires_cashback_remaining == true`: Builder hängt **3** trailing Metas an (Indizes 21–23): zuerst zwei aus User/Quote-TP-Logik (`pump_amm_sell_cashback_first_two_metas`), drittes **nur** aus `sell_cashback_third_meta` (nicht default); fehlt es → klarer `Err` mit Hinweis auf MASTER/Geyser.

## Konkrete Aufgaben

1. **`tests/invariants_dex_connector.rs`** — alle **vier** Aufrufe von `build_swap_ix_from_pool_accounts` (ca. Zeilen 232, 268, 307, 342) um die letzten beiden Argumente ergänzen:
   - Für die **bestehenden** Fee-Meta-Verträge (klassisches SELL, 14er `pool_accounts` v1): **`false, None`** (Verhalten wie vor Scope 46; Meta-Indizes 9/10 unverändert prüfbar).

2. **Neuer Test** (Pflicht): Mindestens ein Test, der **`sell_requires_cashback_remaining: true`** und **`sell_cashback_third_meta: Some(<nicht-default Pubkey>)`** nutzt (SELL-Richtung wie die bestehenden Tests: Base → WSOL), mit gültigem 14er `pool_accounts`-Fixture:
   - Erwartung: `Ok`, eine Swap-IX, **`accounts.len() == 24`**.
   - Sinnvolle Zusatzasserts (ohne RPC): z. B. letztes Meta == übergebenes `third`; die beiden davor sind die vom Builder abgeleiteten Cashback-Metas (konsistent mit `user` / WSOL / Quote-TP).

3. Optional: **`docs/spec/INVARIANTS.md`** Abschnitt **A.3 DEX Connector** um einen kurzen Bullet ergänzen: Extended-SELL / Cashback-Pfad mit 24 Metas und Abhängigkeit von `sell_requires_cashback_remaining` + `sell_cashback_third_meta` (nur wenn du die Spec mitpflegen sollst).

## Prüf-Befehle

```bash
cargo fmt --check
cargo clippy -p ironcrab-eval --all-targets -- -D warnings
cargo test
```

## Erlaubte Änderungen

- `tests/invariants_dex_connector.rs` (Pflicht)
- optional `docs/spec/INVARIANTS.md`

Keine Änderungen an `Iron_crab` (Impl) in diesem Task.
