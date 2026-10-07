// Gemini API service - ported from GeminiService.swift

use base64::{Engine as _, engine::general_purpose};
use serde_json::json;
use super::detect_image_mime_type;

fn is_overloaded_message(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("model is overloaded")
        || m.contains("overloaded")
        || m.contains("try again later")
        || m.contains("resource has been exhausted")
}

fn parse_retry_after_seconds(msg: &str) -> Option<f64> {
    // Gemini sometimes returns: "Please retry in 25.848918211s."
    // We'll parse the first "<number>s" after "retry in".
    let lower = msg.to_lowercase();
    let idx = lower.find("retry in")?;
    let tail = &msg[idx..];
    // Find the first number in the tail.
    let mut num = String::new();
    let mut seen_digit = false;
    for ch in tail.chars() {
        if ch.is_ascii_digit() || (ch == '.' && seen_digit) {
            seen_digit = true;
            num.push(ch);
        } else if seen_digit {
            break;
        }
    }
    if num.is_empty() {
        return None;
    }
    num.parse::<f64>().ok()
}

fn generation_config(max_output_tokens: Option<u32>) -> serde_json::Value {
    match max_output_tokens {
        Some(n) => json!({ "maxOutputTokens": n }),
        None => json!({}),
    }
}

fn is_quota_message(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("quota exceeded") || m.contains("exceeded your current quota") || m.contains("rate limit")
}

async fn post_with_retry(request: reqwest::RequestBuilder) -> Result<serde_json::Value, String> {
    // Small exponential backoff for transient overload errors.
    // Keep this tight so the UI doesn't feel stuck.
    let delays_ms = [350u64, 700u64, 1400u64];
    let mut quota_retries: u32 = 0;

    for (attempt, delay) in delays_ms.iter().enumerate() {
        let req = request
            .try_clone()
            .ok_or("Failed to clone Gemini request".to_string())?;

        let resp = req
            .send()
            .await
            .map_err(|e| format!("❌ Gemini request failed: {}", e))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("❌ Failed to read Gemini response: {}", e))?;

        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| serde_json::json!({ "raw": text }));

        // Success path
        if status.is_success() {
            return Ok(json);
        }

        // Detect overload, retry
        let msg = json["error"]["message"]
            .as_str()
            .unwrap_or("");

        // Respect server-advised retry delay for quota/rate limit errors.
        // (This is separate from "model overloaded".)
        if is_quota_message(msg) {
            if let Some(secs) = parse_retry_after_seconds(msg) {
                // Avoid very long sleeps; keep the UI responsive.
                if secs > 0.0 && secs <= 30.0 && quota_retries < 2 {
                    quota_retries += 1;
                    let ms = (secs * 1000.0).ceil() as u64;
                    println!("⏳ Gemini quota/rate-limit; retrying in {}ms (quota attempt {}/{})", ms, quota_retries, 2);
                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                    continue;
                }
            }
        }

        if is_overloaded_message(msg) && attempt < delays_ms.len() - 1 {
            println!("⏳ Gemini overloaded; retrying in {}ms (attempt {}/{})", delay, attempt + 1, delays_ms.len());
            tokio::time::sleep(std::time::Duration::from_millis(*delay)).await;
            continue;
        }

        return Err(error_message(status, &json, &text));
    }

    Err("❌ Gemini API Error: The model is overloaded. Please try again later.".to_string())
}

fn error_message(status: reqwest::StatusCode, json: &serde_json::Value, text: &str) -> String {
    let msg = json["error"]["message"].as_str().unwrap_or("");
    if !msg.is_empty() {
        if is_quota_message(msg) {
            return format!(
                "❌ Gemini API quota/rate-limit: {}\n\nTip: Check the quota for your Gemini API key in Google AI Studio.",
                msg
            );
        }
        return format!("❌ Gemini API Error: {}", msg);
    }
    // Google always returns JSON errors; an HTML body means a firewall/proxy answered instead.
    if text.trim_start().starts_with('<') {
        return format!(
            "❌ Gemini API blocked by your network (HTTP {}). A firewall or corporate proxy is blocking generativelanguage.googleapis.com. Try a different network.",
            status.as_u16()
        );
    }
    format!("❌ Gemini API Error: HTTP {} {}", status.as_u16(), status.canonical_reason().unwrap_or(""))
}

/// Checks that `api_key` is valid and can access `model`, without generating content (no quota use).
pub async fn validate_key(api_key: &str, model: &str) -> Result<(), String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("⚠️ Please enter an API key.".to_string());
    }
    if model.trim().is_empty() {
        return Err("⚠️ No AI model configured. Please sign in again so the model list can be loaded.".to_string());
    }

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Client build failed: {}", e))?;

    let resp = client
        .get(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}?key={}",
            model, api_key
        ))
        .send()
        .await
        .map_err(|e| format!("❌ Could not reach Google to verify the key: {}", e))?;

    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let text = resp.text().await.unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    let msg = json["error"]["message"].as_str().unwrap_or("").to_lowercase();

    if msg.contains("api key not valid") || msg.contains("api_key_invalid") {
        return Err("❌ Invalid API key. Copy it again from Google AI Studio.".to_string());
    }
    if status.as_u16() == 404 {
        return Err(format!("❌ This key can't access the required model ({}).", model));
    }
    Err(error_message(status, &json, &text))
}

/// Query Gemini with text only
pub async fn query_with_text(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: Option<u32>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Gemini API key not configured.".to_string());
    }
    
    // Note: danger_accept_invalid_certs is used to work around corporate proxy SSL interception
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Client build failed: {}", e))?;
    
    let payload = json!({
        "contents": [{
            "parts": [{"text": prompt}]
        }],
        "generationConfig": generation_config(max_output_tokens)
    });
    
    let request = client
        .post(generate_content_url(model, api_key))
        .json(&payload);
    
    let json = post_with_retry(request).await?;
    
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
    max_output_tokens: Option<u32>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Gemini API key not configured.".to_string());
    }
    
    // Base64 encode image
    let base64_image = general_purpose::STANDARD.encode(image_data);
    let mime_type = detect_image_mime_type(image_data)?;
    
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Client build failed: {}", e))?;
    
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
        }],
        "generationConfig": generation_config(max_output_tokens)
    });
    
    let request = client
        .post(generate_content_url(model, api_key))
        .json(&payload);
    
    let json = post_with_retry(request).await?;
    
    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else if let Some(error) = json["error"]["message"].as_str() {
        Err(format!("❌ Gemini API Error: {}", error))
    } else {
        Err("⚠️ No response from Gemini".to_string())
    }
}

fn generate_content_url(model: &str, api_key: &str) -> String {
    format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    )
}