//! Event-driven Arb Quote-Uhr (I-MD-4 / A.48 Material-Slot, kein Slot-Sustain / C1h Pin-Guards).
//!
//! Quote-SSOT = Account/Vault-Tick + JetStream `PoolCacheUpdate`, nicht Heartbeat-SLAVE-Age.
//! Source-Grep gegen Sibling `Iron_crab/src/bin/*.rs` + `src/market_data/sidefx/handlers.rs`
//! (skip wenn Checkout fehlt). Blackbox gegen oeffentliche `ironcrab::arbitrage::pool_quote` API.
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Blackbox API + dokumentierte Source-Grep-Gates.

use ironcrab::arbitrage::pool_quote::{
    is_quote_fresh, quote_exact_in, state_fingerprint, PoolQuote, QuoteFreshnessConfig, QuoteKind,
    QuotePoolInput, QuoteSide, QuoteVaultInput, DLMM_PROBE_SOL_LAMPORTS, NATIVE_SOL_MINT,
};
use ironcrab::ipc::{PoolCacheUpdate, PoolCacheUpdateType, RecordHeader};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MAX_PRICE_AGE_MS: u64 = 30_000;

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

fn sidefx_handlers_rs_path() -> PathBuf {
    iron_crab_root()
        .join("src")
        .join("market_data")
        .join("sidefx")
        .join("handlers.rs")
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

fn read_sidefx_handlers_source() -> String {
    let path = sidefx_handlers_rs_path();
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
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
    let needles = [format!("async fn {fn_name}"), format!("fn {fn_name}")];
    let start = needles
        .iter()
        .find_map(|needle| source.find(needle))
        .unwrap_or_else(|| panic!("expected fn {fn_name} in source"));
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

fn assert_no_hot_path_rpc(body: &str, context: &str) {
    let forbidden = [
        ".rpc.",
        "get_account(",
        "getMultipleAccounts",
        "get_multiple_accounts",
    ];
    for needle in forbidden {
        assert!(
            !body.contains(needle),
            "{context} must not use hot-path RPC marker `{needle}` (I-7)"
        );
    }
}

/// #437 Slot-Sustain: geyser_slot allein darf Apply nicht wieder erlauben.
fn assert_forbids_slot_sustain_apply_gate(body: &str, context: &str) {
    let sustain_or = body.contains("|| !vault_material_unchanged")
        && (body.contains("update.geyser_slot > existing.update_slot")
            || body.contains("update.geyser_slot >= existing.update_slot"));
    assert!(
        !sustain_or,
        "{context} darf #437-Pattern (geyser_slot > existing OR !vault_material_unchanged) nicht enthalten"
    );
    if body.contains("should_apply") && body.contains("update.geyser_slot") {
        assert!(
            body.contains("vault_material_unchanged"),
            "{context}: should_apply mit geyser_slot muss vault_material_unchanged-Skip enthalten (kein Slot-Advance ohne Material)"
        );
        if let (Some(material_idx), Some(apply_idx)) = (
            body.find("vault_material_unchanged"),
            body.find("should_apply"),
        ) {
            assert!(
                material_idx < apply_idx,
                "{context}: vault_material_unchanged muss vor should_apply kommen (kein Slot-Sustain-Gate)"
            );
        }
    }
}

fn first_vault_write_marker(body: &str) -> Option<usize> {
    [
        "cache.insert",
        "vault_balances.insert",
        "vault_balances.entry",
    ]
    .iter()
    .filter_map(|marker| body.find(marker))
    .min()
}

/// Positiv: unveraendertes Material skippt (return false) vor dem ersten Vault-/Cache-Write.
fn assert_material_unchanged_skips_before_write(body: &str, context: &str) {
    let material_idx = body
        .find("vault_material_unchanged")
        .unwrap_or_else(|| panic!("{context} muss vault_material_unchanged enthalten"));
    let write_idx = first_vault_write_marker(body).unwrap_or_else(|| {
        panic!("{context} muss cache.insert oder vault_balances.insert/entry enthalten")
    });
    assert!(
        material_idx < write_idx,
        "{context}: vault_material_unchanged muss vor cache/vault write kommen (Index-Gate)"
    );
    let between = &body[material_idx..write_idx];
    assert!(
        between.contains("return false"),
        "{context}: nach vault_material_unchanged muss return false vor Write kommen (Apply-Skip)"
    );
}

fn quote_window_change_flag(body: &str) -> Option<&'static str> {
    [
        "quote_window_changed",
        "quote_window_bins_changed",
        "window_fingerprint_changed",
    ]
    .into_iter()
    .find(|flag| body.contains(flag))
}

fn try_block_range(body: &str, open_brace: usize) -> Option<(usize, usize)> {
    if !body[open_brace..].starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, ch) in body[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((open_brace + 1, open_brace + offset));
                }
            }
            _ => {}
        }
    }
    None
}

