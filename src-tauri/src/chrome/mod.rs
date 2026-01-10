// Chrome DevTools Protocol integration
pub mod launcher;

use serde::{Deserialize, Serialize};

// Re-export launcher functions
pub use launcher::{
    launch_chrome_cdp_window,
    is_cdp_accessible,
    get_cdp_status,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromeTab {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(rename = "type")]
    pub tab_type: String,
}

/// Get all Chrome tabs
pub async fn get_all_tabs() -> Result<Vec<ChromeTab>, String> {
    let response = reqwest::get("http://localhost:9222/json/list")
        .await
        .map_err(|e| format!("Chrome CDP not accessible: {}. Click 'Open Chrome CDP' button.", e))?;
    
    let tabs: Vec<ChromeTab> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse tabs: {}", e))?;
    
    let page_tabs: Vec<ChromeTab> = tabs
        .into_iter()
        .filter(|t| t.tab_type == "page")
        .collect();
    
    Ok(page_tabs)
}

/// Activate tab
pub async fn activate_tab(tab_id: &str) -> Result<(), String> {
    let activate_url = format!("http://localhost:9222/json/activate/{}", tab_id);
    reqwest::get(&activate_url)
        .await
        .map_err(|e| format!("Failed to activate: {}", e))?;
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"Google Chrome\" to activate")
            .output()
            .ok();
    }
    
    Ok(())
}

/// Execute JavaScript in tab
pub async fn execute_javascript(tab_id: &str, script: &str) -> Result<String, String> {
    use tokio_tungstenite::connect_async;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    
    let tabs_response = reqwest::get("http://localhost:9222/json/list")
        .await
        .map_err(|e| format!("Failed to get tabs: {}", e))?;
    
    let tabs: Vec<serde_json::Value> = tabs_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse: {}", e))?;
    
    let tab = tabs.iter()
        .find(|t| t["id"].as_str() == Some(tab_id))
        .ok_or("Tab not found")?;
    
    let ws_url = tab["webSocketDebuggerUrl"]
        .as_str()
        .ok_or("No WebSocket URL")?;
    
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("WebSocket failed: {}", e))?;
    
    let (mut write, mut read) = ws_stream.split();
    
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
        .map_err(|e| format!("Send failed: {}", e))?;
    
    if let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| format!("Read failed: {}", e))?;
        let text = msg.to_text().map_err(|e| format!("Invalid: {}", e))?;
        
        let response: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| format!("Parse failed: {}", e))?;
        
        if let Some(result) = response["result"]["result"]["value"].as_str() {
            return Ok(result.to_string());
        }
        
        if let Some(_error) = response["result"]["exceptionDetails"].as_object() {
            return Err("JavaScript error".to_string());
        }
    }
    
    Err("No result".to_string())
}
