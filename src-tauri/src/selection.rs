// Selection capture / paste hotkeys.
//
// Capture: reads the text currently selected in whatever app is frontmost and keeps it in
// app memory (never left on the system clipboard).
//   1. macOS: ask the Accessibility API for the focused element's selected text (works even
//      when the source app blocks copying).
//   2. Fallback (and Windows): simulate the OS copy shortcut, read the clipboard, then restore
//      the previous clipboard text.
//
// Paste: emits the stored text to the frontend, which inserts it into the focused field of the
// app window. No OS key injection and no window activation, so no focus is stolen.

use std::sync::Mutex;
use std::time::Duration;
use tauri::Emitter;

/// Cap on stored text so a huge selection can't bloat memory or the IPC payload.
const MAX_CAPTURE_CHARS: usize = 200_000;

lazy_static::lazy_static! {
    static ref CAPTURED: Mutex<Option<String>> = Mutex::new(None);
}

/// Capture the current selection into app memory. Runs on a background thread because it
/// waits for the hotkey modifiers to be released.
pub fn capture(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let text = capture_selected_text();
        match text {
            Some(t) => {
                let t: String = t.chars().take(MAX_CAPTURE_CHARS).collect();
                println!("📋 [selection] Captured {} chars", t.chars().count());
                if let Ok(mut guard) = CAPTURED.lock() {
                    *guard = Some(t);
                }
                let _ = app.emit("selection-captured", ());
            }
            None => println!("⚠️ [selection] Nothing captured (no selection, or the source blocked it)"),
        }
    });
}

/// Send the stored text to the frontend to be inserted into the focused field.
pub fn paste(app: &tauri::AppHandle) {
    let text = CAPTURED.lock().ok().and_then(|g| g.clone());
    match text {
        Some(t) => {
            let _ = app.emit("hotkey-paste-selection", t);
        }
        None => println!("ℹ️ [selection] Paste hotkey pressed but nothing has been captured yet"),
    }
}

fn capture_selected_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(t) = macos::selected_text_via_accessibility() {
            return Some(t);
        }
    }
    capture_via_clipboard()
}

/// Simulate the OS copy shortcut and read the result, restoring the previous clipboard text.
fn capture_via_clipboard() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let previous = clipboard.get_text().ok();

    // Sentinel lets us tell "copy produced nothing" apart from "clipboard unchanged".
    let sentinel = format!(
        "__ci_selection_sentinel_{}__",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    clipboard.set_text(sentinel.clone()).ok()?;

    wait_for_modifiers_released();
    send_copy_keystroke();

    let mut result = None;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(20));
        if let Ok(t) = clipboard.get_text() {
            if t != sentinel {
                result = Some(t);
                break;
            }
        }
    }

    match previous {
        Some(p) => {
            let _ = clipboard.set_text(p);
        }
        None => {
            let _ = clipboard.clear();
        }
    }

    result.filter(|t| !t.is_empty())
}

