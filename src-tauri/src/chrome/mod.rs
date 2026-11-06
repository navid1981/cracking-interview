// Chrome DevTools Protocol integration
// Supports HTTP mode (--remote-debugging-port) and persistent WS mode
// (chrome://inspect/#remote-debugging toggle — ONE connection, never re-prompted).
pub mod launcher;

use serde::{Deserialize, Serialize};

pub use launcher::{
    get_cdp_port,
    get_cdp_status,
    get_ws_browser_handle,
    is_connected_to_user_chrome,
    launch_chrome_cdp_window,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromeTab {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(rename = "type")]
    pub tab_type: String,
}

fn cdp_http_base() -> String {
    format!("http://localhost:{}", get_cdp_port())
}

// ═══════════════════════════════════════════════════════════════════════════
// Tab listing
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_all_tabs() -> Result<Vec<ChromeTab>, String> {
    if let Some(handle) = get_ws_browser_handle() {
        get_all_tabs_ws(handle).await
    } else {
        get_all_tabs_http().await
    }
}

async fn get_all_tabs_http() -> Result<Vec<ChromeTab>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let tabs: Vec<ChromeTab> = client
        .get(&format!("{}/json/list", cdp_http_base()))
        .send()
        .await
        .map_err(|e| format!("Chrome CDP not accessible: {}. Click 'Open Chrome'.", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse tabs: {}", e))?;

    Ok(filter_page_tabs(tabs))
}

async fn get_all_tabs_ws(handle: launcher::WsBrowserHandle) -> Result<Vec<ChromeTab>, String> {
    let resp = handle
        .send("Target.getTargets", serde_json::json!({}))
        .await?;

    let targets = resp["result"]["targetInfos"]
        .as_array()
        .ok_or("No targetInfos in response")?;

    let tabs = targets
        .iter()
        .filter(|t| {
            let url = t["url"].as_str().unwrap_or("");
            t["type"].as_str() == Some("page")
                && !url.starts_with("chrome://")
                && !url.starts_with("chrome-extension://")
                && !url.starts_with("devtools://")
        })
        .map(|t| ChromeTab {
            id: t["targetId"].as_str().unwrap_or("").to_string(),
            url: t["url"].as_str().unwrap_or("").to_string(),
            title: t["title"].as_str().unwrap_or("").to_string(),
            tab_type: "page".to_string(),
        })
        .collect();

    Ok(tabs)
}

fn filter_page_tabs(tabs: Vec<ChromeTab>) -> Vec<ChromeTab> {
    tabs.into_iter()
        .filter(|t| {
            t.tab_type == "page"
                && !t.url.starts_with("chrome://")
                && !t.url.starts_with("chrome-extension://")
                && !t.url.starts_with("devtools://")
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Tab activation
// ═══════════════════════════════════════════════════════════════════════════

pub async fn activate_tab(tab_id: &str) -> Result<(), String> {
    if let Some(handle) = get_ws_browser_handle() {
        handle
            .send(
                "Target.activateTarget",
                serde_json::json!({"targetId": tab_id}),
            )
            .await?;
    } else {
        reqwest::get(&format!("{}/json/activate/{}", cdp_http_base(), tab_id))
            .await
            .map_err(|e| format!("Failed to activate: {}", e))?;
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Execute JavaScript
// ═══════════════════════════════════════════════════════════════════════════

const EXTRACT_SCRIPT: &str = include_str!("extract.js");

/// The extraction script with per-run settings filled in. Debug builds also ask it to describe
/// the page's editor environment (`probe` in the result, dump file only).
fn extract_script(code_offset: usize) -> String {
    let script = EXTRACT_SCRIPT.replacen(
        "const CODE_NUMBER_OFFSET = 0;",
        &format!("const CODE_NUMBER_OFFSET = {};", code_offset),
        1,
    );
    if cfg!(debug_assertions) {
        script.replacen("const DEBUG_PROBE = false;", "const DEBUG_PROBE = true;", 1)
    } else {
        script
    }
}

/// Runs extract.js in the tab and returns its structured result
/// (`text`, `code`, `images`, `drawn`, ...). The script waits up to 300ms for the DOM to settle.
pub async fn extract_page(tab_id: &str) -> Result<serde_json::Value, String> {
    let raw = execute_javascript(tab_id, &extract_script(0)).await?;
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid extraction result: {}", e))?;
    // Frames from other sites (e.g. an embedded online IDE) are invisible to the page's own script.
    // Read them through the debugger. Best effort: on any failure the parent page's result is kept.
    if has_cross_origin_skips(&value) {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(EMBEDDED_FRAMES_TIMEOUT_SECS),
            read_embedded_frames(tab_id, &mut value),
        )
        .await;
    }
    // Dev builds only: log a summary and dump the full result so it can be inspected.
    #[cfg(debug_assertions)]
    log_extraction(&value, &serde_json::to_string_pretty(&value).unwrap_or_default());
    Ok(value)
}

// ── Cross-origin frames ────────────────────────────────────────────────────

const EMBEDDED_FRAMES_TIMEOUT_SECS: u64 = 6;
const MAX_EMBEDDED_FRAMES: usize = 4;
const MAX_EMBEDDED_FRAME_DEPTH: usize = 2;
const MIN_EMBEDDED_FRAME_SIDE: i64 = 100;
/// Same third-party widget hosts the page script skips (FRAME_DENY_RE in extract.js).
const FRAME_DENYLIST: &[&str] = &[
    "recaptcha", "hcaptcha", "chilipiper", "platform.twitter.com", "twitter.com/widgets",
    "facebook.com/plugins", "intercom", "hotjar", "doubleclick", "googlesyndication",
    "googletagmanager", "google-analytics", "disqus", "drift.com", "zendesk", "hubspot",
    "sprig", "livechat", "tawk.to", "crisp.chat",
];

struct FrameTarget {
    id: String,
    url: String,
}

fn has_cross_origin_skips(value: &serde_json::Value) -> bool {
    value["skippedFrames"]
        .as_array()
        .map_or(false, |a| a.iter().any(|f| f["reason"].as_str() == Some("cross-origin")))
}

fn frame_is_readable(url: &str) -> bool {
    let lower = url.to_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !FRAME_DENYLIST.iter().any(|d| lower.contains(d))
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#']).next().unwrap_or("").to_string()
}

/// Cross-origin frames are separate DevTools targets of type "iframe" whose parent is the page
/// (or an outer frame). Both connection modes list them.
async fn list_child_frames(parent_id: &str) -> Vec<FrameTarget> {
    let pick = |t: &serde_json::Value, id_key: &str, parent_key: &str| -> Option<FrameTarget> {
        if t["type"].as_str() != Some("iframe") || t[parent_key].as_str() != Some(parent_id) {
            return None;
        }
        Some(FrameTarget {
            id: t[id_key].as_str()?.to_string(),
            url: t["url"].as_str().unwrap_or("").to_string(),
        })
    };
    if let Some(handle) = get_ws_browser_handle() {
        let Ok(resp) = handle.send("Target.getTargets", serde_json::json!({})).await else {
            return vec![];
        };
        resp["result"]["targetInfos"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| pick(t, "targetId", "parentFrameId")).collect())
            .unwrap_or_default()
    } else {
        let Ok(resp) = reqwest::get(&format!("{}/json/list", cdp_http_base())).await else {
            return vec![];
        };
        let Ok(list) = resp.json::<Vec<serde_json::Value>>().await else {
            return vec![];
        };
        list.iter().filter_map(|t| pick(t, "id", "parentId")).collect()
    }
}

async fn read_embedded_frames(tab_id: &str, main: &mut serde_json::Value) {
    let mut queue = std::collections::VecDeque::from([(tab_id.to_string(), 0usize)]);
    let mut read = 0usize;
    while let Some((parent, depth)) = queue.pop_front() {
        for frame in list_child_frames(&parent).await {
            if read >= MAX_EMBEDDED_FRAMES {
                return;
            }
            if !frame_is_readable(&frame.url) {
                continue;
            }
            let offset = main["code"].as_array().map_or(0, |a| a.len());
            let script = extract_script(offset);
            let Ok(raw) = execute_javascript(&frame.id, &script).await else {
                continue;
            };
            let Ok(result) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            let side = |k: &str| result["viewport"][k].as_i64().unwrap_or(0);
            if side("width") < MIN_EMBEDDED_FRAME_SIDE || side("height") < MIN_EMBEDDED_FRAME_SIDE {
                continue;
            }
            let text = result["text"].as_str().unwrap_or("").trim().to_string();
            let code = result["code"].as_array().cloned().unwrap_or_default();
            if text.is_empty() && code.is_empty() {
                continue;
            }
            merge_embedded_frame(main, &frame, &result, &text, code);
            read += 1;
            if depth + 1 < MAX_EMBEDDED_FRAME_DEPTH && has_cross_origin_skips(&result) {
                queue.push_back((frame.id.clone(), depth + 1));
            }
        }
    }
}

fn merge_embedded_frame(
    main: &mut serde_json::Value,
    frame: &FrameTarget,
    result: &serde_json::Value,
    text: &str,
    code: Vec<serde_json::Value>,
) {
    let host = host_of(&frame.url);
    let block = format!("--- Embedded frame ({}) ---\n{}\n--- End of embedded frame ---", host, text);
    let joined = match main["text"].as_str().unwrap_or("") {
        "" => block,
        existing => format!("{}\n\n{}", existing, block),
    };
    main["text"] = serde_json::Value::String(joined);
    if let Some(arr) = main["code"].as_array_mut() {
        arr.extend(code);
    }
    let add = |main: &mut serde_json::Value, key: &str, n: i64| {
        main[key] = serde_json::json!(main[key].as_i64().unwrap_or(0) + n);
    };
    add(main, "pageTextLength", result["pageTextLength"].as_i64().unwrap_or(0));
    add(main, "framesRead", 1);
    // This frame was counted as skipped in its parent; whatever it skipped itself still is.
    let still_skipped = result["framesSkipped"].as_i64().unwrap_or(0);
    add(main, "framesSkipped", still_skipped - 1);
    if let Some(list) = main["skippedFrames"].as_array_mut() {
        if let Some(i) = list.iter().position(|f| f["reason"].as_str() == Some("cross-origin")) {
            list.remove(i);
        }
        if let Some(inner) = result["skippedFrames"].as_array() {
            list.extend(inner.iter().cloned());
        }
    }
    if result["truncated"].as_bool() == Some(true) {
        main["truncated"] = serde_json::Value::Bool(true);
    }
    if let Some(probe) = result.get("probe").filter(|p| !p.is_null()) {
        let entry = serde_json::json!({ "frame": frame.url.split(['?', '#']).next().unwrap_or(""), "probe": probe });
        match main["probes"].as_array_mut() {
            Some(arr) => arr.push(entry),
            None => main["probes"] = serde_json::json!([entry]),
        }
    }
    let entry = serde_json::json!({
        "src": frame.url.split(['?', '#']).next().unwrap_or(""),
        "chars": text.chars().count(),
    });
    match main["embeddedFrames"].as_array_mut() {
        Some(arr) => arr.push(entry),
        None => main["embeddedFrames"] = serde_json::json!([entry]),
    }
}

#[cfg(debug_assertions)]
fn log_extraction(value: &serde_json::Value, raw: &str) {
    let len = |k: &str| value[k].as_str().map_or(0, |s| s.chars().count());
    let count = |k: &str| value[k].as_array().map_or(0, |a| a.len());
    println!(
        "📄 [extract] url={} title={:?} root={} text={}ch code_blocks={} images={} drawn={} frames={}/{} skipped truncated={}",
        value["url"].as_str().unwrap_or(""),
        value["title"].as_str().unwrap_or(""),
        value["root"].as_str().unwrap_or(""),
        len("text"),
        count("code"),
        count("images"),
        count("drawn"),
        value["framesRead"],
        value["framesSkipped"],
        value["truncated"],
    );
    if let Some(frames) = value["skippedFrames"].as_array() {
        for f in frames {
            println!(
                "📄 [extract] skipped frame: {} ({}, {}x{})",
                f["src"].as_str().unwrap_or(""),
                f["reason"].as_str().unwrap_or(""),
                f["width"],
                f["height"],
            );
        }
    }
    if let Some(frames) = value["embeddedFrames"].as_array() {
        for f in frames {
            println!(
                "📄 [extract] embedded frame read: {} ({} chars)",
                f["src"].as_str().unwrap_or(""),
                f["chars"],
            );
        }
    }
    if let Some(err) = value["error"].as_str() {
        println!("⚠️ [extract] script error: {}", err);
    }
    let mut path = std::env::temp_dir();
    path.push("cracking_interview_extract.json");
    match std::fs::write(&path, raw) {
        Ok(()) => println!("📄 [extract] full result written to {}", path.display()),
        Err(e) => println!("⚠️ [extract] could not write {}: {}", path.display(), e),
    }
}

pub async fn execute_javascript(tab_id: &str, script: &str) -> Result<String, String> {
    if let Some(handle) = get_ws_browser_handle() {
        execute_javascript_ws(handle, tab_id, script).await
    } else {
        execute_javascript_http(tab_id, script).await
    }
}

async fn execute_javascript_http(tab_id: &str, script: &str) -> Result<String, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    let tabs: Vec<serde_json::Value> = reqwest::get(&format!("{}/json/list", cdp_http_base()))
        .await
        .map_err(|e| format!("Get tabs: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse tabs: {}", e))?;

    let ws_url = tabs
        .iter()
        .find(|t| t["id"].as_str() == Some(tab_id))
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .ok_or("Tab not found")?
        .to_string();

    let (ws, _) = connect_async(&ws_url)
        .await
        .map_err(|e| format!("WS: {}", e))?;
    let (mut write, mut read) = ws.split();

    let cmd = serde_json::json!({
        "id": 1, "method": "Runtime.evaluate",
        "params": {"expression": script, "returnByValue": true, "awaitPromise": true}
    });
    write
        .send(Message::Text(cmd.to_string()))
        .await
        .map_err(|e| format!("Send: {}", e))?;

    if let Some(Ok(msg)) = read.next().await {
        let resp: serde_json::Value = serde_json::from_str(
            msg.to_text().map_err(|e| format!("Invalid: {}", e))?,
        )
        .map_err(|e| format!("Parse: {}", e))?;

        if let Some(val) = resp["result"]["result"]["value"].as_str() {
            return Ok(val.to_string());
        }
        if resp["result"]["exceptionDetails"].as_object().is_some() {
            return Err("JavaScript error".to_string());
        }
    }
    Err("No result".to_string())
}

/// WS mode: attach to the target via the PERSISTENT handle, then run JS.
/// Uses the flat protocol (sessionId) — no new browser WS connections opened.
async fn execute_javascript_ws(
    handle: launcher::WsBrowserHandle,
    tab_id: &str,
    script: &str,
) -> Result<String, String> {
    // Attach once to get a session
    let attach = handle
        .send(
            "Target.attachToTarget",
            serde_json::json!({"targetId": tab_id, "flatten": true}),
        )
        .await?;

    let session_id = attach["result"]["sessionId"]
        .as_str()
        .ok_or("No sessionId in attachToTarget")?
        .to_string();

    let resp = handle
        .send_session(
            "Runtime.evaluate",
            &session_id,
            serde_json::json!({
                "expression": script,
                "returnByValue": true,
                "awaitPromise": true
            }),
        )
        .await?;

    if let Some(val) = resp["result"]["result"]["value"].as_str() {
        return Ok(val.to_string());
    }
    if resp["result"]["exceptionDetails"].as_object().is_some() {
        return Err("JavaScript error".to_string());
    }
    Err("No value in result".to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
// Screenshots
// ═══════════════════════════════════════════════════════════════════════════

pub async fn capture_screenshot(tab_id: &str) -> Result<Vec<u8>, String> {
    if let Some(handle) = get_ws_browser_handle() {
        capture_screenshot_ws(handle, tab_id).await
    } else {
        capture_screenshot_http(tab_id).await
    }
}

async fn capture_screenshot_http(tab_id: &str) -> Result<Vec<u8>, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    let tabs: Vec<serde_json::Value> = reqwest::get(&format!("{}/json/list", cdp_http_base()))
        .await
        .map_err(|e| format!("Get tabs: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse: {}", e))?;

    let ws_url = tabs
        .iter()
        .find(|t| t["id"].as_str() == Some(tab_id))
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .ok_or("Tab not found")?
        .to_string();

    let (ws, _) = connect_async(&ws_url)
        .await
        .map_err(|e| format!("WS: {}", e))?;
    let (mut write, mut read) = ws.split();

    let cmd = serde_json::json!({
        "id": 1, "method": "Page.captureScreenshot",
        "params": {"format": "jpeg", "quality": 80, "captureBeyondViewport": true}
    });
    write
        .send(Message::Text(cmd.to_string()))
        .await
        .map_err(|e| format!("Send: {}", e))?;

    if let Some(Ok(msg)) = read.next().await {
        let resp: serde_json::Value =
            serde_json::from_str(msg.to_text().map_err(|e| format!("Invalid: {}", e))?)
                .map_err(|e| format!("Parse: {}", e))?;
        if let Some(data) = resp["result"]["data"].as_str() {
            let data_str = data.to_string();
            return tauri::async_runtime::spawn_blocking(move || decode_and_maybe_compress(&data_str))
                .await
                .map_err(|e| format!("Task error: {}", e))?;
        }
    }
    Err("No screenshot data".to_string())
}

async fn capture_screenshot_ws(
    handle: launcher::WsBrowserHandle,
    tab_id: &str,
) -> Result<Vec<u8>, String> {
    let attach = handle
        .send(
            "Target.attachToTarget",
            serde_json::json!({"targetId": tab_id, "flatten": true}),
        )
        .await?;

    let session_id = attach["result"]["sessionId"]
        .as_str()
        .ok_or("No sessionId")?
        .to_string();

    let resp = handle
        .send_session(
            "Page.captureScreenshot",
            &session_id,
            serde_json::json!({"format": "jpeg", "quality": 80, "captureBeyondViewport": true}),
        )
        .await?;

    if let Some(data) = resp["result"]["data"].as_str() {
        let data_str = data.to_string();
        return tauri::async_runtime::spawn_blocking(move || decode_and_maybe_compress(&data_str))
            .await
            .map_err(|e| format!("Task error: {}", e))?;
    }
    Err("No screenshot data in response".to_string())
}

fn decode_and_maybe_compress(b64: &str) -> Result<Vec<u8>, String> {
    use base64::{engine::general_purpose, Engine as _};
    let bytes = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64: {}", e))?;

    println!(
        "📸 Screenshot: {} bytes ({:.2} MB)",
        bytes.len(),
        bytes.len() as f64 / 1_000_000.0
    );

    fit_image_bytes(bytes, 4_500_000)
}

/// Returns the image unchanged if it's already png/jpeg/gif/webp and within `max_bytes`;
/// otherwise decodes it and re-encodes as JPEG (transparent areas on white), shrinking
/// until it fits. Fails for formats the `image` crate can't decode (e.g. SVG).
pub fn fit_image_bytes(bytes: Vec<u8>, max_bytes: usize) -> Result<Vec<u8>, String> {
    if bytes.len() <= max_bytes && crate::ai::detect_image_mime_type(&bytes).is_ok() {
        return Ok(bytes);
    }

    use image::ImageReader;
    let img = ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| format!("Read: {}", e))?
        .decode()
        .map_err(|e| format!("Decode: {}", e))?;

    let mut max_side = 2000u32;
    loop {
        let resized = if img.width() > max_side || img.height() > max_side {
            img.resize(max_side, max_side, image::imageops::FilterType::Lanczos3)
        } else {
            img.clone()
        };
        let rgba = resized.to_rgba8();
        let rgb = image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
            let p = rgba.get_pixel(x, y).0;
            let a = p[3] as u32;
            let blend = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])])
        });
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(rgb)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
            .map_err(|e| format!("Encode: {}", e))?;
        if out.len() <= max_bytes || max_side <= 500 {
            println!("✅ Image re-encoded to {} bytes (max side {}px)", out.len(), max_side);
            return Ok(out);
        }
        max_side = max_side * 3 / 4;
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Page images (from Chrome's own resource cache — no new download)
// ═══════════════════════════════════════════════════════════════════════════

type TabWsStream = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// One CDP connection to a tab for several commands in a row.
enum TabSession {
    Ws {
        handle: launcher::WsBrowserHandle,
        session_id: String,
    },
    Http {
        write: futures_util::stream::SplitSink<TabWsStream, tokio_tungstenite::tungstenite::Message>,
        read: futures_util::stream::SplitStream<TabWsStream>,
        next_id: u64,
    },
}

impl TabSession {
    async fn open(tab_id: &str) -> Result<Self, String> {
        if let Some(handle) = get_ws_browser_handle() {
            let attach = handle
                .send("Target.attachToTarget", serde_json::json!({"targetId": tab_id, "flatten": true}))
                .await?;
            let session_id = attach["result"]["sessionId"]
                .as_str()
                .ok_or("No sessionId")?
                .to_string();
            return Ok(TabSession::Ws { handle, session_id });
        }

        use futures_util::StreamExt;
        let tabs: Vec<serde_json::Value> = reqwest::get(&format!("{}/json/list", cdp_http_base()))
            .await
            .map_err(|e| format!("Get tabs: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Parse tabs: {}", e))?;
        let ws_url = tabs
            .iter()
            .find(|t| t["id"].as_str() == Some(tab_id))
            .and_then(|t| t["webSocketDebuggerUrl"].as_str())
            .ok_or("Tab not found")?
            .to_string();
        let (ws, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| format!("WS: {}", e))?;
        let (write, read) = ws.split();
        Ok(TabSession::Http { write, read, next_id: 1 })
    }

    /// Sends a command and returns its `result` object.
    async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        let fut = async {
            match self {
                TabSession::Ws { handle, session_id } => {
                    let resp = handle.send_session(method, session_id, params).await?;
                    if let Some(err) = resp.get("error") {
                        return Err(format!("{}: {}", method, err));
                    }
                    Ok(resp["result"].clone())
                }
                TabSession::Http { write, read, next_id } => {
                    use futures_util::{SinkExt, StreamExt};
                    let id = *next_id;
                    *next_id += 1;
                    let cmd = serde_json::json!({"id": id, "method": method, "params": params});
                    write
                        .send(tokio_tungstenite::tungstenite::Message::Text(cmd.to_string()))
                        .await
                        .map_err(|e| format!("Send: {}", e))?;
                    while let Some(msg) = read.next().await {
                        let msg = msg.map_err(|e| format!("WS read: {}", e))?;
                        let Ok(text) = msg.to_text() else { continue };
                        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { continue };
                        if v["id"].as_u64() != Some(id) {
                            continue; // CDP event or another reply
                        }
                        if let Some(err) = v.get("error") {
                            return Err(format!("{}: {}", method, err));
                        }
                        return Ok(v["result"].clone());
                    }
                    Err("CDP connection closed".to_string())
                }
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(5), fut)
            .await
            .map_err(|_| format!("{} timed out", method))?
    }
}

fn index_frame_resources(node: &serde_json::Value, map: &mut std::collections::HashMap<String, String>) {
    let frame_id = node["frame"]["id"].as_str().unwrap_or("").to_string();
    if let Some(url) = node["frame"]["url"].as_str() {
        map.entry(url.to_string()).or_insert_with(|| frame_id.clone());
    }
    for r in node["resources"].as_array().into_iter().flatten() {
        if let Some(url) = r["url"].as_str() {
            map.entry(url.to_string()).or_insert_with(|| frame_id.clone());
        }
    }
    for child in node["childFrames"].as_array().into_iter().flatten() {
        index_frame_resources(child, map);
    }
}

fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    use base64::{engine::general_purpose, Engine as _};
    let (meta, data) = url.strip_prefix("data:")?.split_once(',')?;
    if meta.ends_with(";base64") {
        general_purpose::STANDARD.decode(data).ok()
    } else {
        Some(data.as_bytes().to_vec())
    }
}

