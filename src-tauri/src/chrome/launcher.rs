// Chrome CDP Launcher
//
// Two connection modes:
//   HTTP mode  – Chrome launched with --remote-debugging-port
//                (full HTTP /json/* debug API on localhost:PORT)
//   WS mode    – Chrome with the "Allow remote debugging" toggle in
//                chrome://inspect/#remote-debugging (Chrome 144+)
//                Chrome only exposes a single browser WebSocket endpoint.
//
// WS-mode design: we maintain ONE persistent WebSocket connection to the
// browser endpoint (opened once when the user grants permission) and
// multiplex every CDP command through it via a background task + channels.
// This means Chrome's permission dialog is shown exactly ONCE per session.

use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use tokio::sync::{mpsc, oneshot, Semaphore};
use std::sync::Arc;

// ── Global state ──────────────────────────────────────────────────────────

static CDP_CHROME_PID: Mutex<Option<u32>> = Mutex::new(None);
static CDP_PORT: AtomicU16 = AtomicU16::new(9222);
/// Monotonically-increasing message ID for CDP commands.
static MSG_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
enum ChromeSource {
    UserChrome,
    AppChrome,
}

lazy_static::lazy_static! {
    static ref LAUNCH_LOCK: Arc<Semaphore> = Arc::new(Semaphore::new(1));
    static ref CHROME_SOURCE: Mutex<Option<ChromeSource>> = Mutex::new(None);
    /// Held when we are in WS mode. None = HTTP mode or not connected.
    static ref WS_HANDLE: Mutex<Option<WsBrowserHandle>> = Mutex::new(None);
}

// ── Public API ────────────────────────────────────────────────────────────

pub fn get_cdp_port() -> u16 {
    CDP_PORT.load(Ordering::Relaxed)
}

/// Returns a clone of the persistent WS handle (WS mode only).
pub fn get_ws_browser_handle() -> Option<WsBrowserHandle> {
    WS_HANDLE.lock().unwrap().clone()
}

// ── Persistent WS connection ──────────────────────────────────────────────

/// A cheap, cloneable handle to the persistent browser WebSocket connection.
/// All CDP commands flow through a single background task that owns the WS stream.
#[derive(Clone)]
pub struct WsBrowserHandle {
    cmd_tx: mpsc::UnboundedSender<BrowserCmd>,
}

struct BrowserCmd {
    id: u64,
    message: String,
    reply: oneshot::Sender<Result<serde_json::Value, String>>,
}

impl WsBrowserHandle {
    /// Send a top-level CDP command (no sessionId).
    pub async fn send(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_raw(method, None, params).await
    }

    /// Send a session-scoped CDP command (flat protocol, requires sessionId).
    pub async fn send_session(
        &self,
        method: &str,
        session_id: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.send_raw(method, Some(session_id), params).await
    }

    async fn send_raw(
        &self,
        method: &str,
        session_id: Option<&str>,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = MSG_ID.fetch_add(1, Ordering::Relaxed);
        let mut msg = serde_json::json!({
            "id": id,
            "method": method,
            "params": params
        });
        if let Some(sid) = session_id {
            msg["sessionId"] = serde_json::Value::String(sid.to_string());
        }

        let (reply_tx, reply_rx) = oneshot::channel();
        self.cmd_tx
            .send(BrowserCmd {
                id,
                message: msg.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| "WS handle channel closed".to_string())?;

        tokio::time::timeout(std::time::Duration::from_secs(30), reply_rx)
            .await
            .map_err(|_| "CDP command timed out after 30s".to_string())?
            .map_err(|_| "Reply channel dropped".to_string())?
    }
}

/// Connect to the browser WS endpoint, spawn a background task that keeps
/// the connection alive, and return a handle to multiplex commands through it.
///
/// The caller is responsible for a timeout around this call — Chrome will
/// accept the WS connection but hold CDP responses until the user clicks
/// "Allow" in the permission dialog.
async fn spawn_ws_browser_connection(url: &str) -> Result<WsBrowserHandle, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    let (ws, _) = connect_async(url)
        .await
        .map_err(|e| format!("WS connect failed: {}", e))?;

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<BrowserCmd>();
    let handle = WsBrowserHandle { cmd_tx };

