# Windows Legacy GDI Display Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Windows `screenshots` crate capture path with a Zoom-style Legacy GDI Screen-DC (`SRCCOPY | CAPTUREBLT`) + Z-Order Per-Window Non-Filtering Composition pipeline that captures all visible windows (including layered and third-party capture-excluded windows) while strictly excluding `CrackingInterview`'s own overlay window.

**Architecture:** Extract shared RGBA-to-JPEG resizing/compression helpers in `src-tauri/src/screenshot.rs`, and introduce a dedicated Windows capture module `src-tauri/src/screenshot_win32.rs` (`#[cfg(target_os = "windows")]`, with platform-independent pure pixel/geometry helpers compiled on all targets for unit testing). Stage 1 blits the physical display via Win32 `CreateDCW`/`GetDC(NULL)` + `BitBlt(..., SRCCOPY | CAPTUREBLT)` while holding an RAII `SelfCaptureExclusionGuard` on any `CrackingInterview` `HWND` so our own overlay is omitted even when Stealth Mode is off. Stage 2 enumerates visible top-level windows in bottom-to-top Z-order, detects third-party windows with `GetWindowDisplayAffinity != 0` (excluding our own PID), renders them via `PrintWindow(PW_RENDERFULLCONTENT)` / `PrintWindow(0)` / `WM_PRINT` / `GetWindowDC`, verifies the buffer is non-blank, and composites it (plus any occluding normal windows above it) onto the display canvas.

**Tech Stack:** Rust (`src-tauri/`), `windows` crate `0.58` (`Win32_Graphics_Gdi`, `Win32_UI_WindowsAndMessaging`, `Win32_Storage_Xps` for `PrintWindow` or raw `extern "system"` link to `user32::PrintWindow`), `image = "0.25"`.

**Spec:** `docs/superpowers/specs/2026-11-06-legacy-gdi-display-capture-design.md`

## Global Constraints

- **`CrackingInterview` overlay must NEVER appear in screenshots or thumbnails:** Every capture skips any `HWND` whose PID equals `std::process::id()` in Stage 2, and temporarily applies `WDA_EXCLUDEFROMCAPTURE (0x11)` via an RAII guard during Stage 1 if our window currently has `WDA_NONE (0)`.
- **No external crate additions required:** Use the existing `windows = "0.58"` crate (and standard `user32.dll` / `gdi32.dll` FFI if any constant/function is outside currently enabled features) so compile times and lockfiles stay clean.
- **Keep macOS capture untouched:** `#[cfg(not(target_os = "windows"))]` continues using `screenshots = "0.7"`.
- **Preserve Tauri command contracts:** `get_displays`, `capture_display_screenshot`, and `capture_display_thumbnail` keep their exact signatures and return types.
- **Blank-buffer protection:** Never overwrite the Stage 1 screen-DC image with an all-zero/all-black buffer from a failed `PrintWindow` call.
- **Update knowledge graph:** After modifying Rust code, run `graphify update .` per workspace rules.

## Review Focus

