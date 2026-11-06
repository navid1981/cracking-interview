//! Windows-specific window self-healing and resilience helpers (Scenarios A1a, A1b).

use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Manager;

/// Spawns a 250ms background thread on Windows that inspects the main window's
/// visibility, topmost z-order, extended styles (`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`),
/// and screen-capture exclusion (`WDA_EXCLUDEFROMCAPTURE = 0x11`).
///
/// Never heals if `resilience::USER_HIDDEN` is true or `resilience::quit_was_authorized()`.
pub fn start_window_self_healer(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));

            if super::quit_was_authorized() {
                break;
            }
            if !super::enabled() || super::USER_HIDDEN.load(Ordering::Relaxed) {
                continue;
            }

            let Some(win) = app.get_webview_window("main") else {
                continue;
            };
            let Ok(hwnd_raw) = win.hwnd() else {
                continue;
            };

            #[cfg(target_os = "windows")]
            unsafe {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::*;

                let hwnd = HWND(hwnd_raw.0 as *mut _);
                if !IsWindow(hwnd).as_bool() {
                    continue;
                }

                // 1. Heal visibility if an external monitor called ShowWindow(SW_HIDE)
                if !IsWindowVisible(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                    super::log_event("Self-healed hidden HWND via SW_SHOWNOACTIVATE + HWND_TOPMOST");
                }

                // 2. Heal topmost & stealth extended styles if stripped
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let is_topmost = (ex_style & (WS_EX_TOPMOST.0 as isize)) != 0;
                if !is_topmost {
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    super::log_event("Self-healed stripped WS_EX_TOPMOST on HWND");
                }

                if crate::STEALTH_ENABLED.load(Ordering::Relaxed) {
                    let kb_active = super::KEYBOARD_INPUT_ACTIVE.load(Ordering::Relaxed);
                    let want_tool = WS_EX_TOOLWINDOW.0 as isize;
                    let want_noact = if kb_active { 0 } else { WS_EX_NOACTIVATE.0 as isize };
                    let unwanted_app = WS_EX_APPWINDOW.0 as isize;

                    let missing_tool = (ex_style & want_tool) == 0;
                    let missing_noact = !kb_active && (ex_style & want_noact) == 0;
                    let has_appwin = (ex_style & unwanted_app) != 0;

                    if missing_tool || missing_noact || has_appwin {
                        let fixed = (ex_style | want_tool | want_noact) & !unwanted_app;
                        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, fixed);
                        super::log_event("Self-healed stripped GWL_EXSTYLE stealth flags");
                    }

                    // 3. Heal WDA_EXCLUDEFROMCAPTURE (0x11)
                    let mut affinity: u32 = 0;
                    if GetWindowDisplayAffinity(hwnd, &mut affinity).is_ok() && affinity != 0x11 {
                        let _ = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11));
                        super::log_event("Self-healed WDA_EXCLUDEFROMCAPTURE (0x11) display affinity");
                    }
                }
            }
        }
    });
}
