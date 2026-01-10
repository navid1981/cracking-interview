// Claude (Anthropic) API service - ported from ClaudeService.swift

use base64::{Engine as _, engine::general_purpose};
use serde_json::json;

/// Query Claude with text only
pub async fn query_with_text(
    prompt: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Claude API key not configured.".to_string());
    }
    
    let endpoint = "https://api.anthropic.com/v1/messages";
    
    let payload = json!({
        "model": model,
        "max_tokens": 4096,
        "messages": [{
            "role": "user",
            "content": prompt
        }]
    });
    
    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("❌ Claude request failed: {}", e))?;
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("❌ Failed to parse Claude response: {}", e))?;
    
    // Extract text from response (same logic as Swift)
    if let Some(content_array) = json["content"].as_array() {
        for content in content_array {
            if let Some(text) = content["text"].as_str() {
                return Ok(text.to_string());
            }
        }
    }
    
    if let Some(error) = json["error"]["message"].as_str() {
        return Err(format!("❌ Claude API Error: {}", error));
    }
    
    Err("⚠️ No response from Claude".to_string())
}

/// Query Claude with image (screenshot)
pub async fn query_with_image(
    prompt: &str,
    image_data: &[u8],
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Claude API key not configured.".to_string());
    }
    
    // Base64 encode image
    let base64_image = general_purpose::STANDARD.encode(image_data);
    
    let endpoint = "https://api.anthropic.com/v1/messages";
    
    let payload = json!({
        "model": model,
        "max_tokens": 4096,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": base64_image
                    }
                },
                {
                    "type": "text",
                    "text": prompt
                }
            ]
        }]
    });
    
    let client = reqwest::Client::new();
    let response = client
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("❌ Claude request failed: {}", e))?;
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("❌ Failed to parse response: {}", e))?;
    
    // Extract text
    if let Some(content_array) = json["content"].as_array() {
        for content in content_array {
            if let Some(text) = content["text"].as_str() {
                return Ok(text.to_string());
            }
        }
    }
    
    if let Some(error) = json["error"]["message"].as_str() {
        return Err(format!("❌ Claude API Error: {}", error));
    }
    
    Err("⚠️ No response from Claude".to_string())
}