const UPDATE_SLOT_FIELD: &str = ".update_slot";

/// Feld-Zuweisung `.update_slot = …`, nicht Vergleich (`==`, `>=`, …) und nicht Fn-Parameter `update_slot: Type`.
fn is_update_slot_field_write(body: &str, dot_pos: usize) -> bool {
    let bytes = body.as_bytes();
    let after_field = dot_pos + UPDATE_SLOT_FIELD.len();
    if after_field > bytes.len() {
        return false;
    }
    let mut idx = after_field;
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b'=' {
        return false;
    }
    if idx + 1 < bytes.len() && bytes[idx + 1] == b'=' {
        return false;
    }
    if idx > 0 && matches!(bytes[idx - 1], b'>' | b'<' | b'!') {
        return false;
    }
    true
}

fn update_slot_write_in_snippet(snippet: &str) -> bool {
    let mut start = 0usize;
    while let Some(rel) = snippet[start..].find(UPDATE_SLOT_FIELD) {
        let dot_pos = start + rel;
        if is_update_slot_field_write(snippet, dot_pos) {
            return true;
        }
        start = dot_pos + 1;
    }
    false
}

fn update_slot_write_positions(body: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut start = 0usize;
    while let Some(rel) = body[start..].find(UPDATE_SLOT_FIELD) {
        let dot_pos = start + rel;
        if is_update_slot_field_write(body, dot_pos) {
            positions.push(dot_pos + 1);
        }
        start = dot_pos + 1;
    }
    positions
}

fn is_negated_window_condition(cond: &str, window_flag: &str) -> bool {
    cond.contains(&format!("!{window_flag}"))
        || cond.contains(&format!("! {window_flag}"))
        || cond.contains(&format!("{window_flag} == false"))
}

fn is_positive_window_condition(cond: &str, window_flag: &str) -> bool {
    cond.contains(window_flag) && !is_negated_window_condition(cond, window_flag)
}

fn if_condition_before_else(body: &str, else_keyword_pos: usize) -> Option<&str> {
    let before = body[..else_keyword_pos].trim_end();
    if !before.ends_with('}') {
        return None;
    }
    let if_pos = before.rfind("if ")?;
    let open = if_pos + before[if_pos..].find('{')?;
    let (_, if_body_end) = try_block_range(body, open)?;
    if if_body_end >= else_keyword_pos || body[if_body_end..else_keyword_pos].trim() != "}" {
        return None;
    }
    Some(body[if_pos + 3..open].trim())
}

fn is_inside_positive_window_if_body(body: &str, write_pos: usize, window_flag: &str) -> bool {
    let mut search = 0usize;
    while search < write_pos {
        let Some(rel) = body[search..write_pos].find("if ") else {
            break;
        };
        let if_pos = search + rel;
        let Some(open_rel) = body[if_pos..write_pos].find('{') else {
            search = if_pos + 3;
            continue;
        };
        let open = if_pos + open_rel;
        let cond = body[if_pos + 3..open].trim();
        if is_positive_window_condition(cond, window_flag) {
            if let Some((body_start, body_end)) = try_block_range(body, open) {
                if write_pos > body_start && write_pos <= body_end {
                    return true;
                }
            }
        }
        search = if_pos + 3;
    }
    false
}

