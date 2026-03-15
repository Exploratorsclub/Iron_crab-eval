//! Request/Reply E2E Harness (I-24c, I-24d)
//!
//! Wiederverwendbare Blackbox-Infrastruktur für Multi-Prozess-E2E-Tests:
//! - NATS/JetStream-Kontext starten
//! - market-data und execution-engine als echte Prozesse starten/stoppen
//!
//! Kein Lesen von Iron_crab/src/ oder Iron_crab/tests/.
//! Keine Assertions auf Implementierungsdetails.

use std::fs;
use std::io::Read;
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

/// Maximale Wartezeit für /status-Readiness (Sekunden).
const STATUS_POLL_TIMEOUT_SECS: u64 = 60;
/// Intervall zwischen /status-Polls (Millisekunden).
const STATUS_POLL_INTERVAL_MS: u64 = 300;

/// Sucht das vorab gebaute Binary (reduziert Startlatenz vs. cargo run).
fn find_binary(name: &str) -> Option<PathBuf> {
    let path = iron_crab_root().join("target").join("debug").join(name);
    if path.exists() {
        Some(path)
    } else {
        None
    }
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
    market_data_metrics_port: u16,
    execution_engine_metrics_port: u16,
    nats_child: Option<Child>,
    market_data_child: Option<Child>,
    execution_engine_child: Option<Child>,
}

