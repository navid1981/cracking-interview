//! macOS-specific resilience helpers (Scenarios B2a, B2b, B2c, B1).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::Manager;

pub const CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
pub const MOVE_TO_ACTIVE_SPACE: u64 = 1 << 1;
#[allow(dead_code)]
pub const STATIONARY: u64 = 1 << 4;
#[allow(dead_code)]
pub const IGNORES_CYCLE: u64 = 1 << 6;
pub const FULL_SCREEN_PRIMARY: u64 = 1 << 7;
pub const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

/// `NSWindowStyleMaskNonactivatingPanel` (`1 << 7`): allows an `NSPanel` to receive mouse clicks
/// (and, when `canBecomeKeyWindow` is `YES`, keyboard input) without activating `NSApplication`
/// or stealing focus from foreground apps such as LockDown Browser.
pub const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: u64 = 1 << 7;

/// Minimum `NSWindow.level` (`NSFloatingWindowLevel = 3`; Tauri's `alwaysOnTop: true` uses `5`).
/// Note: setting `level = 1001` or `NSWindowCollectionBehaviorStationary` breaks macOS
/// `FullScreenAuxiliary` Space joining over full-screen apps (like maximized Chrome).
pub const MIN_FLOATING_WINDOW_LEVEL: i64 = 3;

pub static SIGTERM_INTERCEPTED: AtomicBool = AtomicBool::new(false);
static NONACTIVATING_PANEL_ACTIVE: AtomicBool = AtomicBool::new(false);
static PANEL_CLASS_PTR: OnceLock<usize> = OnceLock::new();
static NSPANEL_PERFORM_KEY_EQUIV_IMP: OnceLock<usize> = OnceLock::new();

extern "C" fn panel_can_become_main(_this: *mut c_void, _sel: *mut c_void) -> bool {
    false
}

extern "C" fn panel_can_become_key(_this: *mut c_void, _sel: *mut c_void) -> bool {
    super::KEYBOARD_INPUT_ACTIVE.load(Ordering::Relaxed)
}