fn has_early_return_on_unchanged_window(body: &str, write_pos: usize, window_flag: &str) -> bool {
    let mut search = 0usize;
    while search < write_pos {
        let Some(rel) = body[search..write_pos].find("if ") else {
            break;
        };
        let if_pos = search + rel;
        let Some(open_rel) = body[if_pos..write_pos].find('{') else {
            search = if_pos + 3;
            continue;
        };
        let open = if_pos + open_rel;
        let cond = body[if_pos + 3..open].trim();
        if is_negated_window_condition(cond, window_flag) {
            if let Some((body_start, body_end)) = try_block_range(body, open) {
                if body_end <= write_pos && body[body_start..body_end].contains("return") {
                    return true;
                }
            }
        }
        search = if_pos + 3;
    }
    false
}

fn update_slot_write_is_gated(body: &str, write_pos: usize, window_flag: &str) -> bool {
    is_inside_positive_window_if_body(body, write_pos, window_flag)
        || has_early_return_on_unchanged_window(body, write_pos, window_flag)
}

/// Positiv: Quote-Window-Flag und update_slot im Body; kein immer-feuernder Else-Bump ohne Flag.
fn assert_quote_window_gates_update_slot(body: &str, context: &str) {
    let window_flag = quote_window_change_flag(body).unwrap_or_else(|| {
        panic!(
            "{context} muss quote_window_changed, quote_window_bins_changed oder window_fingerprint_changed enthalten"
        )
    });
    let writes = update_slot_write_positions(body);
    assert!(
        !writes.is_empty(),
        "{context} muss update_slot setzen (Material-Slot via Quote-Window-Wechsel)"
    );

    let mut search = 0usize;
    while let Some(rel) = body[search..].find("else {") {
        let else_pos = search + rel;
        let open = else_pos + "else ".len();
        let Some((start, end)) = try_block_range(body, open) else {
            search = else_pos + 1;
            continue;
        };
        let else_body = &body[start..end];
        if update_slot_write_in_snippet(else_body) {
            if let Some(cond) = if_condition_before_else(body, else_pos) {
                assert!(
                    !is_positive_window_condition(cond, window_flag),
                    "{context}: else-Zweig darf update_slot nicht setzen (kein Overlay-Bump ohne {window_flag})"
                );
            }
            assert!(
                update_slot_write_is_gated(body, start, window_flag),
                "{context}: else-Zweig mit update_slot muss durch {window_flag} gegated sein (kein Overlay-Bump ohne Flag)"
            );
        }
        search = end + 1;
    }

    for write_pos in writes {
        assert!(
            update_slot_write_is_gated(body, write_pos, window_flag),
            "{context}: update_slot-Zuweisung muss durch {window_flag} gegated sein (kein Overlay-Bump ohne Flag)"
        );
    }
}

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

fn sample_vault(
    token_reserve: u64,
    sol_reserve: u64,
    update_slot: u64,
    updated_at: Instant,
) -> QuoteVaultInput {
    QuoteVaultInput {
        reserve_base: token_reserve,
        reserve_quote: sol_reserve,
        update_slot,
        updated_at,
        active_id: None,
        bin_step: None,
        dlmm_sol_is_x: false,
        dlmm_token_x_mint: None,
    }
}

// --- Blackbox: Quote-Uhr folgt Vault-Event, nicht SLAVE age_ms ---

#[test]
fn quote_freshness_uses_vault_updated_at_within_max_price_age_ms() {
    let pool = sample_pool("pump_amm", "fresh_event_pool");
    let vault = sample_vault(1_000_000_000_000, 1_000_000_000, 100, Instant::now());
    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    )
    .expect("ExecutableMarginal quote");

    let freshness = QuoteFreshnessConfig {
        trade_ttl_ms: MAX_PRICE_AGE_MS,
        state_ttl_ms: 120_000,
    };
    assert!(
        is_quote_fresh(&quote, &freshness, Some(&vault), Instant::now()),
        "frisches Vault.updated_at (Event-Apply) muss quote fresh halten (MAX_PRICE_AGE_MS={MAX_PRICE_AGE_MS})"
    );
}

