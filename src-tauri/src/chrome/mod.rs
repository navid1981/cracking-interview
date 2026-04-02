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

fn is_ws_mode() -> bool {
    get_ws_browser_handle().is_some()
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

    #[cfg(target_os = "macos")]
    std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"Google Chrome\" to activate")
        .output()
        .ok();

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Execute JavaScript
// ═══════════════════════════════════════════════════════════════════════════

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
            return decode_and_maybe_compress(data);
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
        return decode_and_maybe_compress(data);
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

    if bytes.len() <= 4_500_000 {
        return Ok(bytes);
    }

    println!("⚠️  Image too large, re-compressing…");
    use image::ImageReader;
    let img = ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| format!("Read: {}", e))?
        .decode()
        .map_err(|e| format!("Decode: {}", e))?;

    let img = if img.width() > 2000 || img.height() > 2000 {
        let scale = 2000.0 / img.width().max(img.height()) as f32;
        img.resize(
            (img.width() as f32 * scale) as u32,
            (img.height() as f32 * scale) as u32,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .map_err(|e| format!("Encode: {}", e))?;
    println!("✅ Compressed to {} bytes", out.len());
    Ok(out)
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
