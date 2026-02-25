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
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;

static STEALTH_ENABLED: AtomicBool = AtomicBool::new(false);
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use serde::{Serialize, Deserialize};
use base64::{Engine as _, engine::general_purpose};

lazy_static::lazy_static! {
    static ref OAUTH_SERVICE: Arc<google_oauth::GoogleOAuthService> = {
        Arc::new(google_oauth::GoogleOAuthService::new())
    };
    
    // Reusable HTTP client for AI proxy requests (avoids TLS handshake per request)
    static ref AI_PROXY_CLIENT: reqwest::Client = {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(5)
            .build()
            .expect("Failed to create AI proxy HTTP client")
    };
}

// Two dedicated hotkeys:
// - Extract (text) → Solve
// - Screenshot → Solve
//
// Note: we use OS-specific combinations to match user expectations.
#[cfg(target_os = "macos")]
const DEFAULT_SOLVE_TEXT_HOTKEY: &str = "Cmd+1";
#[cfg(target_os = "macos")]
const DEFAULT_SOLVE_SCREENSHOT_HOTKEY: &str = "Cmd+2";

#[cfg(target_os = "windows")]
const DEFAULT_SOLVE_TEXT_HOTKEY: &str = "Alt+1";
#[cfg(target_os = "windows")]
const DEFAULT_SOLVE_SCREENSHOT_HOTKEY: &str = "Alt+2";

// Reasonable fallback for other OSes (Linux)
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_SOLVE_TEXT_HOTKEY: &str = "Ctrl+1";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_SOLVE_SCREENSHOT_HOTKEY: &str = "Ctrl+2";

// Audio (system) hotkey default (configurable).
#[cfg(target_os = "macos")]
const DEFAULT_AUDIO_TOGGLE_HOTKEY: &str = "Cmd+3";
#[cfg(target_os = "windows")]
const DEFAULT_AUDIO_TOGGLE_HOTKEY: &str = "Alt+3";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_AUDIO_TOGGLE_HOTKEY: &str = "Ctrl+3";

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

                // Migrate legacy default hotkeys to new defaults (Cmd+1, Cmd+2, Cmd+3) so existing users
                // pick up the change automatically unless they had customized it.
                let mut needs_save = false;
                
                // Legacy text hotkeys
                let legacy_text = ["Cmd+E", "Alt+E", "Ctrl+E"];
                if legacy_text.contains(&cfg.text.as_str()) {
                    cfg.text = DEFAULT_SOLVE_TEXT_HOTKEY.to_string();
                    needs_save = true;
                }
                
                // Legacy screenshot hotkeys
                let legacy_screenshot = ["Cmd+S", "Alt+S", "Ctrl+S"];
                if legacy_screenshot.contains(&cfg.screenshot.as_str()) {
                    cfg.screenshot = DEFAULT_SOLVE_SCREENSHOT_HOTKEY.to_string();
                    needs_save = true;
                }
                
                // Legacy audio hotkeys
                let legacy_audio = ["Cmd+Shift+A", "Alt+Shift+A", "Ctrl+Shift+A", "Cmd+A", "Alt+A", "Ctrl+A"];
                if legacy_audio.contains(&cfg.audio_toggle.as_str()) {
                    cfg.audio_toggle = DEFAULT_AUDIO_TOGGLE_HOTKEY.to_string();
                    needs_save = true;
                }
                
                if needs_save {
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

/// Reapply screen-capture exclusion after win.show() on Windows.
/// On Windows, hiding then showing a window can reset the WDA_EXCLUDEFROMCAPTURE flag,
/// causing a black rectangle in screen sharing instead of full invisibility.
fn reapply_stealth_after_show(win: &tauri::WebviewWindow) {
    if !STEALTH_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;
        if let Ok(h) = win.hwnd() {
            unsafe {
                let hwnd = HWND(h.0 as *mut _);
                let _ = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11));
            }
            println!("🕵️ [stealth-windows] Reapplied WDA_EXCLUDEFROMCAPTURE after show");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = win;
    }
}

