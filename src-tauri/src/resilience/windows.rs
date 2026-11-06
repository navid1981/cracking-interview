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
        use windows::core::PCWSTR;
        use windows::Win32::Security::Authorization::*;
        use windows::Win32::Security::*;
        use windows::Win32::System::Threading::GetCurrentProcess;

        const PROCESS_TERMINATE_MASK: u32 = 0x0001;
        const PROCESS_SUSPEND_RESUME_MASK: u32 = 0x0800;

        let hproc = GetCurrentProcess();
        let mut orig_dacl: *mut ACL = std::ptr::null_mut();
        let mut sd = PSECURITY_DESCRIPTOR::default();

        if GetSecurityInfo(
            hproc,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut orig_dacl),
            None,
            &mut sd,
        )
        .is_err()
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
        if SetEntriesInAclW(Some(&[ea]), Some(orig_dacl), &mut new_dacl).is_ok() {
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

/// Spawns a 250ms background thread on Windows that inspects the main window's
/// visibility, topmost z-order, extended styles (`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`),
/// and screen-capture exclusion (`WDA_EXCLUDEFROMCAPTURE = 0x11`).
///
/// Never heals if `resilience::USER_HIDDEN` is true or `resilience::quit_was_authorized()`.
pub fn start_window_self_healer(app: tauri::AppHandle) {
    if let Some(dir) = super::get_config_dir() {
        let cfg = super::ResilienceConfig::load_from_dir(&dir);
        if cfg.should_apply_process_dacl() {
            apply_experimental_process_dacl_hardening();
        }
    }
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));

            if super::quit_was_authorized() {
                crate::stealth_hotkey::teardown_stealth_hotkeys();
                app.exit(0);
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
