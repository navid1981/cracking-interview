# Proctor & Kiosk Attack Simulators (`tools/proctor-sim`)

This directory contains standalone attack simulators for **Windows** (`win-proctor.rs`) and **macOS** (`mac-proctor.swift`) used to verify `CrackingInterview`'s resilience across Scenarios A1–A3 and B1–B2.

## Building & Running

### Windows (`win-proctor.rs`)
Compile with zero external dependencies using `rustc`:
```powershell
rustc tools/proctor-sim/win-proctor.rs -o tools/proctor-sim/win-proctor.exe
```

Subcommands:
- `win-proctor.exe close` — Enumerates windows and sends `PostMessageW(hwnd, WM_CLOSE, 0, 0)`.
- `win-proctor.exe hide` — Calls `ShowWindow(hwnd, SW_HIDE)` and strips `WS_EX_TOPMOST`.
- `win-proctor.exe kill` — Calls `OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess`.
- `win-proctor.exe explorer` — Executes `taskkill /F /T /IM explorer.exe` (tree-kill) and restarts Explorer 3s later.
- `win-proctor.exe desktop` — Creates `ProctorSimDesk` via `CreateDesktopW` and switches input desktop for 10s.

### macOS (`mac-proctor.swift`)
Run directly via `swift`:
```bash
swift tools/proctor-sim/mac-proctor.swift <terminate|hide|force-terminate|kiosk-window|aac>
```

> **`force-terminate` sends a real `SIGKILL` via `kill(2)`** and verifies death in the
> process table (`sysctl KERN_PROC_PID`, zombies count as dead). It does *not* use
> `NSRunningApplication.forceTerminate()`, which only *requests* termination through
> LaunchServices — for processes not launched by LaunchServices (e.g. a `cargo run` dev
> binary) it returns `true` yet delivers no SIGKILL at all (the target merely receives a
> polite, handleable SIGTERM), and `NSWorkspace` keeps reporting the dead PID as alive.
> Note the sim SIGKILLs **every** matching instance (GUI + guardian, plus the parked dev
> bootstrap under `tauri dev`): a whole-tree SIGKILL leaves nothing alive to respawn
> (the documented limit — only the opt-in LaunchAgent, `macos_launch_agent_enabled`,
> backstops that). To verify guardian respawn after a hard kill, SIGKILL only the GUI:
> `kill -9 <gui-pid>`.

---

## Verification Matrix (Before vs. After Implementation)

| Scenario | Simulator Command | Baseline (Unmodified App) | Verified Implementation (`resilience` Module) |
| :--- | :--- | :--- | :--- |
| **A1a (Win Close)** | `win-proctor.exe close` | App closes immediately (`0` windows left). | **Blocked:** `WindowEvent::CloseRequested` + `RunEvent::ExitRequested` call `api.prevent_close()` / `api.prevent_exit()` unless `resilience::quit_was_authorized()`. |
| **A1b (Win Hide/Style)** | `win-proctor.exe hide` | Window hidden (`0` visible windows). | **Self-Healed (< 250ms):** `resilience::windows::start_window_self_healer` restores `SW_SHOWNOACTIVATE`, `HWND_TOPMOST`, `WS_EX_TOOLWINDOW \| WS_EX_NOACTIVATE`, and `WDA_EXCLUDEFROMCAPTURE` while respecting `USER_HIDDEN`. |
| **A1d (Win Hard Kill)** | `win-proctor.exe kill` | Process killed permanently (`0` windows). | **Respawned (< 1.5s):** Detached `--guardian` (`resilience::guardian`) detects exit and respawns `--gui --restore-session` with window rect and UI state restored from `session-snapshot.json`. Optional DACL hardening (`harden_process_dacl: true`) denies non-elevated `OpenProcess(PROCESS_TERMINATE)`. |
| **A2 (Explorer Tree Kill)** | `win-proctor.exe explorer` | Killed with Explorer tree if launched from shell; `cmd /C start` URL fails. | **Immune (packaged build only):** Initial launch spawns detached `--guardian` (`CREATE_BREAKAWAY_FROM_JOB \| DETACHED_PROCESS`) and the bootstrap exits immediately, orphaning the guardian outside Explorer's live tree; `open_url_shell_independent` resolves browser binary directly without `cmd.exe` or `explorer.exe`. ⚠️ **Not testable under `tauri dev`:** the dev bootstrap *parks* (to keep Vite alive) and the dev terminal is itself an Explorer descendant, so `taskkill /T` legitimately walks Terminal → … → bootstrap → guardian → GUI (and kills the simulator's own shell before it can restart Explorer). Validate A2 on a packaged build launched from Explorer, with the simulator started via Task Manager → "Run new task" so it isn't inside the tree it kills. In dev, approximate it by ending `explorer.exe` from Task Manager (no `/T`) and confirming the app + link opening still work. |
| **A3 (Win New Desktop)** | `win-proctor.exe desktop` | Stranded on `Default` desktop. | **Shadow Worker / Graceful Recovery:** Guardian polls `OpenInputDesktop` (300ms) and spawns `--desktop-worker <deskName>` onto `WinSta0\<deskName>` with isolated `WebView2_DeskWorker` profile, or stands by cleanly without spinning if DACL denies access. |
| **B1 (macOS AAC Mode)** | `mac-proctor.swift aac` | Killed at session start; never returns. | **AAC-Compatible Recovery:** Yields cleanly when `assessmentagent` is active (`killed-by-os.json`), switches guardian to a 30s probe interval during the lock, and restores the session automatically once the assessment ends. |
| **B2a (macOS Terminate)** | `mac-proctor.swift terminate` | App quits immediately (`0` instances). | **Blocked:** `ExitRequested` prevented & `SIGTERM` intercepted via `libc::sigaction(SIGTERM)` (`resilience::macos::install_sigterm_guard`). |
| **B2b (macOS Hide)** | `mac-proctor.swift hide` | Window hidden until manual hotkey press. | **Self-Healed (< 300ms):** `resilience::macos::start_window_self_healer` calls `orderFrontRegardless` and re-asserts `level = 1001` & `sharingType = 0` when `!USER_HIDDEN`. |
| **B2c (macOS Kiosk Level)** | `mac-proctor.swift kiosk-window` | Kiosk window (`level = 1000`) covers app (`level = 3`). | **Floats Above:** `apply_macos_fullscreen_overlay` sets `NSWindow.level = 1001` (`NSScreenSaverWindowLevel + 1`) and `CanJoinAllSpaces \| Stationary \| IgnoresCycle \| FullScreenAuxiliary`. |
| **B2d (macOS Force Kill)** | `mac-proctor.swift force-terminate` (real `kill(2)` SIGKILL to **all** instances) | Killed by `SIGKILL` permanently. | **GUI-only SIGKILL → Respawned (< 1s):** Detached `--guardian` (running with `NSApplicationActivationPolicyProhibited`) wakes immediately on `kqueue` (`EVFILT_PROC` / `NOTE_EXIT`) and respawns `--gui --restore-session` (verify with `kill -9 <gui-pid>`). **Whole-tree SIGKILL (the sim default) → gone until relaunch:** nothing survives to respawn; only the opt-in LaunchAgent backstops it. |