/// Windows-only: toggle visibility via window opacity instead of hide/show.
/// This avoids the ShowWindow(SW_HIDE/SW_SHOW) cycle which corrupts
/// WDA_EXCLUDEFROMCAPTURE and causes a black rectangle in screen capture.
/// The window stays "visible" to Windows at all times so the display affinity is preserved.
#[cfg(target_os = "windows")]
fn toggle_window_opacity_win32(win: &tauri::WebviewWindow) {
    use std::sync::atomic::AtomicBool;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Graphics::Gdi::COLORREF;

    static WINDOW_HIDDEN: AtomicBool = AtomicBool::new(false);

    let hwnd_raw = match win.hwnd() {
        Ok(h) => h,
        Err(e) => {
            println!("⚠️ [toggle-win32] Could not get HWND: {}", e);
            return;
        }
    };

    unsafe {
        let hwnd = HWND(hwnd_raw.0 as *mut _);

        if WINDOW_HIDDEN.load(Ordering::Relaxed) {
            // Restore: set opacity back to 255 (fully opaque)
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            // Remove WS_EX_TRANSPARENT (click-through) if set
            let new_style = ex_style & !(WS_EX_TRANSPARENT.0 as isize);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
            let _ = win.set_focus();
            WINDOW_HIDDEN.store(false, Ordering::Relaxed);
            println!("🕵️ [toggle-win32] Window restored (opacity=255)");
        } else {
            // Hide: make WS_EX_LAYERED if not already, set opacity to 0, add click-through
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new_style = ex_style | WS_EX_LAYERED.0 as isize | WS_EX_TRANSPARENT.0 as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            WINDOW_HIDDEN.store(true, Ordering::Relaxed);
            println!("🕵️ [toggle-win32] Window hidden (opacity=0, click-through)");
        }
    }
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
                    reapply_stealth_after_show(&win);
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
                    // On Windows with stealth mode, use opacity toggle instead of
                    // hide/show to avoid breaking WDA_EXCLUDEFROMCAPTURE.
                    // ShowWindow(SW_HIDE/SW_SHOW) corrupts the display affinity,
                    // causing a black rectangle in screen capture.
                    #[cfg(target_os = "windows")]
                    if STEALTH_ENABLED.load(Ordering::Relaxed) {
                        toggle_window_opacity_win32(&win);
                        return;
                    }

                    // macOS / non-stealth: use normal hide/show
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

/// Pre-initialize audio capture when user selects Audio tab (instant recording)
#[tauri::command]
async fn warm_audio_capture() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| audio::warm_audio_capture())
        .await
        .map_err(|e| format!("Failed to warm audio capture: {e}"))?
}

/// Cleanup audio capture when user switches away from Audio tab
#[tauri::command]
fn cooldown_audio_capture() {
    audio::cooldown_audio_capture();
}

#[tauri::command]
fn is_audio_recording() -> Result<bool, String> {
    Ok(audio::is_recording())
}

/// Stop audio recording and return the file path
/// Note: Local transcription is disabled - audio is sent directly to AI instead
#[tauri::command]
fn stop_audio_recording_and_transcribe() -> Result<String, String> {
    let audio_path = audio::stop_system_audio_recording()?;
    println!("[Audio] Recording stopped, file: {}", audio_path);
    // Return an error format that frontend can detect to use the audio path
    // Format: "LOCAL_TRANSCRIPTION_DISABLED:<path>" for backward compatibility
    Err(format!("LOCAL_TRANSCRIPTION_DISABLED:{}", audio_path))
}

/// Transcribe an existing audio file
/// Note: Local transcription is disabled - audio is sent directly to AI instead
#[tauri::command]
fn transcribe_audio_file(audio_path: String) -> Result<String, String> {
    // Local transcription disabled - return error with path
    Err(format!("LOCAL_TRANSCRIPTION_DISABLED:{}", audio_path))
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

// Allowed domains for free tier and BYO API key users
const ALLOWED_DOMAINS: &[&str] = &["leetcode.com", "codewars.com", "codeforces.com", "neetcode.io"];

fn validate_source_url(source_url: &Option<String>) -> Result<(), String> {
    if let Some(url) = source_url {
        let is_allowed = ALLOWED_DOMAINS.iter().any(|domain| url.contains(domain));
        if !is_allowed {
            return Err(format!(
                "Domain restriction: This feature only works on coding practice sites ({}). Upgrade to Pro for unlimited access.",
                ALLOWED_DOMAINS.join(", ")
            ));
        }
    }
    // If no source_url provided, allow (backwards compatibility)
    Ok(())
}

#[tauri::command]
async fn query_ai(prompt: String, config: ai::AIConfig, source_url: Option<String>) -> Result<String, String> {
    // Validate domain for BYO API key users
    validate_source_url(&source_url)?;
    ai::query_with_text(&prompt, &config).await
}

#[tauri::command]
async fn query_ai_with_image(
    prompt: String,
    image_path: String,
    config: ai::AIConfig,
    source_url: Option<String>,
) -> Result<String, String> {
    // Validate domain for BYO API key users
    validate_source_url(&source_url)?;
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

/// Response from the AI proxy Edge Function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProxyResponse {
    pub response: String,
    pub usage: Option<AIProxyUsage>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIProxyUsage {
    pub requests_used: i32,
    pub requests_limit: i32,
    pub period_end: Option<String>,
    pub is_paid: bool,
}

/// Open a URL in the default system browser
#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    println!("[OpenURL] Opening: {}", url);
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }
    
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }
    
    Ok(())
}

