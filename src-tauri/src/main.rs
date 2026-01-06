// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome;
mod ai;
mod screenshot;

use tauri::{Emitter, Manager};

// Commands
#[tauri::command]
async fn get_chrome_tabs() -> Result<Vec<chrome::ChromeTab>, String> {
    chrome::get_all_tabs().await
}

#[tauri::command]
async fn test_chrome_cdp() -> Result<String, String> {
    Ok(chrome::get_cdp_status().await)
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

// AUTO-LAUNCH Chrome CDP
#[tauri::command]
async fn start_chrome_cdp() -> Result<String, String> {
    chrome::ensure_chrome_cdp().await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_chrome_tabs,
            test_chrome_cdp,
            extract_tab_text,
            activate_tab,
            extract_leetcode_problem,
            start_chrome_cdp,
        ])
        .setup(|app| {
            println!("🚀 CrackingInterview starting...");
            
            // AUTO-LAUNCH Chrome CDP on startup
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                println!("🔌 Auto-launching Chrome with CDP...");
                
                match chrome::ensure_chrome_cdp().await {
                    Ok(msg) => {
                        println!("✅ {}", msg);
                        app_handle.emit("chrome-ready", ()).ok();
                    }
                    Err(e) => {
                        eprintln!("⚠️  Chrome CDP error: {}", e);
                        app_handle.emit("chrome-error", e).ok();
                    }
                }
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
