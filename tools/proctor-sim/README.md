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

---

## Verification Matrix (Before vs. After)

| Scenario | Simulator Command | Baseline (Unmodified App) | Expected After Resilience Plan |
| :--- | :--- | :--- | :--- |
| **A1a (Win Close)** | `win-proctor.exe close` | App closes immediately (`0` windows left). | `CloseRequested` blocked; window stays visible (`1` window alive). |
| **A1b (Win Hide/Style)** | `win-proctor.exe hide` | Window hidden (`0` visible windows). | 250ms self-healer restores `SW_SHOWNOACTIVATE`, `HWND_TOPMOST`, and `WDA_EXCLUDEFROMCAPTURE` in `< 1s`. |
| **A1d (Win Hard Kill)** | `win-proctor.exe kill` | Process killed permanently (`0` windows). | Detached `--guardian` detects exit via `WaitForSingleObject` and respawns `--gui --restore-session` in `< 2s`. |
| **A2 (Explorer Tree Kill)** | `win-proctor.exe explorer` | Killed with Explorer tree if launched from shell; `cmd /C start` URL fails. | Survives `taskkill /F /T /IM explorer.exe` (reparented under detached guardian); direct browser launch works without Explorer. |
| **A3 (Win New Desktop)** | `win-proctor.exe desktop` | Stranded on `Default` desktop. | Guardian detects `ProctorSimDesk` via `OpenInputDesktop` and spawns `--desktop-worker ProctorSimDesk` (or recovers cleanly on return to `Default` if DACL denies). |
| **B1 (macOS AAC Mode)** | `mac-proctor.swift aac` | Killed at session start; never returns. | Yields cleanly at session start, probes every 30s during active lock, and automatically restores session after lock ends. |
| **B2a (macOS Terminate)** | `mac-proctor.swift terminate` | App quits immediately (`0` instances). | `ExitRequested` prevented & `SIGTERM` ignored; app remains alive. |
| **B2b (macOS Hide)** | `mac-proctor.swift hide` | Window hidden until manual hotkey press. | 300ms self-healer restores visibility via `orderFrontRegardless` in `< 1s` (unless `USER_HIDDEN`). |
| **B2c (macOS Kiosk Level)** | `mac-proctor.swift kiosk-window` | Kiosk window (`level = 1000`) covers app (`level = 3`). | App window (`level = 1001`, `Stationary \| IgnoresCycle`) floats above the level-1000 kiosk window. |
| **B2d (macOS Force Kill)** | `mac-proctor.swift force-terminate` | Killed by `SIGKILL` permanently. | Detached `--guardian` wakes via `kqueue EVFILT_PROC/NOTE_EXIT` and respawns `--gui --restore-session` in `< 2s`. |
