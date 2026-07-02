//! Invariante A.48 (M1 / E-ARB-1): Arb Quote Contract — `pool_quote` public API.
//!
//! 1. **QuoteKind-Pairing:** Cross-DEX 2-hop Round-Trip vergleicht nur Pools mit gleichem `QuoteKind`.
//! 2. **Round-Trip-Screening:** (E-ARB-2 / M2) SOL→Token→SOL, nicht Mid-Spread.
//! 3. **Freshness:** `PoolQuote.fresh` folgt Quote-TTL.
//! 4. **Unified Quoter:** `pool_quote` exportiert aus `ironcrab::arbitrage`.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine `Iron_crab/src/`; API-Grenze only.

use ironcrab::arbitrage::pool_quote::{
    quote_exact_in, quotes_pairable, PoolQuote, QuoteKind, QuotePoolInput, QuoteSide,
    QuoteVaultInput, DLMM_PROBE_SOL_LAMPORTS, NATIVE_SOL_MINT,
};
use rust_decimal::Decimal;
use std::time::Instant;

fn sample_pool(dex: &str, address: &str) -> QuotePoolInput {
    QuotePoolInput {
        pool_address: address.to_string(),
        dex: dex.to_string(),
        token_mint: "TokenMint11111111111111111111111111111111".to_string(),
        trade_price_buy: None,
        trade_price_sell: None,
        trade_updated_at: Instant::now(),
        has_reserve_data: true,
        token_decimals: 6,
    }
}

fn sample_vault(token_reserve: u64, sol_reserve: u64) -> QuoteVaultInput {
    QuoteVaultInput {
        reserve_base: token_reserve,
        reserve_quote: sol_reserve,
        update_slot: 1,
        updated_at: Instant::now(),
        active_id: None,
        bin_step: None,
        dlmm_sol_is_x: false,
        dlmm_token_x_mint: None,
    }
}

#[test]
fn quote_kind_pairing_rejects_cross_kind() {
    let exec_quote = PoolQuote {
        pool_address: "buy_pool".into(),
        dex: "pump_amm".into(),
        kind: QuoteKind::ExecutableMarginal,
        side: QuoteSide::Buy,
        as_of_slot: 1,
        as_of_ts: Instant::now(),
        fresh: true,
        amount_in: 10_000_000,
        amount_out: 50_000,
    };
    let trade_quote = PoolQuote {
        kind: QuoteKind::LastTradeMid,
        ..exec_quote.clone()
    };

    assert!(
        quotes_pairable(&exec_quote, &exec_quote),
        "gleicher QuoteKind muss pairable sein"
    );
    assert!(
        !quotes_pairable(&exec_quote, &trade_quote),
        "ExecutableMarginal vs LastTradeMid darf nicht pairable sein (A.48 QuoteKind-Pairing)"
    );
}

#[test]
fn quote_monotonicity_pump_amm() {
    let pool = sample_pool("pump_amm", "monotonic_pool");
    let vault = sample_vault(1_000_000_000_000, 1_000_000_000);

    let amounts = [1_000_000u64, 10_000_000, 50_000_000];
    let mut outs = Vec::new();
    for amount_in in amounts {
        let quote = quote_exact_in(
            &pool,
            Some(&vault),
            None,
            NATIVE_SOL_MINT,
            &pool.token_mint,
            amount_in,
        )
        .expect("pump_amm CPMM quote");
        assert_eq!(quote.kind, QuoteKind::ExecutableMarginal);
        outs.push(quote.amount_out);
    }

    assert!(
        outs[1] >= outs[0],
        "groesseres amount_in muss mindestens gleiches amount_out liefern (A.1 analog)"
    );
    assert!(
        outs[2] >= outs[1],
        "groesseres amount_in muss mindestens gleiches amount_out liefern (A.1 analog)"
    );
}

#[test]
fn executable_marginal_preferred_over_stale_trade() {
    let mut pool = sample_pool("pump_amm", "prefer_marginal");
    pool.trade_price_buy = Some(Decimal::new(1, 3));
    pool.trade_price_sell = Some(Decimal::new(1, 3));
    pool.trade_updated_at = Instant::now();
    let vault = sample_vault(1_000_000_000_000, 1_000_000_000);

    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    )
    .expect("quote mit Reserves und frischem Trade");

    assert_eq!(
        quote.kind,
        QuoteKind::ExecutableMarginal,
        "bei vorhandenen Reserves muss ExecutableMarginal LastTradeMid vorgezogen werden"
    );
}

#[test]
fn dlmm_quote_requires_bins() {
    let mut pool = sample_pool("meteora_dlmm", "dlmm_no_bins");
    pool.trade_price_buy = Some(Decimal::new(1, 3));
    pool.trade_price_sell = Some(Decimal::new(1, 3));
    pool.trade_updated_at = Instant::now();
    let vault = QuoteVaultInput {
        reserve_base: 1_000_000_000_000,
        reserve_quote: 1_000_000_000,
        update_slot: 1,
        updated_at: Instant::now(),
        active_id: Some(0),
        bin_step: Some(100),
        dlmm_sol_is_x: false,
        dlmm_token_x_mint: Some(pool.token_mint.clone()),
    };

    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    );

    match quote {
        None => {}
        Some(q) => assert_eq!(
            q.kind,
            QuoteKind::LastTradeMid,
            "ohne DLMM-Bins darf kein ExecutableMarginal-Bin-Walker-Quote kommen"
        ),
    }
}

// E-ARB-2 (M2): `two_hop_v2_no_legacy_mid_spread_path_when_enabled`, `round_trip_profit_formula`
// — ausstehend bis Impl M2 (PR nach I-ARB-4..7) gemergt; siehe handoff_eval_arb_quote_m1_m2.md