/// Create a Stripe checkout session via the Supabase Edge Function
#[tauri::command]
async fn create_checkout_session(
    user_id: String,
    user_email: String,
) -> Result<String, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";
    // Supabase anon key - required for Edge Function calls
    const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InV1ZHdwY2p4Ynd0c3poaGNneWJqIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NjQ5MTAzMDksImV4cCI6MjA4MDQ4NjMwOX0.wKsiXAAK3q2pQdR8UGT7gXeBsXUDki-YAuB0CtJ9ZUI";
    
    println!("[Checkout] Creating session for user: {}", user_id);
    
    // Note: danger_accept_invalid_certs is used to work around corporate proxy SSL interception
    // In production with proper certificates, this should be removed
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let payload = serde_json::json!({
        "userId": user_id,
        "userEmail": user_email
    });

    println!("[Checkout] Sending request to Edge Function...");
    
    let response = client
        .post(format!("{}/functions/v1/create-checkout", SUPABASE_URL))
        .header("Content-Type", "application/json")
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("❌ Checkout request failed: {}", e))?;

    let status = response.status();
    let response_text = response.text().await
        .map_err(|e| format!("❌ Failed to read checkout response: {}", e))?;

    println!("[Checkout] Response status: {}, body: {}", status, response_text);

    if !status.is_success() {
        return Err(format!("❌ Checkout failed ({}): {}", status, response_text));
    }

    // Parse the response to get the URL
    let data: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("❌ Failed to parse checkout response: {}", e))?;
    
    let checkout_url = data["url"]
        .as_str()
        .ok_or("❌ No checkout URL in response")?
        .to_string();

    println!("[Checkout] Got checkout URL: {}", checkout_url);
    
    Ok(checkout_url)
}

/// Create a Stripe billing portal session for subscription management
#[tauri::command]
async fn create_billing_portal_session(
    customer_id: String,
) -> Result<String, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";
    const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InV1ZHdwY2p4Ynd0c3poaGNneWJqIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NjQ5MTAzMDksImV4cCI6MjA4MDQ4NjMwOX0.wKsiXAAK3q2pQdR8UGT7gXeBsXUDki-YAuB0CtJ9ZUI";
    
    println!("[Billing Portal] Creating session for customer: {}", customer_id);
    
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let payload = serde_json::json!({
        "customerId": customer_id
    });

    let response = client
        .post(format!("{}/functions/v1/create-billing-portal", SUPABASE_URL))
        .header("Content-Type", "application/json")
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("❌ Billing portal request failed: {}", e))?;

    let status = response.status();
    let response_text = response.text().await
        .map_err(|e| format!("❌ Failed to read billing portal response: {}", e))?;

    println!("[Billing Portal] Response status: {}, body: {}", status, response_text);

    if !status.is_success() {
        return Err(format!("❌ Billing portal failed ({}): {}", status, response_text));
    }

    let data: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("❌ Failed to parse billing portal response: {}", e))?;
    
    let portal_url = data["url"]
        .as_str()
        .ok_or("❌ No portal URL in response")?
        .to_string();

    println!("[Billing Portal] Got portal URL: {}", portal_url);
    
    Ok(portal_url)
}

/// Open a URL in the system's default browser
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    println!("[Open URL] Opening: {}", url);
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }
    
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }
    
    Ok(())
}

/// Sign up a new user via Supabase Auth (bypasses SSL inspection on corporate VPN)
#[tauri::command]
async fn supabase_sign_up(
    email: String,
    password: String,
) -> Result<serde_json::Value, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";
    const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InV1ZHdwY2p4Ynd0c3poaGNneWJqIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NjQ5MTAzMDksImV4cCI6MjA4MDQ4NjMwOX0.wKsiXAAK3q2pQdR8UGT7gXeBsXUDki-YAuB0CtJ9ZUI";
    
    println!("[Auth] Signing up user: {}", email);
    
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let payload = serde_json::json!({
        "email": email,
        "password": password
    });

    let response = client
        .post(format!("{}/auth/v1/signup", SUPABASE_URL))
        .header("Content-Type", "application/json")
        .header("apikey", SUPABASE_ANON_KEY)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("❌ Sign up request failed: {}", e))?;

    let status = response.status();
    let response_text = response.text().await
        .map_err(|e| format!("❌ Failed to read sign up response: {}", e))?;

    println!("[Auth] Sign up response status: {}", status);
    println!("[Auth] Sign up response body: {}", response_text);

    let data: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("❌ Failed to parse sign up response: {}", e))?;

    if !status.is_success() {
        let error_msg = data["error_description"]
            .as_str()
            .or(data["msg"].as_str())
            .or(data["message"].as_str())
            .unwrap_or(&response_text);
        return Err(format!("❌ Sign up failed: {}", error_msg));
    }

    // Supabase returns user data at root level (id, email directly in response)
    // Not nested under "user" key
    println!("[Auth] Sign up success - user id: {:?}, email: {:?}", data.get("id"), data.get("email"));
    
    Ok(data)
}

