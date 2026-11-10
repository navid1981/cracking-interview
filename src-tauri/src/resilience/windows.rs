//! Windows-specific window self-healing and resilience helpers (Scenarios A1a, A1b).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Manager;

/// Spawns a windowless background thread inside `--guardian` that polls `OpenInputDesktop`
/// every 300ms (Scenario A3). When a kiosk app switches to a new Win32 `HDESK`, spawns a
/// `--desktop-worker <deskName>` instance directly onto `WinSta0\<deskName>` with its own
/// WebView2 profile folder, or stands by cleanly if the desktop DACL denies access.
pub fn start_desktop_switch_monitor(config_dir: PathBuf) {
    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        use super::{DesktopSwitchAction, DesktopSwitchState};
        let mut state = DesktopSwitchState::default();
        let mut worker_pid: Option<u32> = None;

        loop {
            std::thread::sleep(Duration::from_millis(300));
            if super::quit_was_authorized_in_dir(&config_dir, super::now_unix_secs()) {
                if let Some(pid) = worker_pid.take() {
                    terminate_pid_win32(pid);
                }
                break;
            }
            if !super::ResilienceConfig::load_from_dir(&config_dir).enabled {
                continue;
            }

            let active_desk = query_active_input_desktop_name();
            let alive = worker_pid.map(is_pid_alive_win32).unwrap_or(false);
            if !alive {
                worker_pid = None;
            }

            match state.on_poll(active_desk.as_deref(), alive) {
                DesktopSwitchAction::NoOp => {}
                DesktopSwitchAction::SpawnWorker { desktop_name } => {
                    super::log_event_in_dir(
                        &config_dir,
                        &format!(
                            "Detected Win32 desktop switch -> '{}'; spawning shadow worker",
                            desktop_name
                        ),
                    );
                    match spawn_worker_on_desktop(&desktop_name, &config_dir) {
                        Ok(pid) => {
                            worker_pid = Some(pid);
                            super::log_event_in_dir(
                                &config_dir,
                                &format!(
                                    "Spawned desktop worker (PID={}) on WinSta0\\{}",
                                    pid, desktop_name
                                ),
                            );
                        }
                        Err(e) => {
                            state.blocked_desktop = Some(desktop_name.clone());
                            super::log_event_in_dir(
                                &config_dir,
                                &format!(
                                    "Could not spawn worker on '{}': {}; standing by for Default",
                                    desktop_name, e
                                ),
                            );
                        }
                    }
                }
                DesktopSwitchAction::StandByForDefault { reason } => {
                    super::log_event_in_dir(&config_dir, &reason);
                }
                DesktopSwitchAction::ReturnToDefault { terminate_worker } => {
                    if terminate_worker {
                        if let Some(pid) = worker_pid.take() {
                            terminate_pid_win32(pid);
                        }
                    }
                    super::log_event_in_dir(
                        &config_dir,
                        "Active input desktop returned to Default; shadow worker cleaned up",
                    );
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
mod win_desktop_ffi {
    pub type HANDLE = isize;
    pub type HDESK = isize;
    pub type BOOL = i32;
    pub type DWORD = u32;

    pub const DF_ALLOWOTHERACCOUNTHOOK: DWORD = 0x0001;
    pub const DESKTOP_READOBJECTS: DWORD = 0x0001;
    pub const DESKTOP_CREATEWINDOW: DWORD = 0x0002;
    pub const DESKTOP_SWITCHDESKTOP: DWORD = 0x0100;
    pub const UOI_NAME: i32 = 2;
    pub const PROCESS_TERMINATE: DWORD = 0x0001;
    pub const PROCESS_SYNCHRONIZE: DWORD = 0x00100000;
    pub const WAIT_OBJECT_0: DWORD = 0;

    #[repr(C)]
    pub struct STARTUPINFOW {
        pub cb: DWORD,
        pub lpReserved: *mut u16,
        pub lpDesktop: *mut u16,
        pub lpTitle: *mut u16,
        pub dwX: DWORD,
        pub dwY: DWORD,
        pub dwXSize: DWORD,
        pub dwYSize: DWORD,
        pub dwXCountChars: DWORD,
        pub dwYCountChars: DWORD,
        pub dwFillAttribute: DWORD,
        pub dwFlags: DWORD,
        pub wShowWindow: u16,
        pub cbReserved2: u16,
        pub lpReserved2: *mut u8,
        pub hStdInput: HANDLE,
        pub hStdOutput: HANDLE,
        pub hStdError: HANDLE,
    }

    #[repr(C)]
    pub struct PROCESS_INFORMATION {
        pub hProcess: HANDLE,
        pub hThread: HANDLE,
        pub dwProcessId: DWORD,
        pub dwThreadId: DWORD,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn OpenInputDesktop(dwFlags: DWORD, fInherit: BOOL, dwDesiredAccess: DWORD) -> HDESK;
        pub fn GetUserObjectInformationW(
            hObj: HANDLE,
            nIndex: i32,
            pvInfo: *mut u8,
            nLength: DWORD,
            lpnLengthNeeded: *mut DWORD,
        ) -> BOOL;
        pub fn CloseDesktop(hDesktop: HDESK) -> BOOL;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateProcessW(
            lpApplicationName: *const u16,
            lpCommandLine: *mut u16,
            lpProcessAttributes: *const u8,
            lpThreadAttributes: *const u8,
            bInheritHandles: BOOL,
            dwCreationFlags: DWORD,
            lpEnvironment: *const u8,
            lpCurrentDirectory: *const u16,
            lpStartupInfo: *const STARTUPINFOW,
            lpProcessInformation: *mut PROCESS_INFORMATION,
        ) -> BOOL;
        pub fn OpenProcess(
            dwDesiredAccess: DWORD,
            bInheritHandle: BOOL,
            dwProcessId: DWORD,
        ) -> HANDLE;
        pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
        pub fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
        pub fn CloseHandle(hObject: HANDLE) -> BOOL;
        pub fn GetLastError() -> DWORD;
    }
}

#[cfg(target_os = "windows")]
fn query_active_input_desktop_name() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use win_desktop_ffi::*;

    unsafe {
        let hdesk = OpenInputDesktop(
            DF_ALLOWOTHERACCOUNTHOOK,
            0,
            DESKTOP_READOBJECTS | DESKTOP_CREATEWINDOW | DESKTOP_SWITCHDESKTOP,
        );
        if hdesk == 0 {
            return None;
        }
        let mut buf = [0u16; 256];
        let mut needed: DWORD = 0;
        let ok = GetUserObjectInformationW(
            hdesk,
            UOI_NAME,
            buf.as_mut_ptr() as *mut u8,
            (buf.len() * 2) as DWORD,
            &mut needed,
        );
        CloseDesktop(hdesk);
        if ok == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(OsString::from_wide(&buf[..len]).to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "windows")]
fn spawn_worker_on_desktop(desktop_name: &str, config_dir: & std::path::Path) -> Result<u32, String> {
    use win_desktop_ffi::*;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let wv2_dir = config_dir.join("WebView2_DeskWorker");
    let _ = std::fs::create_dir_all(&wv2_dir);

    let mut cmd_line: Vec<u16> = format!(
        "\"{}\" --gui --restore-session --desktop-worker \"{}\"\0",
        exe.display(),
        desktop_name
    )
    .encode_utf16()
    .collect();
    let mut desk_wide: Vec<u16> = format!("WinSta0\\{}\0", desktop_name)
        .encode_utf16()
        .collect();

    unsafe {
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as DWORD;
        si.lpDesktop = desk_wide.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessW(
            std::ptr::null(),
            cmd_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0,
            std::ptr::null(),
            std::ptr::null(),
            &si,
            &mut pi,
        );
        if ok == 0 {
            return Err(format!("CreateProcessW failed with err={}", GetLastError()));
        }
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
        Ok(pi.dwProcessId)
    }
}

#[cfg(target_os = "windows")]
fn is_pid_alive_win32(pid: u32) -> bool {
    use win_desktop_ffi::*;
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if h == 0 {
            return false;
        }
        let wait = WaitForSingleObject(h, 0);
        CloseHandle(h);
        wait != WAIT_OBJECT_0
    }
}

#[cfg(target_os = "windows")]
fn terminate_pid_win32(pid: u32) {
    use win_desktop_ffi::*;
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h != 0 {
            let _ = TerminateProcess(h, 0);
            CloseHandle(h);
        }
    }
}

/// Opens an `http(s)` URL on Windows without relying on `explorer.exe` or `cmd.exe /C start`.
/// Resolves the browser binary from Registry or known Chrome/Edge/Brave install paths and spawns directly.
pub fn open_url_shell_independent(url: &str) -> Result<(), String> {
    let validated = super::validate_external_http_url(url)?;

    if let Some(browser_exe) = resolve_windows_browser_exe() {
        if std::process::Command::new(&browser_exe)
            .arg(&validated)
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }

    Err("Could not locate a browser executable (Chrome/Edge) to open URL while Explorer shell is unavailable".to_string())
}

fn resolve_windows_browser_exe() -> Option<PathBuf> {
    // 1. Try resolving UserChoice ProgId -> shell\open\command via reg.exe (works without explorer.exe)
    if let Ok(out) = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
            "/v",
            "ProgId",
        ])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(prog_id) = text
                .lines()
                .find(|l| l.contains("ProgId"))
                .and_then(|l| l.split_whitespace().last())
            {
                let key = format!(r"HKCR\{}\shell\open\command", prog_id);
                if let Ok(cmd_out) = std::process::Command::new("reg")
                    .args(["query", &key, "/ve"])
                    .output()
                {
                    if cmd_out.status.success() {
                        let cmd_text = String::from_utf8_lossy(&cmd_out.stdout);
                        for line in cmd_text.lines() {
                            if let Some(idx) = line.find("REG_SZ") {
                                let template = &line[idx + "REG_SZ".len()..];
                                if let Some(exe) =
                                    super::extract_exe_from_command_template(template)
                                {
                                    let pb = PathBuf::from(exe);
                                    if pb.is_file() {
                                        return Some(pb);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback: Check standard Chrome / Edge / Brave paths directly (zero shell dependency)
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
        PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
        PathBuf::from(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe"),
    ];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.insert(
            0,
            PathBuf::from(&local).join(r"Google\Chrome\Application\chrome.exe"),
        );
        candidates.push(PathBuf::from(&local).join(r"Microsoft\Edge\Application\msedge.exe"));
        candidates.push(
            PathBuf::from(&local).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"),
        );
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Optional experimental process-DACL hardening (Task 9, off by default unless
/// `resilience.json` sets `"harden_process_dacl": true`).
/// Prepends an `ACCESS_DENIED_ACE` for `PROCESS_TERMINATE | PROCESS_SUSPEND_RESUME`
/// for `CURRENT_USER` on `GetCurrentProcess()` so non-elevated same-user monitors
/// receive `ERROR_ACCESS_DENIED` on `OpenProcess(PROCESS_TERMINATE)`.
pub fn apply_experimental_process_dacl_hardening() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::Foundation::ERROR_SUCCESS;
        use windows::Win32::Security::Authorization::*;
        use windows::Win32::Security::*;
        use windows::Win32::System::Threading::GetCurrentProcess;

        const PROCESS_TERMINATE_MASK: u32 = 0x0001;
        const PROCESS_SUSPEND_RESUME_MASK: u32 = 0x0800;

        let hproc = GetCurrentProcess();
        let mut orig_dacl: *mut ACL = std::ptr::null_mut();
        let mut sd = PSECURITY_DESCRIPTOR::default();

        // windows 0.58: GetSecurityInfo takes Option<*mut T> raw out-pointers
        // (not &mut T) and returns WIN32_ERROR (not Result).
        if GetSecurityInfo(
            hproc,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(std::ptr::addr_of_mut!(orig_dacl)),
            None,
            Some(std::ptr::addr_of_mut!(sd)),
        ) != ERROR_SUCCESS
        {
            return;
        }

        let current_user_wide: Vec<u16> = "CURRENT_USER\0".encode_utf16().collect();
        let mut ea = EXPLICIT_ACCESS_W::default();
        ea.grfAccessPermissions = PROCESS_TERMINATE_MASK | PROCESS_SUSPEND_RESUME_MASK;
        ea.grfAccessMode = DENY_ACCESS;
        ea.grfInheritance = NO_INHERITANCE;
        ea.Trustee.TrusteeForm = TRUSTEE_IS_NAME;
        ea.Trustee.TrusteeType = TRUSTEE_IS_USER;
        ea.Trustee.ptstrName = windows::core::PWSTR(current_user_wide.as_ptr() as *mut u16);

        let mut new_dacl: *mut ACL = std::ptr::null_mut();
        // windows 0.58: SetEntriesInAclW takes Option<*const ACL> for the old
        // DACL and a raw *mut *mut ACL out-pointer; returns WIN32_ERROR.
        if SetEntriesInAclW(
            Some(&[ea]),
            Some(orig_dacl as *const ACL),
            std::ptr::addr_of_mut!(new_dacl),
        ) == ERROR_SUCCESS
        {
            let _ = SetSecurityInfo(
                hproc,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                PSID::default(),
                PSID::default(),
                Some(new_dacl),
                None,
            );
            super::log_event("Applied experimental process-DACL hardening (DENY PROCESS_TERMINATE | PROCESS_SUSPEND_RESUME)");
        }
    }
}

static ORIGINAL_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static PROTECT_SUBCLASS_INSTALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Window procedure installed over the Tauri/wry proc to stop external hides
/// at the source instead of re-showing after the fact. Kiosk monitors such as
/// SEB call `ShowWindow(SW_HIDE)` every few seconds; re-showing in response
/// produced a visible close/open flicker (~2-3s) and left WebView2 painting
/// white. Swallowing the hide keeps the window (and its painted surface)
/// untouched. The user's own hide (`USER_HIDDEN`) and authorized quits are
/// always allowed through to the original proc.
#[cfg(target_os = "windows")]
unsafe extern "system" fn protected_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::*;

    // [window-diag] Log every close-path message ON ARRIVAL, before any guard
    // runs. A guardian-respawned GUI once died from an external WM_CLOSE with
    // no block/authorization log at all; arrival logging distinguishes
    // "message never reached this proc" (helper-window / proc replaced) from
    // "guard let it through" (values below) from "died inside the handler"
    // (arrival logged, nothing after). Close attempts are rare, so the
    // filesystem-backed reads are acceptable here.
    if msg == WM_CLOSE
        || msg == WM_DESTROY
        || (msg == WM_SYSCOMMAND && (wparam.0 & 0xFFF0) == SC_CLOSE as usize)
    {
        super::log_event(&format!(
            "[window-diag] close-path arrival msg={:#06x} enabled={} quit_authorized={} user_hidden={}",
            msg,
            super::enabled(),
            super::quit_was_authorized(),
            super::USER_HIDDEN.load(Ordering::Relaxed)
        ));
    }

    // Only the hide/close messages need the (filesystem-backed) authorization
    // and config checks; every other message goes straight to the original
    // proc so the UI thread is not slowed down.
    let block_hide = matches!(
        msg,
        WM_SHOWWINDOW | WM_WINDOWPOSCHANGING | WM_CLOSE | WM_SYSCOMMAND
    ) && super::should_block_external_hide(
        super::USER_HIDDEN.load(Ordering::Relaxed),
        super::quit_was_authorized(),
        super::enabled(),
    );

    match msg {
        // [window-diag] Activation changes are rare and click-correlated;
        // log them (never block) to timestamp focus events against white-out.
        WM_ACTIVATE => {
            let state = wparam.0 & 0xFFFF;
            super::log_event(&format!(
                "[window-diag] WM_ACTIVATE state={} (0=inactive,1=active,2=click-active)",
                state
            ));
        }
        // External ShowWindow(SW_HIDE): wParam == FALSE.
        WM_SHOWWINDOW if wparam.0 == 0 && block_hide => {
            super::log_event("Blocked external hide (WM_SHOWWINDOW) via window-proc subclass");
            return LRESULT(0);
        }
        // External SetWindowPos(SWP_HIDEWINDOW): strip just the hide bit.
        WM_WINDOWPOSCHANGING if block_hide => {
            let pos = lparam.0 as *mut WINDOWPOS;
            if !pos.is_null() {
                let stripped = super::strip_hide_window_flag((*pos).flags.0, true);
                if stripped != (*pos).flags.0 {
                    super::log_event(
                        "Blocked external hide (SWP_HIDEWINDOW) via window-proc subclass",
                    );
                    (*pos).flags = SET_WINDOW_POS_FLAGS(stripped);
                }
            }
        }
        // Defense in depth: Tauri's CloseRequested handler also blocks this.
        WM_CLOSE if super::enabled() && !super::quit_was_authorized() => {
            super::log_event("Blocked external WM_CLOSE via window-proc subclass");
            return LRESULT(0);
        }
        // The user clicking the title-bar X (mouse coords in lParam) is an
        // authorized quit: persist it so the guardian exits cleanly, then
        // fall through to the original proc to close normally. Programmatic
        // SC_CLOSE (lParam = 0, e.g. SEB's close escalation) stays blocked
        // by the next arm.
        WM_SYSCOMMAND
            if super::is_user_mouse_close_command(wparam.0, lparam.0)
                && super::enabled()
                && !super::quit_was_authorized() =>
        {
            super::authorize_quit_without_app_handle();
            super::log_event("User clicked title-bar X (mouse SC_CLOSE); authorizing quit");
        }
        WM_SYSCOMMAND
            if (wparam.0 & 0xFFF0) == SC_CLOSE as usize
                && super::enabled()
                && !super::quit_was_authorized() =>
        {
            super::log_event("Blocked external SC_CLOSE via window-proc subclass");
            return LRESULT(0);
        }
        _ => {}
    }

    let orig = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
    if orig == 0 {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    } else {
        let prev: WNDPROC = std::mem::transmute(orig);
        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    }
}

/// Installs [`protected_window_proc`] on the main window (once per process).
/// Runs on both the normal GUI and `--desktop-worker` instances because both
/// go through the same Tauri setup that starts the self-healer.
#[cfg(target_os = "windows")]
pub fn install_window_protect_subclass(app: &tauri::AppHandle) {
    if PROTECT_SUBCLASS_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        PROTECT_SUBCLASS_INSTALLED.store(false, Ordering::SeqCst);
        return;
    };
    let Ok(hwnd_raw) = win.hwnd() else {
        PROTECT_SUBCLASS_INSTALLED.store(false, Ordering::SeqCst);
        return;
    };
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::*;

        let hwnd = HWND(hwnd_raw.0 as *mut _);
        if !IsWindow(hwnd).as_bool() {
            PROTECT_SUBCLASS_INSTALLED.store(false, Ordering::SeqCst);
            return;
        }
        let prev = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            protected_window_proc as *const () as isize,
        );
        if prev == 0 {
            PROTECT_SUBCLASS_INSTALLED.store(false, Ordering::SeqCst);
            super::log_event("Failed to install window-proc hide-protection subclass");
        } else {
            ORIGINAL_WNDPROC.store(prev, Ordering::SeqCst);
            super::log_event("Installed window-proc hide-protection subclass");
        }
    }
}

/// [window-diag] Snapshot of the main window + its child HWND tree
/// (WebView2 renders into children such as `Chrome_WidgetWin_0`), returned
/// as `(full, signature)`. The self-healer logs the full snapshot only when
/// the *signature* changes — visibility, affinity, foreground, and child
/// visibility — so dragging/resizing the window doesn't spam the log, while
/// a white window can still be attributed to a hidden child or lost focus.
#[cfg(target_os = "windows")]
unsafe fn build_window_diag_snapshot(
    hwnd: windows::Win32::Foundation::HWND,
) -> (String, String) {
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe extern "system" fn enum_child(
        child: windows::Win32::Foundation::HWND,
        lparam: LPARAM,
    ) -> windows::Win32::Foundation::BOOL {
        let lists = &mut *(lparam.0 as *mut (Vec<String>, Vec<String>));
        let mut buf = [0u16; 128];
        let len = GetClassNameW(child, &mut buf);
        let class = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
        let visible = IsWindowVisible(child).as_bool();
        let mut rect: RECT = std::mem::zeroed();
        let _ = GetWindowRect(child, &mut rect);
        lists.0.push(format!(
            "{}:vis={}:{}x{}@{},{}",
            class,
            visible,
            rect.right - rect.left,
            rect.bottom - rect.top,
            rect.left,
            rect.top
        ));
        lists.1.push(format!("{}:vis={}", class, visible));
        windows::Win32::Foundation::BOOL(1)
    }

    let mut children: (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let _ = EnumChildWindows(
        hwnd,
        Some(enum_child),
        LPARAM(&mut children as *mut (Vec<String>, Vec<String>) as isize),
    );
    let mut rect: RECT = std::mem::zeroed();
    let _ = GetWindowRect(hwnd, &mut rect);
    let mut affinity: u32 = 0;
    let _ = GetWindowDisplayAffinity(hwnd, &mut affinity);
    let main_visible = IsWindowVisible(hwnd).as_bool();
    let foreground = GetForegroundWindow() == hwnd;
    let full = format!(
        "main vis={} rect={}x{}@{},{} affinity=0x{:x} foreground={} children=[{}]",
        main_visible,
        rect.right - rect.left,
        rect.bottom - rect.top,
        rect.left,
        rect.top,
        affinity,
        foreground,
        children.0.join("; ")
    );
    let signature = format!(
        "main vis={} affinity=0x{:x} foreground={} children=[{}]",
        main_visible,
        affinity,
        foreground,
        children.1.join("; ")
    );
    (full, signature)
}

/// Spawns a 250ms background thread on Windows that inspects the main window's
/// visibility, topmost z-order, extended styles (`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`),
/// and screen-capture exclusion (`WDA_EXCLUDEFROMCAPTURE = 0x11`).
///
/// Never heals if `resilience::USER_HIDDEN` is true or `resilience::quit_was_authorized()`.
pub fn start_window_self_healer(app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    install_window_protect_subclass(&app);
    if let Some(dir) = super::get_config_dir() {
        let cfg = super::ResilienceConfig::load_from_dir(&dir);
        if cfg.should_apply_process_dacl() {
            apply_experimental_process_dacl_hardening();
        }
    }
    std::thread::spawn(move || {
        let mut heartbeat_lost_logged = false;
        let mut last_window_diag_signature = String::new();
        loop {
            std::thread::sleep(Duration::from_millis(250));

            if super::quit_was_authorized() {
                // Log before exiting: this is a legitimate silent-exit path
                // (authorized quit observed cross-process) and must be
                // distinguishable from an unexplained GUI death in the log.
                super::log_event("Self-healer observed authorized quit; exiting GUI");
                crate::stealth_hotkey::teardown_stealth_hotkeys();
                app.exit(0);
                break;
            }

            // [webview-diag] Detect frontend death (WebView renderer killed /
            // JS crashed) within ~8s and log it once per episode, so a white
            // window is timestamped against external events in the same log.
            {
                let last = super::FRONTEND_HEARTBEAT_MS.load(Ordering::Relaxed);
                let stale = super::frontend_heartbeat_is_stale(last, super::now_unix_ms(), 8_000);
                if stale && !super::USER_HIDDEN.load(Ordering::Relaxed) {
                    if !heartbeat_lost_logged {
                        heartbeat_lost_logged = true;
                        super::log_event(&format!(
                            "[webview-diag] frontend heartbeat lost: renderer/JS presumed dead (last beat {}ms ago)",
                            super::now_unix_ms().saturating_sub(last)
                        ));
                    }
                } else if !stale {
                    heartbeat_lost_logged = false;
                }
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

                // 4. [window-diag] Log the HWND tree whenever it changes, so
                // a white window shows whether WebView2's child windows were
                // hidden, zero-sized, or lost with the main window intact.
                #[cfg(target_os = "windows")]
                {
                    let (snapshot, signature) = build_window_diag_snapshot(hwnd);
                    if signature != last_window_diag_signature {
                        last_window_diag_signature = signature;
                        super::log_event(&format!("[window-diag] {}", snapshot));
                    }
                }

                // 5. Heal WebView2 render-surface children. SEB's blocked
                // ShowWindow(SW_HIDE) on the main window still makes Chromium
                // hide Chrome_RenderWidgetHostHWND / Chrome_WidgetWin_1 /
                // Intermediate D3D Window, and nothing ever shows them again
                // (main window stayed visible) -> permanent white window with
                // a live DOM. Re-show them while the main window is visible
                // and not minimized.
                #[cfg(target_os = "windows")]
                if !IsIconic(hwnd).as_bool() {
                    unsafe extern "system" fn enum_hidden_render_child(
                        child: HWND,
                        lparam: windows::Win32::Foundation::LPARAM,
                    ) -> windows::Win32::Foundation::BOOL {
                        let list = &mut *(lparam.0 as *mut Vec<(HWND, String)>);
                        if !IsWindowVisible(child).as_bool() {
                            let mut buf = [0u16; 128];
                            let len = GetClassNameW(child, &mut buf);
                            let class =
                                String::from_utf16_lossy(&buf[..len.max(0) as usize]).to_string();
                            if super::is_webview_render_surface_class(&class) {
                                list.push((child, class));
                            }
                        }
                        windows::Win32::Foundation::BOOL(1)
                    }

                    let mut hidden: Vec<(HWND, String)> = Vec::new();
                    let _ = EnumChildWindows(
                        hwnd,
                        Some(enum_hidden_render_child),
                        windows::Win32::Foundation::LPARAM(
                            &mut hidden as *mut Vec<(HWND, String)> as isize,
                        ),
                    );
                    for (child, class) in hidden {
                        let _ = ShowWindow(child, SW_SHOWNOACTIVATE);
                        super::log_event(&format!(
                            "Self-healed hidden WebView render child HWND ({}) via SW_SHOWNOACTIVATE",
                            class
                        ));
                    }
                }
            }
        }
    });
}
