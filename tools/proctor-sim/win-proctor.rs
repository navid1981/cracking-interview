//! Windows Proctor & Kiosk Attack Simulator for CrackingInterview
//!
//! Can be compiled standalone with `rustc win-proctor.rs` (uses raw Win32 FFI so zero external crates required)
//! or run via `win-proctor.exe <close|hide|kill|explorer|desktop>`.

#[cfg(target_os = "windows")]
mod win {
    use std::env;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::process::Command;
    use std::ptr;
    use std::thread;
    use std::time::Duration;

    type HWND = isize;
    type HANDLE = isize;
    type HDESK = isize;
    type BOOL = i32;
    type DWORD = u32;
    type UINT = u32;
    type WPARAM = usize;
    type LPARAM = isize;
    type LONG_PTR = isize;

    const WM_CLOSE: UINT = 0x0010;
    const SW_HIDE: i32 = 0;
    const GWL_EXSTYLE: i32 = -20;
    const WS_EX_TOPMOST: LONG_PTR = 0x00000008;
    const HWND_NOTOPMOST: HWND = -2;
    const SWP_NOMOVE: UINT = 0x0002;
    const SWP_NOSIZE: UINT = 0x0001;
    const SWP_NOACTIVATE: UINT = 0x0010;

    const PROCESS_TERMINATE: DWORD = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
    const GENERIC_ALL: DWORD = 0x10000000;
    const DF_ALLOWOTHERACCOUNTHOOK: DWORD = 0x0001;

