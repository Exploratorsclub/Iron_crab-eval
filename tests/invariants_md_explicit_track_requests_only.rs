//! Invariante I-MD-5 / I-MD-6 / A.51: Explicit budget nur via Track-Requests (TX-Tracker ban + Snapshot scope).
//!
//! - I-MD-5: TX-Ingest enqueued kein `MdStateCommand::TrackMint`; unpinned `TrackMint` fuehrt nicht zu
//!   `ExplicitConsumer::Tracker` admission.
//! - I-MD-6: Snapshot persist/restore ohne `ExplicitConsumer::Tracker` owner_groups.
//! - I-MD-1: `pool_mint_map` / `PumpFunPoolMintMapInsert` Sidefx darf weiter existieren.
//!
//! Source-Contract: Sibling `Iron_crab/src/market_data/ingest/tx_handler.rs` und
//! `Iron_crab/src/bin/market_data.rs` (wie CI dual-checkout).
//! Lib-Blackbox: `ironcrab::market_data::track::*` (Snapshot + Admission API).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Lib-Assertions an oeffentlicher Track-API; Source-Grep auf dokumentierte Architektur-Strings.

use ironcrab::market_data::track::{
    owner_group_snapshot_to_disk, restore_admission_from_owner_groups, AdmissionRestoreResult,
    ExplicitConsumer, ExplicitOwnerKey, ExplicitSetSnapshot, FixedCapAdmission, OwnerGroupSnapshot,
    SnapshotConsumer, SnapshotOwnerGroup, EXPLICIT_SET_SNAPSHOT_VERSION,
};
use solana_sdk::pubkey::Pubkey;
use std::fs;
use std::path::PathBuf;

fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

fn iron_crab_src(rel: &str) -> PathBuf {
    iron_crab_root().join("src").join(rel)
}

fn skip_if_no_sibling_iron_crab() -> Option<PathBuf> {
    let tx_handler = iron_crab_src("market_data/ingest/tx_handler.rs");
    if !tx_handler.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder tx_handler.rs nicht lesbar unter {:?}",
            iron_crab_root()
        );
        return None;
    }
    Some(iron_crab_root())
}

