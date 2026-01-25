// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chrome;
mod ai;
mod screenshot;
mod audio;
mod oauth_server;
mod google_oauth;

use std::sync::Arc;
use std::sync::Mutex;
use std::path::PathBuf;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use serde::{Serialize, Deserialize};

lazy_static::lazy_static! {
    static ref OAUTH_SERVICE: Arc<google_oauth::GoogleOAuthService> = {
        Arc::new(google_oauth::GoogleOAuthService::new())
    };
}

// Two dedicated hotkeys:
// - Extract (text) → Solve
// - Screenshot → Solve
//
// Note: we use OS-specific combinations to match user expectations.
#[cfg(target_os = "macos")]
const DEFAULT_SOLVE_TEXT_HOTKEY: &str = "Cmd+E";
#[cfg(target_os = "macos")]
const DEFAULT_SOLVE_SCREENSHOT_HOTKEY: &str = "Cmd+S";

#[cfg(target_os = "windows")]
const DEFAULT_SOLVE_TEXT_HOTKEY: &str = "Alt+E";
#[cfg(target_os = "windows")]
const DEFAULT_SOLVE_SCREENSHOT_HOTKEY: &str = "Alt+S";

// Reasonable fallback for other OSes (Linux)
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_SOLVE_TEXT_HOTKEY: &str = "Ctrl+E";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_SOLVE_SCREENSHOT_HOTKEY: &str = "Ctrl+S";

// Audio (system) hotkey default (configurable).
#[cfg(target_os = "macos")]
const DEFAULT_AUDIO_TOGGLE_HOTKEY: &str = "Cmd+A";
#[cfg(target_os = "windows")]
const DEFAULT_AUDIO_TOGGLE_HOTKEY: &str = "Alt+A";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_AUDIO_TOGGLE_HOTKEY: &str = "Ctrl+A";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HotkeysConfig {
    text: String,
    screenshot: String,
    audio_toggle: String,
    scroll_up: String,
    scroll_down: String,
    move_up: String,
    move_down: String,
    move_left: String,
    move_right: String,
    toggle_visibility: String,
    quit_app: String,
}

struct HotkeysState(Mutex<HotkeysConfig>);

fn default_hotkeys() -> HotkeysConfig {
    HotkeysConfig {
        text: DEFAULT_SOLVE_TEXT_HOTKEY.to_string(),
        screenshot: DEFAULT_SOLVE_SCREENSHOT_HOTKEY.to_string(),
        audio_toggle: DEFAULT_AUDIO_TOGGLE_HOTKEY.to_string(),
        scroll_up: DEFAULT_SCROLL_UP_HOTKEY.to_string(),
        scroll_down: DEFAULT_SCROLL_DOWN_HOTKEY.to_string(),
        move_up: DEFAULT_MOVE_UP_HOTKEY.to_string(),
        move_down: DEFAULT_MOVE_DOWN_HOTKEY.to_string(),
        move_left: DEFAULT_MOVE_LEFT_HOTKEY.to_string(),
        move_right: DEFAULT_MOVE_RIGHT_HOTKEY.to_string(),
        toggle_visibility: DEFAULT_TOGGLE_VISIBILITY_HOTKEY.to_string(),
        quit_app: DEFAULT_QUIT_APP_HOTKEY.to_string(),
    }
}

// Scroll hotkeys defaults (configurable).
#[cfg(target_os = "macos")]
const DEFAULT_SCROLL_UP_HOTKEY: &str = "Cmd+Up";
#[cfg(target_os = "macos")]
const DEFAULT_SCROLL_DOWN_HOTKEY: &str = "Cmd+Down";

#[cfg(not(target_os = "macos"))]
const DEFAULT_SCROLL_UP_HOTKEY: &str = "Ctrl+Up";
#[cfg(not(target_os = "macos"))]
const DEFAULT_SCROLL_DOWN_HOTKEY: &str = "Ctrl+Down";

// Window move hotkeys defaults (configurable).
#[cfg(target_os = "macos")]
const DEFAULT_MOVE_UP_HOTKEY: &str = "Cmd+Shift+Up";
#[cfg(target_os = "macos")]
const DEFAULT_MOVE_DOWN_HOTKEY: &str = "Cmd+Shift+Down";
#[cfg(target_os = "macos")]
const DEFAULT_MOVE_LEFT_HOTKEY: &str = "Cmd+Shift+Left";
#[cfg(target_os = "macos")]
const DEFAULT_MOVE_RIGHT_HOTKEY: &str = "Cmd+Shift+Right";