1. **Stealth Mode OFF when taking a screenshot:** User has turned off Stealth Mode (`WDA_NONE` on `CrackingInterview`), then triggers `capture_display_screenshot` → `SelfCaptureExclusionGuard` must temporarily set `WDA_EXCLUDEFROMCAPTURE (0x11)` before Stage 1 `BitBlt` and restore `WDA_NONE (0)` on drop so `CrackingInterview` is still absent from the screenshot and restored afterward.
2. **Third-party window returns all-black buffer from `PrintWindow`:** Some GPU/DRM surfaces fail `PrintWindow` by returning `TRUE` with all-black (`0,0,0`) pixels → `is_non_blank_bgra` must reject buffers with zero non-black RGB pixels so Stage 1's underlying pixels are not clobbered with a black box, and fall back to the next capture method.
3. **Window partially off-screen or straddling two monitors:** A window at `(-150, 100)` or spanning Display 1 and Display 2 → `composite_bgra_region` must clip both negative destination offsets and right/bottom overflows without panicking or out-of-bounds slicing.
4. **Normal window overlapping on top of a `WDA_EXCLUDEFROMCAPTURE` window:** Window A (protected, lower in Z-order) is partially covered by Window B (normal, higher in Z-order). If Stage 2 paints Window A on top of Stage 1, it would cover Window B unless any visible third-party window *above* the first protected window in Z-order that intersects a protected window's rect is also composited in bottom-to-top painter's order.
5. **High-DPI / negative-coordinate secondary monitors:** Secondary monitor positioned to the left of primary (`rcMonitor.left = -1920`) or scaled at 150% DPI → monitor coordinate mapping and `CreateDCW` / virtual-screen `BitBlt` must use physical pixel dimensions (`DESKTOPHORZRES` / `DESKTOPVERTRES` or `DEVMODEW`) and accurate monitor-relative offsets.

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/screenshot_win32.rs` *(create)* | Pure geometry/pixel composition helpers (compiled on all OSes for unit tests) + `#[cfg(target_os = "windows")]` Legacy GDI Screen-DC capture (`SRCCOPY \| CAPTUREBLT`), `SelfCaptureExclusionGuard`, Z-order window discovery, `PrintWindow`/`WM_PRINT` fallback chain, and monitor enumeration |
| `src-tauri/src/screenshot.rs` *(modify)* | Shared `rgba_to_screenshot_jpeg` / `rgba_to_thumbnail_jpeg` encoding helpers; dispatch `get_all_displays`, `capture_display_screenshot`, and `capture_display_thumbnail` to `screenshot_win32` on Windows and `screenshots` crate on non-Windows |
| `src-tauri/src/main.rs` *(modify)* | Register `mod screenshot_win32;` |

---

### Task 1: Pure Pixel & Z-Order Composition Engine (`src-tauri/src/screenshot_win32.rs`)

Build and unit-test the platform-independent pixel conversion, blank-buffer detection, monitor-rect clipping, self-PID filtering, and painter's Z-order selection logic so it can be verified with `cargo test` on both macOS and Windows.

**Files:**
- Create: `src-tauri/src/screenshot_win32.rs`
- Modify: `src-tauri/src/main.rs` (add `mod screenshot_win32;`)

**Interfaces:**
- Produces:
  - `pub struct RectI32 { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }`
  - `pub struct CandidateWindowMeta { pub hwnd_id: usize, pub pid: u32, pub affinity: u32, pub rect: RectI32, pub is_visible: bool, pub is_iconic: bool, pub is_cloaked: bool }`
  - `pub fn is_non_blank_bgra(bgra: &[u8]) -> bool`
  - `pub fn bgra_to_rgba_in_place(buf: &mut [u8])`
  - `pub fn composite_bgra_region(canvas: &mut [u8], canvas_w: u32, canvas_h: u32, monitor_origin: (i32, i32), src: &[u8], src_w: u32, src_h: u32, window_pos: (i32, i32))`
  - `pub fn select_windows_to_composite(top_to_bottom: &[CandidateWindowMeta], self_pid: u32, monitor_rect: RectI32) -> Vec<usize>` (returns `hwnd_id`s in **bottom-to-top** painter order: starts at the lowest visible third-party window with `affinity != 0` on `monitor_rect`, and includes that window plus any visible third-party window above it in Z-order whose rect intersects the union of composited protected window rects, while strictly excluding `pid == self_pid`).

