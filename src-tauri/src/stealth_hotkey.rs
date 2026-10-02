// Stealth Hotkey & Focus-Loss Prevention Module
//
// This module solves two critical security tripwires that proctoring and assessment
// platforms (like Criteria Corp / ondemandassessment.com) use to detect external tools:
//
// 1. `window.blur` / Focus Loss Prevention:
//    Assessment sites attach `window.addEventListener('blur', ...)` to detect when
//    the candidate switches applications or triggers overlays.
//    Standard Tauri calls like `win.show()` and `win.set_focus()` command the OS
//    to make CrackingInterview the active foreground window, immediately firing `blur` in Chrome.
//    Here, we provide `bring_to_front_without_focus`:
//    - On macOS: Uses raw AppKit `orderFrontRegardless` to order the window to the front
//      without making it key window and without activating NSApplication.
//    - On Windows: Uses `ShowWindow(hwnd, SW_SHOWNOACTIVATE)` and
//      `SetWindowPos(hwnd, HWND_TOPMOST, ..., SWP_NOACTIVATE | SWP_SHOWWINDOW)`,
//      plus `WS_EX_NOACTIVATE` window style so clicks never steal focus.
//
// 2. `keydown` Leak Prevention (Event Swallowing):
//    Assessment sites attach `window.addEventListener('keydown', ...)` to log shortcuts.
//    Chrome also reserves shortcuts like Cmd+1 / Cmd+2 to switch tabs.
//    If global shortcuts aren't swallowed, Chrome receives the keydown event and
//    switches tabs (triggering `visibilitychange` and `blur`).
//    Here, we provide low-level OS hooks that swallow hotkeys:
//    - On macOS: Uses CoreGraphics `CGEventTapCreate` at `kCGHeadInsertEventTap`.
//      When our hotkey is pressed, the callback returns NULL, completely deleting
//      the event from the OS event stream before Chrome or any app can see it.
//    - On Windows: Uses `SetWindowsHookExW(WH_KEYBOARD_LL, ...)`.
//      When our hotkey is pressed, the hook procedure returns LRESULT(1), dropping
//      the message before Windows posts it to Chrome's message queue.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

// ── Global State ────────────────────────────────────────────────────────────

lazy_static::lazy_static! {
    static ref ACTIVE_HOTKEYS: Mutex<Vec<ParsedHotkey>> = Mutex::new(Vec::new());
    static ref APP_HANDLE: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);
}

static SWALLOWING_ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    SolveText,
    SolveScreenshot,
    AudioToggle,
    ScrollUp,
    ScrollDown,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ToggleVisibility,
    QuitApp,
}

