// Gemini API service - ported from GeminiService.swift

use base64::{Engine as _, engine::general_purpose};
use serde_json::json;
use super::detect_image_mime_type;

/// Query Gemini with text only
pub async fn query_with_text(
    prompt: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Gemini API key not configured.".to_string());
    }
    
    // Check if this is an OAuth token (starts with "ya29.") or API key
    let is_oauth = api_key.starts_with("ya29.");
    
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| format!("Client build failed: {}", e))?;
    
    let payload = json!({
        "contents": [{
            "parts": [{"text": prompt}]
        }]
    });
    
    let response = if is_oauth {
        // OAuth token - use Authorization header
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model
        );
        
        println!("🔐 Using OAuth token with Authorization header");
        
        client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("❌ Gemini request failed: {}", e))?
    } else {
        // API key - use query parameter
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );
        
        println!("🔑 Using API key in query parameter");
        
        client
            .post(&endpoint)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("❌ Gemini request failed: {}", e))?
    };
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("❌ Failed to parse Gemini response: {}", e))?;
    
    // Extract text from response
    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else if let Some(error) = json["error"]["message"].as_str() {
        Err(format!("❌ Gemini API Error: {}", error))
    } else {
        Err("⚠️ No response from Gemini".to_string())
    }
}

/// Query Gemini with image (screenshot)
pub async fn query_with_image(
    prompt: &str,
    image_data: &[u8],
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Gemini API key not configured.".to_string());
    }
    
    // Check if this is an OAuth token or API key
    let is_oauth = api_key.starts_with("ya29.");
    
    // Base64 encode image
    let base64_image = general_purpose::STANDARD.encode(image_data);
    let mime_type = detect_image_mime_type(image_data)?;
    
    let client = reqwest::Client::new();
    
    let payload = json!({
        "contents": [{
            "parts": [
                {"text": prompt},
                {
                    "inline_data": {
                        "mime_type": mime_type,
                        "data": base64_image
                    }
                }
            ]
        }]
    });
    
    let response = if is_oauth {
        // OAuth token - use Authorization header
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model
        );
        
        client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("❌ Gemini request failed: {}", e))?
    } else {
        // API key - use query parameter
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );
        
        client
            .post(&endpoint)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("❌ Gemini request failed: {}", e))?
    };
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("❌ Failed to parse response: {}", e))?;
    
    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else if let Some(error) = json["error"]["message"].as_str() {
        Err(format!("❌ Gemini API Error: {}", error))
    } else {
        Err("⚠️ No response from Gemini".to_string())
    }
}