/// Sign in a user via Supabase Auth (bypasses SSL inspection on corporate VPN)
#[tauri::command]
async fn supabase_sign_in(
    email: String,
    password: String,
) -> Result<serde_json::Value, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";
    const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InV1ZHdwY2p4Ynd0c3poaGNneWJqIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NjQ5MTAzMDksImV4cCI6MjA4MDQ4NjMwOX0.wKsiXAAK3q2pQdR8UGT7gXeBsXUDki-YAuB0CtJ9ZUI";
    
    println!("[Auth] Signing in user: {}", email);
    
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let payload = serde_json::json!({
        "email": email,
        "password": password
    });

    let response = client
        .post(format!("{}/auth/v1/token?grant_type=password", SUPABASE_URL))
        .header("Content-Type", "application/json")
        .header("apikey", SUPABASE_ANON_KEY)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("❌ Sign in request failed: {}", e))?;

    let status = response.status();
    let response_text = response.text().await
        .map_err(|e| format!("❌ Failed to read sign in response: {}", e))?;

    println!("[Auth] Sign in response status: {}", status);
    println!("[Auth] Sign in response body: {}", response_text);

    let data: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("❌ Failed to parse sign in response: {}", e))?;

    if !status.is_success() {
        let error_msg = data["error_description"]
            .as_str()
            .or(data["msg"].as_str())
            .or(data["message"].as_str())
            .unwrap_or(&response_text);
        return Err(format!("❌ Sign in failed: {}", error_msg));
    }

    Ok(data)
}

/// Query AI via the Supabase Edge Function proxy
/// This enforces quotas and uses OpenRouter as the backend
#[tauri::command]
async fn query_ai_via_proxy(
    prompt: String,
    model: String,
    access_token: String,
    source_url: Option<String>,
) -> Result<AIProxyResponse, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";
    
    println!("[Rust AI Proxy] Starting request to ai-proxy...");
    println!("[Rust AI Proxy] Model: {}", model);
    println!("[Rust AI Proxy] Prompt length: {} chars", prompt.len());
    println!("[Rust AI Proxy] Source URL: {:?}", source_url);
    
    
    // Use static client (reuses TLS connections)
    let client = &*AI_PROXY_CLIENT;

    let messages = serde_json::json!([
        {
            "role": "user",
            "content": prompt
        }
    ]);

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "max_tokens": 16384,
        "source_url": source_url
    });

    println!("[Rust AI Proxy] Sending POST to {}/functions/v1/ai-proxy", SUPABASE_URL);
    let start = std::time::Instant::now();
    
    let response = client
        .post(format!("{}/functions/v1/ai-proxy", SUPABASE_URL))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            let elapsed = start.elapsed();
            println!("[Rust AI Proxy] Request FAILED after {:?}", elapsed);
            if e.is_timeout() {
                println!("[Rust AI Proxy] Error type: TIMEOUT");
                "❌ AI request timed out after 30 seconds. Please try again.".to_string()
            } else {
                println!("[Rust AI Proxy] Error type: {}", e);
                format!("❌ AI Proxy request failed: {}", e)
            }
        })?;

    let elapsed = start.elapsed();
    println!("[Rust AI Proxy] Response received after {:?}", elapsed);
    
    let status = response.status();
    println!("[Rust AI Proxy] Response status: {}", status);
    
    println!("[Rust AI Proxy] Reading response body...");
    let body_start = std::time::Instant::now();
    let response_text = response.text().await
        .map_err(|e| {
            println!("[Rust AI Proxy] Body read FAILED after {:?}", body_start.elapsed());
            format!("❌ Failed to read AI Proxy response: {}", e)
        })?;
    
    println!("[Rust AI Proxy] Body read in {:?}, length: {} chars", body_start.elapsed(), response_text.len());

    // Parse the response
    println!("[Rust AI Proxy] Parsing JSON response...");
    let proxy_response: serde_json::Value = serde_json::from_str(&response_text)
        .unwrap_or_else(|e| {
            println!("[Rust AI Proxy] JSON parse error: {}", e);
            serde_json::json!({ "error": response_text })
        });

    if !status.is_success() {
        let error_msg = proxy_response["error"]
            .as_str()
            .unwrap_or(&response_text);
        println!("[Rust AI Proxy] Non-success status, error: {}", error_msg);
        return Err(format!("❌ AI Proxy Error ({}): {}", status.as_u16(), error_msg));
    }

    // Parse usage info (use .get() to avoid panic on missing keys)
    let usage = proxy_response["usage"].as_object().map(|u| AIProxyUsage {
        requests_used: u.get("requests_used").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        requests_limit: u.get("requests_limit").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        period_end: u.get("period_end").and_then(|v| v.as_str()).map(|s| s.to_string()),
        is_paid: u.get("is_paid").and_then(|v| v.as_bool()).unwrap_or(false),
    });

    let ai_response_text = proxy_response["response"]
        .as_str()
        .unwrap_or("")
        .to_string();

    println!("[Rust AI Proxy] SUCCESS! Response length: {} chars", ai_response_text.len());
    
    Ok(AIProxyResponse {
        response: ai_response_text,
        usage,
        error: None,
    })
}

