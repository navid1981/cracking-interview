//! macOS-specific resilience helpers (Scenarios B2a, B2b, B2c, B1).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;

pub const CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
pub const MOVE_TO_ACTIVE_SPACE: u64 = 1 << 1;
pub const STATIONARY: u64 = 1 << 4;
pub const IGNORES_CYCLE: u64 = 1 << 6;
pub const FULL_SCREEN_PRIMARY: u64 = 1 << 7;
pub const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

/// `NSScreenSaverWindowLevel` (1000) + 1 so our overlay floats above legacy kiosk presentation windows.
pub const KIOSK_OVERLAY_WINDOW_LEVEL: i64 = 1001;

pub static SIGTERM_INTERCEPTED: AtomicBool = AtomicBool::new(false);

pub fn compute_kiosk_collection_behavior(current: u64) -> u64 {
    (current | CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY)
        & !MOVE_TO_ACTIVE_SPACE
        & !FULL_SCREEN_PRIMARY
}

extern "C" fn sigterm_handler(_sig: libc::c_int) {
    if super::AUTHORIZED_QUIT.load(Ordering::SeqCst) {
        unsafe {
            libc::_exit(0);
        }
    }
    SIGTERM_INTERCEPTED.store(true, Ordering::SeqCst);
}

/// Installs a POSIX `SIGTERM` signal handler so external `kill <pid>` or kiosk termination sweeps
/// do not kill the process unless `AUTHORIZED_QUIT` is set. Leaves `SIGINT` and `SIGQUIT` at
/// default behavior so Ctrl+C in terminal dev builds works normally.
pub fn install_sigterm_guard() {
    unsafe {
        let mut act: libc::sigaction = std::mem::zeroed();
        act.sa_sigaction = sigterm_handler as usize;
        libc::sigemptyset(&mut act.sa_mask);
        act.sa_flags = libc::SA_RESTART;
        let _ = libc::sigaction(libc::SIGTERM, &act, std::ptr::null_mut());
    }
}

/// Spawns a 300ms macOS self-healing thread that dispatches onto the main thread to verify:
/// 1. If `SIGTERM_INTERCEPTED` was set, logs the intercepted signal and checks if we should yield to `assessmentagent`.
/// 2. If `!USER_HIDDEN`, verifies `ns_window.isVisible`, `ns_window.level == 1001`, and `sharingType == 0` (when stealth is enabled).
pub fn start_window_self_healer(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Check for a lockdown (AAC / SEB) on the first iteration, then ~every 3s.
        let mut lockdown_check_counter: u32 = 9;
        loop {
            std::thread::sleep(Duration::from_millis(300));

            if super::quit_was_authorized() {
                crate::stealth_hotkey::teardown_stealth_hotkeys();
                app.exit(0);
                break;
            }
            if !super::enabled() {
                continue;
            }

            // Proactively yield to a macOS lockdown instead of waiting for
            // SEB's terminate request: SEB-mac rechecks 1s after asking apps
            // to quit and shows its "Prohibited Processes" dialog for any
            // survivor. Exiting cleanly (marked killed-by-OS) lets SEB start
            // without the dialog, and the guardian probe-respawns us after
            // the lockdown ends.
            lockdown_check_counter += 1;
            if lockdown_check_counter >= 10 {
                lockdown_check_counter = 0;
                if super::should_yield_to_assessment(&app) {
                    super::log_event(
                        "Detected macOS lockdown (AAC/SEB); yielding for post-lockdown recovery",
                    );
                    super::mark_killed_by_os(&app);
                    crate::stealth_hotkey::teardown_stealth_hotkeys();
                    std::process::exit(0);
                }
            }

            if SIGTERM_INTERCEPTED.swap(false, Ordering::SeqCst) {
                if super::should_yield_to_assessment(&app) {
                    super::mark_killed_by_os(&app);
                    crate::stealth_hotkey::teardown_stealth_hotkeys();
                    std::process::exit(0);
                }
                super::log_event("Intercepted and ignored external SIGTERM on macOS");
            }

            if super::USER_HIDDEN.load(Ordering::Relaxed) {
                continue;
            }

            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                if super::USER_HIDDEN.load(Ordering::Relaxed) || super::quit_was_authorized() {
                    return;
                }
                let Some(win) = handle.get_webview_window("main") else {
                    return;
                };
                let Ok(ns_win_raw) = win.ns_window() else {
                    return;
                };
                if ns_win_raw.is_null() {
                    return;
                }
                let ns_window = ns_win_raw as *mut c_void;

                extern "C" {
                    fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
                    fn objc_msgSend();
                }
                type GetBool = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
                type GetI64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64;
                type GetU64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64;

                unsafe {
                    let is_vis_sel = sel_registerName(b"isVisible\0".as_ptr() as *const _);
                    let level_sel = sel_registerName(b"level\0".as_ptr() as *const _);
                    let sharing_sel = sel_registerName(b"sharingType\0".as_ptr() as *const _);

                    let get_bool: GetBool = std::mem::transmute(objc_msgSend as *const ());
                    let get_i64: GetI64 = std::mem::transmute(objc_msgSend as *const ());
                    let get_u64: GetU64 = std::mem::transmute(objc_msgSend as *const ());

                    let visible = get_bool(ns_window, is_vis_sel);
                    if !visible {
                        crate::stealth_hotkey::bring_to_front_without_focus(&win);
                        super::log_event(
                            "Self-healed hidden NSWindow via orderFrontRegardless (macOS)",
                        );
                    }

                    let level = get_i64(ns_window, level_sel);
                    if level != KIOSK_OVERLAY_WINDOW_LEVEL {
                        crate::apply_macos_fullscreen_overlay(&win);
                        super::log_event(&format!(
                            "Self-healed NSWindow level {} -> {}",
                            level, KIOSK_OVERLAY_WINDOW_LEVEL
                        ));
                    }

                    if crate::STEALTH_ENABLED.load(Ordering::Relaxed) {
                        let sharing = get_u64(ns_window, sharing_sel);
                        if sharing != 0 {
                            crate::apply_macos_screen_capture_protection(&win);
                            super::log_event(
                                "Self-healed NSWindow sharingType -> 0 (NSWindowSharingNone)",
                            );
                        }
                    }
                }
            });
        }
    });
}

