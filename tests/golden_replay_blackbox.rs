//! Golden Replay Blackbox Test (INVARIANTS.md A.10)
//!
//! Spawnt die execution-engine mit --replay und vergleicht die Output-DecisionRecords
//! gegen die erwarteten Fixtures. Validiert Replay-Determinismus der echten Engine.
//!
//! Hinweis: Die Engine schreibt wegen täglicher Rotation in {stem}-YYYYMMDD.jsonl,
//! nicht direkt in den übergebenen Pfad. find_replay_output_file() ermittelt die tatsächliche Datei.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pfad zu Iron_crab (Geschwister-Ordner von ironcrab-eval)
fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

/// Pfad zu den Intents-Fixtures (in Iron_crab)
fn intents_fixtures_dir() -> PathBuf {
    iron_crab_root()
        .join("tests")
        .join("fixtures")
        .join("golden_replays")
}

/// Pfad zu den Eval-Expected-Fixtures (in ironcrab-eval, wir kontrollieren diese)
fn expected_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden_replays")
}

/// Spawnt execution-engine mit --replay und gibt den Output-Pfad zurück.
fn run_replay(intents_path: &Path, output_path: &Path) -> Result<(), String> {
    let manifest = iron_crab_root().join("Cargo.toml");
    if !manifest.exists() {
        return Err(format!("Iron_crab not found at {:?}", iron_crab_root()));
    }

    let status = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            manifest.to_str().unwrap(),
            "--bin",
            "execution-engine",
            "--",
            "--replay",
            "--replay-intents",
            intents_path.to_str().unwrap(),
            "--replay-output",
            output_path.to_str().unwrap(),
        ])
        .current_dir(iron_crab_root().parent().unwrap())
        .status()
        .map_err(|e| format!("Failed to run execution-engine: {}", e))?;

    if !status.success() {
        return Err(format!("execution-engine --replay exited with {}", status));
    }
    Ok(())
}

/// Ermittelt die tatsächliche Output-Datei. Die Engine schreibt wegen täglicher Rotation
/// in {stem}-YYYYMMDD.jsonl, nicht in den exakten übergebenen Pfad.
fn find_replay_output_file(output_path: &Path) -> Result<PathBuf, String> {
    let log_dir = output_path.parent().ok_or("output_path has no parent")?;
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("golden_decisions");
    let prefix = format!("{}-", stem);
    for e in fs::read_dir(log_dir).map_err(|e| format!("read_dir {}: {}", log_dir.display(), e))? {
        let e = e.map_err(|e| format!("read_dir entry: {}", e))?;
        let p = e.path();
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(&prefix) && name.ends_with(".jsonl") {
                return Ok(p);
            }
        }
    }
    Err(format!(
        "No replay output file matching {}-*.jsonl in {}",
        stem,
        log_dir.display()
    ))
}

/// Minimales Decision-Format für Vergleich (intent_id, outcome, primary_reject_reason, checks)
#[derive(Debug, serde::Deserialize)]
struct GoldenDecisionCompare {
    intent_id: String,
    outcome: String,
    #[serde(default)]
    primary_reject_reason: Option<String>,
    checks: Vec<GoldenCheckCompare>,
}

#[derive(Debug, serde::Deserialize)]
struct GoldenCheckCompare {
    check_name: String,
    passed: bool,
    #[serde(default)]
    reason_code: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    details: Option<String>,
}

fn load_decisions(path: &Path) -> Result<Vec<GoldenDecisionCompare>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("Open {}: {}", path.display(), e))?;
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read line: {}", e))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let dec: GoldenDecisionCompare =
            serde_json::from_str(line).map_err(|e| format!("Parse decision: {} - {}", e, line))?;
        out.push(dec);
    }
    Ok(out)
}

fn assert_decisions_match(actual: &[GoldenDecisionCompare], expected: &[GoldenDecisionCompare]) {
    assert_eq!(actual.len(), expected.len(), "Decision count mismatch");
    for (a, e) in actual.iter().zip(expected.iter()) {
        assert_eq!(a.intent_id, e.intent_id, "intent_id mismatch");
        assert_eq!(a.outcome, e.outcome, "outcome mismatch for {}", a.intent_id);
        assert_eq!(
            a.primary_reject_reason, e.primary_reject_reason,
            "primary_reject_reason mismatch for {}",
            a.intent_id
        );
        assert_eq!(
            a.checks.len(),
            e.checks.len(),
            "check count mismatch for {}",
            a.intent_id
        );
        for (ac, ec) in a.checks.iter().zip(e.checks.iter()) {
            assert_eq!(ac.check_name, ec.check_name, "check name mismatch");
            assert_eq!(
                ac.passed, ec.passed,
                "check {} pass/fail mismatch for {}",
                ac.check_name, a.intent_id
            );
            assert_eq!(
                ac.reason_code, ec.reason_code,
                "check {} reason mismatch for {}",
                ac.check_name, a.intent_id
            );
        }
    }
}

/// rejected_trade-Fixture: Intents werden an Risk-Checks abgelehnt.
#[test]
fn golden_replay_rejected_trade() {
    let intents = intents_fixtures_dir().join("rejected_trade_intents.jsonl");
    let expected_path = expected_fixtures_dir().join("rejected_trade_expected.jsonl");

    let tmp = tempfile::tempdir().expect("temp dir");
    let output_path = tmp.path().join("replay_decisions.jsonl");
    run_replay(&intents, &output_path).expect("replay run failed");

    let actual_path = find_replay_output_file(&output_path).expect("find replay output");
    let actual = load_decisions(&actual_path).expect("load actual");
    let expected = load_decisions(&expected_path).expect("load expected");
    assert_decisions_match(&actual, &expected);
}

/// sim_failed-Fixture: Intent passiert alle Checks, Simulation schlägt fehl.
#[test]
fn golden_replay_sim_failed() {
    let intents = intents_fixtures_dir().join("sim_failed_intents.jsonl");
    let expected_path = expected_fixtures_dir().join("sim_failed_expected.jsonl");

    let tmp = tempfile::tempdir().expect("temp dir");
    let output_path = tmp.path().join("replay_decisions.jsonl");
    run_replay(&intents, &output_path).expect("replay run failed");

    let actual_path = find_replay_output_file(&output_path).expect("find replay output");
    let actual = load_decisions(&actual_path).expect("load actual");
    let expected = load_decisions(&expected_path).expect("load expected");
    assert_decisions_match(&actual, &expected);
}