impl RequestReplyE2eHarness {
    /// Erstellt einen neuen Harness mit Temp-Dir und NATS-Config.
    pub fn new() -> Result<Self, String> {
        let temp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {}", e))?;
        let port = free_port()?;
        let md_port = free_port()?;
        let ee_port = free_port()?;
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
            market_data_metrics_port: md_port,
            execution_engine_metrics_port: ee_port,
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
        let log_dir = self._temp_dir.path().join("market_data_log");
        fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {}", e))?;

        let args = [
            "--nats-url",
            &self.nats_url,
            "--simulate",
            "--metrics-port",
            &self.market_data_metrics_port.to_string(),
            "--log-dir",
            log_dir.to_str().unwrap(),
        ];

        let child = if let Some(bin) = find_binary("market-data") {
            Command::new(bin)
                .args(args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
        } else {
            let manifest = iron_crab_root().join("Cargo.toml");
            if !manifest.exists() {
                return Err(format!("Iron_crab nicht gefunden: {:?}", iron_crab_root()));
            }
            Command::new("cargo")
                .args([
                    "run",
                    "--manifest-path",
                    manifest.to_str().unwrap(),
                    "--bin",
                    "market-data",
                    "--",
                ])
                .args(args)
                .current_dir(iron_crab_root().parent().unwrap())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
        }
        .map_err(|e| format!("market-data spawn: {}", e))?;

        self.market_data_child = Some(child);
        wait_for_component_status(
            self.market_data_metrics_port,
            "market-data",
            &mut self.market_data_child,
        )?;
        Ok(())
    }

    /// Startet execution-engine (ohne Keys; dry-run).
    pub fn start_execution_engine(&mut self) -> Result<(), String> {
        let log_dir = self._temp_dir.path().join("execution_engine_log");
        fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {}", e))?;

        let args = [
            "--nats-url",
            &self.nats_url,
            "--dry-run",
            "--metrics-port",
            &self.execution_engine_metrics_port.to_string(),
            "--log-dir",
            log_dir.to_str().unwrap(),
        ];

        let child = if let Some(bin) = find_binary("execution-engine") {
            Command::new(bin)
                .args(args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
        } else {
            let manifest = iron_crab_root().join("Cargo.toml");
            if !manifest.exists() {
                return Err(format!("Iron_crab nicht gefunden: {:?}", iron_crab_root()));
            }
            Command::new("cargo")
                .args([
                    "run",
                    "--manifest-path",
                    manifest.to_str().unwrap(),
                    "--bin",
                    "execution-engine",
                    "--",
                ])
                .args(args)
                .current_dir(iron_crab_root().parent().unwrap())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
        }
        .map_err(|e| format!("execution-engine spawn: {}", e))?;

        self.execution_engine_child = Some(child);
        wait_for_component_status(
            self.execution_engine_metrics_port,
            "execution-engine",
            &mut self.execution_engine_child,
        )?;
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

    /// Metrics-Port von market-data (für /status).
    pub fn market_data_metrics_port(&self) -> u16 {
        self.market_data_metrics_port
    }

    /// Metrics-Port von execution-engine (für /status).
    pub fn execution_engine_metrics_port(&self) -> u16 {
        self.execution_engine_metrics_port
    }

    /// Temp-Dir-Pfad (für Tests).
    pub fn temp_dir_path(&self) -> &Path {
        self._temp_dir.path()
    }
}

/// Wartet gebunden auf /status (HTTP 200). Bei Timeout: diagnostische Infos.
fn wait_for_component_status(
    port: u16,
    component: &str,
    child: &mut Option<Child>,
) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/status", port);
    let deadline = std::time::Instant::now() + Duration::from_secs(STATUS_POLL_TIMEOUT_SECS);
    while std::time::Instant::now() < deadline {
        if let Ok(resp) = reqwest::blocking::get(&url) {
            if resp.status().is_success() {
                let body = resp.text().unwrap_or_default();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(ready) = json.get("ready").and_then(|v| v.as_bool()) {
                        if !ready {
                            std::thread::sleep(Duration::from_millis(STATUS_POLL_INTERVAL_MS));
                            continue;
                        }
                    }
                }
                return Ok(());
            }
        }
        if let Some(c) = child {
            if c.try_wait().ok().flatten().is_some() {
                return Err(format!(
                    "{} vorzeitig beendet: {}",
                    component,
                    format_process_diagnostics(c)
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(STATUS_POLL_INTERVAL_MS));
    }
    let diag = child
        .as_mut()
        .map(format_process_diagnostics)
        .unwrap_or_else(|| "Prozess-Handle nicht verfügbar".to_string());
    Err(format!(
        "{} /status nach {}s nicht erreichbar. {}",
        component, STATUS_POLL_TIMEOUT_SECS, diag
    ))
}

fn read_pipe_excerpt(mut r: impl Read) -> String {
    let mut buf = Vec::new();
    let _ = r.read_to_end(&mut buf);
    let s = String::from_utf8_lossy(&buf);
    s.chars()
        .rev()
        .take(1500)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

/// Sammelt Blackbox-Diagnostik aus Prozess (stdout/stderr, Exit-Status).
fn format_process_diagnostics(child: &mut Child) -> String {
    let mut parts = Vec::new();
    if let Ok(Some(status)) = child.try_wait() {
        parts.push(format!("exit={:?}", status));
    } else {
        parts.push("Prozess läuft noch".to_string());
    }
    if let Some(p) = child.stdout.take() {
        let s = read_pipe_excerpt(p);
        if !s.trim().is_empty() {
            parts.push(format!("stdout (letzte 1500 Zeichen):\n{}", s));
        }
    }
    if let Some(p) = child.stderr.take() {
        let s = read_pipe_excerpt(p);
        if !s.trim().is_empty() {
            parts.push(format!("stderr (letzte 1500 Zeichen):\n{}", s));
        }
    }
    parts.join(". ")
}

/// Prüft die öffentliche /status-Readiness eines Binaries (Blackbox).
/// GET /status muss 200 liefern; optional: JSON mit ready=true.
fn verify_component_status(port: u16, component: &str) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/status", port);
    let resp =
        reqwest::blocking::get(&url).map_err(|e| format!("{} /status GET: {}", component, e))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{} /status: {} (erwartet 2xx)", component, status));
    }
    let body = resp
        .text()
        .map_err(|e| format!("{} /status text: {}", component, e))?;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(ready) = json.get("ready").and_then(|v| v.as_bool()) {
            if !ready {
                return Err(format!("{} /status: ready=false", component));
            }
        }
    }
    Ok(())
}

/// Echte Readiness-Prüfung: /status beider Prozesse + NATS Pub/Sub funktioniert.
/// Beweist, dass der Harness für spätere Request/Reply-Tests brauchbar ist.
fn verify_harness_readiness(nats_url: &str, md_port: u16, ee_port: u16) -> Result<(), String> {
    verify_component_status(md_port, "market-data")?;
    verify_component_status(ee_port, "execution-engine")?;

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

    // Readiness bereits via /status-Poll in start_* sichergestellt
    assert!(
        harness.processes_are_running(),
        "NATS, market-data oder execution-engine sind bereits beendet"
    );
    verify_harness_readiness(
        harness.nats_url(),
        harness.market_data_metrics_port(),
        harness.execution_engine_metrics_port(),
    )
    .expect("Harness-Readiness: /status beider Prozesse + NATS Pub/Sub müssen funktionieren");

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
