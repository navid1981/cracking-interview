// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome;
mod ai;
mod screenshot;

use tauri::Manager;

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
async fn extract_leetcode_problem(tab_id: String) -> Result<String, String> {
    let js = r#"
    JSON.stringify({
        title: document.querySelector('[data-cy="question-title"]')?.innerText || 
               document.querySelector('.css-v3d350')?.innerText || 
               document.querySelector('h1')?.innerText || 
               'Title not found',
        difficulty: document.querySelector('[diff]')?.getAttribute('diff') || 
                   document.querySelector('.text-difficulty-easy, .text-difficulty-medium, .text-difficulty-hard')?.innerText ||
                   'Unknown',
        description: document.querySelector('[data-track-load="description-content"]')?.innerText ||
                    document.querySelector('.elfjS')?.innerText ||
                    document.querySelector('.question-content')?.innerText ||
                    'Description not found',
        platform: window.location.hostname,
        url: window.location.href
    })
    "#;
    
    chrome::execute_javascript(&tab_id, js).await
}

// ============================================================================
// AI COMMANDS
// ============================================================================

#[tauri::command]
async fn query_ai(
    prompt: String,
    config: ai::AIConfig,
) -> Result<String, String> {
    println!("🤖 Querying AI: {}", config.selected_model);
    println!("🔑 API key present: Gemini={}, Claude={}", 
        !config.gemini_api_key.is_empty(),
        !config.claude_api_key.is_empty()
    );
    
    ai::query_with_text(&prompt, &config).await
}

#[tauri::command]
async fn query_ai_with_image(
    prompt: String,
    image_path: String,
    config: ai::AIConfig,
) -> Result<String, String> {
    println!("🤖 Querying AI with image: {}", config.selected_model);
    
    // Read image file
    let image_data = std::fs::read(&image_path)
        .map_err(|e| format!("Failed to read image: {}", e))?;
    
    ai::query_with_image(&prompt, &image_data, &config).await
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            // Chrome CDP
            get_chrome_tabs,
            get_cdp_status,
            open_chrome_cdp,
            extract_tab_text,
            activate_tab,
            extract_leetcode_problem,
            // AI
            query_ai,
            query_ai_with_image,
        ])
        .setup(|app| {
            println!("🚀 CrackingInterview starting...");
            println!("💡 Click 'Open Chrome CDP' button to launch debugging Chrome");
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
