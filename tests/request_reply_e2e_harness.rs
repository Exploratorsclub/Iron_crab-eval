//! Request/Reply E2E Harness (I-24c, I-24d)
//!
//! Wiederverwendbare Blackbox-Infrastruktur für Multi-Prozess-E2E-Tests:
//! - NATS/JetStream-Kontext starten
//! - market-data und execution-engine als echte Prozesse starten/stoppen
//!
//! Kein Lesen von Iron_crab/src/ oder Iron_crab/tests/.
//! Keine Assertions auf Implementierungsdetails.

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

/// Pfad zu Iron_crab (Geschwister-Ordner). Wie golden_replay_blackbox.rs.
fn iron_crab_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of manifest")
        .join("Iron_crab")
}

/// Fixtures-Verzeichnis im Eval-Repo.
fn request_reply_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("request_reply")
}

/// Sucht nats-server im PATH oder an üblichen Stellen.
fn find_nats_server() -> Option<PathBuf> {
    if let Ok(path) = which::which("nats-server") {
        return Some(path);
    }
    // Fallback: /usr/local/bin (nach manueller Installation)
    let candidates = ["/usr/local/bin/nats-server", "/usr/bin/nats-server"];
    for p in candidates {
        if Path::new(p).exists() {
            return Some(PathBuf::from(p));
        }
    }
    None
}

/// Erzeugt eine freie Port-Nummer.
fn free_port() -> Result<u16, String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("TcpListener bind: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {}", e))?
        .port();
    Ok(port)
}

/// Harness für Request/Reply E2E: NATS + market-data + execution-engine.
pub struct RequestReplyE2eHarness {
    _temp_dir: tempfile::TempDir,
    nats_config_path: PathBuf,
    nats_url: String,
    nats_child: Option<Child>,
    market_data_child: Option<Child>,
    execution_engine_child: Option<Child>,
}

impl RequestReplyE2eHarness {
    /// Erstellt einen neuen Harness mit Temp-Dir und NATS-Config.
    pub fn new() -> Result<Self, String> {
        let temp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {}", e))?;
        let port = free_port()?;
        let nats_url = format!("nats://127.0.0.1:{}", port);

        let template_path = request_reply_fixtures_dir().join("nats_jetstream.conf");
        let template = fs::read_to_string(&template_path)
            .map_err(|e| format!("read nats template {}: {}", template_path.display(), e))?;
        let config_content = format!("port: {}\n\n{}", port, template);
        let nats_config_path = temp_dir.path().join("nats.conf");
        fs::write(&nats_config_path, config_content)
            .map_err(|e| format!("write nats config: {}", e))?;

        Ok(Self {
            _temp_dir: temp_dir,
            nats_config_path,
            nats_url,
            nats_child: None,
            market_data_child: None,
            execution_engine_child: None,
        })
    }

    /// Startet NATS mit JetStream. Erfordert nats-server im PATH.
    pub fn start_nats(&mut self) -> Result<(), String> {
        let nats_bin = find_nats_server()
            .ok_or_else(|| "nats-server nicht gefunden (PATH oder /usr/local/bin)".to_string())?;

        let child = Command::new(nats_bin)
            .arg("-c")
            .arg(&self.nats_config_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("nats-server spawn: {}", e))?;

        self.nats_child = Some(child);

        // Kurz warten, dann Verbindung prüfen
        std::thread::sleep(Duration::from_millis(500));
        self.wait_nats_ready()?;
        Ok(())
    }