pub const LAUNCH_AGENT_LABEL: &str = "com.crackinginterview.keepalive";
pub const LAUNCH_AGENT_FILE_NAME: &str = "com.crackinginterview.keepalive.plist";

pub fn render_launch_agent_plist(exe_path: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>--guardian</string>
    </array>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ThrottleInterval</key>
    <integer>2</integer>
</dict>
</plist>
"#,
        label = LAUNCH_AGENT_LABEL,
        exe = exe_path
    )
}

pub fn set_launch_agent_enabled_in_dir(
    launch_agents_dir: &std::path::Path,
    exe_path: &str,
    enabled: bool,
) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(launch_agents_dir).map_err(|e| e.to_string())?;
    let plist_path = launch_agents_dir.join(LAUNCH_AGENT_FILE_NAME);
    if enabled {
        let xml = render_launch_agent_plist(exe_path);
        super::write_atomic(&plist_path, xml.as_bytes())?;
    } else if plist_path.exists() {
        let _ = std::fs::remove_file(&plist_path);
    }
    Ok(plist_path)
}

#[allow(dead_code)]
pub fn set_launch_agent_enabled(enabled: bool) -> Result<(), String> {
    let home = std::env::var_os("HOME").ok_or("HOME environment variable not set")?;
    let launch_agents_dir = std::path::PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy();
    let _ = set_launch_agent_enabled_in_dir(&launch_agents_dir, &exe_str, enabled)?;
    super::log_event(&format!(
        "macOS LaunchAgent keepalive set to enabled={}",
        enabled
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_kiosk_collection_behavior_bits() {
        let initial = MOVE_TO_ACTIVE_SPACE | FULL_SCREEN_PRIMARY;
        let updated = compute_kiosk_collection_behavior(initial);
        assert_ne!(updated & CAN_JOIN_ALL_SPACES, 0);
        assert_ne!(updated & STATIONARY, 0);
        assert_ne!(updated & IGNORES_CYCLE, 0);
        assert_ne!(updated & FULL_SCREEN_AUXILIARY, 0);
        assert_eq!(updated & MOVE_TO_ACTIVE_SPACE, 0);
        assert_eq!(updated & FULL_SCREEN_PRIMARY, 0);
        assert_eq!(KIOSK_OVERLAY_WINDOW_LEVEL, 1001);
    }

    #[test]
    fn test_sigterm_guard_intercepts_unauthorized_sigterm() {
        super::super::AUTHORIZED_QUIT.store(false, Ordering::SeqCst);
        SIGTERM_INTERCEPTED.store(false, Ordering::SeqCst);
        install_sigterm_guard();
        unsafe {
            libc::raise(libc::SIGTERM);
        }
        assert!(SIGTERM_INTERCEPTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_launch_agent_plist_generation_and_toggle() {
        let dir = std::env::temp_dir().join(format!("ci_la_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let exe = "/Applications/CrackingInterview.app/Contents/MacOS/cracking-interview";
        let plist_path = set_launch_agent_enabled_in_dir(&dir, exe, true).unwrap();
        assert!(plist_path.exists());
        let contents = std::fs::read_to_string(&plist_path).unwrap();
        assert!(contents.contains("com.crackinginterview.keepalive"));
        assert!(contents.contains("--guardian"));
        assert!(contents.contains("<key>SuccessfulExit</key>"));
        assert!(contents.contains("<false/>"));

        let _ = set_launch_agent_enabled_in_dir(&dir, exe, false).unwrap();
        assert!(!plist_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