/// Image bytes for each URL, taken from what the tab already loaded (`Page.getResourceContent`),
/// so login-gated images work and nothing is downloaded again. `None` where unavailable.
pub async fn get_resource_images(tab_id: &str, urls: &[String]) -> Vec<Option<Vec<u8>>> {
    use base64::{engine::general_purpose, Engine as _};
    let mut out: Vec<Option<Vec<u8>>> = urls.iter().map(|u| decode_data_url(u)).collect();
    if out.iter().zip(urls).all(|(o, u)| o.is_some() || u.starts_with("data:")) {
        return out;
    }

    let mut session = match TabSession::open(tab_id).await {
        Ok(s) => s,
        Err(e) => {
            println!("⚠️ Page images: could not open tab session: {}", e);
            return out;
        }
    };
    let _ = session.call("Page.enable", serde_json::json!({})).await;
    let tree = match session.call("Page.getResourceTree", serde_json::json!({})).await {
        Ok(t) => t,
        Err(e) => {
            println!("⚠️ Page images: getResourceTree failed: {}", e);
            return out;
        }
    };
    let main_frame = tree["frameTree"]["frame"]["id"].as_str().unwrap_or("").to_string();
    let mut url_frames = std::collections::HashMap::new();
    index_frame_resources(&tree["frameTree"], &mut url_frames);

    for (i, url) in urls.iter().enumerate() {
        if out[i].is_some() || url.starts_with("data:") {
            continue;
        }
        let frame_id = url_frames.get(url).cloned().unwrap_or_else(|| main_frame.clone());
        match session
            .call("Page.getResourceContent", serde_json::json!({"frameId": frame_id, "url": url}))
            .await
        {
            Ok(res) => {
                let content = res["content"].as_str().unwrap_or("");
                out[i] = if res["base64Encoded"].as_bool() == Some(true) {
                    general_purpose::STANDARD.decode(content).ok()
                } else {
                    Some(content.as_bytes().to_vec())
                };
            }
            Err(e) => println!("⚠️ Page image {} not in Chrome's cache: {}", i + 1, e),
        }
    }
    out
}