fn read_iron_crab_source(rel: &str) -> String {
    let path = iron_crab_src(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Extrahiert den Rust-Funktionsblock ab `fn {name}` / `async fn {name}` inkl. geschweifter Klammern.
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

/// `match pin { None => { ... } }` Arm in `apply_track_mint`.
fn extract_apply_track_mint_none_arm(source: &str) -> String {
    let fn_body = extract_fn_block(source, "apply_track_mint");
    let match_pin = fn_body
        .find("match pin")
        .unwrap_or_else(|| panic!("expected `match pin` in apply_track_mint"));
    let none_start = fn_body[match_pin..]
        .find("None =>")
        .map(|i| match_pin + i)
        .unwrap_or_else(|| panic!("expected `None =>` arm in apply_track_mint"));
    let brace_start = fn_body[none_start..]
        .find('{')
        .map(|i| none_start + i)
        .expect("expected opening brace for None arm");
    let mut depth = 0usize;
    let mut end = brace_start;
    for (offset, ch) in fn_body[brace_start..].char_indices() {
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
    fn_body[none_start..=end].to_string()
}

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn pool(seed: u8) -> Pubkey {
    Pubkey::new_from_array([
        0x50,
        seed,
        seed.wrapping_mul(3),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        seed,
    ])
}

fn wallet_group(mint: Pubkey) -> OwnerGroupSnapshot {
    OwnerGroupSnapshot {
        consumer: ExplicitConsumer::Wallet,
        owner_key: ExplicitOwnerKey::Wallet,
        pubkeys: vec![mint],
    }
}

fn arb_group(pool_pk: Pubkey, legs: Vec<Pubkey>) -> OwnerGroupSnapshot {
    OwnerGroupSnapshot {
        consumer: ExplicitConsumer::Arb,
        owner_key: ExplicitOwnerKey::Pool(pool_pk),
        pubkeys: legs,
    }
}

fn tracker_group(mint: Pubkey) -> OwnerGroupSnapshot {
    OwnerGroupSnapshot {
        consumer: ExplicitConsumer::Tracker,
        owner_key: ExplicitOwnerKey::Mint(mint),
        pubkeys: vec![mint],
    }
}

/// I-MD-6 persist scope: Tracker owner_groups werden nicht serialisiert.
/// Ersetzen durch `ironcrab::market_data::track::filter_owner_groups_for_snapshot_persist` nach Impl A.51.
fn filter_owner_groups_for_snapshot_persist(
    groups: &[OwnerGroupSnapshot],
) -> Vec<OwnerGroupSnapshot> {
    groups
        .iter()
        .filter(|g| g.consumer != ExplicitConsumer::Tracker)
        .cloned()
        .collect()
}

/// I-MD-6 restore scope: nur Wallet / Momentum / Arb werden re-admittiert.
/// Ersetzen durch `ironcrab::market_data::track::filter_owner_groups_for_snapshot_restore` nach Impl A.51.
fn filter_owner_groups_for_snapshot_restore(
    groups: &[OwnerGroupSnapshot],
) -> Vec<OwnerGroupSnapshot> {
    groups
        .iter()
        .filter(|g| {
            matches!(
                g.consumer,
                ExplicitConsumer::Wallet | ExplicitConsumer::Momentum | ExplicitConsumer::Arb
            )
        })
        .cloned()
        .collect()
}

fn count_tracker_disk_groups(groups: &[SnapshotOwnerGroup]) -> usize {
    groups
        .iter()
        .filter(|g| g.consumer == SnapshotConsumer::Tracker)
        .count()
}

fn admission_has_tracker_owner(admission: &FixedCapAdmission) -> bool {
    admission
        .snapshot_owner_groups()
        .iter()
        .any(|g| g.consumer == ExplicitConsumer::Tracker)
}

// --- A) TX-Pfad Source-Contract ---

/// A.51 / I-MD-5: `handle_geyser_transaction_update` enqueued kein TrackMint.
#[test]
fn tx_ingest_no_track_mint_enqueue() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_iron_crab_source("market_data/ingest/tx_handler.rs");
    let tx_body = extract_fn_block(&source, "handle_geyser_transaction_update");
    assert!(
        !tx_body.contains("MdStateCommand::TrackMint"),
        "handle_geyser_transaction_update must not enqueue MdStateCommand::TrackMint (I-MD-5 A.51)"
    );
    assert!(
        !tx_body.contains("TrackMint {"),
        "handle_geyser_transaction_update must not reference TrackMint command (I-MD-5 A.51)"
    );

    let md_source = read_iron_crab_source("bin/market_data.rs");
    if md_source.contains("async fn handle_geyser_transaction")
        || md_source.contains("fn handle_geyser_transaction")
    {
        let tx_delegate = extract_fn_block(&md_source, "handle_geyser_transaction");
        assert!(
            !tx_delegate.contains("MdStateCommand::TrackMint"),
            "handle_geyser_transaction wrapper must not enqueue MdStateCommand::TrackMint"
        );
        assert!(
            !tx_delegate.contains("TrackMint {"),
            "handle_geyser_transaction wrapper must not reference TrackMint command"
        );
    }
}

/// A.51 / I-MD-1: `pool_mint_map` Sidefx bleibt im TX-Pfad erlaubt.
#[test]
fn tx_ingest_pool_mint_map_sidefx_unchanged() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_iron_crab_source("market_data/ingest/tx_handler.rs");
    let tx_body = extract_fn_block(&source, "handle_geyser_transaction_update");
    assert!(
        tx_body.contains("PumpFunPoolMintMapInsert")
            || tx_body.contains("pool_mint_map"),
        "handle_geyser_transaction_update may keep PumpFunPoolMintMapInsert / pool_mint_map sidefx (I-MD-1)"
    );
}

// --- B) Snapshot persist/restore Lib-Blackbox ---

/// I-MD-6: persist filter entfernt Tracker owner_groups aus Snapshot-Output.
#[test]
fn i_md_6_snapshot_persist_excludes_tracker() {
    let wallet_mint = pk(1);
    let arb_pool = pool(2);
    let arb_leg = pk(3);
    let tracker_mint = pk(4);

    let groups = vec![
        wallet_group(wallet_mint),
        arb_group(arb_pool, vec![arb_leg]),
        tracker_group(tracker_mint),
    ];
    let filtered = filter_owner_groups_for_snapshot_persist(&groups);
    assert_eq!(filtered.len(), 2);
    assert!(
        filtered
            .iter()
            .all(|g| g.consumer != ExplicitConsumer::Tracker),
        "persist scope must exclude all Tracker owner_groups"
    );

    let disk_groups: Vec<SnapshotOwnerGroup> =
        filtered.iter().map(owner_group_snapshot_to_disk).collect();
    assert_eq!(count_tracker_disk_groups(&disk_groups), 0);

    let mut snapshot = ExplicitSetSnapshot::new(Some("eval-a51".into()));
    snapshot.version = EXPLICIT_SET_SNAPSHOT_VERSION;
    snapshot.owner_groups = disk_groups;
    assert_eq!(count_tracker_disk_groups(&snapshot.owner_groups), 0);
    assert_eq!(snapshot.owner_groups.len(), 2);
}

/// I-MD-6: Legacy-Snapshot mit Tracker+Arb → restore filter → nur Arb+Wallet admitted.
#[test]
fn i_md_6_snapshot_restore_strips_legacy_tracker() {
    let wallet_mint = pk(10);
    let arb_pool = pool(11);
    let arb_leg = pk(12);
    let tracker_mint = pk(13);

    let legacy_json = format!(
        r#"{{
            "version": {version},
            "saved_at_unix": 1,
            "run_id": "legacy-eval",
            "rows": [],
            "owner_groups": [
                {{
                    "consumer": "Wallet",
                    "owner": {{ "kind": "wallet", "pubkey": null }},
                    "pubkeys": ["{wallet}"]
                }},
                {{
                    "consumer": "Arb",
                    "owner": {{ "kind": "pool", "pubkey": "{arb_pool}" }},
                    "pubkeys": ["{arb_leg}"]
                }},
                {{
                    "consumer": "Tracker",
                    "owner": {{ "kind": "mint", "pubkey": "{tracker}" }},
                    "pubkeys": ["{tracker}"]
                }}
            ],
            "pool_mint_map": [],
            "momentum_pools": [],
            "arb_pools": ["{arb_pool}"]
        }}"#,
        version = EXPLICIT_SET_SNAPSHOT_VERSION,
        wallet = wallet_mint,
        arb_pool = arb_pool,
        arb_leg = arb_leg,
        tracker = tracker_mint,
    );

    let snapshot: ExplicitSetSnapshot =
        serde_json::from_str(&legacy_json).expect("parse legacy snapshot fixture");
    let parsed_groups = snapshot.to_owner_group_snapshots();
    assert_eq!(parsed_groups.len(), 3);

    let restore_groups = filter_owner_groups_for_snapshot_restore(&parsed_groups);
    assert_eq!(restore_groups.len(), 2);
    assert!(restore_groups
        .iter()
        .all(|g| g.consumer != ExplicitConsumer::Tracker));

    let mut admission = FixedCapAdmission::new(16);
    assert_eq!(
        restore_admission_from_owner_groups(&mut admission, &restore_groups),
        AdmissionRestoreResult::Restored
    );
    assert!(!admission_has_tracker_owner(&admission));
    assert!(admission.contains(&wallet_mint));
    assert!(admission.contains(&arb_leg));
    assert!(!admission.contains(&tracker_mint));
}

