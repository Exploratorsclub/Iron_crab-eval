//! Invariante A.48 (E-ARB-1 + E-ARB-2): Arb Quote Contract — `pool_quote` public API + v2 structural gate.
//!
//! 1. **QuoteKind-Pairing (ExecutableMarginal-only):** Cross-DEX 2-hop Round-Trip nur `ExecutableMarginal`.
//! 2. **Round-Trip-Screening:** 2-hop v2 Profit aus SOL→Token→SOL, nicht Mid-Spread Reserve vs Trade.
//! 3. **Freshness:** `PoolQuote.fresh` folgt State-TTL (`state_fingerprint` fuer ExecutableMarginal).
//! 4. **Unified Quoter:** `pool_quote` exportiert aus `ironcrab::arbitrage`.
//! 5. **Pairing nur Age vs Head (A.48 / PR #440):** Default `arb_max_leg_slot_delta = 0`;
//!    kein relativer `|buy−sell|`-Reject; Chain-Head-Age (`leg_slot_too_old`), `chain_slot == 0` fail-closed.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Blackbox API + dokumentierte Source-Grep-Gates (wie `invariants_arb_track_requests.rs`).

use ironcrab::arbitrage::pool_quote::{
    is_usable_quote_kind, quote_exact_in, quotes_pairable, round_trip_profit_lamports, PoolQuote,
    QuoteKind, QuotePoolInput, QuoteSide, QuoteVaultInput, RoundTripLeg, DLMM_PROBE_SOL_LAMPORTS,
    NATIVE_SOL_MINT,
};
use ironcrab::metrics::{
    record_arb_quote_pair_slot_delta, ARB_TWO_HOP_V2_REJECTED_SLOT_DELTA_EXCEEDED,
};
use rust_decimal::Decimal;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
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

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let path = iron_crab_bin_rs("arb_strategy");
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder arb_strategy.rs nicht lesbar unter {:?}",
            iron_crab_root()
        );
        return None;
    }
    Some(iron_crab_root())
}

fn read_bin_source(name: &str) -> String {
    let path = iron_crab_bin_rs(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Production code only — test modules in the same file must not affect grep gates.
fn production_bin_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod ") {
        return &source[..idx];
    }
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source section")
}

/// Extrahiert den Rust-Funktionsblock ab `fn {name}(…)` inkl. geschweifter Klammern.
fn extract_fn_block(source: &str, fn_name: &str) -> String {
    let needle = format!("fn {fn_name}(");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("expected fn {fn_name}( in arb_strategy.rs"));
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

fn extract_arb_config_default_body(prod: &str) -> String {
    let needle = "impl Default for ArbConfig";
    let start = prod
        .find(needle)
        .unwrap_or_else(|| panic!("expected {needle} in arb_strategy.rs"));
    extract_fn_block(&prod[start..], "default")
}

/// A.48: Nach `record_arb_quote_pair_slot_delta` darf kein Pflicht-`SlotDeltaExceeded`-Reject folgen.
fn assert_v2_no_relative_slot_delta_reject_after_record(v2_body: &str) {
    let record_pos = v2_body
        .find("record_arb_quote_pair_slot_delta")
        .unwrap_or_else(|| {
            panic!("check_arbitrage_v2 muss record_arb_quote_pair_slot_delta fuer Observability nutzen")
        });
    let after_record = &v2_body[record_pos..];
    assert!(
        !after_record.contains("ArbTwoHopV2RejectReason::SlotDeltaExceeded"),
        "relativer Slot-Delta-Reject (SlotDeltaExceeded) nach record_arb_quote_pair_slot_delta ist entfernt (A.48)"
    );
    assert!(
        !after_record.contains("arb_max_leg_slot_delta > 0 && slot_delta >"),
        "relativer |buy−sell|-Gate-Block muss entfernt sein (A.48 Pairing nur Age vs Head)"
    );
}

