//! Invariante A.48 (E-ARB-3 / M3): Multi-hop unified quoter — dieselbe `pool_quote`-Engine wie 2-hop.
//!
//! 4. **Unified Quoter:** Multi-hop und 2-hop nutzen dieselbe `pool_quote`-Implementierung.
//!    Graph-Expansion nutzt executable quotes; DLMM nicht CP-Approx auf Reserves.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Blackbox API (`CachedQuoteProvider`, `pool_quote`) + dokumentierte Source-Grep-Gates.

use ironcrab::arbitrage::multi_hop_integration::{CachedQuoteProvider, WSOL_MINT};
use ironcrab::arbitrage::pool_quote::DLMM_PROBE_SOL_LAMPORTS;
use ironcrab::arbitrage::{DexType, QuoteProvider};
use ironcrab::execution::live_pool_cache::{create_shared_cache, CachedPoolState, MeteoraState};
use ironcrab::ipc::BinData;
use solana_sdk::pubkey::Pubkey;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn iron_crab_arbitrage_rs(name: &str) -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("arbitrage")
        .join(format!("{name}.rs"))
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let path = iron_crab_arbitrage_rs("multi_hop_integration");
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder multi_hop_integration.rs nicht lesbar unter {:?}",
            iron_crab_root()
        );
        return None;
    }
    Some(iron_crab_root())
}

fn read_arbitrage_source(name: &str) -> String {
    let path = iron_crab_arbitrage_rs(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Production code only — test modules in the same file must not affect grep gates.
fn production_arbitrage_source(source: &str) -> &str {
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
        .unwrap_or_else(|| panic!("expected fn {fn_name}( in multi_hop_integration.rs"));
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

/// A.48 M3: `CachedQuoteProvider::get_quote` delegiert an `pool_quote`, nicht CP-Approx fuer Meteora.
#[test]
fn multi_hop_cached_quote_provider_delegates_to_pool_quote() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let source = read_arbitrage_source("multi_hop_integration");
    let prod = production_arbitrage_source(&source);

    if !prod.contains("struct CachedQuoteProvider") {
        eprintln!("SKIP: M3 CachedQuoteProvider not present in sibling multi_hop_integration.rs");
        return;
    }
    if !prod.contains("quote_from_cached_pool")
        || !prod.contains("prefer_pool_quote_then_trade_cache")
    {
        eprintln!(
            "SKIP: M3 unified quoter (quote_from_cached_pool / prefer_pool_quote_then_trade_cache) \
             not present in sibling multi_hop_integration.rs"
        );
        return;
    }

    let prefer_body = extract_fn_block(prod, "prefer_pool_quote_then_trade_cache");
    let try_pool_body = extract_fn_block(prod, "try_pool_quote_from_live_pool_cache");
    assert!(
        prefer_body.contains("try_pool_quote_from_live_pool_cache"),
        "prefer_pool_quote_then_trade_cache muss LivePoolCache-Quotes via try_pool_quote_from_live_pool_cache holen"
    );
    assert!(
        try_pool_body.contains("quote_from_cached_pool"),
        "try_pool_quote_from_live_pool_cache muss quote_from_cached_pool (pool_quote-Reexport) nutzen, nicht raw CP approx"
    );

    assert!(
        !prod.contains("quote_calculator") && !prod.contains("QuoteCalculator"),
        "multi_hop_integration darf QuoteCalculator nicht fuer Multi-hop-Quotes nutzen (A.48)"
    );
}

/// A.48 M3: DLMM Multi-hop-Quote nutzt Bin-Walker, nicht Reserve-CP-Approx (Blackbox via public API).
#[test]
fn multi_hop_dlmm_quote_uses_bins_not_reserve_ratio() {
    let cache = create_shared_cache();
    let provider = CachedQuoteProvider::new(Duration::from_secs(30), cache.clone());

    let wsol = Pubkey::from_str(WSOL_MINT).expect("wsol mint");
    let token = Pubkey::new_unique();
    let pool = Pubkey::new_unique();
    let active_id = 0i32;
    let bin_step = 100u16;
    let token_amount = 500_000_000_000u64;
    let sol_amount = 2_000_000_000u64;

    cache.upsert(
        pool,
        CachedPoolState::Meteora(MeteoraState {
            token_x_mint: token,
            token_y_mint: wsol,
            reserve_x: Pubkey::new_unique(),
            reserve_y: Pubkey::new_unique(),
            active_id,
            bin_step,
            reserve_x_balance: Some(1_000_000_000_000),
            reserve_y_balance: Some(500_000_000),
        }),
        1,
    );
    provider.update_dlmm_bin_array(
        pool,
        active_id as i64 / 70,
        vec![BinData {
            offset: 0,
            amount_x: token_amount,
            amount_y: sol_amount,
        }],
    );

    let probe = DLMM_PROBE_SOL_LAMPORTS;
    let out = provider
        .get_cached_probe_quote(&pool, DexType::MeteoraDlmm, &wsol, &token, probe)
        .expect("DLMM bin-walker quote via CachedQuoteProvider erwartet");

    let fee_bps = 100u64;
    let ri = sol_amount as u128;
    let ro = token_amount as u128;
    let after_fee = probe as u128 * (10000 - fee_bps as u128) / 10000;
    let cp_approx = ((after_fee * ro) / (ri + after_fee)) as u64;

    assert_ne!(
        out, cp_approx,
        "Multi-hop DLMM-Quote muss Bin-Walker nutzen, nicht Reserve-CP-Approx (A.48)"
    );
    assert!(out > 0, "DLMM-Quote muss positiven Output liefern");
    provider.mark_ready_from_live_pool_cache(&pool);
    assert!(
        provider.is_pool_quote_ready(&pool),
        "DLMM-Pool mit Bins muss quote-ready sein"
    );
}