#[cfg(target_os = "windows")]
const DEFAULT_MOVE_UP_HOTKEY: &str = "Alt+Shift+Up";
#[cfg(target_os = "windows")]
const DEFAULT_MOVE_DOWN_HOTKEY: &str = "Alt+Shift+Down";
#[cfg(target_os = "windows")]
const DEFAULT_MOVE_LEFT_HOTKEY: &str = "Alt+Shift+Left";
#[cfg(target_os = "windows")]
const DEFAULT_MOVE_RIGHT_HOTKEY: &str = "Alt+Shift+Right";

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_MOVE_UP_HOTKEY: &str = "Ctrl+Shift+Up";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_MOVE_DOWN_HOTKEY: &str = "Ctrl+Shift+Down";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_MOVE_LEFT_HOTKEY: &str = "Ctrl+Shift+Left";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_MOVE_RIGHT_HOTKEY: &str = "Ctrl+Shift+Right";

// Toggle app visibility (show/hide) defaults (configurable).
#[cfg(target_os = "macos")]
const DEFAULT_TOGGLE_VISIBILITY_HOTKEY: &str = "Cmd+Shift+H";
#[cfg(target_os = "windows")]
const DEFAULT_TOGGLE_VISIBILITY_HOTKEY: &str = "Alt+Shift+H";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_TOGGLE_VISIBILITY_HOTKEY: &str = "Ctrl+Shift+H";

// Quit app defaults (configurable).
#[cfg(target_os = "macos")]
const DEFAULT_QUIT_APP_HOTKEY: &str = "Cmd+Shift+Q";
#[cfg(target_os = "windows")]
const DEFAULT_QUIT_APP_HOTKEY: &str = "Alt+Shift+Q";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_QUIT_APP_HOTKEY: &str = "Ctrl+Shift+Q";

fn hotkeys_config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve app config dir: {}", e))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    Ok(dir.join("hotkeys.json"))
}

#[derive(Debug, Clone, Deserialize)]
struct HotkeysConfigFile {
    text: Option<String>,
    screenshot: Option<String>,
    audio_toggle: Option<String>,
    scroll_up: Option<String>,
    scroll_down: Option<String>,
    move_up: Option<String>,
    move_down: Option<String>,
    move_left: Option<String>,
    move_right: Option<String>,
    toggle_visibility: Option<String>,
    quit_app: Option<String>,
}

fn load_hotkeys_from_disk(app: &tauri::AppHandle) -> HotkeysConfig {
    let path = match hotkeys_config_path(app) {
        Ok(p) => p,
        Err(e) => {
            println!("⚠️ Hotkeys: {}", e);
            return default_hotkeys();
        }
    };

    if !path.exists() {
        return default_hotkeys();
    }

    match std::fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<HotkeysConfigFile>(&json) {
            Ok(cfg_file) => {
                let mut cfg = default_hotkeys();
                if let Some(v) = cfg_file.text { if !v.trim().is_empty() { cfg.text = v; } }
                if let Some(v) = cfg_file.screenshot { if !v.trim().is_empty() { cfg.screenshot = v; } }
                if let Some(v) = cfg_file.audio_toggle { if !v.trim().is_empty() { cfg.audio_toggle = v; } }
                if let Some(v) = cfg_file.scroll_up { if !v.trim().is_empty() { cfg.scroll_up = v; } }
                if let Some(v) = cfg_file.scroll_down { if !v.trim().is_empty() { cfg.scroll_down = v; } }
                if let Some(v) = cfg_file.move_up { if !v.trim().is_empty() { cfg.move_up = v; } }
                if let Some(v) = cfg_file.move_down { if !v.trim().is_empty() { cfg.move_down = v; } }
                if let Some(v) = cfg_file.move_left { if !v.trim().is_empty() { cfg.move_left = v; } }
                if let Some(v) = cfg_file.move_right { if !v.trim().is_empty() { cfg.move_right = v; } }
                if let Some(v) = cfg_file.toggle_visibility { if !v.trim().is_empty() { cfg.toggle_visibility = v; } }
                if let Some(v) = cfg_file.quit_app { if !v.trim().is_empty() { cfg.quit_app = v; } }

                // Migrate legacy default audio hotkey to new default (Cmd+A / Alt+A) so existing users
                // pick up the change automatically unless they had customized it.
                let legacy_audio = [
                    "Cmd+Shift+A",
                    "Alt+Shift+A",
                    "Ctrl+Shift+A",
                ];
                if legacy_audio.contains(&cfg.audio_toggle.as_str()) {
                    cfg.audio_toggle = DEFAULT_AUDIO_TOGGLE_HOTKEY.to_string();
                    // Best-effort persist so it sticks across restarts.
                    save_hotkeys_to_disk(app, &cfg).ok();
                }
                cfg
            }
            Err(e) => {
                println!("⚠️ Failed to parse hotkeys.json: {}", e);
                default_hotkeys()
            }
        },
        Err(e) => {
            println!("⚠️ Failed to read hotkeys.json: {}", e);
            default_hotkeys()
        }
    }
}

