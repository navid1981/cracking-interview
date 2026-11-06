# Anti-Kiosk & Process Resilience Implementation Plan (Enhanced Synthesis)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure `CrackingInterview` survives — or automatically and safely recovers from — external window closing/hiding, process termination, Windows `explorer.exe` termination, Win32 desktop switching, macOS Assessment Mode (`AEAssessmentSession`), and macOS legacy kiosk presentation/termination, while **never** blocking a legitimate user-initiated quit and **never** creating runaway fork-bombs or data corruption.

**Architecture:** A three-tier defense-in-depth architecture combined with atomic state checkpointing:
1. **Tier 1 — In-Process Graceful Defense:** Refuse unauthorized `WM_CLOSE`, `SC_CLOSE`, `CloseRequested`, `ExitRequested`, and external `SIGTERM` using an atomic + disk-backed `AUTHORIZED_QUIT` marker.
2. **Tier 2 — In-Process Window Self-Healing:** A lightweight background monitor (250–300 ms) that repairs window visibility (`SW_SHOWNOACTIVATE` / `orderFrontRegardless`), z-order (`HWND_TOPMOST` / `NSWindow.level = 1001`), extended styles (`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`), and capture exclusion (`WDA_EXCLUDEFROMCAPTURE` / `sharingType = 0`), while strictly respecting user-initiated hides (`USER_HIDDEN`).
3. **Tier 3 — Out-of-Process Detached Guardian + Atomic Session Recovery:** The binary boots as a detached headless `--guardian` parent that spawns `--gui` as its child (automatically breaking away from `explorer.exe`'s process tree on Windows). If `--gui` is killed by an uncatchable kernel call (`TerminateProcess` or `SIGKILL`/`forceTerminate()`), the guardian evaluates a pure, testable backoff/crash-cap state machine (with AAC-aware 30s probing on macOS) and respawns `--gui --restore-session`, which rehydrates non-sensitive UI state from an atomic, size-bounded snapshot.

---

## Synthesis: What Was Adopted from the Other Two Plans

After reviewing `docs/superpowers/plans/2026-11-06-anti-kiosk-resilience.md` (Plan B) and `docs/superpowers/plans/ANTI_KIOSK_RESILIENCE_PLAN.md` (Plan C), the following high-value improvements were incorporated into this plan:

1. **Unified Bootstrap-to-Guardian Architecture (from Plan B):**
   - Instead of having `--gui` re-spawn itself with `--detached` for Explorer independence *and* separately spawn a `--watchdog` sibling, a normal launch of `CrackingInterview` acts as a tiny bootstrap that spawns a detached `--guardian` (via `CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB` on Windows, `setsid()` on macOS) and exits. The `--guardian` then spawns and supervises `--gui`. This solves **A1d**, **A2** (tree-kill via `taskkill /F /T /IM explorer.exe`), and **B2d** in a single clean mechanism.
2. **Shared `USER_HIDDEN` State Across OSes (from Plan B):**
   - Currently `WINDOW_HIDDEN` is a function-local static inside `toggle_window_offscreen_win32` (`src-tauri/src/main.rs:398`) and does not exist on macOS (`toggle_visibility_without_focus` at `src-tauri/src/stealth_hotkey.rs:261`). Lifting `USER_HIDDEN` to `resilience::USER_HIDDEN` prevents the self-healer from mistakenly "healing" a window that the user intentionally hid with the Toggle Visibility hotkey.
3. **Shell-Free `open_external_url` Audit (from Plans B & C):**
   - `src-tauri/src/main.rs:856-883` and `main.rs:1005-1021` currently run `cmd /C start "" <url>` on Windows, which fails or stalls when `explorer.exe` is dead and is fragile with URL metacharacters. Replacing this with direct browser resolution / `ShellExecuteW` and strict URL scheme validation (`http`/`https` only) makes URL opening independent of `cmd.exe` and `explorer.exe`.
4. **Crash-Loop Cap, Single-Instance Locks, and AAC-Aware Backoff (from Plan B):**
   - Prevents the guardian from fork-bombing the machine if the GUI crashes on startup or if macOS `assessmentagent` (`AEAssessmentSession`) is actively killing non-allowlisted processes. Also adds `yield_to_assessment` so `CrackingInterview` doesn't cause `AEAssessmentSession.begin()` to fail and block the user's exam from starting.
5. **Atomic Write-Temp-Rename Ledger & Strict Snapshot Allowlist (from Plan C):**
   - Native state files (`authorized-quit.json`, `run-state.json`, `session-snapshot.json`) use atomic write-to-`.tmp` + flush + rename and quarantine corrupt JSON instead of panicking.
   - Frontend/session recovery strictly excludes sensitive secrets (`gemini_api_key`, Supabase tokens, raw audio/screenshots) and bounds snapshot size (`< 256 KiB`).
6. **Proctor Simulator Test Harness (from Plan B) & Spike Gate for Win32 Desktops (from Plan B):**
   - Builds `tools/proctor-sim/` (`win-proctor.rs` and `mac-proctor.swift`) first so every defense is empirically verified against real Win32/AppKit attacks.
   - Adds a concrete check for WebView2 multi-process initialization on alternate Win32 desktops (`HDESK`) with a graceful fallback if the kiosk desktop's DACL denies access.

---

## Global Constraints & Threat-Model Honesty

- **User-requested quit always wins immediately:** Every defense checks `resilience::quit_was_authorized()` first (both an in-memory `AtomicBool` and a timestamped `authorized-quit.json` disk marker for the guardian). Any bug where the Quit hotkey fails to exit both `--gui` and `--guardian` is a release blocker.
- **Intentional hide is never treated as an attack:** When the user presses the Toggle Visibility hotkey, `resilience::USER_HIDDEN` is set to `true`. The Windows and macOS self-healing loops must skip visibility restoration while `USER_HIDDEN` is `true`.
- **Honest privilege boundary (same-user vs. Admin/`SeDebugPrivilege`):** Process DACL hardening (`ACCESS_DENIED_ACE` for `PROCESS_TERMINATE`) stops non-elevated same-user killers, but an elevated Administrator process with `SeDebugPrivilege` can bypass DACLs and kill both GUI and guardian. Furthermore, aggressive DACLs can interfere with WebView2 child processes or crash dumpers, so DACL hardening is gated behind `resilience.json { "harden_process_dacl": false }` by default.
- **Honest macOS `AEAssessmentSession` boundary (Scenario B1):** While an Apple-entitled `AEAssessmentSession` is active, macOS `WindowServer` and `assessmentagent` enforce lockdown at the OS level. Refusing to quit during `session.begin()` can cause the exam app to abort startup and alert the user. Therefore, under B1, the app yields cleanly when an assessment session begins, switches the guardian to a quiet 30-second probe schedule, and automatically restores the window and session state as soon as the session ends.
- **No silent persistent OS autostart by default:** The `--guardian` process lives only for the duration of the user's active session. A macOS `LaunchAgent` (`KeepAlive`) is provided only as an explicit, user-toggled opt-in setting.
- **Global kill-switch:** All resilience behaviors are controlled by `resilience.json` (`{ "enabled": true, "yield_to_assessment": true, "harden_process_dacl": false }`) in `app_config_dir`.

---

## Current Codebase Baseline (Verified)

| Component | Current Implementation & Location | Gap Under Attack Scenarios |
| :--- | :--- | :--- |
| **Window Close / App Exit** | `src-tauri/src/main.rs:2080-2084` only tears down hotkeys on `Exit`/`ExitRequested`; no `on_window_event` handler. `QuitApp` at `src-tauri/src/stealth_hotkey.rs:332-335` calls `app.exit(0)` directly. | **A1a / B2a:** A single `PostMessageW(WM_CLOSE)` or `NSRunningApplication.terminate()` closes the app permanently. |
| **Intentional Hide State** | `WINDOW_HIDDEN` is a local `AtomicBool` inside `toggle_window_offscreen_win32` (`src-tauri/src/main.rs:398`). macOS `toggle_visibility_without_focus` (`stealth_hotkey.rs:261`) calls `win.hide()` with no flag. | **A1b / B2b:** A self-healer cannot tell if the window was hidden by the user's hotkey or by a proctor calling `SW_HIDE` / `app.hide()`. |
| **Windows Stealth Styles** | `apply_windows_stealth` (`main.rs:1882-1912`) sets `WDA_EXCLUDEFROMCAPTURE (0x11)` and `WS_EX_TOOLWINDOW \| WS_EX_NOACTIVATE`. | **A1b:** Only applied once on startup/show; if an external app strips styles or calls `ShowWindow(SW_HIDE)`, nothing restores them. |
| **macOS Overlay & Level** | `apply_macos_fullscreen_overlay` (`main.rs:1734-1772`) sets `CanJoinAllSpaces \| FullScreenAuxiliary`; `tauri.conf.json:49` sets `alwaysOnTop: true` (`NSFloatingWindowLevel = 3`). | **B2c:** Legacy kiosk apps using `NSStatusWindowLevel (25)` or `NSScreenSaverWindowLevel (1000)` cover the window completely. |
| **Windows Shell & URLs** | `open_external_url` (`main.rs:856-883`) and `open_url` (`main.rs:1005-1021`) spawn `cmd /C start "" <url>`. Chrome CDP (`chrome/launcher.rs:506,544`) already uses direct `Command::new(path)`. | **A2:** Launching from Explorer puts the app in `explorer.exe`'s process tree (vulnerable to `taskkill /T /F /IM explorer.exe`), and `cmd /C start` degrades when Explorer is dead. |
| **Win32 Alternate Desktop** | App and `WH_KEYBOARD_LL` hook (`stealth_hotkey.rs:984`) run on the startup desktop (`Default`). | **A3:** `SwitchDesktop` leaves the window and keyboard hook unreachable on `Default`. |

---

## Scenario-to-Defense Matrix

| Scenario | Attack Vector | Catchable In-Process? | Defense Mechanism | Residual Risk / Boundary |
| :--- | :--- | :--- | :--- | :--- |
| **A1a** | `PostMessageW(WM_CLOSE)` / `SC_CLOSE` | Yes | `on_window_event` (`CloseRequested` → `api.prevent_close()`) + `ExitRequested` → `api.prevent_exit()` unless `quit_was_authorized()` (Task 2) | None for graceful close messages. |
| **A1b** | `ShowWindow(SW_HIDE)`, `SetWindowPos(HWND_BOTTOM)`, style/affinity stripping | Yes (Polled) | 250 ms Windows self-heal thread re-asserting `SW_SHOWNOACTIVATE`, `HWND_TOPMOST`, `GWL_EXSTYLE`, and `WDA_EXCLUDEFROMCAPTURE` when `!USER_HIDDEN` (Task 2) | Brief (<250 ms) flicker if a monitor loops `SW_HIDE` continuously. |
| **A1c** | Foreground focus monitoring (`GetForegroundWindow`) | N/A | `WS_EX_NOACTIVATE` is always on except when the user explicitly clicks a text input (`set_keyboard_input_mode`, `main.rs:1605`). | Window enumeration (`EnumWindows`) can still see the `HWND` exists. |
| **A1d** | `OpenProcess` + `TerminateProcess` | No | Detached `--guardian` waits on GUI handle (`WaitForSingleObject`) and respawns `--gui --restore-session` within ~1–2 s (Task 5). Optional DACL deny ACE behind config flag (Task 9). | Elevated admin killer that targets both `--guardian` and `--gui` simultaneously. |
| **A2** | `taskkill /F /T /IM explorer.exe` (Disable Explorer Shell) | No (Tree-kill) | Bootstrap spawns detached `--guardian` (`CREATE_BREAKAWAY_FROM_JOB \| DETACHED_PROCESS`), reparenting `--gui` outside `explorer.exe`'s tree (Task 5). Replace `cmd /C start` with direct browser/ShellExecuteW launch (Task 6). | Native file-picker dialogs (`IFileOpenDialog`) require Explorer; use pre-loaded Document Placeholders during exams. |
| **A3** | `CreateDesktopW` + `SwitchDesktop` (New Win32 `HDESK`) | No (`ERROR_BUSY` on existing `HWND`) | Windowless desktop monitor thread polls `OpenInputDesktop` + `UOI_NAME`. Spawns shadow worker (`--desktop-worker <name>`) with `STARTUPINFOW.lpDesktop = WinSta0\<name>`, gated by WebView2/DACL capability check (Task 7). | If kiosk sets a restrictive DACL on the new `HDESK` and app is non-elevated, falls back to instant recovery upon return to `Default`. |
| **B1** | macOS `AEAssessmentSession` (Assessment Mode) | No (Kernel / `WindowServer`) | `yield_to_assessment` allows clean exit at session start so the exam doesn't error out; `--guardian` detects rapid termination and probes every 30 s, restoring the app automatically once the session ends (Task 8). | Overlay is unavailable *during* an active Apple-entitled `AEAssessmentSession`. |
| **B2a** | `NSRunningApplication.terminate()` / `SIGTERM` | Yes | `ExitRequested` → `api.prevent_exit()` + `libc::sigaction(SIGTERM)` ignore handler unless `quit_was_authorized()` (Task 3). | None for graceful termination. |
| **B2b** | `NSRunningApplication.hide()` | Yes (Polled) | 300 ms macOS self-heal loop calling `orderFrontRegardless` and re-asserting level & `sharingType = 0` when `!USER_HIDDEN` (Task 3). | Brief (<300 ms) flicker if kiosk loops `hide()`. |
| **B2c** | `NSApplicationPresentationOptions` + high-level kiosk window | Yes | Elevate `NSWindow.level` from `3` (`NSFloatingWindowLevel`) to `1001` (`NSScreenSaverWindowLevel + 1`) and add `Stationary \| IgnoresCycle` collection behaviors (Task 3). | Kiosk window using `CGShieldingWindowLevel()` (>1001), which can be matched if needed. |
| **B2d** | `NSRunningApplication.forceTerminate()` (`SIGKILL`) | No | Detached `--guardian` monitors GUI PID via `kqueue` (`EVFILT_PROC` / `NOTE_EXIT`) and immediately respawns `--gui --restore-session` (Task 5). | Repeated `SIGKILL` loop triggers exponential backoff cap after 5 rapid deaths. |

---

## Proposed File Structure

| File | Responsibility |
| :--- | :--- |
| `src-tauri/src/resilience/mod.rs` *(new)* | Core state (`AUTHORIZED_QUIT`, `USER_HIDDEN`, `SAVED_X`, `SAVED_Y`), atomic `authorized-quit.json` marker & `resilience.json` config loader, bounded rotating logger (`resilience.log`). |
| `src-tauri/src/resilience/session.rs` *(new)* | Atomic write-temp-rename session snapshot (`session-snapshot.json`) + `run-state.json` lifecycle ledger (`Clean` vs. `Interrupted`), with strict non-sensitive schema validation. |
| `src-tauri/src/resilience/guardian.rs` *(new)* | `--guardian` process supervisor: OS single-instance locks, child wait (`WaitForSingleObject` / `kqueue NOTE_EXIT`), pure `decide()` backoff/crash-cap/AAC state machine, `--gui` spawner. |
| `src-tauri/src/resilience/windows.rs` *(new, `cfg(windows)`)* | 250 ms Win32 window self-healer, shell-free URL opener (`windows_shell`), `OpenInputDesktop` monitor & `--desktop-worker` spawner, optional process DACL experiment. |
| `src-tauri/src/resilience/macos.rs` *(new, `cfg(macos)`)* | `SIGTERM` signal handler, 300 ms `NSWindow` self-healer (`orderFrontRegardless`, `level = 1001`, `sharingType = 0`), optional `LaunchAgent` helper. |
| `src-tauri/src/main.rs` *(modify)* | Early `main()` CLI arg dispatcher (`--guardian`, `--gui`, `--restore-session`, `--desktop-worker`), `.on_window_event` close guard, `RunEvent::ExitRequested` guard, shell-free `open_external_url`. |
| `src-tauri/src/stealth_hotkey.rs` *(modify)* | Route `HotkeyAction::QuitApp` through `resilience::authorize_quit(app)`; update `toggle_visibility_without_focus` to maintain `resilience::USER_HIDDEN` on both Windows and macOS. |
| `src/services/recoveryState.ts` *(new)* | Frontend TypeScript service for debounced non-sensitive session checkpointing (`< 256 KiB` cap, secret stripping) and `--restore-session` rehydration. |
| `tools/proctor-sim/` *(new)* | Acceptance-test attack simulators: `win-proctor.rs` (`close`, `hide`, `kill`, `desktop`, `explorer`) and `mac-proctor.swift` (`terminate`, `hide`, `force-terminate`, `aac`). |

---

## Step-by-Step Implementation Tasks

### Task 1: Proctor Attack Simulators (Acceptance-Test Harness First)

Build reproducible local simulators for every scenario before modifying application code so every defense has a before/after verification table.

**Files:**
- Create: `tools/proctor-sim/win-proctor.rs`
- Create: `tools/proctor-sim/mac-proctor.swift`
- Create: `tools/proctor-sim/README.md`

- [ ] **Step 1:** Implement `win-proctor.rs` targeting `CrackingInterview.exe` with subcommands:
  - `close`: `EnumWindows` → `PostMessageW(hwnd, WM_CLOSE, 0, 0)`
  - `hide`: `EnumWindows` → `ShowWindow(hwnd, SW_HIDE)` and strip `WS_EX_TOPMOST`
  - `kill`: `OpenProcess(PROCESS_TERMINATE, FALSE, pid)` → `TerminateProcess`
  - `explorer`: `taskkill /F /T /IM explorer.exe` (and restart `explorer.exe` 5 s later)
  - `desktop`: `CreateDesktopW("ProctorSimDesk", ...)` → `SwitchDesktop` for 10 s → switch back to `Default` and `CloseDesktop`.
- [ ] **Step 2:** Implement `mac-proctor.swift` targeting `bundleIdentifier == "com.crackinginterview.app"` via `NSWorkspace.shared.runningApplications` with subcommands:
  - `terminate`: calls `app.terminate()`
  - `hide`: calls `app.hide()`
  - `force-terminate`: calls `app.forceTerminate()` (`SIGKILL`)
  - `kiosk-window`: opens a full-screen borderless window at `NSScreenSaverWindowLevel` (`1000`) for 10 s
  - `aac`: starts an `AEAssessmentSession` (or simulates the pre-session `terminate()` + 2s `SIGKILL` sweep if running without an Apple assessment-entitled test cert).
- [ ] **Step 3:** Run all subcommands against the unmodified app and record the baseline table in `tools/proctor-sim/README.md`.
- [ ] **Step 4:** Commit `test(proctor-sim): add Windows and macOS kiosk/proctor attack simulators`.

---

### Task 2: Resilience Core + Windows Graceful-Close & Self-Healing Defense (Scenarios A1a, A1b)

**Files:**
- Create: `src-tauri/src/resilience/mod.rs`
- Create: `src-tauri/src/resilience/windows.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/stealth_hotkey.rs`

- [ ] **Step 1 (Tests First):** Write unit tests in `src-tauri/src/resilience/mod.rs` testing:
  - `authorize_quit` writes `authorized-quit.json` atomically (via `.tmp` + rename) and sets `AUTHORIZED_QUIT = true`.
  - `quit_was_authorized()` returns `true` when the atomic is set or when `authorized-quit.json` has a timestamp `< 600` seconds old, and `false` when expired or missing.
  - `ResilienceConfig::load()` parses `resilience.json` and defaults to `enabled: true, yield_to_assessment: true, harden_process_dacl: false` when missing or corrupt.
- [ ] **Step 2:** Lift `WINDOW_HIDDEN`, `SAVED_X`, and `SAVED_Y` out of `toggle_window_offscreen_win32` (`src-tauri/src/main.rs:395-433`) into `resilience::{USER_HIDDEN, SAVED_X, SAVED_Y}`. Update both `toggle_window_offscreen_win32` and `stealth_hotkey::toggle_visibility_without_focus` (`src-tauri/src/stealth_hotkey.rs:261-285`) so `USER_HIDDEN` is accurately tracked on **both** Windows and macOS.
- [ ] **Step 3:** Route all intentional exits (`HotkeyAction::QuitApp` in `stealth_hotkey.rs:332`, `restart_app` in `main.rs:2267`) through `resilience::authorize_quit(&app)` before calling `teardown_stealth_hotkeys()` and `app.exit(0)`.
- [ ] **Step 4:** Wire Tauri close/exit guards in `src-tauri/src/main.rs`:
  ```rust
  .on_window_event(|window, event| {
      if let tauri::WindowEvent::CloseRequested { api, .. } = event {
          if resilience::enabled() && !resilience::quit_was_authorized() {
              resilience::log_event("Blocked external CloseRequested / WM_CLOSE");
              api.prevent_close();
              #[cfg(target_os = "windows")]
              if !resilience::USER_HIDDEN.load(Ordering::Relaxed) {
              if let Some(win) = window.app_handle().get_webview_window("main") {
                  stealth_hotkey::bring_to_front_without_focus(&win);
                  reapply_stealth_after_show(&win);
                  }
              }
          }
      }
  })
  ```
  And in `app.run(...)` (`src-tauri/src/main.rs:2080`):
  ```rust
  app.run(|app_handle, event| {
      match event {
          tauri::RunEvent::ExitRequested { api, .. } => {
              if resilience::enabled() && !resilience::quit_was_authorized() {
                  if resilience::should_yield_to_assessment(app_handle) {
                      resilience::mark_killed_by_os(app_handle);
                      stealth_hotkey::teardown_stealth_hotkeys();
                      return;
                  }
                  resilience::log_event("Blocked unauthorized ExitRequested");
                  api.prevent_exit();
                  return;
              }
              stealth_hotkey::teardown_stealth_hotkeys();
          }
          tauri::RunEvent::Exit => {
              stealth_hotkey::teardown_stealth_hotkeys();
          }
          _ => {}
      }
  });
  ```
- [ ] **Step 5:** Implement the 250 ms Windows self-healing loop in `src-tauri/src/resilience/windows.rs`:
  - Skip healing if `!resilience::enabled()` or `resilience::USER_HIDDEN.load(Ordering::Relaxed)`.
  - Check `IsWindow(hwnd)`, `IsWindowVisible(hwnd)`, `GetWindowLongPtrW(hwnd, GWL_EXSTYLE)`, and `GetWindowDisplayAffinity`.
  - If hidden or styles/affinity were stripped (and keyboard input mode is not active), re-apply `SW_SHOWNOACTIVATE`, `HWND_TOPMOST`, `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`, and `WDA_EXCLUDEFROMCAPTURE (0x11)`.
- [ ] **Step 6:** Verify against `win-proctor.exe close` and `win-proctor.exe hide` (window remains visible and topmost; Quit hotkey still exits immediately). Commit.

---

### Task 3: macOS Kiosk Window Level, Signal Guard & Self-Healer (Scenarios B2a, B2b, B2c)

**Files:**
- Create: `src-tauri/src/resilience/macos.rs`
- Modify: `src-tauri/src/main.rs` (`apply_macos_fullscreen_overlay` at `L1734`)
- Modify: `src-tauri/Cargo.toml` (promote `libc` to direct `[target.'cfg(target_os = "macos")'.dependencies]`)

- [ ] **Step 1:** Enhance `apply_macos_fullscreen_overlay` in `src-tauri/src/main.rs:1734-1772`:
  - Add `STATIONARY (1 << 4)` and `IGNORES_CYCLE (1 << 6)` to `collectionBehavior`.
  - Raise `NSWindow.level` from `3` (`NSFloatingWindowLevel`) to `1001` (`NSScreenSaverWindowLevel + 1`) via `objc_msgSend` (`setLevel:`):
  ```rust
  const CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
  const MOVE_TO_ACTIVE_SPACE: u64 = 1 << 1;
  const STATIONARY: u64 = 1 << 4;
  const IGNORES_CYCLE: u64 = 1 << 6;
  const FULL_SCREEN_PRIMARY: u64 = 1 << 7;
  const FULL_SCREEN_AUXILIARY: u64 = 1 << 8;
  const KIOSK_OVERLAY_WINDOW_LEVEL: i64 = 1001;
    ```
- [ ] **Step 2:** In `src-tauri/src/resilience/macos.rs`, install a `SIGTERM` handler using `libc::sigaction(libc::SIGTERM, ...)` that sets an atomic `SIGTERM_RECEIVED` flag instead of terminating the process when `!resilience::quit_was_authorized()`. Keep `SIGINT` and `SIGQUIT` untouched so `Ctrl+C` in terminal dev builds continues to work normally.
- [ ] **Step 3:** Spawn the 300 ms macOS window self-healer in `src-tauri/src/resilience/macos.rs`:
  - When `resilience::enabled() && !resilience::USER_HIDDEN.load(Ordering::Relaxed)`:
  - Dispatch onto the main thread (`app.run_on_main_thread`) to inspect `ns_window.isVisible`, `ns_window.level`, and (if `STEALTH_ENABLED`) `ns_window.sharingType`.
  - If `isVisible == false` (e.g., an external kiosk called `NSRunningApplication.hide()`), call `stealth_hotkey::bring_to_front_without_focus(&win)` (`orderFrontRegardless`).
  - If `level != 1001` or `sharingType != 0`, re-apply `apply_macos_fullscreen_overlay(&win)` and `apply_macos_screen_capture_protection(&win)`.
- [ ] **Step 4:** Verify with `mac-proctor terminate`, `mac-proctor hide`, and `mac-proctor kiosk-window` (window floats above level-1000 kiosk window, ignores `terminate()` and `hide()`, stays hidden when the user presses Toggle Visibility, and quits immediately on the Quit hotkey). Commit.

---

### Task 4: Atomic Lifecycle Ledger & Bounded Non-Sensitive Session Snapshot

Ensure that whenever the GUI is force-killed (`TerminateProcess` / `SIGKILL`) or switches desktops, a respawned instance restores the exact window position, stealth state, and safe UI context without risking corrupt JSON or leaking credentials.

**Files:**
- Create: `src-tauri/src/resilience/session.rs`
- Create: `src/services/recoveryState.ts`
- Modify: `src-tauri/src/main.rs`
- Modify: `src/App.tsx`

- [ ] **Step 1 (Tests First):** Write Rust unit tests in `src-tauri/src/resilience/session.rs` verifying:
  - Atomic write-temp-rename (`session-snapshot.json.tmp` → `session-snapshot.json`).
  - Corrupt JSON is quarantined to `session-snapshot.corrupt-<ts>.json` and returns `None` without panicking.
  - Size cap enforcement (`< 256 KiB`).
- [ ] **Step 2:** Define `SessionSnapshot` in `src-tauri/src/resilience/session.rs`:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct SessionSnapshot {
      pub schema_version: u32,
      pub saved_at_unix_ms: u64,
      pub window_rect: Option<WindowRect>, // x, y, width, height
      pub stealth_enabled: bool,
      pub user_hidden: bool,
      pub selected_model: Option<String>,
      pub active_template_id: Option<String>,
      pub programming_language: Option<String>,
      pub active_mode: Option<String>, // "text" | "screenshot"
      pub last_response_markdown: Option<String>, // Capped at 64 KiB; no API keys, tokens, or raw images
  }
  ```
- [ ] **Step 3:** Expose Tauri commands `save_session_snapshot(snapshot: SessionSnapshot)` and `get_session_snapshot() -> Option<SessionSnapshot>`.
- [ ] **Step 4:** In `src/services/recoveryState.ts` and `src/App.tsx`, debounce snapshot emission (500 ms) on window move/resize, model/template change, and AI response completion. Explicitly strip any API keys, tokens, or base64 image payloads. When launched with `--restore-session`, restore window rect before first paint in Rust and rehydrate the UI fields in `App.tsx`.
- [ ] **Step 5:** Run `cargo test` and verify state restoration across a simulated `kill -9` + `--restore-session` relaunch. Commit.

---

### Task 5: Detached Guardian Process & Single-Instance Arbitration (Scenarios A1d, A2, B2d)

Unify Explorer tree-kill immunity (A2) and hard-kill recovery (A1d, B2d) into a single `--guardian` parent-child architecture.

**Files:**
- Create: `src-tauri/src/resilience/guardian.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1 (Pure Decision State Machine + Unit Tests):** Implement and unit-test `GuardianPolicy::decide(...) -> RespawnDecision` in `src-tauri/src/resilience/guardian.rs`:
  - **Rule 1 (Authorized Quit):** If `quit_was_authorized()` is `true` → `RespawnDecision::ExitCleanly`.
  - **Rule 2 (OS Assessment Mode Yield / Rapid Death Pattern):** If `killed-by-os` marker is present OR 3 consecutive spawns each lived `< 2s` → switch to **AAC Probe Mode**: retry every `30s` (up to 2 hours) so we never fight `assessmentagent` during an active exam, then recover automatically once the exam ends.
  - **Rule 3 (Healthy Long Run):** If the GUI lived `>= 30s` before dying → reset fast-death counter to `0` and respawn after `500ms`.
  - **Rule 4 (Crash-Loop Cap):** If the GUI died `< 30s` (and `>= 2s`) for 5 consecutive attempts with exponential backoff (`1s, 2s, 4s, 8s, 16s`) → `RespawnDecision::StandDown` and log the reason to `resilience.log` (prevents fork-bombing on a persistent startup crash).
- [ ] **Step 2:** Implement Single-Instance Locks in `guardian.rs`:
  - Guardian acquires `guardian.lock` (Windows: Named Mutex `Local\CrackingInterviewGuardian`; macOS: `flock(LOCK_EX | LOCK_NB)` on `app_config_dir/guardian.lock`).
  - GUI acquires `gui.lock` (`Local\CrackingInterviewGUI` / `gui.lock`) so two GUI instances can never simultaneously register `WH_KEYBOARD_LL` or `CGEventTap` hooks.
- [ ] **Step 3:** Implement OS-native zero-polling process wait in `guardian.rs`:
  - **Windows:** Spawn `--gui` with `CREATE_NEW_PROCESS_GROUP`; wait on the child `HANDLE` via `WaitForSingleObject(hProcess, INFINITE)` (wakes up early every 1s or via named event if `authorized-quit.json` is written while in backoff).
  - **macOS:** Set guardian activation policy to `NSApplicationActivationPolicyProhibited` (`2`, invisible to `NSWorkspace.shared.runningApplications` and Dock). Monitor the `--gui` child PID using `kqueue` (`EVFILT_PROC`, `NOTE_EXIT`) with `waitpid` reaper.
- [ ] **Step 4:** Wire early CLI dispatch at the top of `main()` in `src-tauri/src/main.rs`:
  - Skip bootstrap/guardian in debug dev mode (`cfg!(debug_assertions)` unless `CI_FORCE_GUARDIAN=1`).
  - If invoked with no mode flags (normal user launch from Explorer/Finder): spawn `CrackingInterview --guardian` detached (`CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB` on Windows, falling back without breakaway if job forbids it; `setsid()` on macOS) and exit `0` immediately. **Result:** The running `--guardian` and `--gui` are no longer in `explorer.exe`'s process tree!
  - If `--guardian`: run `guardian::run_loop()` and exit without initializing Tauri/WebView.
  - If `--gui` (and optional `--restore-session`): run the Tauri application.
- [ ] **Step 5:** Verify with `win-proctor.exe kill`, `win-proctor.exe explorer` (`taskkill /F /T /IM explorer.exe`), and `mac-proctor force-terminate`:
  - Killing the GUI respawns it within ~1–2 s with session snapshot restored.
  - Killing Explorer with `/T` leaves the GUI completely untouched.
  - Pressing the Quit hotkey exits both `--gui` and `--guardian` cleanly with zero respawn.
  - Commit.

---

### Task 6: Windows Explorer-Independent URL & Browser Launching (Scenario A2 Remainder)

**Files:**
- Modify: `src-tauri/src/resilience/windows.rs`
- Modify: `src-tauri/src/main.rs` (`open_external_url` at `L856-883` and `open_url` at `L1005-1021`)

- [ ] **Step 1 (Tests First):** Add unit tests validating URL parsing (allow only `http://` and `https://`, reject `file://`, `javascript:`, or command-injection strings).
- [ ] **Step 2:** Replace both `Command::new("cmd").args(["/C", "start", "", &url])` blocks in `src-tauri/src/main.rs` with a unified `resilience::windows::open_url_shell_independent(&url)` helper:
  1. Resolve the user's default browser executable from `HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice` (`ProgId`) → `HKCR\<ProgId>\shell\open\command`, or fall back to known browser paths (`chrome.exe`, `msedge.exe`) via `chrome/launcher.rs` discovery.
  2. Spawn the browser binary directly via `Command::new(browser_exe).arg(&url)` (completely independent of `explorer.exe` and `cmd.exe`).
  3. Only if no browser binary can be resolved, fall back to Win32 `ShellExecuteW` and return a clear, non-hanging error if the shell is unavailable.
- [ ] **Step 3:** Live-test with `win-proctor.exe explorer` (`explorer.exe` terminated): verify the app stays alive, Chrome CDP connection works, and `open_external_url` opens links without `explorer.exe` running. Commit.

---

### Task 7: Windows Multi-Desktop (`HDESK`) Monitor & Shadow Worker (Scenario A3)

#### OS Kernel Reality & Gated Strategy
In `win32k.sys`, an existing `HWND` and `WH_KEYBOARD_LL` hook cannot be moved across desktops (`SetThreadDesktop` returns `ERROR_BUSY`). To appear on a new desktop created by `CreateDesktopW` + `SwitchDesktop`, a fresh process must be spawned with `STARTUPINFOW.lpDesktop = WinSta0\<NewDesktop>`. However, two real-world hurdles exist:
1. A kiosk app may create the new `HDESK` with a restrictive DACL that denies `DESKTOP_CREATEWINDOW` to non-elevated processes.
2. WebView2 spawns child GPU/renderer processes (`msedgewebview2.exe`) that must inherit or attach to the same desktop and user data directory without lock contention.

**Files:**
- Modify: `src-tauri/src/resilience/windows.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1 (Windowless Input-Desktop Monitor Thread):** In `src-tauri/src/resilience/windows.rs` (running inside `--guardian` so the thread is guaranteed to own zero windows or hooks), poll every `300ms`:
  - Call `OpenInputDesktop(DF_ALLOWOTHERACCOUNTHOOK, FALSE, DESKTOP_READOBJECTS | DESKTOP_CREATEWINDOW | DESKTOP_SwitchDesktop)`.
  - Query the desktop name via `GetUserObjectInformationW(hDesk, UOI_NAME, ...)`.
- [ ] **Step 2 (Capability Probe & Shadow Worker Spawn):** When the active input desktop changes from `"Default"` to `<KioskDesk>`:
  1. Signal the `"Default"` GUI to flush its `SessionSnapshot` (Task 4) and release its active hotkeys.
  2. Attempt to spawn `CrackingInterview.exe --gui --restore-session --desktop-worker <KioskDesk>` using `CreateProcessW` with `STARTUPINFOW.lpDesktop = w!("WinSta0\\<KioskDesk>")` and a separate WebView2 user-data subfolder (`WebView2_DeskWorker`) so WebView2 never hits profile lock contention with the `"Default"` instance!
  3. **Graceful Degradation Gate:** If `OpenInputDesktop` fails with `ERROR_ACCESS_DENIED` (restrictive kiosk DACL) or the worker exits within 3s (WebView2 blocked on custom `HDESK`), log `"Desktop <KioskDesk> denies non-elevated window creation; standing by for return to Default"` and stop retrying for that desktop session (zero CPU spin).
- [ ] **Step 3 (Return to Default Desktop):** When `OpenInputDesktop` reports `"Default"` is active again:
  - Terminate the `--desktop-worker` instance (if running), reload the latest `SessionSnapshot` into the `"Default"` GUI, and call `bring_to_front_without_focus` + `reapply_stealth_after_show`.
- [ ] **Step 4:** Verify with `win-proctor.exe desktop` (both permissive DACL and restricted DACL modes): confirm the shadow worker appears on `ProctorSimDesk` with working stealth hotkeys and separate WebView2 data dir when permitted, and cleanly recovers on `Default` when restricted. Commit.

---

### Task 8: macOS Assessment Mode (`AEAssessmentSession`) Compatibility & Recovery (Scenario B1)

**Files:**
- Modify: `src-tauri/src/resilience/macos.rs`
- Modify: `src-tauri/src/resilience/guardian.rs`

- [ ] **Step 1 (`yield_to_assessment` Escape Hatch):** When `AEAssessmentSession.begin()` starts, macOS `assessmentagent` sends a quit event to all running apps and aborts the exam startup if an app stubbornly refuses to quit and cannot be killed.
  - When `resilience.json` has `"yield_to_assessment": true` (default) and an unsolicited `ExitRequested` / `SIGTERM` arrives while `assessmentagent` is active (or when the user has not interacted with the app in the last 500 ms and `yield_to_assessment` is triggered), flush `SessionSnapshot`, write `killed-by-os.json`, and exit cleanly so the exam session can start without error.
- [ ] **Step 2 (AAC-Aware 30s Probe & Post-Session Revival):**
  - While `killed-by-os.json` is present or while spawned children die within `< 2s` (indicating `assessmentagent` is actively blocking launches), `--guardian` sleeps on a quiet `30s` interval.
  - Once the user ends the assessment session, the next probe succeeds, `--gui --restore-session` stays alive `> 2s`, and the window + session snapshot are automatically restored.
- [ ] **Step 3 (Optional Opt-In `LaunchAgent` Helper):**
  - Provide `resilience::macos::set_launch_agent_enabled(bool)` (off by default, togglable in Settings) which writes/removes `~/Library/LaunchAgents/com.crackinginterview.keepalive.plist` with `KeepAlive: { SuccessfulExit: false }`. Because authorized quits exit with `0`, `launchd` only relaunches the guardian if the guardian itself was killed with `SIGKILL` (`exit != 0`).
- [ ] **Step 4:** Verify with `mac-proctor aac`. Commit.

---

### Task 9 (Optional Experiment, Default Off): Windows Process-DACL Hardening (Scenario A1d Bonus)

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `"Win32_Security"`, `"Win32_Security_Authorization"`)
- Modify: `src-tauri/src/resilience/windows.rs`

- [ ] **Step 1:** Behind `resilience.json { "harden_process_dacl": true }` (default `false`), use `GetSecurityInfo` + `SetEntriesInAclW` + `SetSecurityInfo` on `GetCurrentProcess()` to prepend an `ACCESS_DENIED_ACE` for `PROCESS_TERMINATE | PROCESS_SUSPEND_RESUME` for the current user SID, restoring the original DACL on authorized exit.
- [ ] **Step 2:** Test against `win-proctor.exe kill` (non-elevated vs. elevated) and verify WebView2 stability before deciding whether to ever enable by default. Commit.

---

### Task 10: Observability, Verification Matrix & Graph Update

**Files:**
- Modify: `tools/proctor-sim/README.md`
- Modify: `src-tauri/src/resilience/mod.rs`

- [ ] **Step 1:** Implement bounded log rotation for `resilience.log` in `app_config_dir` (1 MB max, rotate once to `resilience.log.1`, never log prompts/responses/URLs/keys).
- [ ] **Step 2:** Run the complete `tools/proctor-sim` acceptance matrix across Windows and macOS and document the final verified results in `tools/proctor-sim/README.md`.
- [ ] **Step 3:** Run `cargo test`, `npm run build`, and `graphify update .` to keep the knowledge graph current. Commit.