/// A.48: `leg_slot_too_old` vor Spread/Profit; `chain_slot == 0` fail-closed (kein `if chain_slot > 0`-Skip).
fn assert_v2_chain_head_age_gate_before_spread(v2_body: &str) {
    let leg_old_pos = v2_body
        .find("ArbTwoHopV2RejectReason::LegSlotTooOld")
        .unwrap_or_else(|| panic!("check_arbitrage_v2 muss LegSlotTooOld Age-Gate haben (A.48)"));
    assert!(
        v2_body.contains("saturating_sub(buy_as_of_slot)")
            || v2_body.contains("saturating_sub(sell_as_of_slot)"),
        "Age-Gate muss chain_head − leg.as_of_slot nutzen (A.48)"
    );
    assert!(
        !v2_body.contains("if chain_slot > 0"),
        "Age-Gate darf chain_slot==0 nicht ueberspringen (A.48 fail-closed)"
    );
    for marker in [
        "ArbTwoHopV2RejectReason::RoundTripSpreadBelowMin",
        "RoundTripSpreadBelowMin",
        "min_spread_bps",
    ] {
        if let Some(pos) = v2_body.find(marker) {
            assert!(
                leg_old_pos < pos,
                "LegSlotTooOld muss vor Spread-Gate ({marker}) kommen (A.48)"
            );
        }
    }
}

// --- E-ARB-1 (M1) ---

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
        state_fingerprint: 0,
        amount_in: 10_000_000,
        amount_out: 50_000,
    };
    let trade_quote = PoolQuote {
        kind: QuoteKind::LastTradeMid,
        ..exec_quote.clone()
    };

    assert!(
        quotes_pairable(&exec_quote, &exec_quote),
        "ExecutableMarginal muss mit sich selbst pairable sein"
    );
    assert!(
        !quotes_pairable(&exec_quote, &trade_quote),
        "ExecutableMarginal vs LastTradeMid darf nicht pairable sein (A.48 QuoteKind-Pairing)"
    );
}

