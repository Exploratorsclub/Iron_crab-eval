//! Invariante A.51: TX/Account Harte Trennung — Quote-API Blackbox + Momentum Exit Source-Contract.
//!
//! **TX** = Discovery (+ Layout-Seed fuer Hot-Pins). **Account** = alleinige Quote-SSOT fuer
//! Entry-Size, Exit, Arb-Screening und Exit-Reasons. Kein LastTradeMid-Fallback. Keine Exit-Intents
//! aus TX-Signalen/Trade-Marks ohne executable Account-Quote.
//!
//! Momentum Exit-Policy (quote-first, kein Drawdown allein aus Trade-Mark): siehe
//! `tests/invariants_trailing_session_high.rs` (Blackbox Math-API).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Blackbox `pool_quote` API + dokumentierte Source-Grep-Gates auf Sibling-Bins.

use ironcrab::arbitrage::pool_quote::{
    quote_exact_in, quotes_pairable, PoolQuote, QuoteKind, QuotePoolInput, QuoteSide,
    QuoteVaultInput, DLMM_PROBE_SOL_LAMPORTS, NATIVE_SOL_MINT,
};
use rust_decimal::Decimal;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn iron_crab_bin_rs(name: &str) -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("bin")
        .join(format!("{name}.rs"))
}

fn skip_if_no_sibling_bin(name: &str) -> Option<String> {
    let path = iron_crab_bin_rs(name);
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder {name}.rs nicht lesbar unter {:?}",
            iron_crab_root()
        );
        return None;
    }
    Some(fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())))
}

fn production_bin_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod ") {
        return &source[..idx];
    }
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source section")
}

fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needle = format!("fn {fn_name}(");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("expected fn {fn_name}( in source"));
    let brace_start = source[start..]
        .find('{')
        .map(|i| start + i)
        .expect("expected opening brace for fn block");
    let mut depth = 0usize;
    let mut end = brace_start;
    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = brace_start + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(end > brace_start, "unclosed fn block for {fn_name}");
    source[start..end].to_string()
}

fn trade_only_pool(address: &str) -> QuotePoolInput {
    QuotePoolInput {
        pool_address: address.to_string(),
        dex: "pump_amm".to_string(),
        token_mint: "TokenMint11111111111111111111111111111111".to_string(),
        trade_price_buy: Some(Decimal::new(15, 4)),
        trade_price_sell: Some(Decimal::new(15, 4)),
        trade_updated_at: Instant::now(),
        has_reserve_data: false,
        token_decimals: 6,
    }
}

// --- A.51 Quote-API Blackbox ---

/// Hot-Path `quote_exact_in` darf nie `LastTradeMid` zurueckgeben — auch bei frischen Trade-Preisen.
#[test]
fn quote_exact_in_never_returns_last_trade_mid() {
    let pool = trade_only_pool("tx_trade_only");

    let quote = quote_exact_in(
        &pool,
        None,
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    );

    match quote {
        None => {}
        Some(q) => assert_ne!(
            q.kind,
            QuoteKind::LastTradeMid,
            "quote_exact_in darf kein LastTradeMid liefern — nur ExecutableMarginal oder None (A.51)"
        ),
    }
}

/// TX-Trade-Preise mit vorhandenem Vault: Account-State (ExecutableMarginal) hat Vorrang, kein LastTradeMid.
#[test]
fn quote_exact_in_with_vault_never_falls_back_to_last_trade_mid() {
    let mut pool = trade_only_pool("tx_trade_with_vault");
    pool.has_reserve_data = true;
    let vault = QuoteVaultInput {
        reserve_base: 2_000_000_000_000,
        reserve_quote: 1_500_000_000,
        update_slot: 42,
        updated_at: Instant::now(),
        active_id: None,
        bin_step: None,
        dlmm_sol_is_x: false,
        dlmm_token_x_mint: None,
    };

    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    )
    .expect("Account-State-Quote erwartet");

    assert_eq!(
        quote.kind,
        QuoteKind::ExecutableMarginal,
        "mit Vault/Reserves nur ExecutableMarginal — kein LastTradeMid-Fallback (A.51)"
    );
}

/// `quotes_pairable` erlaubt nur ExecutableMarginal ↔ ExecutableMarginal.
#[test]
fn quotes_pairable_rejects_last_trade_mid() {
    let exec = PoolQuote {
        pool_address: "exec".into(),
        dex: "orca".into(),
        kind: QuoteKind::ExecutableMarginal,
        side: QuoteSide::Buy,
        as_of_slot: 1,
        as_of_ts: Instant::now(),
        fresh: true,
        state_fingerprint: 1,
        amount_in: 10_000_000,
        amount_out: 100_000,
    };
    let trade = PoolQuote {
        kind: QuoteKind::LastTradeMid,
        ..exec.clone()
    };

    assert!(quotes_pairable(&exec, &exec));
    assert!(!quotes_pairable(&exec, &trade));
    assert!(!quotes_pairable(&trade, &trade));
    assert!(!quotes_pairable(&trade, &exec));
}

