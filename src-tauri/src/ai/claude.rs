// Claude API integration
// Port of your ClaudeService.swift

// PLACEHOLDER - Will implement in Phase 2, Day 4-5
pub async fn query_with_text(
    _prompt: &str,
    _api_key: &str,
    _model: &str,
) -> Result<String, String> {
    Ok("Claude service - Coming in Phase 2!".to_string())
}

pub async fn query_with_image(
    _prompt: &str,
    _image_data: &[u8],
    _api_key: &str,
    _model: &str,
) -> Result<String, String> {
    Ok("Claude image service - Coming in Phase 2!".to_string())
}