/// A.48 / A.51: LastTradeMid ist kein brauchbares Screening-Quote (nur ExecutableMarginal).
#[test]
fn last_trade_mid_not_usable_quote_kind() {
    assert!(
        is_usable_quote_kind(QuoteKind::ExecutableMarginal),
        "ExecutableMarginal muss usable sein (A.48)"
    );
    assert!(
        !is_usable_quote_kind(QuoteKind::LastTradeMid),
        "LastTradeMid darf nicht usable sein — nur ExecutableMarginal (A.48/A.51)"
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
fn executable_marginal_from_account_state_only() {
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
    .expect("quote mit Account-State (Reserves)");

    assert_eq!(
        quote.kind,
        QuoteKind::ExecutableMarginal,
        "bei vorhandenen Reserves muss ExecutableMarginal aus Account-State kommen"
    );
}

/// A.51: Frische TX-Trade-Preise ohne Account-State duerfen kein Screening-Quote liefern.
#[test]
fn trade_only_pool_without_account_state_returns_none() {
    let mut pool = sample_pool("pump_amm", "trade_only_no_vault");
    pool.has_reserve_data = false;
    pool.trade_price_buy = Some(Decimal::new(2, 3));
    pool.trade_price_sell = Some(Decimal::new(2, 3));
    pool.trade_updated_at = Instant::now();

    let quote = quote_exact_in(
        &pool,
        None,
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    );

    assert!(
        quote.is_none(),
        "ohne Account-State (Vault/Bins) darf quote_exact_in kein LastTradeMid-Fallback liefern (A.51)"
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

    assert!(
        quote.is_none(),
        "ohne DLMM-Bins darf kein ExecutableMarginal-Quote und kein LastTradeMid-Fallback kommen (A.48/A.51)"
    );
}

// --- E-ARB-2 (M2) ---

/// A.48 Round-Trip-Screening: profit = sol_back - probe - fees (fixture pools, kein RPC).
/// Buy-Leg CPMM (raydium), nicht Orca: ab A.54 liefert Orca `quote_exact_in` ohne Tick-Arrays
/// `None` — dieser Formeltest bleibt bewusst Vault-x·y=k (supports_cpmm), unabhaengig von Orca CLMM.
#[test]
fn round_trip_profit_formula() {
    let pool_buy = sample_pool("raydium", "round_trip_buy");
    let pool_sell = sample_pool("pump_amm", "round_trip_sell");
    let vault_buy = sample_vault(1_000_000_000_000, 900_000_000);
    let vault_sell = sample_vault(1_000_000_000_000, 1_100_000_000);

    let buy_leg = RoundTripLeg {
        pool: &pool_buy,
        vault: Some(&vault_buy),
        dlmm_bins: None,
    };
    let sell_leg = RoundTripLeg {
        pool: &pool_sell,
        vault: Some(&vault_sell),
        dlmm_bins: None,
    };

    let probe = DLMM_PROBE_SOL_LAMPORTS;
    let tx_cost = 0u64;
    let profit =
        round_trip_profit_lamports(&buy_leg, &sell_leg, probe, tx_cost).expect("round-trip quote");

    assert!(
        profit > 0,
        "A.48: SOL→Token→SOL Round-Trip muss positiven Profit liefern (profit={profit}, probe={probe})"
    );

    let buy_quote = quote_exact_in(
        &pool_buy,
        Some(&vault_buy),
        None,
        NATIVE_SOL_MINT,
        &pool_buy.token_mint,
        probe,
    )
    .expect("buy leg");
    let sell_quote = quote_exact_in(
        &pool_sell,
        Some(&vault_sell),
        None,
        &pool_sell.token_mint,
        NATIVE_SOL_MINT,
        buy_quote.amount_out,
    )
    .expect("sell leg");
    let expected = sell_quote.amount_out as i64 - probe as i64 - tx_cost as i64;
    assert_eq!(
        profit, expected,
        "round_trip_profit_lamports muss sol_back - probe - fees sein"
    );
    assert_eq!(
        buy_quote.kind, sell_quote.kind,
        "A.48 QuoteKind-Pairing: buy/sell muessen gleichen QuoteKind haben"
    );
    assert_eq!(
        buy_quote.kind,
        QuoteKind::ExecutableMarginal,
        "Round-Trip-Screening nur ExecutableMarginal (A.48/A.51)"
    );
}

/// A.48 v2-Pfad: bei `arb_two_hop_v2_enabled` keine `comparable_price`-Opportunity-Entscheid.
#[test]
fn two_hop_v2_no_legacy_mid_spread_path_when_enabled() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);

    if !prod.contains("fn check_arbitrage_v2") {
        eprintln!("SKIP: M2 check_arbitrage_v2 not present in sibling arb_strategy.rs");
        return;
    }

    let v2_body = extract_fn_block(prod, "check_arbitrage_v2");
    assert!(
        !v2_body.contains("comparable_price_sol_per_token"),
        "check_arbitrage_v2 darf comparable_price_sol_per_token nicht fuer Opportunity-Entscheid nutzen (A.48)"
    );
    assert!(
        v2_body.contains("select_round_trip_pools")
            || v2_body.contains("round_trip")
            || v2_body.contains("sell_quote.amount_out"),
        "check_arbitrage_v2 muss Round-Trip-Quotes nutzen"
    );

    let check_body = extract_fn_block(prod, "check_arbitrage");
    assert!(
        check_body.contains("arb_two_hop_v2_enabled") && check_body.contains("check_arbitrage_v2"),
        "check_arbitrage muss bei arb_two_hop_v2_enabled an check_arbitrage_v2 delegieren"
    );
}

/// A.48/A.51: v2-Screening darf LastTradeMid weder paaren noch als Fallback nutzen.
#[test]
fn two_hop_v2_screening_executable_marginal_only_in_sibling() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);

    if !prod.contains("fn check_arbitrage_v2") {
        eprintln!("SKIP: check_arbitrage_v2 not present in sibling arb_strategy.rs");
        return;
    }

    let v2_body = extract_fn_block(prod, "check_arbitrage_v2");
    assert!(
        !v2_body.contains("QuoteKind::LastTradeMid") && !v2_body.contains("LastTradeMid"),
        "check_arbitrage_v2 darf LastTradeMid nicht fuer Screening nutzen (A.48/A.51)"
    );
}

// --- A.48 Pairing: Age vs Head (nicht relativer Slot-Delta) ---

/// Blackbox: Slot-Delta-Histogramm ist Observability — kein Pairing-Reject.
#[test]
fn record_arb_quote_pair_slot_delta_does_not_increment_slot_delta_exceeded_reject() {
    let before = ARB_TWO_HOP_V2_REJECTED_SLOT_DELTA_EXCEEDED.load(Ordering::Relaxed);
    record_arb_quote_pair_slot_delta(10, 100);
    let after = ARB_TWO_HOP_V2_REJECTED_SLOT_DELTA_EXCEEDED.load(Ordering::Relaxed);
    assert_eq!(
        before,
        after,
        "|buy−sell|=90 darf allein kein SlotDeltaExceeded ausloesen — Pairing nutzt nur Age vs Head (A.48)"
    );
}