fn save_hotkeys_to_disk(app: &tauri::AppHandle, cfg: &HotkeysConfig) -> Result<(), String> {
    let path = hotkeys_config_path(app)?;
    let json = serde_json::to_string_pretty(cfg).map_err(|e| format!("Serialize failed: {}", e))?;
    std::fs::write(path, json).map_err(|e| format!("Write failed: {}", e))?;
    Ok(())
}

fn normalize_hotkey(input: &str) -> String {
    let mut s = input.trim().to_string();
    if s.is_empty() {
        return s;
    }

    // Normalize common user-friendly tokens.
    // Examples accepted:
    // - "Command + E" -> "Cmd+E"
    // - "Cmd + E" -> "Cmd+E"
    // - "Alt + S" -> "Alt+S"
    // - "Ctrl + Shift + L" -> "Ctrl+Shift+L"
    let replacements = [
        ("command", "cmd"),
        ("cmd", "cmd"),
        ("control", "ctrl"),
        ("ctrl", "ctrl"),
        ("option", "alt"),
        ("alt", "alt"),
        ("shift", "shift"),
        ("cmdorctrl", "cmdorctrl"),
    ];

    // Remove spaces and unify separators.
    s = s.replace(" ", "");
    s = s.replace("-", "+");

    let lower = s.to_lowercase();
    // Rebuild by scanning tokens split by '+'
    let parts: Vec<String> = lower
        .split('+')
        .filter(|p| !p.is_empty())
        .map(|p| {
            for (from, to) in replacements {
                if p == from {
                    return to.to_string();
                }
            }
            p.to_string()
        })
        .collect();

    // Capitalize modifiers and uppercase single-letter keys.
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        let formatted = match p.as_str() {
            "cmd" => "Cmd".to_string(),
            "ctrl" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "cmdorctrl" => "CmdOrCtrl".to_string(),
            other => {
                if other.len() == 1 {
                    other.to_uppercase()
                } else {
                    // Keep key names (e.g. "f1") as-is but uppercase first letter.
                    let mut chars = other.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => other.to_string(),
                    }
                }
            }
        };
        out.push(formatted);
    }
    out.join("+")
}

fn unregister_hotkey_best_effort(app: &tauri::AppHandle, hotkey: &str) {
    // We ignore errors here (e.g., if not registered).
    let _ = app.global_shortcut().unregister(hotkey);
}

fn register_hotkey(
    app: &tauri::AppHandle,
    hotkey: &str,
    event_name: &'static str,
    label: &'static str,
) -> Result<(), String> {
    let hk = hotkey.to_string();
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                println!("⌨️ Hotkey pressed ({}): {}", label, hk);
                // Make it obvious something happened: bring the main window to the front.
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }
                handle.emit(event_name, ()).ok();
            }
        })
        .map_err(|e| e.to_string())
}

fn register_hotkeys(app: &tauri::AppHandle, cfg: &HotkeysConfig) -> Result<(), String> {
    register_hotkey(app, &cfg.text, "hotkey-solve-text", "text")?;
    register_hotkey(app, &cfg.screenshot, "hotkey-solve-screenshot", "screenshot")?;
    register_hotkey(app, &cfg.audio_toggle, "hotkey-audio-toggle", "audio-toggle")?;
    register_hotkey(app, &cfg.scroll_up, "hotkey-scroll-up", "scroll-up")?;
    register_hotkey(app, &cfg.scroll_down, "hotkey-scroll-down", "scroll-down")?;
    register_hotkey(app, &cfg.move_up, "hotkey-move-up", "move-up")?;
    register_hotkey(app, &cfg.move_down, "hotkey-move-down", "move-down")?;
    register_hotkey(app, &cfg.move_left, "hotkey-move-left", "move-left")?;
    register_hotkey(app, &cfg.move_right, "hotkey-move-right", "move-right")?;
    register_toggle_visibility_hotkey(app, &cfg.toggle_visibility)?;
    register_quit_app_hotkey(app, &cfg.quit_app)?;
    Ok(())
}