    type WNDENUMPROC = unsafe extern "system" fn(HWND, LPARAM) -> BOOL;

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(lpEnumFunc: WNDENUMPROC, lParam: LPARAM) -> BOOL;
        fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut DWORD) -> DWORD;
        fn IsWindowVisible(hWnd: HWND) -> BOOL;
        fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
        fn GetClassNameW(hWnd: HWND, lpClassName: *mut u16, nMaxCount: i32) -> i32;
        fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
        fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> LONG_PTR;
        fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: LONG_PTR) -> LONG_PTR;
        fn SetWindowPos(
            hWnd: HWND,
            hWndInsertAfter: HWND,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: UINT,
        ) -> BOOL;
        fn CreateDesktopW(
            lpszDesktop: *const u16,
            lpszDevice: *const u16,
            pDevmode: *const u8,
            dwFlags: DWORD,
            dwDesiredAccess: DWORD,
            lpsa: *const u8,
        ) -> HDESK;
        fn OpenInputDesktop(dwFlags: DWORD, fInherit: BOOL, dwDesiredAccess: DWORD) -> HDESK;
        fn GetThreadDesktop(dwThreadId: DWORD) -> HDESK;
        fn SwitchDesktop(hDesktop: HDESK) -> BOOL;
        fn CloseDesktop(hDesktop: HDESK) -> BOOL;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
        fn TerminateProcess(hProcess: HANDLE, uExitCode: UINT) -> BOOL;
        fn QueryFullProcessImageNameW(
            hProcess: HANDLE,
            dwFlags: DWORD,
            lpExeName: *mut u16,
            lpdwSize: *mut DWORD,
        ) -> BOOL;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
        fn GetCurrentThreadId() -> DWORD;
        fn GetLastError() -> DWORD;
    }

    struct SearchCtx {
        targets: Vec<(HWND, DWORD)>,
    }

    unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam as *mut SearchCtx);
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return 1;
        }
        let hproc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if hproc != 0 {
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as DWORD;
            if QueryFullProcessImageNameW(hproc, 0, buf.as_mut_ptr(), &mut size) != 0 {
                let path = OsString::from_wide(&buf[..size as usize])
                    .to_string_lossy()
                    .to_lowercase();
                if path.contains("crackinginterview") || path.contains("cracking-interview") {
                    ctx.targets.push((hwnd, pid));
                }
            }
            CloseHandle(hproc);
        }
        1
    }

    fn window_class(hwnd: HWND) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            return "<unknown>".to_string();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }

    fn find_targets() -> Vec<(HWND, DWORD)> {
        let mut ctx = SearchCtx {
            targets: Vec::new(),
        };
        unsafe {
            EnumWindows(enum_windows_cb, &mut ctx as *mut SearchCtx as LPARAM);
        }
        ctx.targets
    }

    pub fn run() {
        let cmd = env::args().nth(1).unwrap_or_else(|| "help".to_string());
        match cmd.as_str() {
            "close" => {
                let targets = find_targets();
                println!("[win-proctor] Found {} window(s) for CrackingInterview", targets.len());
                for (hwnd, pid) in &targets {
                    unsafe {
                        PostMessageW(*hwnd, WM_CLOSE, 0, 0);
                    }
                    println!(
                        "[win-proctor] Sent WM_CLOSE to HWND={:#x} (PID={}) class='{}' visible={}",
                        hwnd,
                        pid,
                        window_class(*hwnd),
                        unsafe { IsWindowVisible(*hwnd) != 0 }
                    );
                }
                thread::sleep(Duration::from_secs(2));
                let after = find_targets();
                println!(
                    "[win-proctor] 2s after WM_CLOSE: {} window(s) still present",
                    after.len()
                );
            }
            "hide" => {
                let targets = find_targets();
                println!("[win-proctor] Found {} window(s) for CrackingInterview", targets.len());
                for (hwnd, pid) in &targets {
                    unsafe {
                        if IsWindowVisible(*hwnd) != 0 {
                            ShowWindow(*hwnd, SW_HIDE);
                            let ex = GetWindowLongPtrW(*hwnd, GWL_EXSTYLE);
                            SetWindowLongPtrW(*hwnd, GWL_EXSTYLE, ex & !WS_EX_TOPMOST);
                            SetWindowPos(
                                *hwnd,
                                HWND_NOTOPMOST,
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                            println!(
                                "[win-proctor] Hid and stripped topmost from HWND={:#x} (PID={})",
                                hwnd, pid
                            );
                        }
                    }
                }
                thread::sleep(Duration::from_secs(2));
                let visible_count = find_targets()
                    .into_iter()
                    .filter(|(h, _)| unsafe { IsWindowVisible(*h) != 0 })
                    .count();
                println!(
                    "[win-proctor] 2s after SW_HIDE: {} visible window(s)",
                    visible_count
                );
            }
            "kill" => {
                let targets = find_targets();
                let mut pids: Vec<DWORD> = targets.into_iter().map(|(_, p)| p).collect();
                pids.sort_unstable();
                pids.dedup();
                for pid in pids {
                    unsafe {
                        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
                        if h == 0 {
                            println!(
                                "[win-proctor] OpenProcess(PROCESS_TERMINATE) DENIED for PID={} (err={})",
                                pid,
                                GetLastError()
                            );
                        } else {
                            let ok = TerminateProcess(h, 1);
                            println!(
                                "[win-proctor] TerminateProcess(PID={}) -> result={}",
                                pid, ok
                            );
                            CloseHandle(h);
                        }
                    }
                }
                thread::sleep(Duration::from_secs(3));
                let after = find_targets();
                println!(
                    "[win-proctor] 3s after TerminateProcess: {} window(s) present",
                    after.len()
                );
            }
            "explorer" => {
                println!("[win-proctor] Running taskkill /F /T /IM explorer.exe ...");
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/IM", "explorer.exe"])
                    .status();
                thread::sleep(Duration::from_secs(3));
                let after = find_targets();
                println!(
                    "[win-proctor] 3s after Explorer tree-kill: {} CrackingInterview window(s) alive",
                    after.len()
                );
                println!("[win-proctor] Restarting explorer.exe ...");
                let _ = Command::new("explorer.exe").spawn();
            }
            "desktop" => unsafe {
                let orig_desk = GetThreadDesktop(GetCurrentThreadId());
                let name: Vec<u16> = "ProctorSimDesk\0".encode_utf16().collect();
                let new_desk = CreateDesktopW(
                    name.as_ptr(),
                    ptr::null(),
                    ptr::null(),
                    DF_ALLOWOTHERACCOUNTHOOK,
                    GENERIC_ALL,
                    ptr::null(),
                );
                if new_desk == 0 {
                    println!(
                        "[win-proctor] CreateDesktopW failed: err={}",
                        GetLastError()
                    );
                    return;
                }
                println!("[win-proctor] Switching to ProctorSimDesk for 10 seconds...");
                SwitchDesktop(new_desk);
                thread::sleep(Duration::from_secs(10));
                SwitchDesktop(orig_desk);
                CloseDesktop(new_desk);
                let _ = OpenInputDesktop(0, 0, GENERIC_ALL);
                println!("[win-proctor] Switched back to Default desktop.");
            },
            _ => {
                println!("Usage: win-proctor <close|hide|kill|explorer|desktop>");
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn main() {
    win::run();
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("win-proctor.rs is a Windows-only simulator. On macOS, use mac-proctor.swift.");
}
