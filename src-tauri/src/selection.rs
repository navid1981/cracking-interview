// Selection capture / paste hotkeys.
//
// Capture: reads the text currently selected in whatever app is frontmost and keeps it in
// app memory (never left on the system clipboard).
//   1. macOS: ask the Accessibility API for the focused element's selected text (walking
//      up AXParent for WebKit/Chromium views; works even when the source app blocks copying).
//   2. Windows: ask Windows UI Automation (IUIAutomation TextPattern, including CefSharp/
//      Chromium document nodes) for the selected text without sending Ctrl+C or touching
//      the clipboard (works even when Safe Exam Browser blocks copy/paste & clipboard).
//   3. Fallback: simulate the OS copy shortcut, read the clipboard, then restore
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

fn truncate_capture(text: String) -> Option<String> {
    let trimmed: String = text.chars().take(MAX_CAPTURE_CHARS).collect();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn join_selection_fragments<I>(fragments: I) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    let parts: Vec<String> = fragments.into_iter().filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        None
    } else {
        truncate_capture(parts.join("\n"))
    }
}

/// Capture the current selection into app memory. Runs on a background thread because it
/// may query UIA/AX or wait for hotkey modifiers to be released.
pub fn capture(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let text = capture_selected_text();
        match text {
            Some(t) => {
                let t = truncate_capture(t).unwrap_or_default();
                let count = t.chars().count();
                println!("📋 [selection] Captured {} chars", count);
                crate::resilience::log_event(&format!("[selection] Stored {} chars in app memory", count));
                if let Ok(mut guard) = CAPTURED.lock() {
                    *guard = Some(t);
                }
                let _ = app.emit("selection-captured", ());
            }
            None => {
                println!("⚠️ [selection] Nothing captured (no selection, or the source blocked it)");
                crate::resilience::log_event(
                    "[selection] Nothing captured (no selection, or source blocked UIA/AX and clipboard)",
                );
            }
        }
    });
}

/// Send the stored text to the frontend to be inserted into the focused field.
pub fn paste(app: &tauri::AppHandle) {
    let text = CAPTURED.lock().ok().and_then(|g| g.clone());
    match text {
        Some(t) => {
            crate::resilience::log_event(&format!(
                "[selection] Pasting {} chars to frontend",
                t.chars().count()
            ));
            let _ = app.emit("hotkey-paste-selection", t);
        }
        None => {
            println!("ℹ️ [selection] Paste hotkey pressed but nothing has been captured yet");
            crate::resilience::log_event(
                "[selection] Paste hotkey pressed but nothing has been captured yet",
            );
        }
    }
}

fn capture_selected_text() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(t) = macos::selected_text_via_accessibility() {
            crate::resilience::log_event(&format!(
                "[selection] Captured {} chars via macOS Accessibility",
                t.chars().count()
            ));
            return Some(t);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(t) = windows_impl::selected_text_via_uia() {
            crate::resilience::log_event(&format!(
                "[selection] Captured {} chars via Windows UI Automation",
                t.chars().count()
            ));
            return Some(t);
        }
    }
    let res = capture_via_clipboard();
    if let Some(ref t) = res {
        crate::resilience::log_event(&format!(
            "[selection] Captured {} chars via clipboard fallback",
            t.chars().count()
        ));
    }
    res
}