/// Query AI via proxy with an image (base64 encoded)
#[tauri::command]
async fn query_ai_via_proxy_with_image(
    prompt: String,
    image_path: String,
    model: String,
    access_token: String,
    source_url: Option<String>,
) -> Result<AIProxyResponse, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";

    println!("[Rust AI Proxy Image] Starting request to ai-proxy...");
    println!("[Rust AI Proxy Image] Model: {}", model);
    println!("[Rust AI Proxy Image] Prompt length: {} chars", prompt.len());
    println!("[Rust AI Proxy Image] Image path: {}", image_path);
    println!("[Rust AI Proxy Image] Source URL: {:?}", source_url);


    // Read and encode image
    let image_data = std::fs::read(&image_path)
        .map_err(|e| format!("Failed to read image: {}", e))?;
    
    let mime_type = ai::detect_image_mime_type(&image_data)?;
    let base64_image = general_purpose::STANDARD.encode(&image_data);
    println!("[Rust AI Proxy Image] Image size: {} bytes, mime: {}", image_data.len(), mime_type);

    // Use static client (reuses TLS connections)
    let client = &*AI_PROXY_CLIENT;

    let messages = serde_json::json!([
        {
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { 
                    "type": "image_url", 
                    "image_url": { 
                        "url": format!("data:{};base64,{}", mime_type, base64_image)
                    }
                }
            ]
        }
    ]);

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "max_tokens": 16384,
        "source_url": source_url
    });

    println!("[Rust AI Proxy Image] Sending POST to {}/functions/v1/ai-proxy", SUPABASE_URL);
    let start = std::time::Instant::now();

    let response = client
        .post(format!("{}/functions/v1/ai-proxy", SUPABASE_URL))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            let elapsed = start.elapsed();
            println!("[Rust AI Proxy Image] Request FAILED after {:?}", elapsed);
            if e.is_timeout() {
                println!("[Rust AI Proxy Image] Error type: TIMEOUT");
                "❌ AI request timed out after 30 seconds. Please try again.".to_string()
            } else {
                println!("[Rust AI Proxy Image] Error type: {}", e);
                format!("❌ AI Proxy request failed: {}", e)
            }
        })?;

    let elapsed = start.elapsed();
    println!("[Rust AI Proxy Image] Response received after {:?}", elapsed);

    let status = response.status();
    println!("[Rust AI Proxy Image] Response status: {}", status);

    println!("[Rust AI Proxy Image] Reading response body...");
    let body_start = std::time::Instant::now();
    let response_text = response.text().await
        .map_err(|e| {
            println!("[Rust AI Proxy Image] Body read FAILED after {:?}", body_start.elapsed());
            format!("❌ Failed to read AI Proxy response: {}", e)
        })?;

    println!("[Rust AI Proxy Image] Body read in {:?}, length: {} chars", body_start.elapsed(), response_text.len());

    println!("[Rust AI Proxy Image] Parsing JSON response...");
    let proxy_response: serde_json::Value = serde_json::from_str(&response_text)
        .unwrap_or_else(|e| {
            println!("[Rust AI Proxy Image] JSON parse error: {}", e);
            serde_json::json!({ "error": response_text })
        });

    if !status.is_success() {
        let error_msg = proxy_response["error"]
            .as_str()
            .unwrap_or(&response_text);
        println!("[Rust AI Proxy Image] Non-success status, error: {}", error_msg);
        return Err(format!("❌ AI Proxy Error ({}): {}", status.as_u16(), error_msg));
    }

    // Parse usage info (use .get() to avoid panic on missing keys)
    let usage = proxy_response["usage"].as_object().map(|u| AIProxyUsage {
        requests_used: u.get("requests_used").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        requests_limit: u.get("requests_limit").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        period_end: u.get("period_end").and_then(|v| v.as_str()).map(|s| s.to_string()),
        is_paid: u.get("is_paid").and_then(|v| v.as_bool()).unwrap_or(false),
    });

    let ai_response_text = proxy_response["response"]
        .as_str()
        .unwrap_or("")
        .to_string();

    println!("[Rust AI Proxy Image] SUCCESS! Response length: {} chars", ai_response_text.len());

    Ok(AIProxyResponse {
        response: ai_response_text,
        usage,
        error: None,
    })
}