/// Scrolls a chart/drawing (found by the rect extract.js reported) into view if less than half of
/// it is visible, so the tab screenshot includes it. Returns "scrolled", "visible" or "not-found".
pub async fn scroll_drawing_into_view(tab_id: &str, rect: &serde_json::Value) -> Result<String, String> {
    let script = format!(
        r#"((t) => {{
  for (const el of document.querySelectorAll('canvas, svg')) {{
    const r = el.getBoundingClientRect();
    if (Math.abs(r.top - t.top) > 2 || Math.abs(r.left - t.left) > 2 || Math.abs(r.width - t.width) > 2 || Math.abs(r.height - t.height) > 2) continue;
    const visH = Math.max(0, Math.min(r.bottom, innerHeight) - Math.max(r.top, 0));
    const visW = Math.max(0, Math.min(r.right, innerWidth) - Math.max(r.left, 0));
    if ((visH * visW) / Math.max(1, r.width * r.height) >= 0.5) return 'visible';
    el.scrollIntoView({{ block: 'center', inline: 'nearest' }});
    return 'scrolled';
  }}
  return 'not-found';
}})({})"#,
        rect
    );
    execute_javascript(tab_id, &script).await
}

// ═══════════════════════════════════════════════════════════════════════════
// Thumbnails
// ═══════════════════════════════════════════════════════════════════════════