- [ ] **Step 1: Write the failing unit tests in `src-tauri/src/screenshot_win32.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_bgra_detection_rejects_all_black_or_empty_and_accepts_content() {
        assert!(!is_non_blank_bgra(&[]));
        // 4 pixels of pure black with opaque alpha (B=0, G=0, R=0, A=255)
        let black = [0u8, 0, 0, 255].repeat(4);
        assert!(!is_non_blank_bgra(&black));

        // 1 pixel with non-zero RGB
        let mut with_pixel = black.clone();
        with_pixel[2] = 12; // R = 12
        assert!(is_non_blank_bgra(&with_pixel));
    }

    #[test]
    fn bgra_to_rgba_swaps_red_and_blue_channels_and_sets_opaque_alpha() {
        let mut buf = vec![10u8, 20, 30, 0, 40, 50, 60, 128];
        bgra_to_rgba_in_place(&mut buf);
        assert_eq!(buf, vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }

    #[test]
    fn composite_bgra_clips_negative_and_overflow_coordinates_across_monitors() {
        // 4x4 monitor canvas located at (-4, 0) in virtual desktop space
        let mut canvas = vec![0u8; 4 * 4 * 4];
        // 3x3 window at (-5, 2) -> overlaps monitor at x=[-4..-2] (canvas x=0..2), y=[2..4] (canvas y=2..4)
        let src_pixel = [100u8, 150, 200, 255];
        let src = src_pixel.repeat(3 * 3);
        composite_bgra_region(&mut canvas, 4, 4, (-4, 0), &src, 3, 3, (-5, 2));

        // Canvas pixel (0, 2) and (1, 2) should be updated; (2, 2) untouched
        let idx_0_2 = (2 * 4 + 0) * 4;
        let idx_1_2 = (2 * 4 + 1) * 4;
        let idx_2_2 = (2 * 4 + 2) * 4;
        assert_eq!(&canvas[idx_0_2..idx_0_2 + 4], &src_pixel);
        assert_eq!(&canvas[idx_1_2..idx_1_2 + 4], &src_pixel);
        assert_eq!(&canvas[idx_2_2..idx_2_2 + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn select_windows_excludes_self_pid_and_preserves_occlusion_above_protected_window() {
        let monitor = RectI32 { left: 0, top: 0, right: 1920, bottom: 1080 };
        let self_pid = 4242;
        // EnumWindows order: top-to-bottom
        let top_to_bottom = vec![
            // 1. Our own CrackingInterview overlay on top (MUST be excluded even if affinity != 0)
            CandidateWindowMeta {
                hwnd_id: 10,
                pid: self_pid,
                affinity: 0x11,
                rect: RectI32 { left: 100, top: 100, right: 500, bottom: 500 },
                is_visible: true,
                is_iconic: false,
                is_cloaked: false,
            },
            // 2. Normal third-party window overlapping the protected window (must re-composite on top of #3)
            CandidateWindowMeta {
                hwnd_id: 20,
                pid: 9001,
                affinity: 0,
                rect: RectI32 { left: 200, top: 200, right: 600, bottom: 600 },
                is_visible: true,
                is_iconic: false,
                is_cloaked: false,
            },
            // 3. Third-party WDA_EXCLUDEFROMCAPTURE window
            CandidateWindowMeta {
                hwnd_id: 30,
                pid: 8888,
                affinity: 0x11,
                rect: RectI32 { left: 150, top: 150, right: 450, bottom: 450 },
                is_visible: true,
                is_iconic: false,
                is_cloaked: false,
            },
            // 4. Background normal window below the protected window (already in Stage 1 BitBlt, skip)
            CandidateWindowMeta {
                hwnd_id: 40,
                pid: 7777,
                affinity: 0,
                rect: RectI32 { left: 0, top: 0, right: 1920, bottom: 1080 },
                is_visible: true,
                is_iconic: false,
                is_cloaked: false,
            },
        ];

        // Expected bottom-to-top painter order: first 30 (protected), then 20 (occluding normal window above it)
        let selected = select_windows_to_composite(&top_to_bottom, self_pid, monitor);
        assert_eq!(selected, vec![30, 20]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml screenshot_win32`
Expected: FAIL (module/functions not yet implemented).