fn register_toggle_visibility_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    let hk = hotkey.to_string();
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                println!("⌨️ Hotkey pressed (toggle): {}", hk);
                if let Some(win) = handle.get_webview_window("main") {
                    match win.is_visible() {
                        Ok(true) => {
                            let _ = win.hide();
                        }
                        Ok(false) => {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                        Err(_) => {
                            // Best-effort fallback
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                } else {
                    println!("⚠️ Could not find main window to toggle visibility");
                }
            }
        })
        .map_err(|e| e.to_string())
}

fn register_quit_app_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    let hk = hotkey.to_string();
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(hotkey, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                println!("⌨️ Hotkey pressed (quit): {}", hk);
                handle.exit(0);
            }
        })
        .map_err(|e| e.to_string())
}

fn validate_hotkey_for_global_use(hotkey: &str) -> Result<(), String> {
    // Many OS-level global shortcut systems do not reliably deliver "Shift-only" combinations
    // (e.g. Shift+L is essentially just typing an uppercase letter). Require a "real" modifier.
    let parts: Vec<&str> = hotkey.split('+').filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        return Err("Hotkey must include at least one modifier (e.g. Cmd+E, Alt+S, Ctrl+Shift+L)".to_string());
    }

    let mut has_cmd = false;
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_cmdorctrl = false;
    let mut has_shift = false;
    let mut has_non_modifier = false;

    for p in &parts {
        match *p {
            "Cmd" => has_cmd = true,
            "Ctrl" => has_ctrl = true,
            "Alt" => has_alt = true,
            "CmdOrCtrl" => has_cmdorctrl = true,
            "Shift" => has_shift = true,
            _ => has_non_modifier = true,
        }
    }

    if !has_non_modifier {
        return Err("Hotkey must include a non-modifier key (e.g. Cmd+E)".to_string());
    }

    // Reject Shift-only.
    let has_primary_modifier = has_cmd || has_ctrl || has_alt || has_cmdorctrl;
    if !has_primary_modifier {
        if has_shift {
            return Err("Shift-only hotkeys (e.g. Shift+L) are not supported for global shortcuts. Please include Cmd/Ctrl/Alt (e.g. Cmd+Shift+L).".to_string());
        }
        return Err("Hotkey must include Cmd/Ctrl/Alt (and optionally Shift). Example: Cmd+E".to_string());
    }

    Ok(())
}

fn validate_scroll_hotkey(hotkey: &str) -> Result<(), String> {
    // Basic sanity: require Up/Down for scroll hotkeys so users don't accidentally bind them to letters.
    let lower = hotkey.to_lowercase();
    if lower.ends_with("+up") || lower.ends_with("+down") {
        Ok(())
    } else {
        Err("Scroll hotkeys must end with Up or Down (e.g. Cmd+Up, Ctrl+Down)".to_string())
    }
}

fn validate_move_hotkey(hotkey: &str) -> Result<(), String> {
    let lower = hotkey.to_lowercase();
    if lower.ends_with("+up") || lower.ends_with("+down") || lower.ends_with("+left") || lower.ends_with("+right") {
        Ok(())
    } else {
        Err("Move hotkeys must end with Up/Down/Left/Right (e.g. Cmd+Shift+Left)".to_string())
    }
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

// ============================================================================
// AUDIO RECORDING (SYSTEM AUDIO / LOOPBACK)
// ============================================================================

#[tauri::command]
async fn start_audio_recording() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| audio::start_system_audio_recording())
        .await
        .map_err(|e| format!("Failed to start audio recording task: {e}"))?
}

#[tauri::command]
fn stop_audio_recording() -> Result<String, String> {
    audio::stop_system_audio_recording()
}

