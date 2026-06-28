//! Invariant: Frisch erzeugter Momentum-Tier-1-BUY-Intent erreicht die Execution Engine
//! innerhalb von 250 ms nach `RecordHeader.ts_unix_ms` (`process_intent`-Eintritt), auch unter
//! Prod-aehnlicher JetStream-Nebenlast (Wallet-Snapshot + PoolCache).
//!
//! Erwartung vor Impl-Fix: **rot** (Prod: nur ~12,6 % ≤250 ms; Ø ~4942 ms header→receive).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; Blackbox via NATS DecisionRecords + Harness;
//! kein Lesen von `Iron_crab/src/`.
//!
//! Manueller Lauf (nats-server + Iron_crab-Binaries erforderlich):
//! `cargo test --test invariants_intent_delivery_latency_under_load -- --ignored`

mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use common::request_reply_e2e_harness::{
    now_unix_ms, publish_trade_intent_jetstream, seed_wallet_balance_snapshot_jetstream,
    wait_for_intent_decision_latency_ms, wait_until_wallet_snapshot_visible_in_jetstream,
    JetStreamLoadGenerator, RequestReplyE2eHarness, INTENT_DELIVERY_SLO_MS,
};
use ironcrab::ipc::{
    ExplicitAmount, IntentOrigin, IntentTier, TradeIntent, TradeResources, TradeSide, TradingRegime,
};
use solana_sdk::pubkey::Pubkey;

const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";
const SPL_TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const WSOL_DECIMALS: u8 = 9;

/// Anzahl unterschiedlicher Mints/Pools fuer Last-Generator (Zyklus).
const LOAD_TOKEN_MINT_COUNT: usize = 64;
const LOAD_POOL_ADDRESS_COUNT: usize = 32;

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let root = iron_crab_root();
    if !root.join("Cargo.toml").exists() {
        eprintln!(
            "SKIP: Iron_crab nicht als Sibling gefunden unter {:?}",
            root
        );
        return None;
    }
    Some(root)
}

fn momentum_tier1_buy_intent(
    token_mint: &str,
    pool_address: &str,
    intent_id: String,
    header_ts_ms: u64,
) -> TradeIntent {
    let resources = TradeResources {
        input_mint: WSOL_MINT.to_string(),
        output_mint: token_mint.to_string(),
        pools: vec![pool_address.to_string()],
        accounts: vec![],
        token_program: None,
    };

    let mut intent = TradeIntent::new(
        "momentum-bot",
        "eval-intent-delivery",
        "run-e2e",
        intent_id,
        "momentum-bot",
        IntentTier::Tier1,
        IntentOrigin::StrategyA,
        ExplicitAmount::new(50_000_000, WSOL_DECIMALS),
        resources,
        100,
        500,
        TradeSide::Buy,
        TradingRegime::Early,
    );
    intent.ttl_ms = Some(5000);
    intent.header.ts_unix_ms = header_ts_ms;
    let mut md = HashMap::new();
    md.insert("dex".to_string(), "pump_amm".to_string());
    intent.metadata = md;
    intent
}

/// E2E: Intent-Delivery-SLO unter JetStream-Last.
///
/// requires nats-server — in CI mit `#[ignore]`; lokal:
/// `cargo test --test invariants_intent_delivery_latency_under_load -- --ignored`
#[test]
#[ignore = "requires nats-server and Iron_crab binaries; cargo test --test invariants_intent_delivery_latency_under_load -- --ignored"]
fn intent_delivery_latency_under_jetstream_load_within_slo() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    if let Err(e) = harness.start_nats() {
        if e.contains("nats-server nicht gefunden") {
            eprintln!("SKIP: {}", e);
            return;
        }
        panic!("nats start: {}", e);
    }

    let (kp_path, wallet_pubkey) = harness
        .write_eval_treasury_keypair()
        .expect("fixture keypair");

    let token_mint_pk = Pubkey::new_unique();
    let token_mint = token_mint_pk.to_string();
    let pool_address_pk = Pubkey::new_unique();
    let pool_address = pool_address_pk.to_string();

    let wsol_balance_raw: u64 = 500_000_000;
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    rt.block_on(seed_wallet_balance_snapshot_jetstream(
        harness.nats_url(),
        &wallet_pubkey,
        WSOL_MINT,
        wsol_balance_raw,
        WSOL_DECIMALS,
        SPL_TOKEN_PROGRAM,
    ))
    .expect("seed WSOL wallet snapshot");

    rt.block_on(wait_until_wallet_snapshot_visible_in_jetstream(
        harness.nats_url(),
        &wallet_pubkey,
        WSOL_MINT,
        wsol_balance_raw,
    ))
    .expect("WSOL snapshot visible before EE start");

    let load_mints: Vec<String> = (0..LOAD_TOKEN_MINT_COUNT)
        .map(|i| Pubkey::new_from_array([i as u8; 32]).to_string())
        .collect();
    let load_pools: Vec<String> = (0..LOAD_POOL_ADDRESS_COUNT)
        .map(|i| Pubkey::new_from_array([(i + 64) as u8; 32]).to_string())
        .collect();

    let mut load_gen = JetStreamLoadGenerator::start(
        harness.nats_url().to_string(),
        wallet_pubkey.clone(),
        load_mints,
        load_pools,
        token_mint.clone(),
    )
    .expect("start jetstream load generator");

    harness
        .start_execution_engine_with_eval_wallet(&kp_path)
        .expect("execution-engine start with eval wallet");

    JetStreamLoadGenerator::warmup();

    let intent_id = format!(
        "int-eval-delivery-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let intent_header_ts_ms = now_unix_ms();
    let intent = momentum_tier1_buy_intent(
        &token_mint,
        &pool_address,
        intent_id.clone(),
        intent_header_ts_ms,
    );

    let nats_url = harness.nats_url().to_string();
    let metrics_port = harness.execution_engine_metrics_port();

    rt.block_on(publish_trade_intent_jetstream(&nats_url, &intent))
        .expect("jetstream publish trade intent");

    let latency_result = rt.block_on(wait_for_intent_decision_latency_ms(
        &nats_url,
        &intent_id,
        intent_header_ts_ms,
    ));

    let metrics_snippet =
        common::request_reply_e2e_harness::probe_execution_intent_header_to_receive_ms(
            metrics_port,
        )
        .unwrap_or_else(|e| format!("(metrics nicht lesbar: {e})"));

    load_gen.stop();
    harness.stop();
    let diag = harness.capture_eval_e2e_diagnostics();

    let latency_ms = latency_result.unwrap_or_else(|e| {
        panic!(
            "Intent-Delivery-Latenz nicht messbar.\n{e}\n\nintent_id={intent_id}\n\
             intent_header_ts_ms={intent_header_ts_ms}\n\
             SLO={INTENT_DELIVERY_SLO_MS}ms\n\
             metrics:\n{metrics_snippet}\n\n{diag}"
        );
    });

    assert!(
        latency_ms <= INTENT_DELIVERY_SLO_MS,
        "Intent muss EE innerhalb von {INTENT_DELIVERY_SLO_MS}ms nach RecordHeader.ts_unix_ms \
         erreichen (process_intent / DecisionRecord); gemessen={latency_ms}ms intent_id={intent_id}\n\
         metrics:\n{metrics_snippet}\n\n{diag}"
    );
}
