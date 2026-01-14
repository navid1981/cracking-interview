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


/// Capture screenshot of tab
pub async fn capture_screenshot(tab_id: &str) -> Result<Vec<u8>, String> {
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
    
    // Capture as JPEG with quality 80 to stay under 5MB
    let command = serde_json::json!({
        "id": 1,
        "method": "Page.captureScreenshot",
        "params": {
            "format": "jpeg",
            "quality": 80,
            "captureBeyondViewport": true
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
        
        if let Some(data) = response["result"]["data"].as_str() {
            use base64::{Engine as _, engine::general_purpose};
            let bytes = general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("Base64 decode failed: {}", e))?;
            
            println!("📸 Chrome tab screenshot: {} bytes ({:.2} MB)", bytes.len(), bytes.len() as f64 / 1_000_000.0);
            
            // If over 4.5MB, we need to re-compress
            if bytes.len() > 4_500_000 {
                println!("⚠️  Image too large, compressing...");
                
                // Load as image and re-encode with lower quality
                use image::io::Reader as ImageReader;
                let img = ImageReader::new(std::io::Cursor::new(&bytes))
                    .with_guessed_format()
                    .map_err(|e| format!("Failed to read image: {}", e))?
                    .decode()
                    .map_err(|e| format!("Failed to decode: {}", e))?;
                
                // Resize if needed (max 2000px)
                let img = if img.width() > 2000 || img.height() > 2000 {
                    let scale = 2000.0 / img.width().max(img.height()) as f32;
                    let new_w = (img.width() as f32 * scale) as u32;
                    let new_h = (img.height() as f32 * scale) as u32;
                    println!("📏 Resizing to {}x{}", new_w, new_h);
                    img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
                } else {
                    img
                };
                
                // Re-encode as JPEG with lower quality
                let mut jpeg_bytes = Vec::new();
                img.write_to(
                    &mut std::io::Cursor::new(&mut jpeg_bytes),
                    image::ImageFormat::Jpeg
                ).map_err(|e| format!("Failed to encode: {}", e))?;
                
                println!("✅ Compressed to {} bytes ({:.2} MB)", jpeg_bytes.len(), jpeg_bytes.len() as f64 / 1_000_000.0);
                return Ok(jpeg_bytes);
            }
            
            return Ok(bytes);
        }
        
        if let Some(_error) = response["result"]["error"].as_object() {
            return Err("Screenshot capture failed".to_string());
        }
    }
    
    Err("No screenshot data received".to_string())
}


/// Capture thumbnail (small screenshot) of tab
pub async fn capture_thumbnail(tab_id: &str) -> Result<Vec<u8>, String> {
    use tokio_tungstenite::connect_async;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    
    // Get WebSocket URL for the tab
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
    
    // Connect to WebSocket
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("WebSocket failed: {}", e))?;
    
    let (mut write, mut read) = ws_stream.split();
    
    // Capture small thumbnail (quality: 50, format: jpeg for smaller size)
    let command = serde_json::json!({
        "id": 1,
        "method": "Page.captureScreenshot",
        "params": {
            "format": "jpeg",
            "quality": 50,
            "captureBeyondViewport": false,
            "fromSurface": true
        }
    });
    
    write.send(Message::Text(command.to_string()))
        .await
        .map_err(|e| format!("Send failed: {}", e))?;
    
    // Read response
    if let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| format!("Read failed: {}", e))?;
        let text = msg.to_text().map_err(|e| format!("Invalid: {}", e))?;
        
        let response: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| format!("Parse failed: {}", e))?;
        
        // Extract base64 image data
        if let Some(data) = response["result"]["data"].as_str() {
            // Decode base64 to bytes
            use base64::{Engine as _, engine::general_purpose};
            let bytes = general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("Base64 decode failed: {}", e))?;
            
            return Ok(bytes);
        }
    }
    
    Err("No thumbnail data received".to_string())
}
