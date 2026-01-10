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
        gemini::query_with_text(prompt, &config.gemini_api_key, model.model_string()).await
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
        gemini::query_with_image(prompt, image_data, &config.gemini_api_key, model.model_string()).await
    } else {
        claude::query_with_image(prompt, image_data, &config.claude_api_key, model.model_string()).await
    }
}
