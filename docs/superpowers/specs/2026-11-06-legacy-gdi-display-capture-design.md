# Windows Legacy GDI Display Capture Design Spec

**Date:** 2026-11-06
**Status:** Approved

## 1. Intent & Problem Statement

Currently, `src-tauri/src/screenshot.rs` relies on `screenshots = "0.7"` for display screenshots (`capture_display_screenshot`) and thumbnails (`capture_display_thumbnail`). On Windows, `screenshots 0.7` uses `CreateDCW` + `StretchBlt(..., SRCCOPY)` without `CAPTUREBLT`, and modern Windows capture pipelines (Windows Graphics Capture / filtered DXGI) omit or blank out windows that use `WS_EX_LAYERED` or `SetWindowDisplayAffinity` (`WDA_EXCLUDEFROMCAPTURE = 0x11`, `WDA_MONITOR = 0x01`).

The goal is to replace the Windows capture path with a Zoom-style **Legacy GDI Screen-DC + Per-Window Non-Filtering Composition Pipeline** so that `capture_display_screenshot` and `capture_display_thumbnail` capture what the user sees on the display—including layered windows and third-party capture-excluded windows—while **never** including the `CrackingInterview` overlay window itself in the captured image.

## 2. Goals, Non-Goals & Anti-Goals

### Goals
- **Legacy GDI Screen-DC Base Capture (`SRCCOPY | CAPTUREBLT`):** Capture the target monitor's physical pixel rectangle directly from the Win32 screen/display `HDC` using `BitBlt` with `SRCCOPY | CAPTUREBLT` (`0x00CC0020 | 0x40000000`) so layered (`WS_EX_LAYERED`) and translucent windows are included.
- **Z-Order Per-Window Composition for Third-Party Protected Windows:** Enumerate visible top-level windows on the target monitor (`EnumWindows`), detect third-party windows with non-zero display affinity (`GetWindowDisplayAffinity` returning `WDA_MONITOR = 0x01` or `WDA_EXCLUDEFROMCAPTURE = 0x11`), capture their window surfaces via legacy GDI window-paint paths (`PrintWindow(..., PW_RENDERFULLCONTENT)` → `PrintWindow(..., 0)` → `WM_PRINT` / `GetWindowDC`), and composite non-blank surfaces onto the display canvas in bottom-to-top Z-order.
- **Strict Self-Exclusion (`CrackingInterview` Overlay Never Captured):** Ensure `CrackingInterview`'s own window(s) are never included in the screenshot or thumbnail:
  1. Never composite any `HWND` whose owning PID (`GetWindowThreadProcessId`) equals `std::process::id()`.
  2. If Stealth Mode is currently disabled (`STEALTH_ENABLED == false`, meaning `WDA_EXCLUDEFROMCAPTURE` is not set on our own `HWND`), temporarily apply `WDA_EXCLUDEFROMCAPTURE (0x11)` to our own `HWND` for the duration of the Stage 1 `BitBlt` and restore `WDA_NONE (0)` immediately afterward so the base screen-DC blit also omits our overlay without any visual flicker.
- **Consistent Multi-Monitor & High-DPI Geometry:** Enumerate physical monitors deterministically (`EnumDisplayMonitors` + `GetMonitorInfoW` + `GetDeviceCaps(DESKTOPHORZRES / DESKTOPVERTRES)`) so `get_all_displays()`, `capture_display_screenshot()`, and `capture_display_thumbnail()` agree on display indices, primary monitor flags, and physical pixel dimensions.

### Non-Goals
- Changing macOS display capture behavior (`screenshots 0.7` remains the macOS implementation).
- Injecting DLLs or kernel drivers into third-party processes (cross-process memory injection triggers anti-cheat/EDR and is out of scope).
- Changing the frontend Tauri command signatures (`get_displays`, `capture_display_screenshot`, `get_display_thumbnail`).

### Anti-Goals
- Capturing the `CrackingInterview` overlay window in any screenshot sent to the AI or shown in thumbnails.
- Replacing a valid screen-DC region with an all-black buffer if a third-party hardware-accelerated/DRM window refuses `PrintWindow` (blank/all-zero window captures must be discarded before compositing).

## 3. Architecture & Pipeline Design

