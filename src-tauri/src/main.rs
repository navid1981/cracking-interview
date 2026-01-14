// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome;
mod ai;
mod screenshot;
mod oauth_server;
mod google_oauth;

use tauri::Manager;
use std::sync::Arc;

lazy_static::lazy_static! {
    static ref OAUTH_SERVICE: Arc<google_oauth::GoogleOAuthService> = {
        Arc::new(google_oauth::GoogleOAuthService::new())
    };
}

// ============================================================================
// CHROME CDP COMMANDS
// ============================================================================

#[tauri::command]
async fn get_chrome_tabs() -> Result<Vec<chrome::ChromeTab>, String> {
    chrome::get_all_tabs().await
}

#[tauri::command]
async fn get_cdp_status() -> Result<String, String> {
    Ok(chrome::get_cdp_status().await)
}

#[tauri::command]
async fn open_chrome_cdp() -> Result<String, String> {
    chrome::launch_chrome_cdp_window().await
}

#[tauri::command]
async fn extract_tab_text(tab_id: String) -> Result<String, String> {
    chrome::execute_javascript(&tab_id, "document.body.innerText").await
}

#[tauri::command]
async fn activate_tab(tab_id: String) -> Result<(), String> {
    chrome::activate_tab(&tab_id).await
}

#[tauri::command]
async fn capture_tab_screenshot(tab_id: String) -> Result<String, String> {
    let screenshot_bytes = chrome::capture_screenshot(&tab_id).await?;
    let mut screenshot_path = std::env::temp_dir();
    screenshot_path.push("cracking_interview_screenshot.png");
    std::fs::write(&screenshot_path, screenshot_bytes)
        .map_err(|e| format!("Failed to save screenshot: {}", e))?;
    Ok(screenshot_path.to_str().ok_or("Invalid path")?.to_string())
}

#[tauri::command]
async fn get_tab_thumbnail(tab_id: String) -> Result<String, String> {
    let thumbnail_bytes = chrome::capture_thumbnail(&tab_id).await?;
    use base64::{Engine as _, engine::general_purpose};
    let base64_data = general_purpose::STANDARD.encode(&thumbnail_bytes);
    Ok(format!("data:image/jpeg;base64,{}", base64_data))
}


// ============================================================================
// SCREEN CAPTURE COMMANDS
// ============================================================================

#[tauri::command]
async fn get_displays() -> Result<Vec<screenshot::DisplayInfo>, String> {
    screenshot::get_all_displays()
}

#[tauri::command]
async fn capture_display_screenshot(display_id: String) -> Result<String, String> {
    let screenshot_bytes = screenshot::capture_display_screenshot(&display_id)?;
    let mut screenshot_path = std::env::temp_dir();
    screenshot_path.push("cracking_interview_display_screenshot.jpg");
    std::fs::write(&screenshot_path, screenshot_bytes)
        .map_err(|e| format!("Failed to save screenshot: {}", e))?;
    Ok(screenshot_path.to_str().ok_or("Invalid path")?.to_string())
}

#[tauri::command]
async fn get_display_thumbnail(display_id: String) -> Result<String, String> {
    let thumbnail_bytes = screenshot::capture_display_thumbnail(&display_id)?;
    use base64::{Engine as _, engine::general_purpose};
    let base64_data = general_purpose::STANDARD.encode(&thumbnail_bytes);
    Ok(format!("data:image/jpeg;base64,{}", base64_data))
}


// ============================================================================
// AI COMMANDS
// ============================================================================

#[tauri::command]
async fn query_ai(prompt: String, config: ai::AIConfig) -> Result<String, String> {
    ai::query_with_text(&prompt, &config).await
}

#[tauri::command]
async fn query_ai_with_image(
    prompt: String,
    image_path: String,
    config: ai::AIConfig,
) -> Result<String, String> {
    let image_data = std::fs::read(&image_path)
        .map_err(|e| format!("Failed to read image: {}", e))?;
    ai::query_with_image(&prompt, &image_data, &config).await
}


// ============================================================================
// GOOGLE OAUTH COMMANDS
// ============================================================================

#[tauri::command]
fn start_google_oauth() -> Result<String, String> {
    let (_, code_receiver) = oauth_server::start_oauth_server()?;
    let auth_url = OAUTH_SERVICE.get_auth_url();
    
    println!("🔐 Auth URL: {}", auth_url);
    
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&auth_url).spawn().ok();
    
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/C", "start", &auth_url]).spawn().ok();
    
    // Wait for code (blocks this thread for up to 120 seconds)
    let code = code_receiver.recv_timeout(std::time::Duration::from_secs(120))
        .map_err(|_| "Authentication timeout - please try again".to_string())?;
    
    println!("✅ Got authorization code");
    
    // Exchange code for tokens (need async runtime)
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tokens = rt.block_on(OAUTH_SERVICE.exchange_code(&code))?;
    
    // Save tokens
    let token_path = std::env::temp_dir().join("cracking_interview_google_tokens.json");
    rt.block_on(OAUTH_SERVICE.save_tokens(token_path.to_str().unwrap()))?;
    
    println!("✅ Tokens saved");
    Ok("Google Sign-In successful! Tokens saved.".to_string())
}

#[tauri::command]
fn get_google_token_status() -> Result<bool, String> {
    let token_path = std::env::temp_dir().join("cracking_interview_google_tokens.json");
    Ok(token_path.exists())
}


// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // Load environment variables from .env file (must be in project root)
    if let Err(e) = dotenv::from_filename("../.env") {
        println!("⚠️  Warning: Could not load .env file: {}", e);
        println!("💡 Google OAuth will not work without GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET");
    } else {
        println!("✅ Loaded environment variables from .env");
    }
    
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_chrome_tabs,
            get_cdp_status,
            open_chrome_cdp,
            extract_tab_text,
            activate_tab,
            capture_tab_screenshot,
            get_tab_thumbnail,
            get_displays,
            capture_display_screenshot,
            get_display_thumbnail,
            query_ai,
            query_ai_with_image,
            start_google_oauth,
            get_google_token_status,
            clear_google_tokens,
        ])
        .setup(|_| {
            println!("🚀 CrackingInterview starting...");
            screenshot::request_screen_recording_permission();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[tauri::command]
fn clear_google_tokens() -> Result<String, String> {
    let token_path = std::env::temp_dir().join("cracking_interview_google_tokens.json");
    
    if token_path.exists() {
        std::fs::remove_file(token_path)
            .map_err(|e| format!("Failed to delete tokens: {}", e))?;
        println!("✅ Google tokens cleared");
        Ok("Signed out successfully".to_string())
    } else {
        Ok("No tokens to clear".to_string())
    }
}