#[test]
fn stale_quote_as_of_ts_not_healed_by_fingerprint_match_alone() {
    let pool = sample_pool("pump_amm", "stale_as_of_ts_pool");
    let vault = sample_vault(1_000_000_000_000, 1_000_000_000, 100, Instant::now());
    let fingerprint = state_fingerprint(&vault);
    let stale_quote = PoolQuote {
        pool_address: pool.pool_address.clone(),
        dex: pool.dex.clone(),
        kind: QuoteKind::ExecutableMarginal,
        side: QuoteSide::Buy,
        as_of_slot: 100,
        as_of_ts: Instant::now() - Duration::from_millis(MAX_PRICE_AGE_MS + 5_000),
        fresh: false,
        state_fingerprint: fingerprint,
        amount_in: DLMM_PROBE_SOL_LAMPORTS,
        amount_out: 1,
    };
    let freshness = QuoteFreshnessConfig {
        trade_ttl_ms: MAX_PRICE_AGE_MS,
        state_ttl_ms: MAX_PRICE_AGE_MS,
    };
    assert!(
        !is_quote_fresh(&stale_quote, &freshness, Some(&vault), Instant::now()),
        "hypothetische SLAVE-Frische (vault.updated_at=now) darf altes quote.as_of_ts nicht heilen"
    );
}

#[test]
fn quote_exact_in_mirrors_vault_update_slot_field() {
    // A.48: Apply darf vault.update_slot ohne Material-Wechsel nicht bumpen (Source-Grep).
    // quote_exact_in kopiert das Vault-Feld — das testen wir hier, ohne Slot-Sustain als Soll zu verkaufen.
    // A.54: Orca-CLMM braucht Tick-Arrays; Fixture nur CPMM (pump_amm) fuer Slot-Spiegel, nicht Orca-Pfad.
    let pool = sample_pool("pump_amm", "vault_field_mirror_pool");
    let vault = sample_vault(1_000_000_000_000, 1_000_000_000, 42, Instant::now());
    let quote = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    )
    .expect("quote");
    assert_eq!(
        quote.as_of_slot, vault.update_slot,
        "PoolQuote.as_of_slot spiegelt vault.update_slot (Apply-Policy separat via Source-Grep)"
    );
}

#[test]
fn heartbeat_requote_with_unchanged_vault_keeps_as_of_slot() {
    let pool = sample_pool("pump_amm", "heartbeat_slot_guard_pool");
    let updated_at = Instant::now() - Duration::from_secs(5);
    let vault = sample_vault(1_000_000_000_000, 1_000_000_000, 200, updated_at);
    let first = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    )
    .expect("first quote");
    let second = quote_exact_in(
        &pool,
        Some(&vault),
        None,
        NATIVE_SOL_MINT,
        &pool.token_mint,
        DLMM_PROBE_SOL_LAMPORTS,
    )
    .expect("second quote");
    assert_eq!(
        first.as_of_slot, second.as_of_slot,
        "identischer Material-Fingerprint + unveraenderter vault.update_slot: as_of_slot darf nicht vorruecken (A.48 Heartbeat)"
    );
    assert_eq!(first.as_of_slot, 200);
    assert_eq!(first.state_fingerprint, second.state_fingerprint);
}

#[test]
fn pool_cache_update_balance_updated_exposes_geyser_slot() {
    let update = PoolCacheUpdate {
        header: RecordHeader::new("ironcrab-eval", "test", "run-event-slot"),
        pool_address: "Pool111111111111111111111111111111111111111".into(),
        dex: "orca".into(),
        base_mint: NATIVE_SOL_MINT.to_string(),
        quote_mint: "TokenMint11111111111111111111111111111111".into(),
        base_reserve: 1_000_000,
        quote_reserve: 2_000_000,
        geyser_slot: 777,
        liquidity_lamports: Some(2_000_000),
        update_type: PoolCacheUpdateType::BalanceUpdated,
        metadata: None,
    };
    assert_eq!(
        update.geyser_slot, 777,
        "JetStream PoolCacheUpdate muss Tick-Slot (geyser_slot) tragen (I-MD-4)"
    );
}

// --- Source-Grep: Event-Apply consume_vault_seed_from_pool_cache_update ---