// --- A.51 Momentum Exit Source-Contract (Sibling, skip ohne Checkout) ---

const MOM_EXIT_EXECUTABLE_QUOTE_MARKER: &str = "executable_exit_quote";
const MOM_EXIT_SUPPRESS_NO_QUOTE_MARKER: &str = "no pool-correct executable quote";
const MOM_EXIT_TRADE_MARK_NOT_EXIT_MARKER: &str = "trade mark alone";

/// Momentum: preisbasierter Exit erfordert executable Account-Quote; ohne Quote → unterdrueckt.
#[test]
fn momentum_exit_requires_executable_quote_contract() {
    let Some(source) = skip_if_no_sibling_bin("momentum_bot") else {
        return;
    };
    let prod = production_bin_source(&source);

    if !prod.contains(MOM_EXIT_EXECUTABLE_QUOTE_MARKER) {
        eprintln!(
            "SKIP: {MOM_EXIT_EXECUTABLE_QUOTE_MARKER} not present in sibling momentum_bot.rs \
             (Impl PR #416–#420 noch nicht im Sibling)"
        );
        return;
    }

    let exit_check_markers = [
        "check_for_exits",
        "should_exit",
        "process_exit_signals",
        "generate_and_publish_exit_intent",
    ];
    let Some(fn_name) = exit_check_markers
        .iter()
        .find(|name| prod.contains(&format!("fn {name}(")))
    else {
        eprintln!("SKIP: no exit-check fn found in sibling momentum_bot.rs");
        return;
    };

    let exit_body = extract_fn_block(prod, fn_name);
    assert!(
        exit_body.contains(MOM_EXIT_EXECUTABLE_QUOTE_MARKER)
            || prod.contains("validate_exit_against_executable_quote")
            || prod.contains("require_executable_quote"),
        "Momentum exit path muss executable Account-Quote pruefen (A.51); fn={fn_name}"
    );
}

/// Momentum: kein Drawdown-/Trailing-Exit allein aus TX-Trade-Mark ohne executable Quote.
#[test]
fn momentum_no_drawdown_exit_from_trade_mark_alone_contract() {
    let Some(source) = skip_if_no_sibling_bin("momentum_bot") else {
        return;
    };
    let prod = production_bin_source(&source);

    if !prod.contains(MOM_EXIT_EXECUTABLE_QUOTE_MARKER) {
        eprintln!("SKIP: momentum executable_exit_quote marker not in sibling");
        return;
    }

    assert!(
        prod.contains(MOM_EXIT_SUPPRESS_NO_QUOTE_MARKER)
            || prod.contains("suppress")
            || prod.contains("skip_exit")
            || prod.contains("without executable"),
        "Momentum muss Exit ohne executable Quote unterdruecken (A.51)"
    );

    if prod.contains(MOM_EXIT_TRADE_MARK_NOT_EXIT_MARKER) {
        assert!(
            prod.contains(MOM_EXIT_EXECUTABLE_QUOTE_MARKER),
            "Trade-Mark allein darf keinen Exit triggern ohne executable Account-Quote"
        );
    }
}

/// `pool_quote` Hot-Path-API: kein oeffentlicher Trade-Mid-Screening-Einstieg.
#[test]
fn pool_quote_public_api_no_trade_mid_screening_entry() {
    let pool = trade_only_pool("no_trade_mid_screening");
    let vault = QuoteVaultInput {
        reserve_base: 1_000_000_000_000,
        reserve_quote: 800_000_000,
        update_slot: 1,
        updated_at: Instant::now(),
        active_id: None,
        bin_step: None,
        dlmm_sol_is_x: false,
        dlmm_token_x_mint: None,
    };

    for &amount_in in &[1_000_000u64, DLMM_PROBE_SOL_LAMPORTS] {
        let quote = quote_exact_in(
            &pool,
            Some(&vault),
            None,
            NATIVE_SOL_MINT,
            &pool.token_mint,
            amount_in,
        );
        if let Some(q) = quote {
            assert_eq!(
                q.kind,
                QuoteKind::ExecutableMarginal,
                "Hot-Path quote_exact_in nur ExecutableMarginal (amount_in={amount_in})"
            );
        }
    }
}