/// Query AI via proxy with audio (base64 encoded)
/// Audio is sent directly to Gemini model via OpenRouter - no transcription needed
#[tauri::command]
async fn query_ai_via_proxy_with_audio(
    prompt: String,
    audio_path: String,
    model: String,
    access_token: String,
) -> Result<AIProxyResponse, String> {
    const SUPABASE_URL: &str = "https://uudwpcjxbwtszhhcgybj.supabase.co";

    println!("[Rust AI Proxy Audio] Starting request to ai-proxy...");
    println!("[Rust AI Proxy Audio] Model: {}", model);
    println!("[Rust AI Proxy Audio] Prompt length: {} chars", prompt.len());
    println!("[Rust AI Proxy Audio] Audio path: {}", audio_path);

    // Read and encode audio
    let audio_data = std::fs::read(&audio_path)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;
    
    // Validate audio file size (must have actual content)
    if audio_data.len() < 1000 {
        return Err(format!(
            "Audio file is too small ({} bytes). Recording may have failed or been too short.",
            audio_data.len()
        ));
    }
    
    let mime_type = audio::detect_audio_mime_type(&audio_data)?;
    let base64_audio = general_purpose::STANDARD.encode(&audio_data);
    println!("[Rust AI Proxy Audio] Audio size: {} bytes, mime: {}", audio_data.len(), mime_type);

    // Use static client (reuses TLS connections)
    let client = &*AI_PROXY_CLIENT;

    // Build multimodal message with audio
    // OpenRouter expects input_audio format per docs:
    // https://openrouter.ai/docs/guides/overview/multimodal/audio
    let audio_format = match mime_type {
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mp3" | "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        "audio/aac" => "aac",
        "audio/m4a" => "m4a",
        _ => "wav", // default to wav
    };
    
    let messages = serde_json::json!([
        {
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { 
                    "type": "input_audio", 
                    "input_audio": { 
                        "data": base64_audio,
                        "format": audio_format
                    }
                }
            ]
        }
    ]);

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "max_tokens": 16384
    });

    println!("[Rust AI Proxy Audio] Sending POST to {}/functions/v1/ai-proxy", SUPABASE_URL);
    let start = std::time::Instant::now();

    let response = client
        .post(format!("{}/functions/v1/ai-proxy", SUPABASE_URL))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            let elapsed = start.elapsed();
            println!("[Rust AI Proxy Audio] Request FAILED after {:?}", elapsed);
            if e.is_timeout() {
                println!("[Rust AI Proxy Audio] Error type: TIMEOUT");
                "❌ AI request timed out after 30 seconds. Please try again.".to_string()
            } else {
                println!("[Rust AI Proxy Audio] Error type: {}", e);
                format!("❌ AI Proxy request failed: {}", e)
            }
        })?;

    let elapsed = start.elapsed();
    println!("[Rust AI Proxy Audio] Response received after {:?}", elapsed);

    let status = response.status();
    println!("[Rust AI Proxy Audio] Response status: {}", status);

    println!("[Rust AI Proxy Audio] Reading response body...");
    let body_start = std::time::Instant::now();
    let response_text = response.text().await
        .map_err(|e| {
            println!("[Rust AI Proxy Audio] Body read FAILED after {:?}", body_start.elapsed());
            format!("❌ Failed to read AI Proxy response: {}", e)
        })?;

    println!("[Rust AI Proxy Audio] Body read in {:?}, length: {} chars", body_start.elapsed(), response_text.len());

    println!("[Rust AI Proxy Audio] Parsing JSON response...");
    let proxy_response: serde_json::Value = serde_json::from_str(&response_text)
        .unwrap_or_else(|e| {
            println!("[Rust AI Proxy Audio] JSON parse error: {}", e);
            serde_json::json!({ "error": response_text })
        });

    if !status.is_success() {
        let error_msg = proxy_response["error"]
            .as_str()
            .unwrap_or(&response_text);
        println!("[Rust AI Proxy Audio] Non-success status, error: {}", error_msg);
        return Err(format!("❌ AI Proxy Error ({}): {}", status.as_u16(), error_msg));
    }

    // Parse usage info
    let usage = proxy_response["usage"].as_object().map(|u| AIProxyUsage {
        requests_used: u.get("requests_used").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        requests_limit: u.get("requests_limit").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        period_end: u.get("period_end").and_then(|v| v.as_str()).map(|s| s.to_string()),
        is_paid: u.get("is_paid").and_then(|v| v.as_bool()).unwrap_or(false),
    });

    let ai_response_text = proxy_response["response"]
        .as_str()
        .unwrap_or("")
        .to_string();

    println!("[Rust AI Proxy Audio] SUCCESS! Response length: {} chars", ai_response_text.len());

    Ok(AIProxyResponse {
        response: ai_response_text,
        usage,
        error: None,
    })
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
// STEALTH MODE – Platform-specific helpers
// ============================================================================