#[test]
fn event_vault_apply_uses_geyser_slot_and_instant_now_not_slave_age() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);
    if !prod.contains("fn consume_vault_seed_from_pool_cache_update") {
        eprintln!("SKIP: consume_vault_seed_from_pool_cache_update not in sibling arb_strategy.rs");
        return;
    }

    let body = extract_fn_block(prod, "consume_vault_seed_from_pool_cache_update");
    assert!(
        (body.contains("update_slot: update.geyser_slot")
            || (body.contains("update.geyser_slot") && body.contains("update_slot")))
            && body.contains("Instant::now()"),
        "Event-Apply muss update_slot aus update.geyser_slot und updated_at via Instant::now() setzen"
    );
    assert!(
        !body.contains("get_with_metadata"),
        "consume_vault_seed_from_pool_cache_update darf SLAVE get_with_metadata/age_ms nicht als Uhr nutzen"
    );
    assert!(
        !body.contains("age_ms"),
        "consume_vault_seed_from_pool_cache_update darf age_ms nicht als Quote-Uhr nutzen"
    );
    assert!(
        body.contains("inc_arb_vault_balance_applied_total"),
        "Event-Apply muss inc_arb_vault_balance_applied_total inkrementieren"
    );
    assert!(
        body.contains("vault_material_unchanged"),
        "Event-Apply muss unveraendertes Material vor Schreiben filtern (A.48 kein Slot-Sustain)"
    );
    assert_forbids_slot_sustain_apply_gate(&body, "consume_vault_seed_from_pool_cache_update");
    assert_material_unchanged_skips_before_write(
        &body,
        "consume_vault_seed_from_pool_cache_update",
    );
    assert!(
        !body.contains("inc_arb_vault_live_snapshot_seeded_total"),
        "C1h live_snapshot_seeded darf nicht im JetStream-Event-Apply-Pfad liegen"
    );
    assert!(
        body.contains("reserve_base == 0 && reserve_quote == 0")
            || body.contains("0 && reserve_quote == 0"),
        "0/0 Reserves duerfen nicht in vault_balances geschrieben werden"
    );
}

// --- Source-Grep: C1h darf Event-frische Pin-Zeile nicht zuruecksetzen ---

#[test]
fn c1h_pin_guards_reject_stale_slave_and_preserve_event_fresh_rows() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);

    for fn_name in [
        "pin_slave_snapshot_age_allowed",
        "pin_slave_refresh_allowed",
        "try_seed_vault_from_live_cache",
        "try_refresh_vault_from_live_cache",
    ] {
        if !prod.contains(&format!("fn {fn_name}")) {
            eprintln!("SKIP: {fn_name} not in sibling arb_strategy.rs");
            return;
        }
    }

    let age_body = extract_fn_block(prod, "pin_slave_snapshot_age_allowed");
    assert!(
        age_body.contains("MAX_PRICE_AGE_MS"),
        "pin_slave_snapshot_age_allowed muss MAX_PRICE_AGE_MS (30_000) verwenden"
    );

    let refresh_body = extract_fn_block(prod, "pin_slave_refresh_allowed");
    assert!(
        refresh_body.contains("pin_slave_snapshot_age_allowed")
            && refresh_body.contains("existing.updated_at.elapsed()")
            && refresh_body.contains("vault_material_unchanged"),
        "pin_slave_refresh_allowed muss stale SLAVE age, event-frische Zeilen und unveraendertes Material filtern"
    );

    let seed_body = extract_fn_block(prod, "try_seed_vault_from_live_cache");
    assert!(
        seed_body.contains("pin_slave_snapshot_age_allowed"),
        "Pin-Seed nur wenn SLAVE age_ms <= MAX_PRICE_AGE_MS"
    );
    assert!(
        seed_body.contains("contains_key"),
        "try_seed_vault_from_live_cache seedet nur fehlende Keys"
    );

    let refresh_cache_body = extract_fn_block(prod, "try_refresh_vault_from_live_cache");
    assert!(
        refresh_cache_body.contains("pin_slave_refresh_allowed"),
        "Pin-Refresh muss pin_slave_refresh_allowed nutzen"
    );
}

// --- Source-Grep: Heartbeat spoofed Material-Slot nicht ---