#[tauri::command]
fn is_audio_recording() -> Result<bool, String> {
    Ok(audio::is_recording())
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

#[tauri::command]
async fn query_ai_with_audio(
    prompt: String,
    audio_path: String,
    config: ai::AIConfig,
) -> Result<String, String> {
    let audio_bytes = std::fs::read(&audio_path)
        .map_err(|e| format!("Failed to read audio at {}: {}", audio_path, e))?;

    // Validate that we captured actual WAV audio samples.
    // A header-only file (often ~4KB due to filesystem block size) means we received no audio buffers.
    let meta_len = std::fs::metadata(&audio_path)
        .map(|m| m.len() as usize)
        .unwrap_or(audio_bytes.len());
    if meta_len < 128 {
        return Err(format!(
            "Recorded audio file is too small ({} bytes) at {}. This feature records **system output audio** (sound coming from your speakers/headphones), not your microphone. Play something (e.g., YouTube) or make sure Zoom meeting audio is audible, then record for a few seconds.",
            meta_len,
            audio_path
        ));
    }
    match hound::WavReader::open(&audio_path) {
        Ok(reader) => {
            let spec = reader.spec();
            let samples = reader.duration() as u64;
            if samples == 0 {
                return Err(format!(
                    "No audio samples were captured (file is header-only, {} bytes) at {}. This records **system output audio** (other people speaking / app audio), not your microphone. Ensure system audio is playing and record for a few seconds.",
                    meta_len,
                    audio_path
                ));
            }
            // If it's extremely short, still allow sending to Gemini, but warn in logs.
            let channels = spec.channels.max(1) as u64;
            let frames = samples / channels;
            if frames < (spec.sample_rate as u64 / 5) {
                println!(
                    "🎙️ audio: very short recording: frames={} (~{}ms) spec={{rate={}, ch={}}} path={}",
                    frames,
                    (frames * 1000) / (spec.sample_rate as u64).max(1),
                    spec.sample_rate,
                    spec.channels,
                    audio_path
                );
            }
        }
        Err(e) => {
            return Err(format!(
                "Recorded audio is not a valid WAV file at {} ({} bytes). Error: {}",
                audio_path, meta_len, e
            ));
        }
    }
    ai::query_with_audio(&prompt, &audio_bytes, &config).await
}


// ============================================================================
// GOOGLE OAUTH COMMANDS
// ============================================================================

#[tauri::command]
fn start_google_oauth() -> Result<String, String> {
    let (redirect_uri, code_receiver) = oauth_server::start_oauth_server()?;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let auth_url = rt.block_on(OAUTH_SERVICE.get_auth_url(&redirect_uri));
    
    println!("🔐 Auth URL: {}", auth_url);
    
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&auth_url).spawn().ok();
    
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/C", "start", &auth_url]).spawn().ok();
    
    // Wait for code (blocks this thread for up to 120 seconds)
    let (code, state) = code_receiver.recv_timeout(std::time::Duration::from_secs(120))
        .map_err(|_| "Authentication timeout - please try again".to_string())?;
    
    println!("✅ Got authorization code");
    
    // Exchange code for tokens (PKCE desktop flow; no client_secret needed)
    match rt.block_on(OAUTH_SERVICE.exchange_code(&code, &state, &redirect_uri)) {
        Ok(tokens) => {
            println!(
                "✅ OAuth token exchange success. refresh_token_present={} user_email={}",
                tokens.refresh_token.is_some(),
                tokens.user_email.clone().unwrap_or_else(|| "<unknown>".to_string())
            );
        }
        Err(e) => {
            println!("❌ OAuth token exchange failed: {}", e);
            return Err(e);
        }
    }
    
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
        println!("💡 Google OAuth will not work without GOOGLE_CLIENT_ID");
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
            start_audio_recording,
            stop_audio_recording,
            is_audio_recording,
            query_ai,
            query_ai_with_image,
            query_ai_with_audio,
            start_google_oauth,
            get_google_token_status,
            clear_google_tokens,
            resize_window,
            get_window_inner_size,
            get_os,
            get_hotkeys,
            set_hotkeys,
            reset_hotkeys_to_default,
            move_window_by,
            frontend_log,
        ])
        .setup(|app| {
            println!("🚀 CrackingInterview starting...");
            screenshot::request_screen_recording_permission();

            // Load configured hotkeys, store in state, and register them.
            let cfg = load_hotkeys_from_disk(app.handle());
            app.manage(HotkeysState(Mutex::new(cfg.clone())));

            // Register configured hotkeys; if it fails, fall back to defaults.
            if let Err(e) = register_hotkeys(app.handle(), &cfg) {
                println!("⚠️ Failed to register configured hotkeys: {}", e);
                let defaults = default_hotkeys();
                println!("↩️ Falling back to defaults: text={}, screenshot={}", defaults.text, defaults.screenshot);
                // Best-effort: try again with defaults
                register_hotkeys(app.handle(), &defaults).ok();
                // Persist defaults so future startups are consistent
                save_hotkeys_to_disk(app.handle(), &defaults).ok();
                if let Some(state) = app.try_state::<HotkeysState>() {
                    if let Ok(mut guard) = state.0.lock() {
                        *guard = defaults;
                    }
                }
            }

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

// ============================================================================
// WINDOW MOVE COMMANDS
// ============================================================================

#[tauri::command]
async fn move_window_by(window: tauri::Window, dx: i32, dy: i32) -> Result<(), String> {
    use tauri::{Position, PhysicalPosition};
    use tokio::time::{sleep, Duration};

    let start = window.outer_position().map_err(|e| e.to_string())?;
    let start_x = start.x;
    let start_y = start.y;

    // Animate in small steps for a smooth feel.
    let steps: i32 = 10;
    let step_dx = dx as f64 / steps as f64;
    let step_dy = dy as f64 / steps as f64;

    for i in 1..=steps {
        let x = start_x + (step_dx * i as f64).round() as i32;
        let y = start_y + (step_dy * i as f64).round() as i32;
        window
            .set_position(Position::Physical(PhysicalPosition { x, y }))
            .map_err(|e| e.to_string())?;
        sleep(Duration::from_millis(10)).await;
    }

    Ok(())
}

// ============================================================================
// WINDOW RESIZE COMMANDS
// ============================================================================

#[tauri::command]
async fn resize_window(window: tauri::Window, width: f64, height: f64) -> Result<(), String> {
    use tauri::Size;
    println!("🪟 resize_window called: {}x{}", width, height);
    window.set_size(Size::Logical(tauri::LogicalSize { width, height }))
        .map_err(|e| e.to_string())?;
    if let (Ok(physical), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
        let logical: tauri::LogicalSize<f64> = physical.to_logical(scale);
        println!(
            "🪟 resize_window after set_size: logical={:.0}x{:.0} scale={:.2} physical={}x{}",
            logical.width,
            logical.height,
            scale,
            physical.width,
            physical.height
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct WindowInnerSize {
    /// Logical pixels (DPI-independent). This is what the frontend expects for consistent sizing across monitors.
    width: f64,
    height: f64,
}

#[tauri::command]
fn get_window_inner_size(window: tauri::Window) -> Result<WindowInnerSize, String> {
    let physical = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let logical: tauri::LogicalSize<f64> = physical.to_logical(scale);
    println!(
        "🪟 get_window_inner_size: logical={:.0}x{:.0} scale={:.2} physical={}x{}",
        logical.width,
        logical.height,
        scale,
        physical.width,
        physical.height
    );
    Ok(WindowInnerSize {
        width: logical.width,
        height: logical.height,
    })
}

/// Simple logger so frontend can write to Rust stdout (useful for debugging in `tauri dev` terminals).
#[tauri::command]
fn frontend_log(message: String) -> Result<(), String> {
    println!("🖥️ FE: {}", message);
    Ok(())
}

/// Used by the Settings UI to show OS-specific hotkeys without requiring a frontend OS plugin.
#[tauri::command]
fn get_os() -> Result<String, String> {
    Ok(std::env::consts::OS.to_string())
}

// ============================================================================
// HOTKEY CONFIG COMMANDS
// ============================================================================

#[tauri::command]
fn get_hotkeys(state: tauri::State<'_, HotkeysState>) -> Result<HotkeysConfig, String> {
    state
        .0
        .lock()
        .map(|cfg| cfg.clone())
        .map_err(|_| "Hotkeys state lock poisoned".to_string())
}

#[tauri::command]
fn reset_hotkeys_to_default(
    app: tauri::AppHandle,
    state: tauri::State<'_, HotkeysState>,
) -> Result<HotkeysConfig, String> {
    let defaults = default_hotkeys();
    // Unregister current, register defaults
    let current = get_hotkeys(state.clone())?;
    unregister_hotkey_best_effort(&app, &current.text);
    unregister_hotkey_best_effort(&app, &current.screenshot);
    unregister_hotkey_best_effort(&app, &current.audio_toggle);
    unregister_hotkey_best_effort(&app, &current.scroll_up);
    unregister_hotkey_best_effort(&app, &current.scroll_down);
    unregister_hotkey_best_effort(&app, &current.move_up);
    unregister_hotkey_best_effort(&app, &current.move_down);
    unregister_hotkey_best_effort(&app, &current.move_left);
    unregister_hotkey_best_effort(&app, &current.move_right);
    unregister_hotkey_best_effort(&app, &current.toggle_visibility);
    unregister_hotkey_best_effort(&app, &current.quit_app);

    register_hotkeys(&app, &defaults)?;
    save_hotkeys_to_disk(&app, &defaults)?;

    state
        .0
        .lock()
        .map_err(|_| "Hotkeys state lock poisoned".to_string())
        .map(|mut guard| {
            *guard = defaults.clone();
            defaults
        })
}

#[tauri::command]
fn set_hotkeys(
    app: tauri::AppHandle,
    state: tauri::State<'_, HotkeysState>,
    text_hotkey: String,
    screenshot_hotkey: String,
    audio_toggle_hotkey: String,
    scroll_up_hotkey: String,
    scroll_down_hotkey: String,
    move_up_hotkey: String,
    move_down_hotkey: String,
    move_left_hotkey: String,
    move_right_hotkey: String,
    toggle_visibility_hotkey: String,
    quit_app_hotkey: String,
) -> Result<HotkeysConfig, String> {
    let text = normalize_hotkey(&text_hotkey);
    let screenshot = normalize_hotkey(&screenshot_hotkey);
    let audio_toggle = normalize_hotkey(&audio_toggle_hotkey);
    let scroll_up = normalize_hotkey(&scroll_up_hotkey);
    let scroll_down = normalize_hotkey(&scroll_down_hotkey);
    let move_up = normalize_hotkey(&move_up_hotkey);
    let move_down = normalize_hotkey(&move_down_hotkey);
    let move_left = normalize_hotkey(&move_left_hotkey);
    let move_right = normalize_hotkey(&move_right_hotkey);
    let toggle_visibility = normalize_hotkey(&toggle_visibility_hotkey);
    let quit_app = normalize_hotkey(&quit_app_hotkey);

    if text.is_empty()
        || screenshot.is_empty()
        || audio_toggle.is_empty()
        || scroll_up.is_empty()
        || scroll_down.is_empty()
        || move_up.is_empty()
        || move_down.is_empty()
        || move_left.is_empty()
        || move_right.is_empty()
        || toggle_visibility.is_empty()
        || quit_app.is_empty()
    {
        return Err("Hotkeys cannot be empty".to_string());
    }
    if text == screenshot {
        return Err("Text and Screenshot hotkeys must be different".to_string());
    }
    if audio_toggle == text || audio_toggle == screenshot {
        return Err("Audio hotkey must be different from Text/Screenshot hotkeys".to_string());
    }
    if scroll_up == scroll_down {
        return Err("Scroll Up and Scroll Down hotkeys must be different".to_string());
    }
    if move_up == move_down || move_up == move_left || move_up == move_right || move_down == move_left || move_down == move_right || move_left == move_right {
        return Err("Move hotkeys must be different".to_string());
    }

    let all = [
        text.as_str(),
        screenshot.as_str(),
        audio_toggle.as_str(),
        scroll_up.as_str(),
        scroll_down.as_str(),
        move_up.as_str(),
        move_down.as_str(),
        move_left.as_str(),
        move_right.as_str(),
        toggle_visibility.as_str(),
        quit_app.as_str(),
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            if all[i] == all[j] {
                return Err("All hotkeys must be different".to_string());
            }
        }
    }

    // Validate before touching current registrations, so we never leave the app without working hotkeys.
    validate_hotkey_for_global_use(&text)?;
    validate_hotkey_for_global_use(&screenshot)?;
    validate_hotkey_for_global_use(&audio_toggle)?;
    validate_hotkey_for_global_use(&scroll_up)?;
    validate_hotkey_for_global_use(&scroll_down)?;
    validate_hotkey_for_global_use(&move_up)?;
    validate_hotkey_for_global_use(&move_down)?;
    validate_hotkey_for_global_use(&move_left)?;
    validate_hotkey_for_global_use(&move_right)?;
    validate_hotkey_for_global_use(&toggle_visibility)?;
    validate_hotkey_for_global_use(&quit_app)?;
    validate_scroll_hotkey(&scroll_up)?;
    validate_scroll_hotkey(&scroll_down)?;
    validate_move_hotkey(&move_up)?;
    validate_move_hotkey(&move_down)?;
    validate_move_hotkey(&move_left)?;
    validate_move_hotkey(&move_right)?;

    let previous = get_hotkeys(state.clone())?;

    // Remove previous registrations, then attempt to register the new ones.
    unregister_hotkey_best_effort(&app, &previous.text);
    unregister_hotkey_best_effort(&app, &previous.screenshot);
    unregister_hotkey_best_effort(&app, &previous.audio_toggle);
    unregister_hotkey_best_effort(&app, &previous.scroll_up);
    unregister_hotkey_best_effort(&app, &previous.scroll_down);
    unregister_hotkey_best_effort(&app, &previous.move_up);
    unregister_hotkey_best_effort(&app, &previous.move_down);
    unregister_hotkey_best_effort(&app, &previous.move_left);
    unregister_hotkey_best_effort(&app, &previous.move_right);
    unregister_hotkey_best_effort(&app, &previous.toggle_visibility);
    unregister_hotkey_best_effort(&app, &previous.quit_app);

    let next = HotkeysConfig {
        text,
        screenshot,
        audio_toggle,
        scroll_up,
        scroll_down,
        move_up,
        move_down,
        move_left,
        move_right,
        toggle_visibility,
        quit_app,
    };

    // Register text first, then screenshot; rollback if anything fails.
    if let Err(e) = register_hotkey(&app, &next.text, "hotkey-solve-text", "text") {
        // Restore old
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register text hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.screenshot, "hotkey-solve-screenshot", "screenshot") {
        // Unregister the new text and restore old
        unregister_hotkey_best_effort(&app, &next.text);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register screenshot hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.audio_toggle, "hotkey-audio-toggle", "audio-toggle") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register audio hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.scroll_up, "hotkey-scroll-up", "scroll-up") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register scroll up hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.scroll_down, "hotkey-scroll-down", "scroll-down") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register scroll down hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.move_up, "hotkey-move-up", "move-up") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        unregister_hotkey_best_effort(&app, &next.scroll_down);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register move up hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.move_down, "hotkey-move-down", "move-down") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        unregister_hotkey_best_effort(&app, &next.scroll_down);
        unregister_hotkey_best_effort(&app, &next.move_up);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register move down hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.move_left, "hotkey-move-left", "move-left") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        unregister_hotkey_best_effort(&app, &next.scroll_down);
        unregister_hotkey_best_effort(&app, &next.move_up);
        unregister_hotkey_best_effort(&app, &next.move_down);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register move left hotkey: {}", e));
    }

    if let Err(e) = register_hotkey(&app, &next.move_right, "hotkey-move-right", "move-right") {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        unregister_hotkey_best_effort(&app, &next.scroll_down);
        unregister_hotkey_best_effort(&app, &next.move_up);
        unregister_hotkey_best_effort(&app, &next.move_down);
        unregister_hotkey_best_effort(&app, &next.move_left);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register move right hotkey: {}", e));
    }

    if let Err(e) = register_toggle_visibility_hotkey(&app, &next.toggle_visibility) {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        unregister_hotkey_best_effort(&app, &next.scroll_down);
        unregister_hotkey_best_effort(&app, &next.move_up);
        unregister_hotkey_best_effort(&app, &next.move_down);
        unregister_hotkey_best_effort(&app, &next.move_left);
        unregister_hotkey_best_effort(&app, &next.move_right);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register toggle hotkey: {}", e));
    }

    if let Err(e) = register_quit_app_hotkey(&app, &next.quit_app) {
        unregister_hotkey_best_effort(&app, &next.text);
        unregister_hotkey_best_effort(&app, &next.screenshot);
        unregister_hotkey_best_effort(&app, &next.audio_toggle);
        unregister_hotkey_best_effort(&app, &next.scroll_up);
        unregister_hotkey_best_effort(&app, &next.scroll_down);
        unregister_hotkey_best_effort(&app, &next.move_up);
        unregister_hotkey_best_effort(&app, &next.move_down);
        unregister_hotkey_best_effort(&app, &next.move_left);
        unregister_hotkey_best_effort(&app, &next.move_right);
        unregister_hotkey_best_effort(&app, &next.toggle_visibility);
        register_hotkeys(&app, &previous).ok();
        return Err(format!("Failed to register quit hotkey: {}", e));
    }

    save_hotkeys_to_disk(&app, &next)?;

    state
        .0
        .lock()
        .map_err(|_| "Hotkeys state lock poisoned".to_string())
        .map(|mut guard| {
            *guard = next.clone();
            next
        })
}