    tokio::spawn(async move {
        let (mut write, mut read) = ws.split();
        let mut pending: HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>> =
            HashMap::new();

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(BrowserCmd { id, message, reply }) => {
                            pending.insert(id, reply);
                            if write.send(Message::Text(message)).await.is_err() {
                                break;
                            }
                        }
                        None => break, // All handles dropped
                    }
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(resp) =
                                serde_json::from_str::<serde_json::Value>(&text)
                            {
                                // Route response to the waiting caller by id.
                                // CDP events have no "id" field — they are ignored.
                                if let Some(id) = resp["id"].as_u64() {
                                    if let Some(tx) = pending.remove(&id) {
                                        let _ = tx.send(Ok(resp));
                                    }
                                }
                            }
                        }
                        None
                        | Some(Err(_))
                        | Some(Ok(Message::Close(_))) => {
                            println!("🔌 WS browser connection closed");
                            for (_, tx) in pending.drain() {
                                let _ = tx.send(Err("WS connection closed".to_string()));
                            }
                            break;
                        }
                        _ => {} // Binary frames etc.
                    }
                }
            }
        }

        println!("🔌 WS browser task ended — clearing connection state");
        *WS_HANDLE.lock().unwrap() = None;
        *CHROME_SOURCE.lock().unwrap() = None;
    });

    Ok(handle)
}

// ── Accessibility checks ──────────────────────────────────────────────────