impl HotkeyAction {
    #[allow(dead_code)]
    pub fn event_name(&self) -> Option<&'static str> {
        match self {
            HotkeyAction::SolveText => Some("hotkey-solve-text"),
            HotkeyAction::SolveScreenshot => Some("hotkey-solve-screenshot"),
            HotkeyAction::AudioToggle => Some("hotkey-audio-toggle"),
            HotkeyAction::ScrollUp => Some("hotkey-scroll-up"),
            HotkeyAction::ScrollDown => Some("hotkey-scroll-down"),
            HotkeyAction::MoveUp => Some("hotkey-move-up"),
            HotkeyAction::MoveDown => Some("hotkey-move-down"),
            HotkeyAction::MoveLeft => Some("hotkey-move-left"),
            HotkeyAction::MoveRight => Some("hotkey-move-right"),
            HotkeyAction::ToggleVisibility => None,
            HotkeyAction::QuitApp => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            HotkeyAction::SolveText => "solve-text",
            HotkeyAction::SolveScreenshot => "solve-screenshot",
            HotkeyAction::AudioToggle => "audio-toggle",
            HotkeyAction::ScrollUp => "scroll-up",
            HotkeyAction::ScrollDown => "scroll-down",
            HotkeyAction::MoveUp => "move-up",
            HotkeyAction::MoveDown => "move-down",
            HotkeyAction::MoveLeft => "move-left",
            HotkeyAction::MoveRight => "move-right",
            HotkeyAction::ToggleVisibility => "toggle-visibility",
            HotkeyAction::QuitApp => "quit-app",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedHotkey {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key_code: u32,
    pub action: HotkeyAction,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Returns whether low-level OS event swallowing is currently active.
pub fn is_swallowing_active() -> bool {
    SWALLOWING_ACTIVE.load(Ordering::Relaxed)
}

/// Check if accessibility / input monitoring permission is granted (macOS).
pub fn is_accessibility_granted() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows low-level keyboard hooks do not require special accessibility permissions
        true
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Bring the main window to the front without stealing focus from Chrome or the active app.
pub fn bring_to_front_without_focus(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::c_void;
        extern "C" {
            fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
            fn objc_msgSend();
        }
        type SendNoArg = unsafe extern "C" fn(*mut c_void, *mut c_void);

        unsafe {
            if let Ok(ptr) = win.ns_window() {
                let ns_win = ptr as *mut c_void;
                if !ns_win.is_null() {
                    // orderFrontRegardless brings the window to the front within its level
                    // (floating/always-on-top) WITHOUT activating NSApp and WITHOUT becoming key.
                    let order_front_sel = sel_registerName(b"orderFrontRegardless\0".as_ptr() as *const _);
                    let send: SendNoArg = std::mem::transmute(objc_msgSend as *const ());
                    send(ns_win, order_front_sel);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;

        if let Ok(h) = win.hwnd() {
            unsafe {
                let hwnd = HWND(h.0 as *mut _);
                // SW_SHOWNOACTIVATE (4): Displays window without activating it.
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = win.show();
    }
}

/// Toggle window visibility without stealing focus from the active foreground app.
pub fn toggle_visibility_without_focus(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        {
            if crate::STEALTH_ENABLED.load(Ordering::Relaxed) {
                crate::toggle_window_offscreen_win32(&win);
                return;
            }
        }

        match win.is_visible() {
            Ok(true) => {
                let _ = win.hide();
            }
            Ok(false) => {
                bring_to_front_without_focus(&win);
                crate::reapply_stealth_after_show(&win);
            }
            Err(_) => {
                bring_to_front_without_focus(&win);
                crate::reapply_stealth_after_show(&win);
            }
        }
    }
}

/// Dispatch a matched hotkey action safely in the background.
pub fn dispatch_action(app: &tauri::AppHandle, action: HotkeyAction) {
    match action {
        HotkeyAction::SolveText => {
            if let Some(win) = app.get_webview_window("main") {
                bring_to_front_without_focus(&win);
                crate::reapply_stealth_after_show(&win);
            }
            let _ = app.emit("hotkey-solve-text", ());
        }
        HotkeyAction::SolveScreenshot => {
            if let Some(win) = app.get_webview_window("main") {
                bring_to_front_without_focus(&win);
                crate::reapply_stealth_after_show(&win);
            }
            let _ = app.emit("hotkey-solve-screenshot", ());
        }
        HotkeyAction::AudioToggle => {
            if let Some(win) = app.get_webview_window("main") {
                bring_to_front_without_focus(&win);
                crate::reapply_stealth_after_show(&win);
            }
            let _ = app.emit("hotkey-audio-toggle", ());
        }
        HotkeyAction::ScrollUp => {
            let _ = app.emit("hotkey-scroll-up", ());
        }
        HotkeyAction::ScrollDown => {
            let _ = app.emit("hotkey-scroll-down", ());
        }
        HotkeyAction::MoveUp => {
            let _ = app.emit("hotkey-move-up", ());
        }
        HotkeyAction::MoveDown => {
            let _ = app.emit("hotkey-move-down", ());
        }
        HotkeyAction::MoveLeft => {
            let _ = app.emit("hotkey-move-left", ());
        }
        HotkeyAction::MoveRight => {
            let _ = app.emit("hotkey-move-right", ());
        }
        HotkeyAction::ToggleVisibility => {
            toggle_visibility_without_focus(app);
        }
        HotkeyAction::QuitApp => {
            app.exit(0);
        }
    }
}

/// Register all configured hotkeys with low-level event swallowing.
/// Returns true if OS-level event swallowing is successfully active, false if fallback is needed.
pub fn setup_stealth_hotkeys(app: &tauri::AppHandle, cfg: &crate::HotkeysConfig) -> bool {
    let mut parsed_list = Vec::new();

    let pairs = [
        (&cfg.text, HotkeyAction::SolveText),
        (&cfg.screenshot, HotkeyAction::SolveScreenshot),
        (&cfg.audio_toggle, HotkeyAction::AudioToggle),
        (&cfg.scroll_up, HotkeyAction::ScrollUp),
        (&cfg.scroll_down, HotkeyAction::ScrollDown),
        (&cfg.move_up, HotkeyAction::MoveUp),
        (&cfg.move_down, HotkeyAction::MoveDown),
        (&cfg.move_left, HotkeyAction::MoveLeft),
        (&cfg.move_right, HotkeyAction::MoveRight),
        (&cfg.toggle_visibility, HotkeyAction::ToggleVisibility),
        (&cfg.quit_app, HotkeyAction::QuitApp),
    ];

    for (hotkey_str, action) in pairs {
        if let Some(parsed) = parse_hotkey_string(hotkey_str, action) {
            parsed_list.push(parsed);
        }
    }

    {
        let mut guard = ACTIVE_HOTKEYS.lock().unwrap();
        *guard = parsed_list;
    }
    {
        let mut app_guard = APP_HANDLE.lock().unwrap();
        *app_guard = Some(app.clone());
    }

    // Initialize OS hook once
    if !HOOK_INITIALIZED.load(Ordering::Relaxed) {
        #[cfg(target_os = "macos")]
        {
            init_macos_event_tap();
        }
        #[cfg(target_os = "windows")]
        {
            init_windows_hook();
        }
        HOOK_INITIALIZED.store(true, Ordering::Relaxed);
    }

    SWALLOWING_ACTIVE.load(Ordering::Relaxed)
}

// ── Hotkey String Parsing ───────────────────────────────────────────────────

fn parse_hotkey_string(input: &str, action: HotkeyAction) -> Option<ParsedHotkey> {
    let parts: Vec<&str> = input.split('+').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut cmd = false;
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut key_str = "";

    for p in parts {
        if p.eq_ignore_ascii_case("cmd") || p.eq_ignore_ascii_case("command") || p.eq_ignore_ascii_case("super") {
            cmd = true;
        } else if p.eq_ignore_ascii_case("ctrl") || p.eq_ignore_ascii_case("control") {
            ctrl = true;
        } else if p.eq_ignore_ascii_case("alt") || p.eq_ignore_ascii_case("option") {
            alt = true;
        } else if p.eq_ignore_ascii_case("shift") {
            shift = true;
        } else if p.eq_ignore_ascii_case("cmdorctrl") {
            #[cfg(target_os = "macos")]
            { cmd = true; }
            #[cfg(not(target_os = "macos"))]
            { ctrl = true; }
        } else {
            key_str = p;
        }
    }

    if key_str.is_empty() {
        return None;
    }

    let key_code = os_keycode_from_str(key_str)?;

    Some(ParsedHotkey {
        cmd,
        ctrl,
        alt,
        shift,
        key_code,
        action,
    })
}

fn os_keycode_from_str(key: &str) -> Option<u32> {
    #[cfg(target_os = "macos")]
    {
        match key.to_uppercase().as_str() {
            "1" => Some(18),
            "2" => Some(19),
            "3" => Some(20),
            "4" => Some(21),
            "5" => Some(23),
            "6" => Some(22),
            "7" => Some(26),
            "8" => Some(28),
            "9" => Some(25),
            "0" => Some(29),
            "A" => Some(0),
            "B" => Some(11),
            "C" => Some(8),
            "D" => Some(2),
            "E" => Some(14),
            "F" => Some(3),
            "G" => Some(5),
            "H" => Some(4),
            "I" => Some(34),
            "J" => Some(38),
            "K" => Some(40),
            "L" => Some(37),
            "M" => Some(46),
            "N" => Some(45),
            "O" => Some(31),
            "P" => Some(35),
            "Q" => Some(12),
            "R" => Some(15),
            "S" => Some(1),
            "T" => Some(17),
            "U" => Some(32),
            "V" => Some(9),
            "W" => Some(13),
            "X" => Some(7),
            "Y" => Some(16),
            "Z" => Some(6),
            "UP" | "ARROWUP" => Some(126),
            "DOWN" | "ARROWDOWN" => Some(125),
            "LEFT" | "ARROWLEFT" => Some(123),
            "RIGHT" | "ARROWRIGHT" => Some(124),
            "ENTER" | "RETURN" => Some(36),
            "TAB" => Some(48),
            "SPACE" => Some(49),
            "ESCAPE" | "ESC" => Some(53),
            "F1" => Some(122),
            "F2" => Some(120),
            "F3" => Some(99),
            "F4" => Some(118),
            "F5" => Some(96),
            "F6" => Some(97),
            "F7" => Some(98),
            "F8" => Some(100),
            "F9" => Some(101),
            "F10" => Some(109),
            "F11" => Some(103),
            "F12" => Some(111),
            _ => None,
        }
    }

    #[cfg(target_os = "windows")]
    {
        match key.to_uppercase().as_str() {
            "0" => Some(0x30),
            "1" => Some(0x31),
            "2" => Some(0x32),
            "3" => Some(0x33),
            "4" => Some(0x34),
            "5" => Some(0x35),
            "6" => Some(0x36),
            "7" => Some(0x37),
            "8" => Some(0x38),
            "9" => Some(0x39),
            "A" => Some(0x41),
            "B" => Some(0x42),
            "C" => Some(0x43),
            "D" => Some(0x44),
            "E" => Some(0x45),
            "F" => Some(0x46),
            "G" => Some(0x47),
            "H" => Some(0x48),
            "I" => Some(0x49),
            "J" => Some(0x4A),
            "K" => Some(0x4B),
            "L" => Some(0x4C),
            "M" => Some(0x4D),
            "N" => Some(0x4E),
            "O" => Some(0x4F),
            "P" => Some(0x50),
            "Q" => Some(0x51),
            "R" => Some(0x52),
            "S" => Some(0x53),
            "T" => Some(0x54),
            "U" => Some(0x55),
            "V" => Some(0x56),
            "W" => Some(0x57),
            "X" => Some(0x58),
            "Y" => Some(0x59),
            "Z" => Some(0x5A),
            "UP" | "ARROWUP" => Some(0x26),
            "DOWN" | "ARROWDOWN" => Some(0x28),
            "LEFT" | "ARROWLEFT" => Some(0x25),
            "RIGHT" | "ARROWRIGHT" => Some(0x27),
            "ENTER" | "RETURN" => Some(0x0D),
            "TAB" => Some(0x09),
            "SPACE" => Some(0x20),
            "ESCAPE" | "ESC" => Some(0x1B),
            "F1" => Some(0x70),
            "F2" => Some(0x71),
            "F3" => Some(0x72),
            "F4" => Some(0x73),
            "F5" => Some(0x74),
            "F6" => Some(0x75),
            "F7" => Some(0x76),
            "F8" => Some(0x77),
            "F9" => Some(0x78),
            "F10" => Some(0x79),
            "F11" => Some(0x7A),
            "F12" => Some(0x7B),
            _ => None,
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = key;
        None
    }
}

// ── macOS Low-Level Event Tap Implementation ────────────────────────────────

#[cfg(target_os = "macos")]
type CGEventRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CGEventTapProxy = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFMachPortRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFRunLoopSourceRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFRunLoopRef = *mut std::ffi::c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const std::ffi::c_void;

#[cfg(target_os = "macos")]
type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    r#type: u32,
    event: CGEventRef,
    refcon: *mut std::ffi::c_void,
) -> CGEventRef;

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        eventsOfInterest: u64,
        callback: CGEventTapCallBack,
        refcon: *mut std::ffi::c_void,
    ) -> CFMachPortRef;

    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: *mut std::ffi::c_void,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    static kCFRunLoopCommonModes: CFStringRef;
}

#[cfg(target_os = "macos")]
static MAC_EVENT_TAP: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "macos")]
unsafe extern "C" fn macos_event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _refcon: *mut std::ffi::c_void,
) -> CGEventRef {
    // kCGEventTapDisabledByTimeout = 0xFFFFFFFE, kCGEventTapDisabledByUserInput = 0xFFFFFFFD
    if event_type == 0xFFFFFFFE || event_type == 0xFFFFFFFD {
        let tap = MAC_EVENT_TAP.load(Ordering::Relaxed) as CFMachPortRef;
        if !tap.is_null() {
            CGEventTapEnable(tap, true);
        }
        return event;
    }

    // kCGEventKeyDown = 10
    if event_type == 10 {
        let flags = CGEventGetFlags(event);
        // kCGKeyboardEventKeycode = 9
        let keycode = CGEventGetIntegerValueField(event, 9) as u32;

        // Modifier masks on macOS:
        // Command:   0x00100000 (1 << 20)
        // Shift:     0x00020000 (1 << 17)
        // Control:   0x00080000 (1 << 19)
        // Alternate: 0x00040000 (1 << 18)
        let has_cmd = (flags & 0x00100000) != 0;
        let has_shift = (flags & 0x00020000) != 0;
        let has_ctrl = (flags & 0x00080000) != 0;
        let has_alt = (flags & 0x00040000) != 0;

        let matched_action = {
            let guard = ACTIVE_HOTKEYS.lock().unwrap();
            guard.iter().find(|h| {
                h.cmd == has_cmd
                    && h.shift == has_shift
                    && h.ctrl == has_ctrl
                    && h.alt == has_alt
                    && h.key_code == keycode
            }).map(|h| h.action)
        };

        if let Some(action) = matched_action {
            println!("🕵️ [stealth-hotkey-macOS] Swallowed key event for {}", action.label());

            // Dispatch on a background thread so the event tap callback returns in microseconds
            if let Some(app) = APP_HANDLE.lock().unwrap().clone() {
                std::thread::spawn(move || {
                    dispatch_action(&app, action);
                });
            }

            // SWALLOW THE KEY EVENT: Returning NULL deletes the event completely.
            // Chrome never sees keydown, never switches tabs, and DOM listeners never fire.
            return std::ptr::null_mut();
        }
    }

    event
}

#[cfg(target_os = "macos")]
fn init_macos_event_tap() {
    std::thread::spawn(|| {
        unsafe {
            // kCGSessionEventTap = 1, kCGHeadInsertEventTap = 0, kCGEventTapOptionDefault = 0
            // eventsOfInterest: 1 << 10 (kCGEventKeyDown)
            let tap = CGEventTapCreate(
                1,
                0,
                0,
                1 << 10,
                macos_event_tap_callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                println!("⚠️ [stealth-hotkey-macOS] CGEventTapCreate returned NULL. Check Accessibility permissions in System Settings.");
                SWALLOWING_ACTIVE.store(false, Ordering::Relaxed);
                return;
            }

            MAC_EVENT_TAP.store(tap as isize, Ordering::Relaxed);
            let loop_source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap, 0);
            if loop_source.is_null() {
                println!("⚠️ [stealth-hotkey-macOS] CFMachPortCreateRunLoopSource failed.");
                SWALLOWING_ACTIVE.store(false, Ordering::Relaxed);
                return;
            }

            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, loop_source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            SWALLOWING_ACTIVE.store(true, Ordering::Relaxed);
            println!("🕵️ [stealth-hotkey-macOS] ✅ Active CGEventTap installed at kCGHeadInsertEventTap. Key event swallowing ENABLED.");

            CFRunLoopRun();
        }
    });
}