pub async fn capture_thumbnail(tab_id: &str) -> Result<Vec<u8>, String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        capture_thumbnail_inner(tab_id),
    )
    .await
    {
        Ok(r) => r,
        Err(_) => Err("Thumbnail timed out".to_string()),
    }
}

async fn capture_thumbnail_inner(tab_id: &str) -> Result<Vec<u8>, String> {
    if let Some(handle) = get_ws_browser_handle() {
        capture_thumbnail_ws(handle, tab_id).await
    } else {
        capture_thumbnail_http(tab_id).await
    }
}

async fn capture_thumbnail_http(tab_id: &str) -> Result<Vec<u8>, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("Client: {}", e))?;

    let tabs: Vec<serde_json::Value> = client
        .get(&format!("{}/json/list", cdp_http_base()))
        .send()
        .await
        .map_err(|e| format!("Get tabs: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse: {}", e))?;

    let ws_url = tabs
        .iter()
        .find(|t| t["id"].as_str() == Some(tab_id))
        .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        .ok_or("Tab not found")?
        .to_string();

    let (ws, _) = connect_async(&ws_url)
        .await
        .map_err(|e| format!("WS: {}", e))?;
    let (mut write, mut read) = ws.split();

    let cmd = serde_json::json!({
        "id": 1, "method": "Page.captureScreenshot",
        "params": {"format": "jpeg", "quality": 50, "captureBeyondViewport": false, "fromSurface": true}
    });
    write
        .send(Message::Text(cmd.to_string()))
        .await
        .map_err(|e| format!("Send: {}", e))?;

    if let Some(Ok(msg)) = read.next().await {
        let resp: serde_json::Value =
            serde_json::from_str(msg.to_text().map_err(|e| format!("Invalid: {}", e))?)
                .map_err(|e| format!("Parse: {}", e))?;
        if let Some(data) = resp["result"]["data"].as_str() {
            use base64::{engine::general_purpose, Engine as _};
            return general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("Base64: {}", e));
        }
    }
    Err("No thumbnail data".to_string())
}

