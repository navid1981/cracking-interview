// Chrome DevTools Protocol integration
// This replaces your ChromeTabManager.swift AppleScript approach!

pub mod launcher;

use serde::{Deserialize, Serialize};

// Re-export launcher functions
pub use launcher::{ensure_chrome_cdp, get_cdp_status, is_cdp_accessible};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromeTab {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(rename = "type")]
    pub tab_type: String,
}

/// Get all Chrome tabs using Chrome DevTools Protocol
/// Works on macOS, Windows, AND Linux!
pub async fn get_all_tabs() -> Result<Vec<ChromeTab>, String> {
    // Connect to Chrome's debugging port (9222)
    let response = reqwest::get("http://localhost:9222/json/list")
        .await
        .map_err(|e| {
            format!(
                "Failed to connect to Chrome CDP: {}\n\n\
                Make sure Chrome is running with:\n\
                --remote-debugging-port=9222\n\n\
                macOS: /Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome --remote-debugging-port=9222 &\n\
                Windows: \"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\" --remote-debugging-port=9222",
                e
            )
        })?;
    
    let tabs: Vec<ChromeTab> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Chrome tabs: {}", e))?;
    
    // Filter to only actual page tabs (not extensions, background pages, etc.)
    let page_tabs: Vec<ChromeTab> = tabs
        .into_iter()
        .filter(|t| t.tab_type == "page")
        .collect();
    
    Ok(page_tabs)
}

/// Activate a specific Chrome tab
/// Replaces your ChromeTabManager.activateTab()
pub async fn activate_tab(tab_id: &str) -> Result<(), String> {
    let activate_url = format!("http://localhost:9222/json/activate/{}", tab_id);
    
    reqwest::get(&activate_url)
        .await
        .map_err(|e| format!("Failed to activate tab: {}", e))?;
    
    // Bring Chrome window to front (platform-specific)
    #[cfg(target_os = "macos")]
    bring_chrome_to_front_macos()?;
    
    #[cfg(target_os = "windows")]
    bring_chrome_to_front_windows()?;
    
    Ok(())
}

/// Execute JavaScript in a Chrome tab using CDP WebSocket
/// This is WAY more powerful than AppleScript!
pub async fn execute_javascript(tab_id: &str, script: &str) -> Result<String, String> {
    use tokio_tungstenite::connect_async;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    
    // Get the WebSocket debugger URL for this tab
    let tabs_response = reqwest::get("http://localhost:9222/json/list")
        .await
        .map_err(|e| format!("Failed to get tabs: {}", e))?;
    
    let tabs: Vec<serde_json::Value> = tabs_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse tabs: {}", e))?;
    
    let tab = tabs.iter()
        .find(|t| t["id"].as_str() == Some(tab_id))
        .ok_or("Tab not found")?;
    
    let ws_url = tab["webSocketDebuggerUrl"]
        .as_str()
        .ok_or("No WebSocket URL found for tab")?;
    
    // Connect via WebSocket to the tab
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("WebSocket connection failed: {}", e))?;
    
    let (mut write, mut read) = ws_stream.split();
    
    // Send Runtime.evaluate command to execute JavaScript
    let command = serde_json::json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": {
            "expression": script,
            "returnByValue": true,
            "awaitPromise": true
        }
    });
    
    write.send(Message::Text(command.to_string()))
        .await
        .map_err(|e| format!("Failed to send command: {}", e))?;
    
    // Read the response
    if let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| format!("Failed to read response: {}", e))?;
        let text = msg.to_text().map_err(|e| format!("Invalid message: {}", e))?;
        
        let response: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        // Extract the result
        if let Some(result) = response["result"]["result"]["value"].as_str() {
            return Ok(result.to_string());
        }
        
        // Check for errors
        if let Some(error) = response["result"]["exceptionDetails"].as_object() {
            return Err(format!("JavaScript error: {:?}", error));
        }
    }
    
    Err("No result from JavaScript execution".to_string())
}

// Platform-specific window management

#[cfg(target_os = "macos")]
fn bring_chrome_to_front_macos() -> Result<(), String> {
    use std::process::Command;
    
    Command::new("osascript")
        .arg("-e")
        .arg("tell application \"Google Chrome\" to activate")
        .output()
        .map_err(|e| format!("Failed to bring Chrome to front: {}", e))?;
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn bring_chrome_to_front_windows() -> Result<(), String> {
    // Windows implementation using Win32 APIs
    // TODO: Implement when we test on Windows
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn bring_chrome_to_front_macos() -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn bring_chrome_to_front_windows() -> Result<(), String> {
    Ok(())
}