extern "C" fn panel_perform_key_equivalent(
    this: *mut c_void,
    sel: *mut c_void,
    event: *mut c_void,
) -> bool {
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    type GetU64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64;
    type GetPtr = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type GetCStr = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const std::ffi::c_char;
    type SendAction =
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void) -> bool;
    type SuperPerformKeyEquiv =
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool;

    unsafe {
        if super::KEYBOARD_INPUT_ACTIVE.load(Ordering::Relaxed) && !event.is_null() {
            const SHIFT: u64 = 1 << 17;
            const CONTROL: u64 = 1 << 18;
            const OPTION: u64 = 1 << 19;
            const COMMAND: u64 = 1 << 20;

            let get_u64: GetU64 = std::mem::transmute(objc_msgSend as *const ());
            let get_ptr: GetPtr = std::mem::transmute(objc_msgSend as *const ());
            let get_cstr: GetCStr = std::mem::transmute(objc_msgSend as *const ());

            let mods = get_u64(
                event,
                sel_registerName(b"modifierFlags\0".as_ptr() as *const _),
            );
            if (mods & COMMAND) != 0 && (mods & (CONTROL | OPTION)) == 0 {
                let chars = get_ptr(
                    event,
                    sel_registerName(b"charactersIgnoringModifiers\0".as_ptr() as *const _),
                );
                if !chars.is_null() {
                    let utf8 = get_cstr(
                        chars,
                        sel_registerName(b"UTF8String\0".as_ptr() as *const _),
                    );
                    if !utf8.is_null() {
                        let first = (*utf8 as u8).to_ascii_lowercase();
                        let second = *utf8.add(1);
                        if second == 0 {
                            let shift = (mods & SHIFT) != 0;
                            let action_name: Option<&[u8]> = match (first, shift) {
                                (b'v', false) => Some(b"paste:\0"),
                                (b'c', false) => Some(b"copy:\0"),
                                (b'x', false) => Some(b"cut:\0"),
                                (b'a', false) => Some(b"selectAll:\0"),
                                (b'z', false) => Some(b"undo:\0"),
                                (b'z', true) => Some(b"redo:\0"),
                                _ => None,
                            };
                            if let Some(name) = action_name {
                                let ns_app_cls =
                                    objc_getClass(b"NSApplication\0".as_ptr() as *const _);
                                let ns_app = get_ptr(
                                    ns_app_cls,
                                    sel_registerName(b"sharedApplication\0".as_ptr() as *const _),
                                );
                                let responder = get_ptr(
                                    this,
                                    sel_registerName(b"firstResponder\0".as_ptr() as *const _),
                                );
                                if !ns_app.is_null() && !responder.is_null() {
                                    let send_action: SendAction =
                                        std::mem::transmute(objc_msgSend as *const ());
                                    let handled = send_action(
                                        ns_app,
                                        sel_registerName(b"sendAction:to:from:\0".as_ptr() as *const _),
                                        sel_registerName(name.as_ptr() as *const _),
                                        responder,
                                        this,
                                    );
                                    if handled {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(&imp_addr) = NSPANEL_PERFORM_KEY_EQUIV_IMP.get() {
            if imp_addr != 0 {
                let super_fn: SuperPerformKeyEquiv = std::mem::transmute(imp_addr);
                return super_fn(this, sel, event);
            }
        }
        false
    }
}

/// Registers (once) a dynamic `NSPanel` subclass (`CrackingInterviewPanel`) matching `TaoWindow`'s
/// `focusable` ivar layout so we can safely convert Tauri's `NSWindow` into a non-activating panel.
fn get_or_register_nonactivating_panel_class() -> Option<*mut c_void> {
    let ptr = *PANEL_CLASS_PTR.get_or_init(|| unsafe {
        extern "C" {
            fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
            fn objc_allocateClassPair(
                superclass: *mut c_void,
                name: *const std::ffi::c_char,
                extra_bytes: usize,
            ) -> *mut c_void;
            fn objc_registerClassPair(cls: *mut c_void);
            fn class_getInstanceSize(cls: *mut c_void) -> usize;
            fn class_addIvar(
                cls: *mut c_void,
                name: *const std::ffi::c_char,
                size: usize,
                alignment: u8,
                types: *const std::ffi::c_char,
            ) -> bool;
            fn class_addMethod(
                cls: *mut c_void,
                name: *mut c_void,
                imp: *const c_void,
                types: *const std::ffi::c_char,
            ) -> bool;
            fn class_getMethodImplementation(cls: *mut c_void, name: *mut c_void) -> *const c_void;
            fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        }

        let existing = objc_getClass(b"CrackingInterviewPanel\0".as_ptr() as *const _);
        if !existing.is_null() {
            return existing as usize;
        }

        let ns_panel = objc_getClass(b"NSPanel\0".as_ptr() as *const _);
        let ns_window = objc_getClass(b"NSWindow\0".as_ptr() as *const _);
        if ns_panel.is_null() || ns_window.is_null() {
            return 0;
        }
        if class_getInstanceSize(ns_panel) != class_getInstanceSize(ns_window) {
            super::log_event(&format!(
                "Skipping non-activating NSPanel conversion: NSPanel size {} != NSWindow size {}",
                class_getInstanceSize(ns_panel),
                class_getInstanceSize(ns_window)
            ));
            return 0;
        }

        let perform_key_equiv_sel =
            sel_registerName(b"performKeyEquivalent:\0".as_ptr() as *const _);
        let super_imp = class_getMethodImplementation(ns_panel, perform_key_equiv_sel);
        let _ = NSPANEL_PERFORM_KEY_EQUIV_IMP.set(super_imp as usize);

        let cls = objc_allocateClassPair(
            ns_panel,
            b"CrackingInterviewPanel\0".as_ptr() as *const _,
            0,
        );
        if cls.is_null() {
            return 0;
        }

        // Preserve TaoWindow's `focusable` ivar so any tao call reading it remains valid.
        let _ = class_addIvar(
            cls,
            b"focusable\0".as_ptr() as *const _,
            1,
            0,
            b"B\0".as_ptr() as *const _,
        );
        let _ = class_addMethod(
            cls,
            sel_registerName(b"canBecomeMainWindow\0".as_ptr() as *const _),
            panel_can_become_main as *const c_void,
            b"B@:\0".as_ptr() as *const _,
        );
        let _ = class_addMethod(
            cls,
            sel_registerName(b"canBecomeKeyWindow\0".as_ptr() as *const _),
            panel_can_become_key as *const c_void,
            b"B@:\0".as_ptr() as *const _,
        );
        let _ = class_addMethod(
            cls,
            perform_key_equiv_sel,
            panel_perform_key_equivalent as *const c_void,
            b"B@:@\0".as_ptr() as *const _,
        );

        objc_registerClassPair(cls);
        cls as usize
    });

    if ptr == 0 {
        None
    } else {
        Some(ptr as *mut c_void)
    }
}

/// Converts the main `NSWindow` into a non-activating `NSPanel` (`NSWindowStyleMaskNonactivatingPanel`)
/// so clicks inside CrackingInterview work on the first click without activating `NSApplication` or
/// stealing focus from foreground apps (such as LockDown Browser). Must be called on the main thread.
pub fn apply_nonactivating_panel(win: &tauri::WebviewWindow) {
    extern "C" {
        fn object_getClass(obj: *mut c_void) -> *mut c_void;
        fn object_setClass(obj: *mut c_void, cls: *mut c_void) -> *mut c_void;
        fn class_getInstanceSize(cls: *mut c_void) -> usize;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    type GetU64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64;
    type SetU64 = unsafe extern "C" fn(*mut c_void, *mut c_void, u64);
    type GetI64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64;
    type SetI64 = unsafe extern "C" fn(*mut c_void, *mut c_void, i64);
    type SetBool = unsafe extern "C" fn(*mut c_void, *mut c_void, bool);

    let Some(panel_cls) = get_or_register_nonactivating_panel_class() else {
        println!("⚠️ [stealth-macOS] Could not register CrackingInterviewPanel class");
        return;
    };

    unsafe {
        let ns_window = match win.ns_window() {
            Ok(ptr) if !ptr.is_null() => ptr as *mut c_void,
            _ => {
                println!("⚠️ [stealth-macOS] Could not get NSWindow for non-activating panel");
                return;
            }
        };

        let orig_cls = object_getClass(ns_window);
        if orig_cls.is_null() {
            return;
        }
        let panel_size = class_getInstanceSize(panel_cls);
        let orig_size = class_getInstanceSize(orig_cls);
        if panel_size > orig_size {
            super::log_event(&format!(
                "Skipping non-activating NSPanel conversion: panel size {} > window size {}",
                panel_size, orig_size
            ));
            return;
        }

        let get_u64: GetU64 = std::mem::transmute(objc_msgSend as *const ());
        let set_u64: SetU64 = std::mem::transmute(objc_msgSend as *const ());
        let get_i64: GetI64 = std::mem::transmute(objc_msgSend as *const ());
        let set_i64: SetI64 = std::mem::transmute(objc_msgSend as *const ());
        let set_bool: SetBool = std::mem::transmute(objc_msgSend as *const ());

        let style_sel = sel_registerName(b"styleMask\0".as_ptr() as *const _);
        let set_style_sel = sel_registerName(b"setStyleMask:\0".as_ptr() as *const _);
        let level_sel = sel_registerName(b"level\0".as_ptr() as *const _);
        let set_level_sel = sel_registerName(b"setLevel:\0".as_ptr() as *const _);
        let set_hides_sel = sel_registerName(b"setHidesOnDeactivate:\0".as_ptr() as *const _);
        let set_modal_sel = sel_registerName(b"setWorksWhenModal:\0".as_ptr() as *const _);

        let orig_style = get_u64(ns_window, style_sel);
        let orig_level = get_i64(ns_window, level_sel);

        if orig_cls != panel_cls {
            object_setClass(ns_window, panel_cls);
        }

        let new_style = orig_style | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL;
        set_u64(ns_window, set_style_sel, new_style);
        set_bool(ns_window, set_hides_sel, false);
        set_bool(ns_window, set_modal_sel, true);

        // Preserve floating window level & fullscreen Space collectionBehavior in case setStyleMask reset them.
        if get_i64(ns_window, level_sel) < orig_level {
            set_i64(ns_window, set_level_sel, orig_level);
        }
        crate::apply_macos_fullscreen_overlay(win);

        NONACTIVATING_PANEL_ACTIVE.store(true, Ordering::SeqCst);
        println!(
            "🕵️ [stealth-macOS] ✅ Converted NSWindow to non-activating NSPanel (styleMask {:#x} → {:#x})",
            orig_style, new_style
        );
        super::log_event(&format!(
            "Converted macOS NSWindow to non-activating NSPanel (styleMask={:#x}, level={})",
            new_style, orig_level
        ));
    }
}

/// Grants or releases key-window status on the non-activating `NSPanel` without activating `NSApp`.
/// Must be called on the main thread after `KEYBOARD_INPUT_ACTIVE` has been updated.
pub fn set_panel_keyboard_focus(win: &tauri::WebviewWindow, enabled: bool) {
    if !NONACTIVATING_PANEL_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    extern "C" {
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    type SendNoArg = unsafe extern "C" fn(*mut c_void, *mut c_void);

    unsafe {
        let Ok(ptr) = win.ns_window() else {
            return;
        };
        if ptr.is_null() {
            return;
        }
        let send: SendNoArg = std::mem::transmute(objc_msgSend as *const ());
        if enabled {
            send(
                ptr as *mut c_void,
                sel_registerName(b"makeKeyWindow\0".as_ptr() as *const _),
            );
        } else {
            send(
                ptr as *mut c_void,
                sel_registerName(b"resignKeyWindow\0".as_ptr() as *const _),
            );
        }
    }
}

pub fn compute_kiosk_collection_behavior(current: u64) -> u64 {
    (current | CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY)
        & !MOVE_TO_ACTIVE_SPACE
        & !FULL_SCREEN_PRIMARY
        & !STATIONARY
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

/// Makes the `--guardian` process ignore SIGTERM. SEB-mac's session-start
/// sweep enumerates *all* processes and matches the guardian's PID to our
/// bundle ID, so an unprotected guardian can be terminated directly,
/// orphaning the GUI. Authorized quit is unaffected: the guardian already
/// polls the authorized-quit marker file whenever its child exits.
pub fn install_guardian_signal_guard() {
    unsafe {
        let mut act: libc::sigaction = std::mem::zeroed();
        act.sa_sigaction = libc::SIG_IGN;
        libc::sigemptyset(&mut act.sa_mask);
        let _ = libc::sigaction(libc::SIGTERM, &act, std::ptr::null_mut());
    }
}

/// Spawns a 300ms macOS self-healing thread that dispatches onto the main thread to verify:
/// 1. If `SIGTERM_INTERCEPTED` was set, logs the intercepted signal and checks if we should yield to `assessmentagent`.
/// 2. If `!USER_HIDDEN`, verifies `ns_window.isVisible`, `ns_window.level == 1001`, and `sharingType == 0` (when stealth is enabled).
///    A miniaturized (Dock-minimized) window also reports `isVisible == false`, so the healer
///    stands down while `isMiniaturized` instead of "healing" the user's minimize back on screen.
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
                if super::should_proactively_yield_to_seb(&app) {
                    super::log_event(
                        "Detected macOS SEB lockdown; yielding for post-lockdown recovery",
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
                let ns_window = ns_win_raw;

                extern "C" {
                    fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
                    fn objc_msgSend();
                }
                type GetBool = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
                type GetI64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i64;
                type GetU64 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> u64;

                unsafe {
                    let is_vis_sel = sel_registerName(b"isVisible\0".as_ptr() as *const _);
                    let is_mini_sel = sel_registerName(b"isMiniaturized\0".as_ptr() as *const _);
                    let level_sel = sel_registerName(b"level\0".as_ptr() as *const _);
                    let behavior_sel =
                        sel_registerName(b"collectionBehavior\0".as_ptr() as *const _);
                    let sharing_sel = sel_registerName(b"sharingType\0".as_ptr() as *const _);

                    let get_bool: GetBool = std::mem::transmute(objc_msgSend as *const ());
                    let get_i64: GetI64 = std::mem::transmute(objc_msgSend as *const ());
                    let get_u64: GetU64 = std::mem::transmute(objc_msgSend as *const ());

                    let visible = get_bool(ns_window, is_vis_sel);

                    // A Dock-minimized window reports isVisible == false, just
                    // like an externally hidden one. Minimizing is a legitimate
                    // user action (yellow title-bar button), not a hide attack:
                    // stand down until the window is deminiaturized, otherwise
                    // orderFrontRegardless bounces it straight back on screen.
                    let miniaturized = get_bool(ns_window, is_mini_sel);
                    if miniaturized {
                        if !super::USER_MINIMIZED.swap(true, Ordering::SeqCst) {
                            super::log_event(
                                "User minimized NSWindow (isMiniaturized); self-healer standing down (macOS)",
                            );
                        }
                        return;
                    }
                    if super::USER_MINIMIZED.swap(false, Ordering::SeqCst) {
                        super::log_event(
                            "NSWindow deminiaturized; self-healer resuming (macOS)",
                        );
                    }

                    if !visible {
                        crate::stealth_hotkey::bring_to_front_without_focus(&win);
                        super::log_event(
                            "Self-healed hidden NSWindow via orderFrontRegardless (macOS)",
                        );
                    }

                    let behavior = get_u64(ns_window, behavior_sel);
                    let want_behavior = CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY;
                    let level = get_i64(ns_window, level_sel);
                    if (behavior & want_behavior) != want_behavior
                        || (behavior & STATIONARY) != 0
                        || level < MIN_FLOATING_WINDOW_LEVEL
                    {
                        let _ = win.set_always_on_top(true);
                        crate::apply_macos_fullscreen_overlay(&win);
                        super::log_event(&format!(
                            "Self-healed NSWindow fullscreen overlay (behavior={:#x}, level={})",
                            behavior, level
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

                        if NONACTIVATING_PANEL_ACTIVE.load(Ordering::Relaxed) {
                            let style_sel =
                                sel_registerName(b"styleMask\0".as_ptr() as *const _);
                            let hides_sel =
                                sel_registerName(b"hidesOnDeactivate\0".as_ptr() as *const _);
                            let style = get_u64(ns_window, style_sel);
                            let hides = get_bool(ns_window, hides_sel);
                            if (style & NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL) == 0 || hides {
                                apply_nonactivating_panel(&win);
                                super::log_event(&format!(
                                    "Self-healed non-activating NSPanel (style={:#x}, hides={})",
                                    style, hides
                                ));
                            }
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
        let initial = MOVE_TO_ACTIVE_SPACE | FULL_SCREEN_PRIMARY | STATIONARY;
        let updated = compute_kiosk_collection_behavior(initial);
        assert_ne!(updated & CAN_JOIN_ALL_SPACES, 0);
        assert_ne!(updated & FULL_SCREEN_AUXILIARY, 0);
        assert_eq!(updated & STATIONARY, 0);
        assert_eq!(updated & MOVE_TO_ACTIVE_SPACE, 0);
        assert_eq!(updated & FULL_SCREEN_PRIMARY, 0);
        assert_eq!(MIN_FLOATING_WINDOW_LEVEL, 3);
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
    fn test_guardian_signal_guard_ignores_sigterm() {
        install_guardian_signal_guard();
        unsafe {
            // Query current disposition: SIGTERM must now be SIG_IGN (1).
            let mut old: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGTERM, std::ptr::null(), &mut old);
            assert_eq!(old.sa_sigaction, libc::SIG_IGN);
            // Restore default so the test harness is unaffected.
            let mut dfl: libc::sigaction = std::mem::zeroed();
            dfl.sa_sigaction = libc::SIG_DFL;
            libc::sigemptyset(&mut dfl.sa_mask);
            libc::sigaction(libc::SIGTERM, &dfl, std::ptr::null_mut());
        }
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

    #[test]
    fn test_nonactivating_panel_class_registration() {
        extern "C" {
            fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
            fn class_getInstanceSize(cls: *mut c_void) -> usize;
        }
        let cls = get_or_register_nonactivating_panel_class()
            .expect("CrackingInterviewPanel class should register on macOS");
        assert!(!cls.is_null());
        unsafe {
            let ns_window = objc_getClass(b"NSWindow\0".as_ptr() as *const _);
            let ns_panel = objc_getClass(b"NSPanel\0".as_ptr() as *const _);
            assert_eq!(class_getInstanceSize(ns_panel), class_getInstanceSize(ns_window));
            assert!(class_getInstanceSize(cls) >= class_getInstanceSize(ns_panel));
        }
        // canBecomeKeyWindow follows KEYBOARD_INPUT_ACTIVE; canBecomeMainWindow is always false.
        assert!(!panel_can_become_main(std::ptr::null_mut(), std::ptr::null_mut()));
        super::super::KEYBOARD_INPUT_ACTIVE.store(false, Ordering::SeqCst);
        assert!(!panel_can_become_key(std::ptr::null_mut(), std::ptr::null_mut()));
        super::super::KEYBOARD_INPUT_ACTIVE.store(true, Ordering::SeqCst);
        assert!(panel_can_become_key(std::ptr::null_mut(), std::ptr::null_mut()));
        super::super::KEYBOARD_INPUT_ACTIVE.store(false, Ordering::SeqCst);
    }
}
