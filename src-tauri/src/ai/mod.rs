// Direct (bring-your-own-key) AI calls. Only Gemini keys are supported.
// Ported from UnifiedAIService.swift

pub mod gemini;

use serde::{Deserialize, Serialize};

/// Config for direct (bring-your-own-key) provider calls.
///
/// `selected_model` (`byo_model`) and `max_output_tokens` are supplied by the server
/// (`get-models` edge function); the app never hardcodes them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    pub selected_model: String,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
}

fn validate_config(config: &AIConfig) -> Result<(), String> {
    if config.selected_model.trim().is_empty() {
        return Err("⚠️ No AI model configured. Please sign in again so the model list can be loaded.".to_string());
    }
    if config.gemini_api_key.is_empty() {
        return Err("⚠️ No API key configured. Add your API key in Settings → AI Models.".to_string());
    }
    Ok(())
}

/// Query AI with text
pub async fn query_with_text(
    prompt: &str,
    config: &AIConfig,
) -> Result<String, String> {
    validate_config(config)?;
    gemini::query_with_text(prompt, &config.gemini_api_key, &config.selected_model, config.max_output_tokens).await
}

/// Query AI with image
pub async fn query_with_image(
    prompt: &str,
    image_data: &[u8],
    config: &AIConfig,
) -> Result<String, String> {
    validate_config(config)?;
    gemini::query_with_image(prompt, image_data, &config.gemini_api_key, &config.selected_model, config.max_output_tokens).await
}

/// Best-effort MIME sniffing based on file signatures ("magic bytes").
/// This avoids provider errors when the image bytes are JPEG but we label them PNG (or vice versa).
pub fn detect_image_mime_type(image_data: &[u8]) -> Result<&'static str, String> {
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