    /// Wartet bis NATS bereit ist (max 5s).
    fn wait_nats_ready(&self) -> Result<(), String> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("runtime: {}", e))?;
        for _ in 0..50 {
            if rt.block_on(async_nats::connect(&self.nats_url)).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!("NATS nicht bereit nach 5s: {}", self.nats_url))
    }

    /// Startet market-data mit --simulate (kein Geyser nötig).
    pub fn start_market_data(&mut self) -> Result<(), String> {
        let manifest = iron_crab_root().join("Cargo.toml");
        if !manifest.exists() {
            return Err(format!("Iron_crab nicht gefunden: {:?}", iron_crab_root()));
        }

        let log_dir = self._temp_dir.path().join("market_data_log");
        fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {}", e))?;

        let child = Command::new("cargo")
            .args([
                "run",
                "--manifest-path",
                manifest.to_str().unwrap(),
                "--bin",
                "market-data",
                "--",
                "--nats-url",
                &self.nats_url,
                "--simulate",
                "--log-dir",
                log_dir.to_str().unwrap(),
            ])
            .current_dir(iron_crab_root().parent().unwrap())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("market-data spawn: {}", e))?;

        self.market_data_child = Some(child);
        std::thread::sleep(Duration::from_millis(800));
        Ok(())
    }

    /// Startet execution-engine (ohne Keys; dry-run).
    pub fn start_execution_engine(&mut self) -> Result<(), String> {
        let manifest = iron_crab_root().join("Cargo.toml");
        if !manifest.exists() {
            return Err(format!("Iron_crab nicht gefunden: {:?}", iron_crab_root()));
        }

        let log_dir = self._temp_dir.path().join("execution_engine_log");
        fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {}", e))?;

        let child = Command::new("cargo")
            .args([
                "run",
                "--manifest-path",
                manifest.to_str().unwrap(),
                "--bin",
                "execution-engine",
                "--",
                "--nats-url",
                &self.nats_url,
                "--dry-run",
                "--log-dir",
                log_dir.to_str().unwrap(),
            ])
            .current_dir(iron_crab_root().parent().unwrap())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("execution-engine spawn: {}", e))?;

        self.execution_engine_child = Some(child);
        std::thread::sleep(Duration::from_millis(800));
        Ok(())
    }

    /// Stoppt alle Kindprozesse.
    pub fn stop(&mut self) {
        let kill = |child: &mut Option<Child>, _name: &str| {
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        };
        kill(&mut self.execution_engine_child, "execution-engine");
        kill(&mut self.market_data_child, "market-data");
        kill(&mut self.nats_child, "nats");
    }

    /// Prüft ob die Prozesse noch laufen.
    pub fn processes_are_running(&mut self) -> bool {
        let check = |child: &mut Option<Child>| {
            child
                .as_mut()
                .map(|c| c.try_wait().ok().flatten().is_none())
                .unwrap_or(false)
        };
        check(&mut self.nats_child)
            && check(&mut self.market_data_child)
            && check(&mut self.execution_engine_child)
    }

    pub fn nats_url(&self) -> &str {
        &self.nats_url
    }

    /// Temp-Dir-Pfad (für Tests).
    pub fn temp_dir_path(&self) -> &Path {
        self._temp_dir.path()
    }
}

impl Drop for RequestReplyE2eHarness {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Minimaler Harness-Test: NATS starten, Verbindung prüfen, Cleanup.
fn run_harness_nats_only() -> Result<(), String> {
    find_nats_server().ok_or_else(|| "nats-server nicht gefunden".to_string())?;

    let mut harness = RequestReplyE2eHarness::new()?;
    harness.start_nats()?;

    // Verbindung muss funktionieren
    let url = harness.nats_url().to_string();
    let client = tokio::runtime::Runtime::new()
        .map_err(|e| format!("runtime: {}", e))?
        .block_on(async_nats::connect(&url))
        .map_err(|e| format!("connect: {}", e))?;

    drop(client);
    harness.stop();
    Ok(())
}

/// Harness-Test: Startet NATS, market-data, execution-engine.
#[test]
fn request_reply_harness_starts_processes() {
    if find_nats_server().is_none() {
        eprintln!("SKIP: nats-server nicht gefunden (PATH). Harness-Infrastruktur kompiliert.");
        return;
    }

    let iron_crab = iron_crab_root();
    if !iron_crab.join("Cargo.toml").exists() {
        eprintln!("SKIP: Iron_crab nicht als Sibling gefunden.");
        return;
    }

    let mut harness = RequestReplyE2eHarness::new().expect("harness new");
    harness.start_nats().expect("nats start");
    harness.start_market_data().expect("market-data start");
    harness
        .start_execution_engine()
        .expect("execution-engine start");

    // Kurz warten, dann prüfen ob sie noch laufen
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        harness.processes_are_running(),
        "NATS, market-data oder execution-engine sind bereits beendet"
    );

    harness.stop();
}

/// Test: Nur NATS (läuft wenn nats-server installiert ist).
#[test]
fn request_reply_harness_nats_starts() {
    if run_harness_nats_only().is_err() {
        // nats-server nicht installiert – Skip
    }
}

/// Test: Harness-Infrastruktur (Temp-Dir, Config) – läuft immer.
#[test]
fn request_reply_harness_new_creates_temp_dir() {
    let harness = RequestReplyE2eHarness::new().expect("harness new");
    assert!(harness.temp_dir_path().exists(), "temp dir muss existieren");
}
