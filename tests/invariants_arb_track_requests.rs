//! Invariante I-4e / A.47 Phase 3 (Hybrid): Arb `track_requests` NATS topic — Publisher-Grep + Schema.
//!
//! Source-Contract gegen Sibling `Iron_crab/src/bin/*.rs` (wie CI dual-checkout).
//! Schema-Roundtrip gegen oeffentliche `ironcrab::nats` Wire-Types (Blackbox).
//!
//! STOP-CHECK (AGENTS.md): nur Eval-Repo; nur Tests; keine Aenderung an `Iron_crab/src/`;
//! Architektur-Grep-Gates auf dokumentierte Topic-/Publish-Marker, keine private API.

use std::fs;
use std::path::PathBuf;

const PUBLISH_CALL_MARKER: &str = "nats.publish(TOPIC_ARB_TRACK_REQUESTS";
const SUBSCRIBE_CALL_MARKER: &str = "nats.subscribe(TOPIC_ARB_TRACK_REQUESTS";

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
    let root = iron_crab_root();
    let path = iron_crab_bin_rs("arb_strategy");
    if !path.is_file() {
        eprintln!(
            "SKIP: Iron_crab Sibling-Checkout fehlt oder arb_strategy.rs nicht lesbar unter {:?}",
            root
        );
        return None;
    }
    Some(root)
}

fn read_bin_source(name: &str) -> String {
    let path = iron_crab_bin_rs(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Production code only — test modules in the same file must not affect grep gates.
fn production_source(source: &str) -> &str {
    source
        .split("#[cfg(test)]")
        .next()
        .expect("production source section")
}

/// Bin sources with inline `#[cfg(test)]` hooks before production code ends (e.g. arb_strategy):
/// strip only trailing `#[cfg(test)] mod …` test modules.
fn production_bin_source(source: &str) -> &str {
    if let Some(idx) = source.find("#[cfg(test)]\nmod ") {
        return &source[..idx];
    }
    production_source(source)
}

fn count_publish_markers(prod_src: &str) -> usize {
    prod_src.matches(PUBLISH_CALL_MARKER).count()
}

fn skip_if_no_phase3_arb_publish_in_sibling() -> bool {
    let path = iron_crab_bin_rs("arb_strategy");
    if !path.is_file() {
        return true;
    }
    let source = read_bin_source("arb_strategy");
    if !production_bin_source(&source).contains("TOPIC_ARB_TRACK_REQUESTS") {
        eprintln!("SKIP: Phase 3 TOPIC_ARB_TRACK_REQUESTS not present in sibling arb_strategy.rs");
        return true;
    }
    false
}

/// Phase 3 (I-4e): Nur `arb_strategy` publiziert `TOPIC_ARB_TRACK_REQUESTS`; nur `market_data` subscribed.
#[test]
fn phase3_only_arb_strategy_publishes_track_requests_topic() {
    if skip_if_no_sibling_iron_crab().is_none() || skip_if_no_phase3_arb_publish_in_sibling() {
        return;
    }

    let arb_src = read_bin_source("arb_strategy");
    let md_src = read_bin_source("market_data");
    let arb_prod = production_bin_source(&arb_src);

    assert!(
        arb_prod.contains(PUBLISH_CALL_MARKER),
        "arb_strategy must publish TOPIC_ARB_TRACK_REQUESTS (nats.publish + topic constant)"
    );
    assert!(
        !arb_prod.contains(SUBSCRIBE_CALL_MARKER),
        "arb_strategy must not subscribe to TOPIC_ARB_TRACK_REQUESTS (publisher-only)"
    );

    assert!(
        md_src.contains(SUBSCRIBE_CALL_MARKER),
        "market_data must subscribe to TOPIC_ARB_TRACK_REQUESTS"
    );
    assert!(
        !md_src.contains(PUBLISH_CALL_MARKER),
        "market_data must not publish TOPIC_ARB_TRACK_REQUESTS"
    );

    for bin in ["momentum_bot", "execution_engine"] {
        let path = iron_crab_bin_rs(bin);
        if !path.is_file() {
            continue;
        }
        let bin_src = read_bin_source(bin);
        let prod = production_bin_source(&bin_src);
        assert!(
            !prod.contains(PUBLISH_CALL_MARKER),
            "{bin} must not publish TOPIC_ARB_TRACK_REQUESTS (I-4e Phase3)"
        );
        assert!(
            !prod.contains(SUBSCRIBE_CALL_MARKER),
            "{bin} must not subscribe to TOPIC_ARB_TRACK_REQUESTS (I-4e Phase3)"
        );
    }

    assert_eq!(
        count_publish_markers(arb_prod),
        1,
        "arb_strategy must have exactly one TOPIC_ARB_TRACK_REQUESTS publish site"
    );
}

/// Phase 3: `ArbTrackRequestsUpdate` JSON roundtrip (Spec sample + topic constant).
#[test]
fn phase3_arb_track_requests_schema_roundtrip() {
    use ironcrab::nats::topics::TOPIC_ARB_TRACK_REQUESTS;
    use ironcrab::nats::{
        ArbTrackActiveEntry, ArbTrackActiveReason, ArbTrackRemovedEntry, ArbTrackRemovedReason,
        ArbTrackRequestsUpdate,
    };

    assert_eq!(TOPIC_ARB_TRACK_REQUESTS, "ironcrab.v1.arb.track_requests");

    let sample_json = r#"{
        "version": 1,
        "ts_unix_ms": 1700000000,
        "active": [
            {
                "pool": "Pool111111111111111111111111111111111111111",
                "reason": "multi_dex"
            }
        ],
        "removed": [
            {
                "pool": "Pool222222222222222222222222222222222222222",
                "reason": "cooldown"
            }
        ],
        "reconcile": true
    }"#;

    let parsed: ArbTrackRequestsUpdate =
        serde_json::from_str(sample_json).expect("deserialize spec sample JSON");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.ts_unix_ms, 1_700_000_000);
    assert!(parsed.reconcile);
    assert_eq!(parsed.active.len(), 1);
    assert_eq!(parsed.removed.len(), 1);
    assert_eq!(parsed.active[0].reason, ArbTrackActiveReason::MultiDex);
    assert_eq!(parsed.removed[0].reason, ArbTrackRemovedReason::Cooldown);

    let roundtrip_json = serde_json::to_string(&parsed).expect("serialize");
    let back: ArbTrackRequestsUpdate =
        serde_json::from_str(&roundtrip_json).expect("deserialize roundtrip");
    assert_eq!(parsed, back);

    let built = ArbTrackRequestsUpdate {
        version: 1,
        ts_unix_ms: 1_700_000_000,
        active: vec![ArbTrackActiveEntry {
            pool: "Pool111111111111111111111111111111111111111".to_string(),
            reason: ArbTrackActiveReason::MultiDex,
        }],
        removed: vec![ArbTrackRemovedEntry {
            pool: "Pool222222222222222222222222222222222222222".to_string(),
            reason: ArbTrackRemovedReason::Cooldown,
        }],
        reconcile: true,
    };
    let built_json = serde_json::to_string(&built).expect("serialize built");
    assert!(built_json.contains("\"reason\":\"multi_dex\""));
    assert!(built_json.contains("\"reason\":\"cooldown\""));
    assert!(built_json.contains("\"reconcile\":true"));

    let minimal_json = r#"{"version":1,"ts_unix_ms":1,"active":[],"removed":[]}"#;
    let minimal: ArbTrackRequestsUpdate = serde_json::from_str(minimal_json).expect("minimal");
    assert!(!minimal.reconcile);
}