#[test]
fn heartbeat_does_not_spoof_material_slot_in_arb_seed_path() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);

    if !prod.contains("fn seed_one_pool_from_live_cache") {
        eprintln!("SKIP: seed_one_pool_from_live_cache not in sibling arb_strategy.rs");
        return;
    }

    let seed_body = extract_fn_block(prod, "seed_one_pool_from_live_cache");
    assert!(
        seed_body.contains("vault_material_unchanged") && seed_body.contains("(false, false)"),
        "identischer Material-Fingerprint bei Seed darf vault.update_slot/updated_at nicht bumpen"
    );

    let fresher_body = extract_fn_block(prod, "live_pool_cache_fresher_than_vault");
    assert!(
        fresher_body.contains("vault_material_unchanged") && fresher_body.contains("return false"),
        "live_pool_cache_fresher_than_vault muss bei unveraendertem Material false liefern (kein Slot-Spoof)"
    );
}

// --- Source-Grep: DLMM Bin-Overlay — Quote-Window-Fingerprint, kein Slot-Sustain ---

#[test]
fn bin_array_overlay_bumps_vault_slot_only_on_quote_window_fingerprint_change() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_bin_source("arb_strategy");
    let prod = production_bin_source(&source);
    if !prod.contains("fn handle_bin_array_update") {
        eprintln!("SKIP: handle_bin_array_update not in sibling arb_strategy.rs");
        return;
    }

    let body = extract_fn_block(prod, "handle_bin_array_update");
    assert!(
        body.contains("dlmm_quote_window_bins_fingerprint")
            || body.contains("quote_window_bins_fingerprint"),
        "handle_bin_array_update muss Quote-Window-Bin-Fingerprint fuer Material-Slot nutzen"
    );
    assert_quote_window_gates_update_slot(&body, "handle_bin_array_update");
}

#[test]
fn market_data_heartbeat_touch_does_not_invent_new_geyser_slot() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_bin_source("market_data");
    let prod = production_bin_source(&source);
    if !prod.contains("fn try_touch_live_pool_reserve_basis_for_hot_pool") {
        eprintln!(
            "SKIP: try_touch_live_pool_reserve_basis_for_hot_pool not in sibling market_data.rs"
        );
        return;
    }

    let touch_body = extract_fn_block(prod, "try_touch_live_pool_reserve_basis_for_hot_pool");
    assert!(
        touch_body.contains("touch_freshness_on_existing_reserve_basis"),
        "Heartbeat touch darf SLAVE-Age sustainen ohne neues Reserve-Material"
    );
    assert!(
        !touch_body.contains("upsert("),
        "try_touch darf keinen kuenstlichen Cache-Slot-Bump via upsert erzeugen"
    );

    if prod.contains("fn try_publish_balance_updated_from_cache_with_heartbeat_observability") {
        let publish_body = extract_fn_block(
            prod,
            "try_publish_balance_updated_from_cache_with_heartbeat_observability",
        );
        assert!(
            publish_body.contains("get_with_metadata") && publish_body.contains("publish_slot"),
            "Heartbeat BalanceUpdated muss bestehenden MASTER-Slot publizieren, nicht erfinden"
        );
    }
}

// --- Source-Grep: I-7 Vault-Tick ohne RPC ---

#[test]
fn vault_tick_sidefx_handler_production_path_no_rpc() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let handlers_path = sidefx_handlers_rs_path();
    if !handlers_path.is_file() {
        eprintln!("SKIP: sidefx/handlers.rs fehlt unter {:?}", handlers_path);
        return;
    }
    let handlers = read_sidefx_handlers_source();
    if !handlers.contains("pub fn md_sidefx_process_vault_balance_tick") {
        eprintln!("SKIP: md_sidefx_process_vault_balance_tick not in sidefx/handlers.rs");
        return;
    }

    let tick_body = extract_fn_block(&handlers, "md_sidefx_process_vault_balance_tick");
    assert_no_hot_path_rpc(&tick_body, "md_sidefx_process_vault_balance_tick");

    let md_source = read_bin_source("market_data");
    let wrapper = extract_fn_block(&md_source, "md_sidefx_process_vault_balance_tick");
    assert!(
        wrapper.contains("sidefx_process_vault_balance_tick"),
        "market_data.rs muss Sidefx-Handler delegieren (Eval-Grep wrapper)"
    );
}