- [ ] **Step 3: Implement the pure functions in `src-tauri/src/screenshot_win32.rs` and declare `mod screenshot_win32;` in `src-tauri/src/main.rs`**

Implement `RectI32` (with `width()`, `height()`, `intersects(&self, other: &RectI32) -> bool`), `CandidateWindowMeta`, `is_non_blank_bgra`, `bgra_to_rgba_in_place` (swapping `B` and `R` and setting `A = 255` so GDI's `0`-alpha RGB32 output is opaque in `RgbaImage`), `composite_bgra_region`, and `select_windows_to_composite`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml screenshot_win32`
Expected: PASS (4 passed).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/screenshot_win32.rs src-tauri/src/main.rs
git commit -m "feat(screenshot): add legacy GDI composition and self-exclusion helpers"
```

---

### Task 2: Windows Legacy GDI Screen-DC & Per-Window Capture Implementation (`src-tauri/src/screenshot_win32.rs`)

Implement the Win32 GDI monitor enumeration, `SelfCaptureExclusionGuard`, Stage 1 `BitBlt(..., SRCCOPY | CAPTUREBLT)` screen-DC capture, and Stage 2 per-window fallback capture (`PrintWindow` / `WM_PRINT` / `GetWindowDC`) inside `#[cfg(target_os = "windows")]`.

**Files:**
- Modify: `src-tauri/src/screenshot_win32.rs`

**Interfaces:**
- Consumes: `RectI32`, `CandidateWindowMeta`, `is_non_blank_bgra`, `bgra_to_rgba_in_place`, `composite_bgra_region`, `select_windows_to_composite` from Task 1; `crate::screenshot::DisplayInfo`.
- Produces (`#[cfg(target_os = "windows")]`):
  - `pub struct RawRgbaFrame { pub width: u32, pub height: u32, pub rgba: Vec<u8> }`
  - `pub fn get_all_displays_win32() -> Result<Vec<crate::screenshot::DisplayInfo>, String>`
  - `pub fn capture_display_legacy_gdi(display_id: &str) -> Result<RawRgbaFrame, String>`

- [ ] **Step 1: Implement Win32 monitor enumeration and `SelfCaptureExclusionGuard` in `src-tauri/src/screenshot_win32.rs`**

Under `#[cfg(target_os = "windows")]`:
- Define `CAPTUREBLT_ROP: ROP_CODE = ROP_CODE(SRCCOPY.0 | 0x40000000);` (`0x40CC0020`).
- Declare raw FFI for `PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> BOOL` and `DwmGetWindowAttribute(hwnd: HWND, dw_attribute: u32, pv_attribute: *mut std::ffi::c_void, cb_attribute: u32) -> windows::core::HRESULT` (`#[link(name = "user32")]` / `#[link(name = "dwmapi")]`) so no extra `windows` crate feature flags are needed.
- Implement `enumerate_monitors_win32() -> Result<Vec<Win32MonitorTarget>, String>` using `EnumDisplayMonitors` + `GetMonitorInfoW`:
  - Sort monitors deterministically (primary display first or in `EnumDisplayMonitors` order matching `get_all_displays_win32()`).
  - Resolve physical dimensions via `CreateDCW(PCWSTR(szDevice.as_ptr()), ...)` + `GetDeviceCaps(hdc, DESKTOPHORZRES (118))` / `GetDeviceCaps(hdc, DESKTOPVERTRES (117))` (falling back to `rcMonitor.right - rcMonitor.left` / `rcMonitor.bottom - rcMonitor.top` if zero).
- Implement `SelfCaptureExclusionGuard`:
  - On creation (`SelfCaptureExclusionGuard::acquire()`), enumerate top-level `HWND`s owned by `std::process::id()`. For each visible `HWND` whose `GetWindowDisplayAffinity` is `0` (`WDA_NONE`), call `SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11))` and store `hwnd` in `restored_hwnds: Vec<isize>`.
  - In `impl Drop for SelfCaptureExclusionGuard`, for each stored `hwnd`, if `!crate::STEALTH_ENABLED.load(Ordering::Relaxed)`, restore `SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0))`.

- [ ] **Step 2: Implement Stage 1 (`capture_monitor_dc_bitblt`) and Stage 2 (`capture_window_surface_gdi` + Z-order composition) in `src-tauri/src/screenshot_win32.rs`**

- **Stage 1 (`capture_monitor_dc_bitblt`):**
  - Hold `_self_guard = SelfCaptureExclusionGuard::acquire();`.
  - Create monitor `HDC` via `CreateDCW(szDevice, ...)` (fallback `GetDC(HWND::default())` with virtual screen offset `(rcMonitor.left, rcMonitor.top)`).
  - Create compatible memory `HDC` (`CreateCompatibleDC`), compatible bitmap (`CreateCompatibleBitmap(hdc_src, phys_w, phys_h)`), select it into `hdc_mem`, and call:
    ```rust
    BitBlt(
        hdc_mem,
        0,
        0,
        phys_w,
        phys_h,
        hdc_src,
        src_x,
        src_y,
        ROP_CODE(SRCCOPY.0 | 0x40000000), // SRCCOPY | CAPTUREBLT
    )
    ```
  - Extract top-down 32-bit BGRA pixels via `GetDIBits` (`biHeight = -phys_h`, `biBitCount = 32`, `BI_RGB`).
- **Stage 2 (`composite_unfiltered_windows`):**
  - Enumerate top-level windows with `EnumWindows`, building `Vec<CandidateWindowMeta>` with `GetWindowThreadProcessId`, `IsWindowVisible`, `IsIconic`, `DwmGetWindowAttribute(..., DWMWA_CLOAKED = 14, ...)`, `GetWindowDisplayAffinity`, and `GetWindowRect`.
  - Call `select_windows_to_composite(&candidates, std::process::id(), monitor.virtual_rect)` to get the ordered `HWND` list.
  - For each selected `HWND`, attempt `capture_single_window_bgra(hwnd, w, h)` using a top-down 32-bpp DIB section in this fallback order until `is_non_blank_bgra(&buf)` is true:
    1. `PrintWindow(hwnd, hdc_win_mem, 0x00000002)` (`PW_RENDERFULLCONTENT`)
    2. `PrintWindow(hwnd, hdc_win_mem, 0)`
    3. `SendMessageTimeoutW(hwnd, WM_PRINT (0x0317), WPARAM(hdc_win_mem.0 as usize), LPARAM(0x36 /* PRF_NONCLIENT | PRF_CLIENT | PRF_ERASEBKGND | PRF_CHILDREN */), SMTO_ABORTIFHUNG, 150, None)`
    4. `GetWindowDC(hwnd)` + `BitBlt(hdc_win_mem, 0, 0, w, h, hdc_win, 0, 0, ROP_CODE(SRCCOPY.0 | 0x40000000))`
  - Scale or map window coordinates to the monitor's physical pixel space if `scale_x` / `scale_y` != 1.0, and call `composite_bgra_region`.
  - Finally call `bgra_to_rgba_in_place(&mut canvas)` and return `RawRgbaFrame { width, height, rgba: canvas }`.

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml screenshot_win32`
Expected: PASS on current machine with zero warnings.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/screenshot_win32.rs
git commit -m "feat(screenshot): implement Win32 legacy GDI SRCCOPY|CAPTUREBLT + PrintWindow pipeline"
```

---

### Task 3: Wire `src-tauri/src/screenshot.rs` to Legacy GDI Pipeline & Add Encoding Tests

Refactor `src-tauri/src/screenshot.rs` so both `capture_display_screenshot` and `capture_display_thumbnail` share clean, tested RGBA-to-JPEG encoding functions and route Windows calls to `screenshot_win32`.

**Files:**
- Modify: `src-tauri/src/screenshot.rs`

**Interfaces:**
- Consumes: `crate::screenshot_win32::{get_all_displays_win32, capture_display_legacy_gdi}` on `#[cfg(target_os = "windows")]`.
- Produces:
  - `pub fn encode_rgba_as_screenshot_jpeg(width: u32, height: u32, rgba_bytes: Vec<u8>) -> Result<Vec<u8>, String>`
  - `pub fn encode_rgba_as_thumbnail_jpeg(width: u32, height: u32, rgba_bytes: Vec<u8>) -> Result<Vec<u8>, String>`
  - Existing public API preserved: `get_all_displays()`, `capture_display_screenshot(display_id: &str)`, `capture_display_thumbnail(display_id: &str)`, `request_screen_recording_permission()`.

- [ ] **Step 1: Write the failing unit tests in `src-tauri/src/screenshot.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_rgba_as_screenshot_jpeg_downscales_large_frames_and_produces_valid_jpeg() {
        let width = 2400u32;
        let height = 1200u32;
        let rgba = vec![180u8; (width * height * 4) as usize];
        let jpeg = encode_rgba_as_screenshot_jpeg(width, height, rgba).expect("jpeg encode");
        // JPEG magic bytes: FF D8 FF
        assert!(jpeg.len() > 3 && jpeg[0] == 0xFF && jpeg[1] == 0xD8 && jpeg[2] == 0xFF);
        let decoded = image::load_from_memory(&jpeg).expect("decode jpeg");
        assert_eq!(decoded.width(), 2000);
        assert_eq!(decoded.height(), 1000);
    }

    #[test]
    fn encode_rgba_as_thumbnail_jpeg_fits_within_160x120() {
        let width = 1920u32;
        let height = 1080u32;
        let rgba = vec![90u8; (width * height * 4) as usize];
        let jpeg = encode_rgba_as_thumbnail_jpeg(width, height, rgba).expect("thumb encode");
        let decoded = image::load_from_memory(&jpeg).expect("decode thumb");
        assert!(decoded.width() <= 160 && decoded.height() <= 120);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml screenshot::tests`
Expected: FAIL (`encode_rgba_as_screenshot_jpeg` not found).

- [ ] **Step 3: Implement shared encoding helpers and platform dispatch in `src-tauri/src/screenshot.rs`**

- Extract `encode_rgba_as_screenshot_jpeg` (2000px max dimension resize, 4.5MB fallback to 1500px) and `encode_rgba_as_thumbnail_jpeg` (160x120 resize) so Windows and macOS use identical post-processing.
- In `get_all_displays()`:
  - `#[cfg(target_os = "windows")]` calls `crate::screenshot_win32::get_all_displays_win32()`.
  - `#[cfg(not(target_os = "windows"))]` keeps the existing `screenshots::Screen::all()` logic.
- In `capture_display_screenshot(display_id: &str)` and `capture_display_thumbnail(display_id: &str)`:
  - `#[cfg(target_os = "windows")]` calls `crate::screenshot_win32::capture_display_legacy_gdi(display_id)` and passes `(frame.width, frame.height, frame.rgba)` to `encode_rgba_as_screenshot_jpeg` / `encode_rgba_as_thumbnail_jpeg`.
  - `#[cfg(not(target_os = "windows"))]` captures via `screenshots::Screen::all()` and passes `image.rgba().to_vec()` to the same helpers.

- [ ] **Step 4: Run tests and update graphify**

Run: `cargo test --manifest-path src-tauri/Cargo.toml screenshot && graphify update .`
Expected: PASS (all `screenshot` and `screenshot_win32` tests pass, `cargo check` clean).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/screenshot.rs
git commit -m "feat(screenshot): wire Windows display capture and thumbnails to legacy GDI pipeline"
```