/// A.48: Default-Pairing verwirft nicht auf |buy−sell| > 2 (`arb_max_leg_slot_delta: 0`).
#[test]
fn arb_config_default_disables_relative_slot_delta_pairing_gate() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);
    if !prod.contains("impl Default for ArbConfig") {
        eprintln!("SKIP: ArbConfig::default not present in sibling arb_strategy.rs");
        return;
    }

    let default_body = extract_arb_config_default_body(prod);
    assert!(
        default_body.contains("arb_max_leg_slot_delta: 0"),
        "ArbConfig::default muss arb_max_leg_slot_delta: 0 setzen (A.48 Pairing nur Age vs Head)"
    );
    assert!(
        !default_body.contains("arb_max_leg_slot_delta: 2"),
        "ArbConfig::default darf nicht arb_max_leg_slot_delta: 2 als Default haben (A.48)"
    );
}

/// A.48: Relativer Slot-Delta-Reject-Block in `check_arbitrage_v2` ist entfernt.
#[test]
fn check_arbitrage_v2_pairing_uses_chain_head_age_not_relative_slot_delta() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);
    if !prod.contains("fn check_arbitrage_v2") {
        eprintln!("SKIP: check_arbitrage_v2 not present in sibling arb_strategy.rs");
        return;
    }

    let v2_body = extract_fn_block(prod, "check_arbitrage_v2");
    assert_v2_no_relative_slot_delta_reject_after_record(&v2_body);
    assert_v2_chain_head_age_gate_before_spread(&v2_body);
}

/// A.48: Idle-Bein (as_of_slot 10) vs Head 100 mit age>16 → `leg_slot_too_old`, nicht `passed_gates`.
#[test]
fn check_arbitrage_v2_idle_leg_rejects_leg_slot_too_old_before_spread_gates() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);
    if !prod.contains("fn check_arbitrage_v2_rejects_stale_leg_vs_chain_head") {
        eprintln!(
            "SKIP: check_arbitrage_v2_rejects_stale_leg_vs_chain_head not in sibling arb_strategy.rs"
        );
        return;
    }

    let test_body = extract_fn_block(prod, "check_arbitrage_v2_rejects_stale_leg_vs_chain_head");
    assert!(
        test_body.contains("chain_head_slot: 100") && test_body.contains("980_000_000"),
        "Stale-Leg-Szenario: buy as_of_slot 10 vs chain head 100 (age 90 > default 16)"
    );
    assert!(
        test_body.contains("arb_max_leg_age_slots: 16"),
        "Age-Gate-Default 16 Slots im Stale-Leg-Contract-Test"
    );
    assert!(
        test_body.contains("ARB_TWO_HOP_V2_REJECTED_LEG_SLOT_TOO_OLD"),
        "Idle-Bein vs Head muss leg_slot_too_old inkrementieren (A.48)"
    );
    assert!(
        !test_body.contains("PassedGates") && !test_body.contains("passed_gates"),
        "Idle-Bein vs Head darf nicht passed_gates erreichen (A.48)"
    );

    let v2_body = extract_fn_block(prod, "check_arbitrage_v2");
    assert_v2_chain_head_age_gate_before_spread(&v2_body);
}

/// A.48: Zwei frische Beine mit |Δ|>2 aber chain−as_of≤16 — relativer Delta allein kein Reject.
#[test]
fn check_arbitrage_v2_large_slot_skew_allowed_when_both_legs_within_age_gate() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);
    if !prod.contains("fn check_arbitrage_v2") {
        eprintln!("SKIP: check_arbitrage_v2 not present in sibling arb_strategy.rs");
        return;
    }

    let default_body = extract_arb_config_default_body(prod);
    assert!(
        default_body.contains("arb_max_leg_slot_delta: 0"),
        "Default arb_max_leg_slot_delta=0: |Δ|>2 allein darf nicht paaren blockieren (A.48)"
    );

    let v2_body = extract_fn_block(prod, "check_arbitrage_v2");
    assert_v2_no_relative_slot_delta_reject_after_record(&v2_body);

    let record_pos = v2_body
        .find("record_arb_quote_pair_slot_delta")
        .expect("record_arb_quote_pair_slot_delta");
    let leg_old_pos = v2_body
        .find("ArbTwoHopV2RejectReason::LegSlotTooOld")
        .expect("LegSlotTooOld");
    assert!(
        record_pos < leg_old_pos,
        "Slot-Delta-Observability darf vor Age-Gate liegen; relativer Delta ist kein Pairing-Reject (A.48)"
    );
}
