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

use futures::StreamExt;

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
    nats_http_port: u16,
    nats_child: Option<Child>,
    market_data_child: Option<Child>,
    execution_engine_child: Option<Child>,
}

impl RequestReplyE2eHarness {
    /// Erstellt einen neuen Harness mit Temp-Dir und NATS-Config.
    pub fn new() -> Result<Self, String> {
        let temp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {}", e))?;
        let port = free_port()?;
        let http_port = free_port()?;
        let nats_url = format!("nats://127.0.0.1:{}", port);

        let template_path = request_reply_fixtures_dir().join("nats_jetstream.conf");
        let template = fs::read_to_string(&template_path)
            .map_err(|e| format!("read nats template {}: {}", template_path.display(), e))?;
        let config_content = format!("port: {}\nhttp_port: {}\n\n{}", port, http_port, template);
        let nats_config_path = temp_dir.path().join("nats.conf");
        fs::write(&nats_config_path, config_content)
            .map_err(|e| format!("write nats config: {}", e))?;

        Ok(Self {
            _temp_dir: temp_dir,
            nats_config_path,
            nats_url,
            nats_http_port: http_port,
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

    /// HTTP-Port des NATS-Monitorings (für /connz).
    pub fn nats_http_port(&self) -> u16 {
        self.nats_http_port
    }

    /// Temp-Dir-Pfad (für Tests).
    pub fn temp_dir_path(&self) -> &Path {
        self._temp_dir.path()
    }
}

/// Prüft, dass market-data und execution-engine tatsächlich mit NATS verbunden sind.
/// Nutzt NATS-Monitoring /connz (Blackbox: öffentlich beobachtbar).
fn verify_process_connections(http_port: u16, min_connections: u32) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/connz", http_port);
    let body = reqwest::blocking::get(&url)
        .map_err(|e| format!("connz GET: {}", e))?
        .error_for_status()
        .map_err(|e| format!("connz status: {}", e))?
        .text()
        .map_err(|e| format!("connz text: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("connz parse: {}", e))?;
    let num = json
        .get("num_connections")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            json.get("connections")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
                .unwrap_or(0)
        });
    if num < min_connections as u64 {
        return Err(format!(
            "connz: {} Verbindungen, mindestens {} erwartet (market-data + execution-engine)",
            num, min_connections
        ));
    }
    Ok(())
}

/// Echte Readiness-Prüfung: Prozess-Verbindungen + NATS Pub/Sub funktioniert.
/// Beweist, dass der Harness für spätere Request/Reply-Tests brauchbar ist.
fn verify_harness_readiness(nats_url: &str, http_port: u16) -> Result<(), String> {
    // Zuerst: market-data und execution-engine müssen verbunden sein
    verify_process_connections(http_port, 2)?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("runtime: {}", e))?;
    rt.block_on(async {
        let client = async_nats::connect(nats_url)
            .await
            .map_err(|e| format!("connect: {}", e))?;
        let mut sub = client
            .subscribe("harness.readiness.test".to_string())
            .await
            .map_err(|e| format!("subscribe: {}", e))?;
        client
            .publish("harness.readiness.test".to_string(), "ping".into())
            .await
            .map_err(|e| format!("publish: {}", e))?;
        let msg = tokio::time::timeout(Duration::from_secs(2), sub.next())
            .await
            .map_err(|_| "timeout: keine Nachricht empfangen")?
            .ok_or("stream ended")?;
        if msg.payload.as_ref() != b"ping" {
            return Err(format!(
                "falsche payload: {:?}",
                String::from_utf8_lossy(msg.payload.as_ref())
            ));
        }
        Ok(())
    })
}

impl Drop for RequestReplyE2eHarness {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Fehlertyp: nur "nats-server nicht gefunden" ist ein Skip-Grund.
const SKIP_REASON: &str = "nats-server nicht gefunden";

/// Minimaler Harness-Test: NATS starten, Verbindung prüfen, Cleanup.
fn run_harness_nats_only() -> Result<(), String> {
    find_nats_server().ok_or_else(|| SKIP_REASON.to_string())?;

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

    std::thread::sleep(Duration::from_secs(2));

    // Echte Readiness: Prozesse laufen UND NATS Pub/Sub funktioniert
    assert!(
        harness.processes_are_running(),
        "NATS, market-data oder execution-engine sind bereits beendet"
    );
    verify_harness_readiness(harness.nats_url(), harness.nats_http_port())
        .expect("Harness-Readiness: Prozess-Verbindungen + NATS Pub/Sub müssen funktionieren");

    harness.stop();
}

/// Test: Nur NATS (läuft wenn nats-server installiert ist).
/// Skip nur wenn nats-server nicht gefunden; alle anderen Fehler schlagen fehl.
#[test]
fn request_reply_harness_nats_starts() {
    match run_harness_nats_only() {
        Ok(()) => {}
        Err(e) if e == SKIP_REASON => eprintln!("SKIP: {}", e),
        Err(e) => panic!("NATS-Start/Config/Verbindung fehlgeschlagen: {}", e),
    }
}

/// Test: Harness-Infrastruktur (Temp-Dir, Config) – läuft immer.
#[test]
fn request_reply_harness_new_creates_temp_dir() {
    let harness = RequestReplyE2eHarness::new().expect("harness new");
    assert!(harness.temp_dir_path().exists(), "temp dir muss existieren");
}