async fn is_http_cdp_accessible(port: u16) -> bool {
    let url = format!("http://localhost:{}/json/list", port);
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    else {
        return false;
    };
    match client.get(&url).send().await {
        Ok(resp) => {
            if let Ok(tabs) = resp.json::<Vec<serde_json::Value>>().await {
                tabs.iter().any(|t| t["type"].as_str() == Some("page"))
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// TCP-level port check — does NOT open a WebSocket, never triggers a Chrome
/// permission dialog. Used for polling in WS mode.
async fn is_port_open(port: u16) -> bool {
    tokio::net::TcpStream::connect(std::net::SocketAddr::from(([127, 0, 0, 1], port)))
        .await
        .is_ok()
}

/// Returns true if CDP is usable in whichever mode is currently active.
///
/// WS mode: cheap TCP check only — never opens a new WS connection.
/// HTTP mode: full /json/list check.
pub async fn is_cdp_accessible() -> bool {
    if get_ws_browser_handle().is_some() {
        return is_port_open(get_cdp_port()).await;
    }
    is_http_cdp_accessible(get_cdp_port()).await
}

// ── User Chrome discovery ─────────────────────────────────────────────────

fn detect_user_chrome_port() -> Option<(u16, String)> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        let candidates = [
            format!("{}/Library/Application Support/Google/Chrome", home),
            format!("{}/Library/Application Support/Google/Chrome Beta", home),
            format!("{}/Library/Application Support/Google/Chrome Dev", home),
            format!("{}/Library/Application Support/Google/Chrome Canary", home),
            format!("{}/Library/Application Support/Chromium", home),
        ];
        for dir in &candidates {
            let path = std::path::PathBuf::from(dir).join("DevToolsActivePort");
            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut lines = content.lines();
                if let (Some(p), Some(ws)) = (lines.next(), lines.next()) {
                    if let Ok(port) = p.trim().parse::<u16>() {
                        println!(
                            "🔍 DevToolsActivePort: port={} path={} ({})",
                            port,
                            ws.trim(),
                            dir
                        );
                        return Some((port, ws.trim().to_string()));
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let up = std::env::var("USERPROFILE").ok()?;
        let candidates = [
            format!("{}\\AppData\\Local\\Google\\Chrome\\User Data", up),
            format!("{}\\AppData\\Local\\Chromium\\User Data", up),
        ];
        for dir in &candidates {
            let path = std::path::PathBuf::from(dir).join("DevToolsActivePort");
            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut lines = content.lines();
                if let (Some(p), Some(ws)) = (lines.next(), lines.next()) {
                    if let Ok(port) = p.trim().parse::<u16>() {
                        return Some((port, ws.trim().to_string()));
                    }
                }
            }
        }
    }

    None
}

// ── Main entry point ──────────────────────────────────────────────────────

pub async fn launch_chrome_cdp_window() -> Result<String, String> {
    let _permit = LAUNCH_LOCK.acquire().await.unwrap();

    // ── Already connected? ───────────────────────────────────────────────
    if is_cdp_accessible().await {
        let source = CHROME_SOURCE.lock().unwrap();
        return match *source {
            Some(ChromeSource::UserChrome) => {
                let mode = if get_ws_browser_handle().is_some() { "WS" } else { "HTTP" };
                Ok(format!("Connected to your Chrome (port {}, {})", get_cdp_port(), mode))
            }
            _ => Ok("Chrome CDP is already running".to_string()),
        };
    }

    // ── Try the user's existing Chrome ───────────────────────────────────
    println!("🔍 Checking for user's Chrome with remote debugging enabled…");

    if let Some((user_port, ws_path)) = detect_user_chrome_port() {
        // Try HTTP mode first (Chrome launched with --remote-debugging-port)
        if is_http_cdp_accessible(user_port).await {
            println!("✅ Connected to user's Chrome via HTTP on port {}", user_port);
            CDP_PORT.store(user_port, Ordering::Relaxed);
            *WS_HANDLE.lock().unwrap() = None;
            *CHROME_SOURCE.lock().unwrap() = Some(ChromeSource::UserChrome);
            return Ok(format!("Connected to your Chrome! (port {})", user_port));
        }

        // Check whether the port is actually open before attempting WebSocket.
        // If it's closed, the DevToolsActivePort file is stale (Chrome is not
        // running or remote debugging is disabled) — fall through to launch a
        // fresh Chrome window instead of showing an error.
        if !is_port_open(user_port).await {
            println!(
                "ℹ️  DevToolsActivePort found (port {}) but port is closed — \
                 Chrome not running or remote debugging not enabled. \
                 Launching new Chrome window…",
                user_port
            );
            // fall through to launch new Chrome
        } else {
            // Port IS open → Chrome is running with remote debugging enabled.
            // Connect via WebSocket (chrome://inspect toggle / WS-only mode).
            // Chrome will show ONE permission dialog; we wait up to 20 s.
            let ws_url = format!("ws://localhost:{}{}", user_port, ws_path);
            println!(
                "🔌 Port {} open, trying persistent WS connection…\n   \
                 ⚠️  Chrome will show a permission dialog — click Allow in Chrome.",
                user_port
            );

            match tokio::time::timeout(
                std::time::Duration::from_secs(20),
                try_ws_connect_and_verify(&ws_url),
            )
            .await
            {
                Ok(Ok(handle)) => {
                    println!("✅ Persistent WS connection established to user's Chrome");
                    CDP_PORT.store(user_port, Ordering::Relaxed);
                    *WS_HANDLE.lock().unwrap() = Some(handle);
                    *CHROME_SOURCE.lock().unwrap() = Some(ChromeSource::UserChrome);
                    return Ok(format!(
                        "Connected to your Chrome! (port {}, WS mode)",
                        user_port
                    ));
                }
                Ok(Err(e)) => {
                    // WS error despite open port (e.g. wrong WS path after browser
                    // restart) — fall through and launch a fresh Chrome window.
                    println!("⚠️  WS connect error: {} — launching new Chrome…", e);
                    // fall through
                }
                Err(_) => {
                    // Timeout: port is open, Chrome is running, but the user
                    // has not yet clicked Allow in the permission dialog.
                    // Return actionable guidance rather than silently launching
                    // a second Chrome window.
                    println!("⏱️  20s timeout — user did not click Allow in time");
                    return Err(
                        "Check Chrome for a permission dialog asking to allow \
                         remote debugging, click Allow, then try again."
                            .to_string(),
                    );
                }
            }
        }
    } else {
        println!(
            "ℹ️  No DevToolsActivePort file found — Chrome not running or \
             remote debugging not enabled. Launching new Chrome window…\n\
             Tip: Enable it at chrome://inspect/#remote-debugging in Chrome 144+."
        );
    }

    // ── Fall back: launch our own Chrome on port 9222 ────────────────────
    CDP_PORT.store(9222, Ordering::Relaxed);
    *WS_HANDLE.lock().unwrap() = None;

    println!("🧹 Cleaning up zombie Chrome on port 9222…");
    if let Err(e) = kill_zombie_chrome() {
        println!("⚠️  Zombie cleanup warning: {}", e);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    println!("🚀 Launching new Chrome CDP window…");

    #[cfg(target_os = "macos")]
    {
        let result = launch_chrome_macos().await;
        if result.is_ok() {
            *CHROME_SOURCE.lock().unwrap() = Some(ChromeSource::AppChrome);
        }
        return result;
    }

    #[cfg(target_os = "windows")]
    {
        let result = launch_chrome_windows().await;
        if result.is_ok() {
            *CHROME_SOURCE.lock().unwrap() = Some(ChromeSource::AppChrome);
        }
        return result;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err("Platform not supported".to_string())
}

/// Connect to the browser WS endpoint and verify it has at least one page target.
/// This is the step that triggers Chrome's permission dialog (once only).
async fn try_ws_connect_and_verify(url: &str) -> Result<WsBrowserHandle, String> {
    let handle = spawn_ws_browser_connection(url).await?;

    // This send is what Chrome holds back until the user clicks Allow.
    let resp = handle
        .send("Target.getTargets", serde_json::json!({}))
        .await?;

    let has_pages = resp["result"]["targetInfos"]
        .as_array()
        .map(|t| t.iter().any(|t| t["type"].as_str() == Some("page")))
        .unwrap_or(false);

    if !has_pages {
        return Err("No page targets found in Chrome".to_string());
    }

    Ok(handle)
}

// ── Zombie cleanup ────────────────────────────────────────────────────────

fn kill_zombie_chrome() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("lsof")
            .args(["-ti", ":9222"])
            .output()
            .map_err(|e| e.to_string())?;
        for pid_str in String::from_utf8_lossy(&out.stdout).lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                if let Ok(chk) = Command::new("ps")
                    .args(["-p", &pid.to_string(), "-o", "comm="])
                    .output()
                {
                    let name = String::from_utf8_lossy(&chk.stdout);
                    if name.contains("Google Chrome") || name.contains("chrome") {
                        println!("🧹 Killing Chrome PID {}", pid);
                        Command::new("kill").arg("-9").arg(pid.to_string()).output().ok();
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let out = Command::new("netstat")
            .args(["-ano", "|", "findstr", ":9222"])
            .output()
            .map_err(|e| e.to_string())?;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(p) = line.split_whitespace().last() {
                if let Ok(pid) = p.parse::<u32>() {
                    if let Ok(o) = Command::new("tasklist")
                        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV"])
                        .output()
                    {
                        if String::from_utf8_lossy(&o.stdout)
                            .to_lowercase()
                            .contains("chrome.exe")
                        {
                            Command::new("taskkill")
                                .args(["/F", "/PID", &pid.to_string()])
                                .output()
                                .ok();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// ── Platform launchers ────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
async fn launch_chrome_macos() -> Result<String, String> {
    let child = Command::new(
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    )
    .arg("--remote-debugging-port=9222")
    .arg("--user-data-dir=/tmp/cracking-interview-chrome")
    .arg("--no-first-run")
    .arg("--no-default-browser-check")
    .arg("https://leetcode.com")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .map_err(|e| format!("Cannot launch Chrome: {}", e))?;

    let pid = child.id();
    println!("✅ Chrome PID: {}", pid);
    *CDP_CHROME_PID.lock().unwrap() = Some(pid);
    std::mem::drop(child);

    for i in 0..30u32 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if is_http_cdp_accessible(9222).await {
            println!("✅ CDP ready after {}ms", i * 500);
            return Ok("Chrome CDP opened!".to_string());
        }
    }
    Err("Timeout after 15s".to_string())
}

#[cfg(target_os = "windows")]
async fn launch_chrome_windows() -> Result<String, String> {
    let paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    for path in &paths {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        let child = Command::new(path)
            .arg("--remote-debugging-port=9222")
            .arg("--user-data-dir=C:\\Temp\\cracking-interview-chrome")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("https://leetcode.com")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
        let pid = child.id();
        *CDP_CHROME_PID.lock().unwrap() = Some(pid);
        for _ in 0..30u32 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if is_http_cdp_accessible(9222).await {
                std::mem::forget(child);
                return Ok("Chrome CDP opened!".to_string());
            }
        }
        return Err("Timeout".to_string());
    }
    Err("Chrome not found".to_string())
}

// ── Status helpers ────────────────────────────────────────────────────────

pub async fn get_cdp_status() -> String {
    if is_cdp_accessible().await {
        "🟢 Chrome Ready".to_string()
    } else {
        *CHROME_SOURCE.lock().unwrap() = None;
        *WS_HANDLE.lock().unwrap() = None;
        "🔴 Chrome Not Running".to_string()
    }
}

pub async fn is_connected_to_user_chrome() -> bool {
    if !is_cdp_accessible().await {
        return false;
    }
    matches!(*CHROME_SOURCE.lock().unwrap(), Some(ChromeSource::UserChrome))
}