/// Simulate the OS copy shortcut and read the result, restoring the previous clipboard text.
fn capture_via_clipboard() -> Option<String> {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            crate::resilience::log_event(&format!("[selection] clipboard open failed: {}", e));
            return None;
        }
    };
    let previous = clipboard.get_text().ok();

    // Sentinel lets us tell "copy produced nothing" apart from "clipboard unchanged".
    let sentinel = format!(
        "__ci_selection_sentinel_{}__",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    if let Err(e) = clipboard.set_text(sentinel.clone()) {
        crate::resilience::log_event(&format!("[selection] clipboard set_text failed: {}", e));
        return None;
    }

    wait_for_modifiers_released();
    send_copy_keystroke();

    let mut result = None;
    let mut saw_cleared = false;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(20));
        match clipboard.get_text() {
            Ok(t) if t != sentinel => {
                if t.is_empty() {
                    saw_cleared = true;
                } else {
                    result = Some(t);
                    break;
                }
            }
            _ => {}
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

    if result.is_none() {
        crate::resilience::log_event(&format!(
            "[selection] clipboard fallback returned nothing (cleared_by_target={})",
            saw_cleared
        ));
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
    /// Walks up `AXParent` because WebKit/Chromium views (including Safe Exam Browser) often
    /// report a child element as focused while `AXSelectedText` lives on the parent `AXWebArea`.
    /// Returns None when permission is missing or the focused app doesn't expose its selection.
    pub fn selected_text_via_accessibility() -> Option<String> {
        unsafe {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return None;
            }
            let focused_attr = cf_string("AXFocusedUIElement");
            let selected_attr = cf_string("AXSelectedText");
            let parent_attr = cf_string("AXParent");

            let mut result: Option<String> = None;
            let mut focused: *mut c_void = std::ptr::null_mut();
            if AXUIElementCopyAttributeValue(system, focused_attr, &mut focused) == K_AX_ERROR_SUCCESS
                && !focused.is_null()
            {
                let mut cur = focused;
                for _ in 0..12 {
                    let mut value: *mut c_void = std::ptr::null_mut();
                    if AXUIElementCopyAttributeValue(cur, selected_attr, &mut value)
                        == K_AX_ERROR_SUCCESS
                        && !value.is_null()
                    {
                        if CFGetTypeID(value) == CFStringGetTypeID() {
                            result = cf_string_to_rust(value).filter(|t| !t.is_empty());
                        }
                        CFRelease(value);
                        if result.is_some() {
                            break;
                        }
                    }

                    let mut next: *mut c_void = std::ptr::null_mut();
                    if AXUIElementCopyAttributeValue(cur, parent_attr, &mut next)
                        != K_AX_ERROR_SUCCESS
                        || next.is_null()
                    {
                        break;
                    }
                    CFRelease(cur);
                    cur = next;
                }
                CFRelease(cur);
            }

            CFRelease(parent_attr);
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

pub(crate) fn is_browser_render_child_class(class: &str) -> bool {
    let lower = class.to_ascii_lowercase();
    matches!(
        class,
        "Chrome_RenderWidgetHostHWND"
            | "Chrome_WidgetWin_0"
            | "Chrome_WidgetWin_1"
            | "CefBrowserWindow"
            | "CefWebViewWnd"
            | "WebViewHost"
            | "Internet Explorer_Server"
            | "MozillaWindowClass"
    ) || lower.contains("chrome")
        || lower.contains("cef")
        || lower.contains("webview")
        || lower.contains("mozilla")
        || lower.contains("internet explorer_server")
}

// ── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::collections::BTreeSet;
    use std::time::Duration;
    use windows::Win32::Foundation::{
        BOOL, CloseHandle, HANDLE, HWND, LPARAM, RPC_E_CHANGED_MODE, S_FALSE, S_OK, WPARAM,
    };
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken,
        QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomationCondition,
        IUIAutomationElement, IUIAutomationTextChildPattern, IUIAutomationTextPattern,
        IUIAutomationTextPattern2, TreeScope_Descendants, UIA_ControlTypePropertyId,
        UIA_DocumentControlTypeId, UIA_IsTextPattern2AvailablePropertyId,
        UIA_IsTextPatternAvailablePropertyId, UIA_TextChildPatternId, UIA_TextPattern2Id,
        UIA_TextPatternId,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassNameW, GetForegroundWindow,
        GetWindowThreadProcessId, IsWindowVisible, SendMessageTimeoutW, OBJID_CLIENT,
        SMTO_ABORTIFHUNG, WM_GETOBJECT,
    };

    const VK_C: u16 = 0x43;

    struct ComGuard {
        should_uninit: bool,
    }

    impl ComGuard {
        fn init() -> Option<Self> {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr == S_OK || hr == S_FALSE {
                Some(Self { should_uninit: true })
            } else if hr == RPC_E_CHANGED_MODE {
                Some(Self {
                    should_uninit: false,
                })
            } else {
                None
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.should_uninit {
                unsafe { CoUninitialize() };
            }
        }
    }

    unsafe fn hwnd_class_name(hwnd: HWND) -> String {
        let mut buf = [0u16; 128];
        let len = GetClassNameW(hwnd, &mut buf);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }

    unsafe fn process_exe_and_elevation(pid: u32) -> (String, Option<bool>) {
        let Ok(hproc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return ("<open-denied>".to_string(), None);
        };
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let exe = if QueryFullProcessImageNameW(
            hproc,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok()
        {
            let full = String::from_utf16_lossy(&buf[..size as usize]);
            full.rsplit('\\').next().unwrap_or(&full).to_string()
        } else {
            "<unknown>".to_string()
        };
        let elevated = query_process_elevation(hproc);
        let _ = CloseHandle(hproc);
        (exe, elevated)
    }

    unsafe fn query_process_elevation(hproc: HANDLE) -> Option<bool> {
        let mut token = HANDLE::default();
        if OpenProcessToken(hproc, TOKEN_QUERY, &mut token).is_err() {
            return None;
        }
        let mut elev = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let res = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut TOKEN_ELEVATION as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        let _ = CloseHandle(token);
        res.ok().map(|_| elev.TokenIsElevated != 0)
    }

    unsafe fn create_uiautomation() -> Option<IUIAutomation> {
        CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
            .or_else(|_| CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER))
            .ok()
    }

    unsafe fn text_from_ranges(tp: &IUIAutomationTextPattern) -> Option<String> {
        let ranges = tp.GetSelection().ok()?;
        let len = ranges.Length().ok()?;
        let mut frags = Vec::with_capacity(len.max(0) as usize);
        for i in 0..len {
            if let Ok(range) = ranges.GetElement(i) {
                let bstr = range
                    .GetText(super::MAX_CAPTURE_CHARS as i32)
                    .or_else(|_| range.GetText(-1))
                    .ok();
                if let Some(bstr) = bstr {
                    let s = bstr.to_string();
                    if !s.is_empty() {
                        frags.push(s);
                    }
                }
            }
        }
        super::join_selection_fragments(frags)
    }

    unsafe fn extract_text_from_element(el: &IUIAutomationElement) -> Option<String> {
        if let Ok(tp) = el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
            if let Some(t) = text_from_ranges(&tp) {
                return Some(t);
            }
        }
        if let Ok(tp2) = el.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) {
            if let Some(t) = text_from_ranges(&tp2) {
                return Some(t);
            }
        }
        None
    }

    unsafe fn extract_from_focused_and_ancestors(
        automation: &IUIAutomation,
    ) -> (Option<String>, String) {
        let focused = match automation.GetFocusedElement() {
            Ok(f) => f,
            Err(e) => return (None, format!("GetFocusedElement err={:?}", e.code())),
        };
        let f_ctrl = focused.CurrentControlType().map(|c| c.0).unwrap_or(-1);
        let f_class = focused
            .CurrentClassName()
            .map(|b| b.to_string())
            .unwrap_or_default();
        let diag = format!("focused(ctrl={},class='{}')", f_ctrl, f_class);

        if let Some(t) = extract_text_from_element(&focused) {
            return (Some(t), diag);
        }
        if let Ok(tcp) =
            focused.GetCurrentPatternAs::<IUIAutomationTextChildPattern>(UIA_TextChildPatternId)
        {
            if let Ok(container) = tcp.TextContainer() {
                if let Some(t) = extract_text_from_element(&container) {
                    return (Some(t), diag);
                }
            }
        }
        if let Ok(walker) = automation.RawViewWalker() {
            let mut cur = focused;
            for _ in 0..16 {
                let Ok(parent) = walker.GetParentElement(&cur) else {
                    break;
                };
                if let Some(t) = extract_text_from_element(&parent) {
                    return (Some(t), diag);
                }
                cur = parent;
            }
        }
        (None, diag)
    }

    unsafe fn collect_top_windows_for_pid(pid: u32, primary: HWND) -> Vec<HWND> {
        struct Ctx {
            pid: u32,
            primary: HWND,
            list: Vec<HWND>,
        }
        unsafe extern "system" fn enum_top(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = &mut *(lparam.0 as *mut Ctx);
            if hwnd != ctx.primary && IsWindowVisible(hwnd).as_bool() {
                let mut wpid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut wpid));
                if wpid == ctx.pid {
                    ctx.list.push(hwnd);
                }
            }
            BOOL(1)
        }
        let mut ctx = Ctx {
            pid,
            primary,
            list: vec![primary],
        };
        if pid != 0 {
            let _ = EnumWindows(Some(enum_top), LPARAM(&mut ctx as *mut Ctx as isize));
        }
        ctx.list
    }

    unsafe fn collect_renderer_children(
        parent: HWND,
        child_classes: &mut BTreeSet<String>,
    ) -> Vec<HWND> {
        struct ChildCtx<'a> {
            list: Vec<HWND>,
            classes: &'a mut BTreeSet<String>,
        }
        unsafe extern "system" fn enum_cb(child: HWND, lparam: LPARAM) -> BOOL {
            let ctx = &mut *(lparam.0 as *mut ChildCtx<'_>);
            let mut buf = [0u16; 128];
            let len = GetClassNameW(child, &mut buf);
            if len > 0 {
                let class = String::from_utf16_lossy(&buf[..len as usize]);
                if ctx.classes.len() < 16 {
                    ctx.classes.insert(class.clone());
                }
                if super::is_browser_render_child_class(&class) {
                    ctx.list.push(child);
                }
            }
            BOOL(1)
        }
        let mut ctx = ChildCtx {
            list: Vec::new(),
            classes: child_classes,
        };
        let _ = EnumChildWindows(
            parent,
            Some(enum_cb),
            LPARAM(&mut ctx as *mut ChildCtx<'_> as isize),
        );
        ctx.list
    }

    unsafe fn build_text_search_condition(
        automation: &IUIAutomation,
    ) -> Option<IUIAutomationCondition> {
        let text_avail = automation
            .CreatePropertyCondition(
                UIA_IsTextPatternAvailablePropertyId,
                &windows::core::VARIANT::from(true),
            )
            .ok();
        let text2_avail = automation
            .CreatePropertyCondition(
                UIA_IsTextPattern2AvailablePropertyId,
                &windows::core::VARIANT::from(true),
            )
            .ok();
        let doc_cond = automation
            .CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &windows::core::VARIANT::from(UIA_DocumentControlTypeId.0),
            )
            .ok();

        let mut cond: Option<IUIAutomationCondition> = None;
        for next in [text_avail, text2_avail, doc_cond].into_iter().flatten() {
            cond = match cond {
                None => Some(next),
                Some(prev) => automation.CreateOrCondition(&prev, &next).ok().or(Some(prev)),
            };
        }
        cond
    }

    unsafe fn wake_accessibility(hwnds: &[HWND]) {
        for &hwnd in hwnds {
            let _ = SendMessageTimeoutW(
                hwnd,
                WM_GETOBJECT,
                WPARAM(0),
                LPARAM(OBJID_CLIENT.0 as isize),
                SMTO_ABORTIFHUNG,
                50,
                None,
            );
        }
    }

    unsafe fn probe_uia_candidates(
        automation: &IUIAutomation,
        candidates: &[HWND],
        search_cond: &Option<IUIAutomationCondition>,
    ) -> (Option<String>, usize) {
        let mut total_matches = 0usize;
        for &hwnd in candidates {
            let Ok(root) = automation.ElementFromHandle(hwnd) else {
                continue;
            };
            if let Some(t) = extract_text_from_element(&root) {
                return (Some(t), total_matches + 1);
            }
            if let Some(ref cond) = search_cond {
                if let Ok(nodes) = root.FindAll(TreeScope_Descendants, cond) {
                    if let Ok(len) = nodes.Length() {
                        total_matches += len.max(0) as usize;
                        for i in 0..len {
                            if let Ok(node) = nodes.GetElement(i) {
                                if let Some(t) = extract_text_from_element(&node) {
                                    return (Some(t), total_matches);
                                }
                            }
                        }
                    }
                }
            }
        }
        (None, total_matches)
    }

    /// Read selected text from the foreground app via Windows UI Automation (`TextPattern`).
    /// Works even when Safe Exam Browser blocks `Ctrl+C` or disables/isolates the clipboard.
    pub fn selected_text_via_uia() -> Option<String> {
        let _com = ComGuard::init()?;
        unsafe {
            let automation = create_uiautomation()?;
            let fg = GetForegroundWindow();
            let mut fg_pid = 0u32;
            if !fg.0.is_null() {
                GetWindowThreadProcessId(fg, Some(&mut fg_pid));
            }
            let (fg_exe, fg_elev) = if fg_pid != 0 {
                process_exe_and_elevation(fg_pid)
            } else {
                ("<none>".to_string(), None)
            };
            let self_elev = query_process_elevation(GetCurrentProcess());
            let fg_class = if !fg.0.is_null() {
                hwnd_class_name(fg)
            } else {
                String::new()
            };

            let (focused_res, focused_diag) = extract_from_focused_and_ancestors(&automation);
            if focused_res.is_some() {
                return focused_res;
            }

            if fg.0.is_null() || fg_pid == GetCurrentProcessId() {
                crate::resilience::log_event(&format!(
                    "[selection] UIA skipped foreground probe (fg_null={} fg_pid={} {})",
                    fg.0.is_null(),
                    fg_pid,
                    focused_diag
                ));
                return None;
            }

            let top_windows = collect_top_windows_for_pid(fg_pid, fg);
            let mut child_classes = BTreeSet::new();
            let mut candidates = Vec::new();
            for &top in &top_windows {
                candidates.extend(collect_renderer_children(top, &mut child_classes));
            }
            candidates.extend(top_windows.iter().copied());

            wake_accessibility(&candidates);
            let search_cond = build_text_search_condition(&automation);

            let (text1, matches1) = probe_uia_candidates(&automation, &candidates, &search_cond);
            if text1.is_some() {
                return text1;
            }

            // Chromium / CefSharp / LockDown Browser initializes its accessibility tree lazily
            // after the first WM_GETOBJECT / UIA call. Give it 150ms and probe once more.
            std::thread::sleep(Duration::from_millis(150));
            let (focused_res2, _) = extract_from_focused_and_ancestors(&automation);
            if focused_res2.is_some() {
                return focused_res2;
            }
            let (text2, matches2) = probe_uia_candidates(&automation, &candidates, &search_cond);
            if text2.is_some() {
                return text2;
            }

            let classes_summary: Vec<String> = child_classes.into_iter().collect();
            crate::resilience::log_event(&format!(
                "[selection] UIA empty: fg_exe='{}' fg_class='{}' fg_elev={:?} self_elev={:?} {} candidates={} child_classes=[{}] matches=({},{})",
                fg_exe,
                fg_class,
                fg_elev,
                self_elev,
                focused_diag,
                candidates.len(),
                classes_summary.join(","),
                matches1,
                matches2
            ));
        }
        None
    }

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
        let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16;
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_capture_rejects_empty_and_caps_length() {
        assert_eq!(truncate_capture(String::new()), None);
        let huge = "a".repeat(MAX_CAPTURE_CHARS + 50);
        let capped = truncate_capture(huge).unwrap();
        assert_eq!(capped.chars().count(), MAX_CAPTURE_CHARS);
    }

    #[test]
    fn test_join_selection_fragments_skips_empty() {
        assert_eq!(
            join_selection_fragments(vec!["".to_string(), "".to_string()]),
            None
        );
        assert_eq!(
            join_selection_fragments(vec![
                "first line".to_string(),
                "".to_string(),
                "second line".to_string()
            ]),
            Some("first line\nsecond line".to_string())
        );
    }

    #[test]
    fn test_is_browser_render_child_class() {
        assert!(is_browser_render_child_class("Chrome_RenderWidgetHostHWND"));
        assert!(is_browser_render_child_class("CefBrowserWindow"));
        assert!(!is_browser_render_child_class("Button"));
    }
}