/// macOS: hide from Dock by setting NSApp activation policy to .accessory (1).
/// This also removes the app from Cmd+Tab.
/// Uses raw ObjC runtime so no extra crate dependency is needed.
#[cfg(target_os = "macos")]
fn apply_macos_dock_hiding() {
    use std::ffi::c_void;

    // Thin wrappers around ObjC runtime functions (shipped with macOS).
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        // We transmute objc_msgSend to the right signature below.
        fn objc_msgSend();
    }

    type SendNoArg = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type SendI64 = unsafe extern "C" fn(*mut c_void, *mut c_void, i64) -> *mut c_void;

    unsafe {
        let cls = objc_getClass(b"NSApplication\0".as_ptr() as *const _);
        if cls.is_null() {
            println!("⚠️ [stealth-macOS] Could not locate NSApplication class");
            return;
        }

        let shared_sel = sel_registerName(b"sharedApplication\0".as_ptr() as *const _);
        let policy_sel = sel_registerName(b"setActivationPolicy:\0".as_ptr() as *const _);

        let send: SendNoArg = std::mem::transmute(objc_msgSend as *const ());
        let send_i64: SendI64 = std::mem::transmute(objc_msgSend as *const ());

        let app = send(cls, shared_sel);
        if app.is_null() {
            println!("⚠️ [stealth-macOS] NSApp.sharedApplication returned nil");
            return;
        }

        // NSApplicationActivationPolicyAccessory = 1
        send_i64(app, policy_sel, 1);
        println!("🕵️ [stealth-macOS] Dock icon hidden (activationPolicy = .accessory)");
    }
}

