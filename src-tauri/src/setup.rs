// Prerequisite checks for the in-app setup checklist (Chrome + macOS privacy permissions).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SetupStatus {
    /// "macos" | "windows" | "other"
    pub platform: &'static str,
    pub chrome_installed: bool,
    /// `None` where the OS doesn't require the permission (Windows/Linux).
    pub accessibility_granted: Option<bool>,
    pub screen_recording_granted: Option<bool>,
}

pub fn status() -> SetupStatus {
    SetupStatus {
        platform: platform(),
        chrome_installed: chrome_installed(),
        accessibility_granted: if cfg!(target_os = "macos") {
            Some(crate::stealth_hotkey::is_accessibility_granted())
        } else {
            None
        },
        screen_recording_granted: screen_recording_granted(),
    }
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "other"
    }
}

/// Same install locations the Chrome launcher uses (`chrome/launcher.rs`).
fn chrome_installed() -> bool {
    #[cfg(target_os = "macos")]
    let paths: &[&str] = &["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"];
    #[cfg(target_os = "windows")]
    let paths: &[&str] = &[
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let paths: &[&str] = &[];

    paths.iter().any(|p| std::path::Path::new(p).exists())
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

/// macOS usually only reports a newly granted Screen Recording permission after the app restarts.
fn screen_recording_granted() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        Some(unsafe { CGPreflightScreenCaptureAccess() })
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Shows the macOS Screen Recording prompt (only the first time) and returns the current state.
pub fn request_screen_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe { CGRequestScreenCaptureAccess() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Opens System Settings → Privacy & Security at the given pane ("accessibility" | "screen_recording").
pub fn open_privacy_settings(pane: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let anchor = match pane {
            "accessibility" => "Privacy_Accessibility",
            "screen_recording" => "Privacy_ScreenCapture",
            other => return Err(format!("Unknown privacy pane: {}", other)),
        };
        std::process::Command::new("open")
            .arg(format!("x-apple.systempreferences:com.apple.preference.security?{}", anchor))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open System Settings: {}", e))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pane;
        Err("Privacy settings are only needed on macOS".to_string())
    }
}