/// I-MD-6 Source-Contract: `build_explicit_set_snapshot` filtert Tracker owner_groups vor Persist.
#[test]
fn i_md_6_source_build_explicit_set_snapshot_excludes_tracker() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_iron_crab_source("bin/market_data.rs");
    let body = extract_fn_block(&source, "build_explicit_set_snapshot");
    assert!(
        body.contains("ExplicitConsumer::Tracker")
            || body.contains("SnapshotConsumer::Tracker")
            || body.contains("filter_owner_groups_for_snapshot_persist")
            || body.contains("filter_snapshot_owner_groups"),
        "build_explicit_set_snapshot must exclude Tracker owner_groups from snapshot persist (I-MD-6)"
    );
}

/// I-MD-6 Source-Contract: Restore filtert Legacy-Tracker vor Admission.
#[test]
fn i_md_6_source_restore_explicit_admission_strips_tracker() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_iron_crab_source("bin/market_data.rs");
    let body = extract_fn_block(&source, "restore_explicit_admission_from_snapshot");
    assert!(
        body.contains("filter_owner_groups_for_snapshot_restore")
            || body.contains("filter_snapshot_owner_groups")
            || (body.contains("ExplicitConsumer::Tracker")
                && body.contains("filter")),
        "restore_explicit_admission_from_snapshot must filter Tracker groups before restore (I-MD-6)"
    );
}

// --- C) Unpinned TrackMint rejected ---

/// I-MD-5: `apply_track_mint(..., None)` darf keine Tracker-Explicit-Admission erzeugen.
#[test]
fn i_md_5_unpinned_track_mint_no_admission() {
    if skip_if_no_sibling_iron_crab().is_none() {
        return;
    }
    let source = read_iron_crab_source("bin/market_data.rs");
    let none_arm = extract_apply_track_mint_none_arm(&source);
    assert!(
        !none_arm.contains("try_admit_owner_group(admission, Self::tracker_mint_owner"),
        "apply_track_mint None arm must not admit ExplicitConsumer::Tracker via tracker_mint_owner (I-MD-5)"
    );
    assert!(
        !none_arm.contains("ExplicitConsumer::Tracker"),
        "apply_track_mint None arm must not reference ExplicitConsumer::Tracker admission (I-MD-5)"
    );
    assert!(
        !none_arm.contains("inc_market_data_tracker_admission_admitted_total"),
        "apply_track_mint None arm must not count tracker explicit admission (I-MD-5)"
    );
}