async fn capture_thumbnail_ws(
    handle: launcher::WsBrowserHandle,
    tab_id: &str,
) -> Result<Vec<u8>, String> {
    let attach = handle
        .send(
            "Target.attachToTarget",
            serde_json::json!({"targetId": tab_id, "flatten": true}),
        )
        .await?;

    let session_id = attach["result"]["sessionId"]
        .as_str()
        .ok_or("No sessionId")?
        .to_string();

    let resp = handle
        .send_session(
            "Page.captureScreenshot",
            &session_id,
            serde_json::json!({"format": "jpeg", "quality": 50, "captureBeyondViewport": false, "fromSurface": true}),
        )
        .await?;

    if let Some(data) = resp["result"]["data"].as_str() {
        use base64::{engine::general_purpose, Engine as _};
        return general_purpose::STANDARD
            .decode(data)
            .map_err(|e| format!("Base64: {}", e));
    }
    Err("No thumbnail data in response".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_png(img: image::RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn fit_keeps_small_supported_image_unchanged() {
        let png = encode_png(image::RgbaImage::from_pixel(40, 40, image::Rgba([10, 20, 30, 255])));
        assert_eq!(fit_image_bytes(png.clone(), 1_000_000).unwrap(), png);
    }

    #[test]
    fn fit_reencodes_oversized_transparent_png_as_jpeg_on_white() {
        // Noisy pixels so PNG compression can't make it small; left half fully transparent.
        let img = image::RgbaImage::from_fn(1600, 1200, |x, y| {
            let n = ((x * 7919 + y * 104729) % 251) as u8;
            image::Rgba([n, n.wrapping_mul(3), n.wrapping_mul(7), if x < 800 { 0 } else { 255 }])
        });
        let png = encode_png(img);
        let max = 300_000;
        assert!(png.len() > max);
        let out = fit_image_bytes(png, max).unwrap();
        assert!(out.len() <= max, "got {} bytes", out.len());
        assert_eq!(crate::ai::detect_image_mime_type(&out).unwrap(), "image/jpeg");
        let decoded = image::load_from_memory(&out).unwrap().to_rgb8();
        let p = decoded.get_pixel(5, 5).0;
        assert!(p.iter().all(|c| *c > 240), "transparent area should be white, got {:?}", p);
    }

    #[test]
    fn fit_rejects_formats_it_cannot_decode() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#.to_vec();
        assert!(fit_image_bytes(svg, 1_000_000).is_err());
    }

    #[test]
    fn decodes_data_urls() {
        assert_eq!(decode_data_url("data:image/png;base64,AAEC").unwrap(), vec![0, 1, 2]);
        assert_eq!(decode_data_url("data:image/svg+xml,<svg/>").unwrap(), b"<svg/>".to_vec());
        assert!(decode_data_url("https://example.com/a.png").is_none());
    }

    /// Serves one fixed HTML page on a free local port (for the cross-origin frame test).
    fn serve_html(html: String) -> u16 {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        port
    }

    /// Runs the real extraction, page-image and scroll code against headless Chrome.
    /// `cargo test -- --ignored chrome_integration` (needs Google Chrome installed).
    #[tokio::test]
    #[ignore]
    async fn chrome_integration() {
        let chrome = std::env::var("CHROME_PATH")
            .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string());
        let dir = std::env::temp_dir().join(format!("ci-extract-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = encode_png(image::RgbaImage::from_fn(600, 400, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
        }));
        std::fs::write(dir.join("puzzle.png"), &png).unwrap();
        std::fs::write(
            dir.join("page.html"),
            r##"<!doctype html><html><body>
<p>Question 7 / 10</p>
<img src="puzzle.png" style="width:450px">
<p>Which box completes the pattern?</p>
<label><input type="radio" name="q">A</label><label><input type="radio" name="q">B</label>
<div style="height:2500px"></div>
<svg width="500" height="300"><rect width="500" height="300" fill="#eee"/><text x="20" y="40">2024</text></svg>
</body></html>"##,
        )
        .unwrap();

        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        launcher::set_cdp_port_for_test(port);
        let mut proc = std::process::Command::new(&chrome)
            .args([
                "--headless=new",
                &format!("--remote-debugging-port={}", port),
                &format!("--user-data-dir={}", dir.join("profile").display()),
                "--no-first-run",
                "--allow-file-access-from-files",
                "--window-size=1200,800",
                "about:blank",
            ])
            .spawn()
            .expect("launch Chrome");

        let result = async {
            let client = reqwest::Client::new();
            let page_url = format!("file://{}", dir.join("page.html").display());
            let mut target = None;
            for _ in 0..50 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if let Ok(resp) = client.put(format!("http://127.0.0.1:{}/json/new?{}", port, page_url)).send().await {
                    if let Ok(v) = resp.json::<serde_json::Value>().await {
                        target = v["id"].as_str().map(|s| s.to_string());
                        break;
                    }
                }
            }
            let tab_id = target.expect("Chrome did not open the test page");
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

            let extraction = extract_page(&tab_id).await.unwrap();
            let text = extraction["text"].as_str().unwrap();
            assert!(text.contains("Question 7 / 10"), "{}", text);
            assert!(text.contains("[image 1]"), "{}", text);
            assert!(text.contains("( ) A"), "{}", text);
            assert!(text.contains("[chart/drawing: see screenshot]"), "{}", text);
            let images = extraction["images"].as_array().unwrap();
            assert_eq!(images.len(), 1);
            assert_eq!(images[0]["naturalWidth"], 600);

            let src = images[0]["src"].as_str().unwrap().to_string();
            let fetched = get_resource_images(&tab_id, &[src]).await;
            assert_eq!(fetched[0].as_deref(), Some(png.as_slice()), "bytes should be the original file");

            let drawn = &extraction["drawn"][0];
            assert_eq!(scroll_drawing_into_view(&tab_id, drawn).await.unwrap(), "scrolled");
            let again = extract_page(&tab_id).await.unwrap();
            assert_eq!(scroll_drawing_into_view(&tab_id, &again["drawn"][0]).await.unwrap(), "visible");

            // A frame from another site (127.0.0.1 vs localhost) is its own DevTools target; the page's
            // own script cannot read it, so its text and editor lines must come from the debugger.
            let inner_port = serve_html(
                "<html><body><p>INNER question: add two numbers</p><div class=\"monaco-editor\"><div class=\"view-lines\"><div class=\"view-line\">def add(a, b):</div></div></div></body></html>".to_string(),
            );
            let outer_port = serve_html(format!(
                "<html><body><p>OUTER shell</p><iframe width=\"800\" height=\"400\" src=\"http://localhost:{}/\"></iframe></body></html>",
                inner_port
            ));
            let resp = client
                .put(format!("http://127.0.0.1:{}/json/new?http://127.0.0.1:{}/", port, outer_port))
                .send()
                .await
                .unwrap();
            let outer_tab = resp.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string();
            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
            let framed = extract_page(&outer_tab).await.unwrap();
            let ftext = framed["text"].as_str().unwrap();
            assert!(ftext.contains("OUTER shell"), "{}", ftext);
            assert!(ftext.contains(&format!("--- Embedded frame (localhost:{}) ---", inner_port)), "{}", ftext);
            assert!(ftext.contains("INNER question: add two numbers"), "{}", ftext);
            assert!(ftext.contains("def add(a, b):"), "{}", ftext);
            assert_eq!(framed["framesRead"], 1, "{}", framed);
            assert_eq!(framed["framesSkipped"], 0, "{}", framed);
            assert_eq!(framed["embeddedFrames"].as_array().unwrap().len(), 1);
        }
        .await;

        let _ = proc.kill();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }
}