#[cfg(target_os = "macos")]
fn wait_for_modifiers_released() {
    macos::wait_for_modifiers_released();
}
#[cfg(target_os = "windows")]
fn wait_for_modifiers_released() {
    windows_impl::wait_for_modifiers_released();
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn wait_for_modifiers_released() {}

#[cfg(target_os = "macos")]
fn send_copy_keystroke() {
    macos::send_copy_keystroke();
}
#[cfg(target_os = "windows")]
fn send_copy_keystroke() {
    windows_impl::send_copy_keystroke();
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn send_copy_keystroke() {}

// ── macOS ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void, CString};
    use std::time::Duration;

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_AX_ERROR_SUCCESS: i32 = 0;

    // CGEventFlags
    const FLAG_SHIFT: u64 = 0x0002_0000;
    const FLAG_CONTROL: u64 = 0x0008_0000;
    const FLAG_ALT: u64 = 0x0004_0000;
    const FLAG_COMMAND: u64 = 0x0010_0000;

    // kVK_ANSI_C
    const KEYCODE_C: u16 = 8;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> *mut c_void;
        fn AXUIElementCopyAttributeValue(
            element: *mut c_void,
            attribute: *const c_void,
            value: *mut *mut c_void,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> *mut c_void;
        fn CFStringGetLength(s: *const c_void) -> isize;
        fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
        fn CFStringGetCString(
            s: *const c_void,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> bool;
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFRelease(cf: *mut c_void);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, key_down: bool) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut c_void);
        fn CGEventSourceFlagsState(state_id: i32) -> u64;
    }

    unsafe fn cf_string(s: &str) -> *mut c_void {
        let c = CString::new(s).unwrap();
        CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8)
    }

    unsafe fn cf_string_to_rust(s: *const c_void) -> Option<String> {
        let len = CFStringGetLength(s);
        let max = CFStringGetMaximumSizeForEncoding(len, K_CF_STRING_ENCODING_UTF8) + 1;
        let mut buf = vec![0u8; max as usize];
        if !CFStringGetCString(s, buf.as_mut_ptr() as *mut c_char, max, K_CF_STRING_ENCODING_UTF8) {
            return None;
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8(buf[..end].to_vec()).ok()
    }

    /// Read the selected text of the system-wide focused element via the Accessibility API.
    /// Returns None when permission is missing or the focused app doesn't expose its selection.
    pub fn selected_text_via_accessibility() -> Option<String> {
        unsafe {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return None;
            }
            let focused_attr = cf_string("AXFocusedUIElement");
            let selected_attr = cf_string("AXSelectedText");

            let mut result: Option<String> = None;
            let mut focused: *mut c_void = std::ptr::null_mut();
            if AXUIElementCopyAttributeValue(system, focused_attr, &mut focused) == K_AX_ERROR_SUCCESS
                && !focused.is_null()
            {
                let mut value: *mut c_void = std::ptr::null_mut();
                if AXUIElementCopyAttributeValue(focused, selected_attr, &mut value) == K_AX_ERROR_SUCCESS
                    && !value.is_null()
                {
                    if CFGetTypeID(value) == CFStringGetTypeID() {
                        result = cf_string_to_rust(value).filter(|t| !t.is_empty());
                    }
                    CFRelease(value);
                }
                CFRelease(focused);
            }

            CFRelease(selected_attr);
            CFRelease(focused_attr);
            CFRelease(system);
            result
        }
    }

    /// Wait (up to ~600ms) until the user has released the hotkey modifiers, so the simulated
    /// copy isn't combined with a still-held Shift/Option.
    pub fn wait_for_modifiers_released() {
        let mask = FLAG_SHIFT | FLAG_CONTROL | FLAG_ALT | FLAG_COMMAND;
        for _ in 0..60 {
            // 0 = kCGEventSourceStateCombinedSessionState
            let flags = unsafe { CGEventSourceFlagsState(0) };
            if flags & mask == 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Post Cmd+C to the frontmost app.
    pub fn send_copy_keystroke() {
        unsafe {
            for key_down in [true, false] {
                let event = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KEYCODE_C, key_down);
                if event.is_null() {
                    continue;
                }
                CGEventSetFlags(event, FLAG_COMMAND);
                // 0 = kCGHIDEventTap
                CGEventPost(0, event);
                CFRelease(event);
            }
        }
    }
}

// ── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::time::Duration;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    const VK_C: u16 = 0x43;

    pub fn wait_for_modifiers_released() {
        let modifiers = [VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN, VK_RWIN];
        for _ in 0..60 {
            let any_down = modifiers
                .iter()
                .any(|vk| unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 });
            if !any_down {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Send Ctrl+C to the foreground app.
    pub fn send_copy_keystroke() {
        let inputs = [
            key(VK_CONTROL, false),
            key(VIRTUAL_KEY(VK_C), false),
            key(VIRTUAL_KEY(VK_C), true),
            key(VK_CONTROL, true),
        ];
        unsafe {
            let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }
}