/// macOS: Exclude window from screen capture (Zoom, Teams, screenshots, etc.)
/// by setting NSWindow.sharingType = NSWindowSharingNone (0).
/// This is the ONLY reliable way to hide window content on macOS.
#[cfg(target_os = "macos")]
fn apply_macos_screen_capture_protection(win: &tauri::WebviewWindow) {
    use std::ffi::c_void;

    extern "C" {
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    type SendNoArg = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type SendU64 = unsafe extern "C" fn(*mut c_void, *mut c_void, u64);

    unsafe {
        // Get the raw NSWindow pointer from Tauri
        let ns_window_ptr = match win.ns_window() {
            Ok(ptr) => ptr as *mut c_void,
            Err(e) => {
                println!("⚠️ [stealth-macOS] Could not get NSWindow: {}", e);
                return;
            }
        };

        if ns_window_ptr.is_null() {
            println!("⚠️ [stealth-macOS] NSWindow pointer is null");
            return;
        }

        // Get selectors
        let set_sharing_sel = sel_registerName(b"setSharingType:\0".as_ptr() as *const _);
        let get_sharing_sel = sel_registerName(b"sharingType\0".as_ptr() as *const _);
        
        let send_u64: SendU64 = std::mem::transmute(objc_msgSend as *const ());
        let send_get: SendNoArg = std::mem::transmute(objc_msgSend as *const ());

        // NSWindowSharingNone = 0 → window is NOT shared (excludes from screen capture)
        send_u64(ns_window_ptr, set_sharing_sel, 0);
        
        // Verify the setting took effect
        let sharing_type = send_get(ns_window_ptr, get_sharing_sel) as u64;
        
        if sharing_type == 0 {
            println!("🕵️ [stealth-macOS] ✅ Window excluded from screen capture (sharingType = 0)");
        } else {
            println!("⚠️ [stealth-macOS] sharingType = {} (expected 0), retrying...", sharing_type);
            
            // Try again
            std::thread::sleep(std::time::Duration::from_millis(100));
            send_u64(ns_window_ptr, set_sharing_sel, 0);
            
            let sharing_type_retry = send_get(ns_window_ptr, get_sharing_sel) as u64;
            if sharing_type_retry == 0 {
                println!("🕵️ [stealth-macOS] ✅ Window excluded from screen capture on retry (sharingType = 0)");
            } else {
                println!("⚠️ [stealth-macOS] Failed to set sharingType = 0 (current value: {})", sharing_type_retry);
            }
        }
    }
}

/// macOS: Restore window to normal visibility (allow screen capture)
/// by setting NSWindow.sharingType = NSWindowSharingReadWrite (1).
#[cfg(target_os = "macos")]
fn restore_macos_screen_capture_visibility(win: &tauri::WebviewWindow) {
    use std::ffi::c_void;

    extern "C" {
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    type SendNoArg = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type SendU64 = unsafe extern "C" fn(*mut c_void, *mut c_void, u64);

    unsafe {
        let ns_window_ptr = match win.ns_window() {
            Ok(ptr) => ptr as *mut c_void,
            Err(e) => {
                println!("⚠️ [normal-macOS] Could not get NSWindow: {}", e);
                return;
            }
        };

        if ns_window_ptr.is_null() {
            println!("⚠️ [normal-macOS] NSWindow pointer is null");
            return;
        }

        let set_sharing_sel = sel_registerName(b"setSharingType:\0".as_ptr() as *const _);
        let get_sharing_sel = sel_registerName(b"sharingType\0".as_ptr() as *const _);
        
        let send_u64: SendU64 = std::mem::transmute(objc_msgSend as *const ());
        let send_get: SendNoArg = std::mem::transmute(objc_msgSend as *const ());

        // NSWindowSharingReadWrite = 1 → window CAN be captured (normal behavior)
        send_u64(ns_window_ptr, set_sharing_sel, 1);
        
        let sharing_type = send_get(ns_window_ptr, get_sharing_sel) as u64;
        println!("👁️ [normal-macOS] ✅ Window visible in screen capture (sharingType = {})", sharing_type);
    }
}

/// Windows: apply WDA_EXCLUDEFROMCAPTURE and WS_EX_TOOLWINDOW via raw Win32 API.
/// This is a belt-and-suspenders fallback in case Tauri's set_content_protected /
/// set_skip_taskbar methods don't fully apply on certain Windows builds.
#[cfg(target_os = "windows")]
fn apply_windows_stealth(win: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Graphics::Gdi::COLORREF;

    // Tauri 2 exposes the HWND through the raw-window-handle.
    // We use the `hwnd()` helper which returns an isize.
    let hwnd_raw = match win.hwnd() {
        Ok(h) => h,
        Err(e) => {
            println!("⚠️ [stealth-windows] Could not get HWND: {}", e);
            return;
        }
    };

    unsafe {
        let hwnd = HWND(hwnd_raw.0 as *mut _);

        // ---- Exclude from screen capture ----
        // WDA_EXCLUDEFROMCAPTURE = 0x11 (Windows 10 2004+)
        let result = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11));
        if let Err(e) = result {
            println!("⚠️ [stealth-windows] SetWindowDisplayAffinity failed: {:?}", e);
        } else {
            println!("🕵️ [stealth-windows] Window excluded from screen capture");
        }

        // ---- Hide from taskbar + Alt+Tab, prepare for opacity toggle ----
        // WS_EX_LAYERED is required for SetLayeredWindowAttributes (opacity toggle).
        // WS_EX_TOOLWINDOW hides from taskbar and Alt+Tab.
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = (ex_style
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_LAYERED.0 as isize)
            & !(WS_EX_APPWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);

        // Set initial opacity to fully opaque
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);

        println!("🕵️ [stealth-windows] Window hidden from taskbar & Alt+Tab (layered+toolwindow)");
    }
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
            warm_audio_capture,
            cooldown_audio_capture,
            is_audio_recording,
            stop_audio_recording_and_transcribe,
            transcribe_audio_file,
            query_ai,
            query_ai_with_image,
            query_ai_with_audio,
            query_ai_via_proxy,
            query_ai_via_proxy_with_image,
            query_ai_via_proxy_with_audio,
            create_checkout_session,
            create_billing_portal_session,
            open_url,
            supabase_sign_up,
            supabase_sign_in,
            open_external_url,
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
            
            // Pre-compile audio recorder helper in background (eliminates first-recording delay)
            audio::prewarm_audio_recorder();

            // ---- Stealth Mode ----
            // Read APP_VISIBILITY from .env: "stealth" hides from screen capture + Dock/Taskbar
            let stealth_mode = std::env::var("APP_VISIBILITY")
                .map(|v| v.to_lowercase() != "normal")
                .unwrap_or(true);

            STEALTH_ENABLED.store(stealth_mode, Ordering::Relaxed);

            if stealth_mode {
                println!("🕵️ Stealth mode ENABLED — hiding from screen capture and Dock/Taskbar");
                
                // macOS: Hide Dock icon immediately (app-level setting)
                #[cfg(target_os = "macos")]
                {
                    apply_macos_dock_hiding();
                }
                
                // Apply window stealth settings immediately (no delay needed with tokio spawn)
                if let Some(win) = app.get_webview_window("main") {
                    #[cfg(target_os = "macos")]
                    {
                        apply_macos_screen_capture_protection(&win);
                    }

                    #[cfg(target_os = "windows")]
                    {
                        apply_windows_stealth(&win);
                    }

                    // Cross-platform Tauri methods (backup/fallback)
                    if let Err(e) = win.set_content_protected(true) {
                        println!("⚠️ set_content_protected failed: {}", e);
                    }
                    if let Err(e) = win.set_skip_taskbar(true) {
                        println!("⚠️ set_skip_taskbar failed: {}", e);
                    }
                } else {
                    println!("⚠️ Could not find main window for stealth mode");
                }
            } else {
                println!("👁️ Normal visibility mode (APP_VISIBILITY != stealth)");
                
                // Restore normal window visibility for screen capture
                if let Some(win) = app.get_webview_window("main") {
                    #[cfg(target_os = "macos")]
                    {
                        restore_macos_screen_capture_visibility(&win);
                    }

                    // Ensure window is NOT skipped in taskbar
                    if let Err(e) = win.set_content_protected(false) {
                        println!("⚠️ set_content_protected(false) failed: {}", e);
                    }
                    if let Err(e) = win.set_skip_taskbar(false) {
                        println!("⚠️ set_skip_taskbar(false) failed: {}", e);
                    }
                }
            }

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