// ── Windows Low-Level Keyboard Hook Implementation ──────────────────────────

#[cfg(target_os = "windows")]
static WIN_HOOK_HANDLE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
unsafe extern "system" fn windows_keyboard_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::*;

    if code >= 0 {
        let msg = wparam.0 as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kbd.vkCode;

            // Check modifier states via GetAsyncKeyState:
            let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
            let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
            let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
            let win = ((GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0)
                || ((GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0);

            let matched_action = {
                let guard = ACTIVE_HOTKEYS.lock().unwrap();
                guard.iter().find(|h| {
                    h.ctrl == ctrl
                        && h.alt == alt
                        && h.shift == shift
                        && h.cmd == win
                        && h.key_code == vk
                }).map(|h| h.action)
            };

            if let Some(action) = matched_action {
                println!("🕵️ [stealth-hotkey-windows] Swallowed key event for {}", action.label());

                if let Some(app) = APP_HANDLE.lock().unwrap().clone() {
                    std::thread::spawn(move || {
                        dispatch_action(&app, action);
                    });
                }

                // SWALLOW THE KEY EVENT: Returning 1 halts propagation.
                // Windows will NOT dispatch WM_KEYDOWN/WM_KEYUP to Chrome.
                return LRESULT(1);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(target_os = "windows")]
fn init_windows_hook() {
    std::thread::spawn(|| {
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(windows_keyboard_proc),
                None,
                0,
            );

            match hook {
                Ok(h) => {
                    WIN_HOOK_HANDLE.store(h.0 as isize, Ordering::Relaxed);
                    SWALLOWING_ACTIVE.store(true, Ordering::Relaxed);
                    println!("🕵️ [stealth-hotkey-windows] ✅ WH_KEYBOARD_LL installed. Key event swallowing ENABLED.");

                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                Err(e) => {
                    println!("⚠️ [stealth-hotkey-windows] Failed to install WH_KEYBOARD_LL: {:?}", e);
                    SWALLOWING_ACTIVE.store(false, Ordering::Relaxed);
                }
            }
        }
    });
}
