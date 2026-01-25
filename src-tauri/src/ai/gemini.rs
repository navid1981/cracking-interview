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

        // Non-retryable error
        if !msg.is_empty() {
            if is_quota_message(msg) {
                return Err(format!(
                    "❌ Gemini API quota/rate-limit: {}\n\nTip: If you’re using Google Sign-In (OAuth), you may still hit shared free-tier limits. To use your own quota, paste your own Gemini API key in Settings → AI Models.",
                    msg
                ));
            }
            return Err(format!("❌ Gemini API Error: {}", msg));
        }
        return Err(format!("❌ Gemini API Error: HTTP {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
    }

    Err("❌ Gemini API Error: The model is overloaded. Please try again later.".to_string())
}

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
    
    let request = if is_oauth {
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
    };
    
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
    
    let request = if is_oauth {
        // OAuth token - use Authorization header
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model
        );
        
        client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
    } else {
        // API key - use query parameter
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );
        
        client
            .post(&endpoint)
            .json(&payload)
    };
    
    let json = post_with_retry(request).await?;
    
    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else if let Some(error) = json["error"]["message"].as_str() {
        Err(format!("❌ Gemini API Error: {}", error))
    } else {
        Err("⚠️ No response from Gemini".to_string())
    }
}

/// Query Gemini with audio (system audio recording).
///
/// Gemini supports audio as inline data, similar to images.
/// We use `audio/wav` since we record a WAV file on disk.
pub async fn query_with_audio(
    prompt: &str,
    audio_data: &[u8],
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("⚠️ Gemini API key not configured.".to_string());
    }

    // OAuth token vs API key
    let is_oauth = api_key.starts_with("ya29.");

    // Base64 encode audio
    let base64_audio = general_purpose::STANDARD.encode(audio_data);

    let client = reqwest::Client::new();

    let payload = json!({
        "contents": [{
            "parts": [
                {"text": prompt},
                {
                    "inline_data": {
                        "mime_type": "audio/wav",
                        "data": base64_audio
                    }
                }
            ]
        }]
    });

    let request = if is_oauth {
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            model
        );
        client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
    } else {
        let endpoint = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );
        client
            .post(&endpoint)
            .json(&payload)
    };

    let json = post_with_retry(request).await?;

    if let Some(text) = json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        Ok(text.to_string())
    } else if let Some(error) = json["error"]["message"].as_str() {
        Err(format!("❌ Gemini API Error: {}", error))
    } else {
        Err("⚠️ No response from Gemini".to_string())
    }
}