### 3.1 Module Layout
- `src-tauri/src/screenshot.rs`: Public cross-platform entrypoints (`DisplayInfo`, `get_all_displays`, `capture_display_screenshot`, `capture_display_thumbnail`, `request_screen_recording_permission`) and shared image-resizing/JPEG-encoding helpers (`encode_screenshot_jpeg`, `encode_thumbnail_jpeg`).
- `src-tauri/src/screenshot_win32.rs` *(new, `#[cfg(target_os = "windows")]`)*: Pure Win32 GDI display enumeration, Legacy GDI `SRCCOPY | CAPTUREBLT` screen-DC capture, Z-order window inspection, non-blank buffer verification, and clipped RGBA/BGRA composition.

### 3.2 Stage 1: Legacy GDI Screen-DC Blit (`SRCCOPY | CAPTUREBLT`)
1. Resolve target monitor by `display_id` index from `enumerate_win32_monitors()`, returning physical desktop coordinates `(mon_x, mon_y, mon_w, mon_h)` and device name `szDevice`.
2. Ensure self-exclusion before blitting:
   - Check all top-level `HWND`s belonging to `std::process::id()`. Any visible `HWND` with `GetWindowDisplayAffinity` == `0` temporarily gets `SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11))` via an RAII guard (`SelfCaptureExclusionGuard`) that restores `0` on drop.
3. Open a monitor/virtual-screen `HDC` via `CreateDCW(szDevice, ...)` (with fallback to `GetDC(HWND(0))` at virtual-screen offset `(mon_x, mon_y)`).
4. Create a 32-bpp top-down DIB section (`CreateDIBSection` with `biHeight = -mon_h`, `BI_RGB`) selected into `CreateCompatibleDC`.
5. Execute `BitBlt(hdc_mem, 0, 0, mon_w, mon_h, hdc_screen, src_x, src_y, SRCCOPY | CAPTUREBLT)`.

### 3.3 Stage 2: Non-Filtering Per-Window Composition Pass
1. Enumerate top-level windows via `EnumWindows` (which returns windows in top-to-bottom Z-order) and reverse the list for **bottom-to-top** painter's composition.
2. Filter candidate windows:
   - Must be `IsWindowVisible(hwnd)` and not iconic (`!IsIconic(hwnd)`).
   - Must not have `WS_EX_TRANSPARENT` or cloaked state (`DWMWA_CLOAKED` if applicable, or zero-area rect).
   - **Self-exclusion:** `GetWindowThreadProcessId(hwnd, &mut pid)` where `pid == std::process::id()` → **skip unconditionally**.
   - Must intersect the target monitor rectangle `(mon_x, mon_y, mon_w, mon_h)`.
   - Must have `GetWindowDisplayAffinity(hwnd, &mut affinity)` where `affinity != 0` (`WDA_MONITOR = 0x01` or `WDA_EXCLUDEFROMCAPTURE = 0x11`). (Windows with `affinity == 0` are already present in Stage 1's `SRCCOPY | CAPTUREBLT` bitmap.)
3. For each candidate protected window:
   - Query its visible bounds (`GetWindowRect`), compute DPI-scaled physical width/height if needed, and allocate an offscreen 32-bpp DIB section `(win_w, win_h)`.
   - Attempt capture in fallback order until `is_non_blank_bgra(buf)` is true:
     a. `PrintWindow(hwnd, hdc_win_mem, PW_RENDERFULLCONTENT (0x00000002))`
     b. `PrintWindow(hwnd, hdc_win_mem, PW_CLIENTONLY (0))`
     c. `SendMessageTimeoutW(hwnd, WM_PRINT, hdc_win_mem, PRF_NONCLIENT | PRF_CLIENT | PRF_CHILDREN | PRF_ERASEBKGND, SMTO_ABORTIFHUNG, 150, ...)`
     d. `GetWindowDC(hwnd)` + `BitBlt(hdc_win_mem, 0, 0, win_w, win_h, hdc_win, 0, 0, SRCCOPY | CAPTUREBLT)`
   - If `is_non_blank_bgra(buf)` is true (i.e. contains non-zero RGB pixels, not an all-black DWM cutout), composite the window buffer onto the Stage 1 monitor buffer at `(win_x - mon_x, win_y - mon_y)` with bounds clipping via `composite_bgra_region`.
   - For windows higher in Z-order that overlap the newly composited protected window and have `affinity == 0` (normal windows sitting *above* the protected window), also re-composite their intersecting region or run the painter's algorithm for any window overlapping the protected window's rect so occlusion order stays accurate.
