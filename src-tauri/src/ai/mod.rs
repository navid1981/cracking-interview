// Unified AI Service - coordinates between Gemini and Claude
// Ported from UnifiedAIService.swift

pub mod gemini;
pub mod claude;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub selected_model: String,
    pub gemini_api_key: String,
    pub claude_api_key: String,
}

/// Available AI models
#[derive(Debug)]
pub enum AIModel {
    Gemini25Flash,
    Claude4Sonnet,
    Claude3Haiku,
}

impl AIModel {
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "gemini-2.5-flash" => Some(Self::Gemini25Flash),
            "claude-sonnet-4-20250514" => Some(Self::Claude4Sonnet),
            "claude-3-5-haiku-20241022" => Some(Self::Claude3Haiku),
            _ => None,
        }
    }
    
    pub fn model_string(&self) -> &str {
        match self {
            Self::Gemini25Flash => "gemini-2.5-flash",
            Self::Claude4Sonnet => "claude-sonnet-4-20250514",
            Self::Claude3Haiku => "claude-3-5-haiku-20241022",
        }
    }
    
    pub fn is_gemini(&self) -> bool {
        matches!(self, Self::Gemini25Flash)
    }
}

/// Query AI with text (routes to correct provider)
pub async fn query_with_text(
    prompt: &str,
    config: &AIConfig,
) -> Result<String, String> {
    println!("🔍 Received model: '{}'", config.selected_model);
    
    let model = AIModel::from_string(&config.selected_model)
        .ok_or_else(|| {
            let error = format!("Invalid AI model: '{}'. Valid: gemini-2.5-flash, claude-sonnet-4-20250514, claude-3-5-haiku-20241022", config.selected_model);
            println!("❌ {}", error);
            error
        })?;
    
    println!("✅ Model parsed: {:?}", model);
    
    if model.is_gemini() {
        println!("→ Routing to Gemini");
        
        // Check for API key or OAuth token
        let api_key = if !config.gemini_api_key.is_empty() {
            config.gemini_api_key.clone()
        } else {
            // Try to get OAuth token
            println!("📝 No API key, checking for OAuth token...");
            match get_google_oauth_token().await {
                Ok(token) => {
                    println!("✅ Using OAuth token");
                    token
                }
                Err(e) => {
                    return Err(format!("⚠️ Gemini API key not configured and OAuth not available: {}", e));
                }
            }
        };
        
        gemini::query_with_text(prompt, &api_key, model.model_string()).await
    } else {
        println!("→ Routing to Claude");
        claude::query_with_text(prompt, &config.claude_api_key, model.model_string()).await
    }
}

/// Query AI with image (routes to correct provider)
pub async fn query_with_image(
    prompt: &str,
    image_data: &[u8],
    config: &AIConfig,
) -> Result<String, String> {
    let model = AIModel::from_string(&config.selected_model)
        .ok_or("Invalid AI model selected")?;
    
    if model.is_gemini() {
        // Check for API key or OAuth token
        let api_key = if !config.gemini_api_key.is_empty() {
            config.gemini_api_key.clone()
        } else {
            // Try to get OAuth token
            match get_google_oauth_token().await {
                Ok(token) => token,
                Err(e) => {
                    return Err(format!("⚠️ Gemini API key not configured and OAuth not available: {}", e));
                }
            }
        };
        
        gemini::query_with_image(prompt, image_data, &api_key, model.model_string()).await
    } else {
        claude::query_with_image(prompt, image_data, &config.claude_api_key, model.model_string()).await
    }
}

// Helper to get OAuth token
async fn get_google_oauth_token() -> Result<String, String> {
    // Load tokens from file
    let token_path = std::env::temp_dir().join("cracking_interview_google_tokens.json");
    
    if !token_path.exists() {
        return Err("No OAuth tokens found".to_string());
    }
    
    let json = std::fs::read_to_string(&token_path)
        .map_err(|e| format!("Failed to read tokens: {}", e))?;
    
    let tokens: crate::google_oauth::GoogleTokens = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse tokens: {}", e))?;
    
    // Check if expired
    let now = chrono::Utc::now().timestamp();
    if now >= tokens.expires_at {
        // Best-effort refresh (if we have a refresh token + client credentials).
        if let Some(refresh_token) = tokens.refresh_token.clone() {
            println!("🔄 OAuth access token expired; attempting refresh...");

            let client_id = std::env::var("GOOGLE_CLIENT_ID")
                .map_err(|_| "OAuth token expired - missing GOOGLE_CLIENT_ID; please set Gemini API key or configure Google OAuth".to_string())?;
            let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
                .map_err(|_| "OAuth token expired - missing GOOGLE_CLIENT_SECRET; please set Gemini API key or configure Google OAuth".to_string())?;

            let client = reqwest::Client::new();
            let params = [
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("refresh_token", refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ];

            let response = client
                .post("https://oauth2.googleapis.com/token")
                .form(&params)
                .send()
                .await
                .map_err(|e| format!("OAuth refresh failed: {}", e))?;

            let token_response: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse OAuth refresh response: {}", e))?;

            let access_token = token_response["access_token"]
                .as_str()
                .ok_or("OAuth refresh response missing access_token")?
                .to_string();

            let expires_in = token_response["expires_in"]
                .as_i64()
                .unwrap_or(3600);

            let refreshed = crate::google_oauth::GoogleTokens {
                access_token: access_token.clone(),
                refresh_token: Some(refresh_token),
                expires_at: chrono::Utc::now().timestamp() + expires_in,
            };

            // Persist refreshed token back to file so future calls succeed.
            let refreshed_json = serde_json::to_string_pretty(&refreshed)
                .map_err(|e| format!("Failed to serialize refreshed tokens: {}", e))?;
            std::fs::write(&token_path, refreshed_json)
                .map_err(|e| format!("Failed to write refreshed tokens: {}", e))?;

            println!("✅ OAuth token refreshed");
            return Ok(access_token);
        }

        return Err("OAuth token expired - please sign in again".to_string());
    }
    
    Ok(tokens.access_token)
}

/// Best-effort MIME sniffing based on file signatures ("magic bytes").
/// This avoids provider errors when the image bytes are JPEG but we label them PNG (or vice versa).
#[allow(dead_code)]
pub(crate) fn detect_image_mime_type(image_data: &[u8]) -> Result<&'static str, String> {
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if image_data.len() >= 8
        && image_data[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    {
        return Ok("image/png");
    }

    // JPEG: FF D8 FF
    if image_data.len() >= 3 && image_data[0..3] == [0xFF, 0xD8, 0xFF] {
        return Ok("image/jpeg");
    }

    // GIF: "GIF87a" or "GIF89a"
    if image_data.len() >= 6
        && (&image_data[0..6] == b"GIF87a" || &image_data[0..6] == b"GIF89a")
    {
        return Ok("image/gif");
    }

    // WebP: "RIFF"...."WEBP"
    if image_data.len() >= 12
        && &image_data[0..4] == b"RIFF"
        && &image_data[8..12] == b"WEBP"
    {
        return Ok("image/webp");
    }

    Err("Unsupported/unknown image format (expected png/jpeg/gif/webp)".to_string())
}
