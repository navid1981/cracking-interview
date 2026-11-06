// Windows Legacy GDI Screen-DC & Per-Window Non-Filtering Composition Engine

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectI32 {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectI32 {
    pub fn width(&self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    pub fn intersects(&self, other: &RectI32) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }
}

#[derive(Debug, Clone)]
pub struct CandidateWindowMeta {
    pub hwnd_id: usize,
    pub pid: u32,
    pub affinity: u32,
    pub rect: RectI32,
    pub is_visible: bool,
    pub is_iconic: bool,
    pub is_cloaked: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RawRgbaFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Scales a window's virtual-screen rectangle into the target monitor's physical pixel space.
pub fn scale_rect_to_physical_monitor(
    win_rect: RectI32,
    mon_virtual_rect: RectI32,
    mon_phys_w: u32,
    mon_phys_h: u32,
) -> RectI32 {
    let vw = mon_virtual_rect.width().max(1) as f64;
    let vh = mon_virtual_rect.height().max(1) as f64;
    let scale_x = (mon_phys_w as f64) / vw;
    let scale_y = (mon_phys_h as f64) / vh;

    let rel_left = ((win_rect.left - mon_virtual_rect.left) as f64 * scale_x).round() as i32;
    let rel_top = ((win_rect.top - mon_virtual_rect.top) as f64 * scale_y).round() as i32;
    let rel_right = ((win_rect.right - mon_virtual_rect.left) as f64 * scale_x).round() as i32;
    let rel_bottom = ((win_rect.bottom - mon_virtual_rect.top) as f64 * scale_y).round() as i32;

    RectI32 {
        left: rel_left,
        top: rel_top,
        right: rel_right.max(rel_left),
        bottom: rel_bottom.max(rel_top),
    }
}

/// Returns true if the 32-bit BGRA buffer has at least 1 pixel and contains
/// any non-zero RGB channel byte (avoiding overwriting the base screen-DC image
/// with an all-black buffer when a hardware/DRM surface rejects PrintWindow).
pub fn is_non_blank_bgra(bgra: &[u8]) -> bool {
    if bgra.len() < 4 || bgra.len() % 4 != 0 {
        return false;
    }
    bgra.chunks_exact(4)
        .any(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
}

/// Converts 32-bit GDI BGRA pixels in place to RGBA and sets alpha to 255.
/// GDI BitBlt/GetDIBits on 32-bpp BI_RGB bitmaps leaves the 4th byte as 0 for
/// standard GDI drawing; setting A=255 prevents transparent/blank JPEGs.
pub fn bgra_to_rgba_in_place(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}

/// Composites a 32-bpp BGRA window surface (`src`) onto a monitor BGRA canvas (`canvas`),
/// clipping against negative and overflow coordinates relative to `monitor_origin`.
pub fn composite_bgra_region(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    monitor_origin: (i32, i32),
    src: &[u8],
    src_w: u32,
    src_h: u32,
    window_pos: (i32, i32),
) {
    if canvas_w == 0 || canvas_h == 0 || src_w == 0 || src_h == 0 {
        return;
    }
    let expected_canvas_len = (canvas_w as usize)
        .saturating_mul(canvas_h as usize)
        .saturating_mul(4);
    let expected_src_len = (src_w as usize)
        .saturating_mul(src_h as usize)
        .saturating_mul(4);
    if canvas.len() < expected_canvas_len || src.len() < expected_src_len {
        return;
    }

    let rel_x = window_pos.0 - monitor_origin.0;
    let rel_y = window_pos.1 - monitor_origin.1;

    let dst_x_start = rel_x.max(0);
    let dst_y_start = rel_y.max(0);
    let dst_x_end = (rel_x + src_w as i32).min(canvas_w as i32);
    let dst_y_end = (rel_y + src_h as i32).min(canvas_h as i32);

    if dst_x_start >= dst_x_end || dst_y_start >= dst_y_end {
        return;
    }

    let copy_w_px = (dst_x_end - dst_x_start) as usize;
    let copy_row_bytes = copy_w_px * 4;
    let src_x_start = (dst_x_start - rel_x) as usize;

    for dst_y in dst_y_start..dst_y_end {
        let src_y = (dst_y - rel_y) as usize;
        let dst_offset = ((dst_y as usize) * (canvas_w as usize) + (dst_x_start as usize)) * 4;
        let src_offset = (src_y * (src_w as usize) + src_x_start) * 4;

        canvas[dst_offset..dst_offset + copy_row_bytes]
            .copy_from_slice(&src[src_offset..src_offset + copy_row_bytes]);
    }
}

/// Given top-to-bottom enumerated windows (`EnumWindows` order), selects the `hwnd_id`s
/// that must be composited onto the Stage 1 screen-DC image in **bottom-to-top** painter order:
/// 1. Strictly excludes any window belonging to `self_pid` (`CrackingInterview` itself).
/// 2. Ignores hidden, iconic (minimized), cloaked, zero-area, or non-intersecting windows.
/// 3. Starts from the lowest third-party window with `affinity != 0` (`WDA_MONITOR` or
///    `WDA_EXCLUDEFROMCAPTURE`) and includes any third-party window above it whose rectangle
///    intersects any already-selected composited window (preserving occlusion when a normal
///    window sits on top of a protected window).
pub fn select_windows_to_composite(
    top_to_bottom: &[CandidateWindowMeta],
    self_pid: u32,
    monitor_rect: RectI32,
) -> Vec<usize> {
    let mut selected_hwnds = Vec::new();
    let mut dirty_rects: Vec<RectI32> = Vec::new();

    // Iterate bottom-to-top (reverse of EnumWindows order)
    for win in top_to_bottom.iter().rev() {
        if win.pid == self_pid {
            continue;
        }
        if !win.is_visible || win.is_iconic || win.is_cloaked {
            continue;
        }
        if win.rect.width() <= 0 || win.rect.height() <= 0 {
            continue;
        }
        if !win.rect.intersects(&monitor_rect) {
            continue;
        }

        let is_protected = win.affinity != 0;
        let overlaps_dirty = dirty_rects.iter().any(|r| win.rect.intersects(r));

        if is_protected || overlaps_dirty {
            selected_hwnds.push(win.hwnd_id);
            dirty_rects.push(win.rect);
        }
    }

    selected_hwnds
}

// ============================================================================
// WIN32 LEGACY GDI IMPLEMENTATION
// ============================================================================

#[cfg(target_os = "windows")]
mod win32_impl {
    use super::*;
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;
    use std::sync::atomic::Ordering;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW, DeleteDC, DeleteObject,
        EnumDisplayMonitors, GetDC, GetDIBits, GetDeviceCaps, GetMonitorInfoW, GetWindowDC,
        ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER,
        DIB_RGB_COLORS, GET_DEVICE_CAPS_INDEX, HDC, HMONITOR, MONITORINFOEXW, RGBQUAD, ROP_CODE,
        SRCCOPY, STRETCH_HALFTONE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowDisplayAffinity, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, SendMessageTimeoutW,
        SetWindowDisplayAffinity, GWL_EXSTYLE, SMTO_ABORTIFHUNG, WINDOW_DISPLAY_AFFINITY,
        WS_EX_TRANSPARENT,
    };

    const CAPTUREBLT_BITS: u32 = 0x4000_0000;
    const DESKTOPHORZRES_IDX: u32 = 118;
    const DESKTOPVERTRES_IDX: u32 = 117;
    const MONITORINFOF_PRIMARY_FLAG: u32 = 1;
    const DWMWA_CLOAKED_ATTR: u32 = 14;
    const PW_RENDERFULLCONTENT_FLAG: u32 = 0x0000_0002;
    const WM_PRINT_MSG: u32 = 0x0317;
    // PRF_NONCLIENT (0x02) | PRF_CLIENT (0x04) | PRF_ERASEBKGND (0x08) | PRF_CHILDREN (0x10)
    const PRF_COMPOSITE_FLAGS: isize = 0x02 | 0x04 | 0x08 | 0x10;

    #[link(name = "user32")]
    extern "system" {
        fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> BOOL;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmGetWindowAttribute(
            hwnd: HWND,
            dw_attribute: u32,
            pv_attribute: *mut c_void,
            cb_attribute: u32,
        ) -> i32;
    }

    #[derive(Clone)]
    struct Win32MonitorTarget {
        sz_device: [u16; 32],
        virtual_rect: RectI32,
        phys_width: u32,
        phys_height: u32,
        is_primary: bool,
    }

    extern "system" fn enum_monitors_proc(
        h_monitor: HMONITOR,
        _hdc: HDC,
        _rc: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list = unsafe { &mut *(lparam.0 as *mut Vec<MONITORINFOEXW>) };
        let mut info: MONITORINFOEXW = unsafe { mem::zeroed() };
        info.monitorInfo.cbSize = mem::size_of::<MONITORINFOEXW>() as u32;
        let ok = unsafe { GetMonitorInfoW(h_monitor, (&mut info as *mut MONITORINFOEXW).cast()) };
        if ok.as_bool() {
            list.push(info);
        }
        BOOL(1)
    }

    fn enumerate_monitors_win32() -> Result<Vec<Win32MonitorTarget>, String> {
        let mut raw_monitors: Vec<MONITORINFOEXW> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                HDC::default(),
                None,
                Some(enum_monitors_proc),
                LPARAM(&mut raw_monitors as *mut Vec<MONITORINFOEXW> as isize),
            )
            .ok()
            .map_err(|e| format!("EnumDisplayMonitors failed: {}", e))?;
        }

        let mut targets = Vec::with_capacity(raw_monitors.len());
        for info in raw_monitors {
            let rc = info.monitorInfo.rcMonitor;
            let virtual_rect = RectI32 {
                left: rc.left,
                top: rc.top,
                right: rc.right,
                bottom: rc.bottom,
            };
            let fallback_w = virtual_rect.width().max(1) as u32;
            let fallback_h = virtual_rect.height().max(1) as u32;

            let (phys_width, phys_height) = unsafe {
                let hdc = CreateDCW(
                    PCWSTR(info.szDevice.as_ptr()),
                    PCWSTR(info.szDevice.as_ptr()),
                    PCWSTR(ptr::null()),
                    None,
                );
                if !hdc.is_invalid() {
                    let pw = GetDeviceCaps(hdc, GET_DEVICE_CAPS_INDEX(DESKTOPHORZRES_IDX));
                    let ph = GetDeviceCaps(hdc, GET_DEVICE_CAPS_INDEX(DESKTOPVERTRES_IDX));
                    let _ = DeleteDC(hdc);
                    (
                        if pw > 0 { pw as u32 } else { fallback_w },
                        if ph > 0 { ph as u32 } else { fallback_h },
                    )
                } else {
                    (fallback_w, fallback_h)
                }
            };

            let is_primary = (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY_FLAG) != 0;
            targets.push(Win32MonitorTarget {
                sz_device: info.szDevice,
                virtual_rect,
                phys_width,
                phys_height,
                is_primary,
            });
        }

        Ok(targets)
    }

    /// RAII guard that ensures `CrackingInterview`'s own windows have `WDA_EXCLUDEFROMCAPTURE`
    /// (`0x11`) set while the Stage 1 screen-DC `BitBlt` executes, and restores `WDA_NONE` (`0`)
    /// on drop if Stealth Mode is currently disabled.
    struct SelfCaptureExclusionGuard {
        temporarily_protected_hwnds: Vec<isize>,
    }

    struct SelfWindowCollector {
        self_pid: u32,
        hwnds: Vec<isize>,
    }

    extern "system" fn collect_self_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let collector = unsafe { &mut *(lparam.0 as *mut SelfWindowCollector) };
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == collector.self_pid {
                let mut affinity: u32 = 0;
                if GetWindowDisplayAffinity(hwnd, &mut affinity).is_ok() && affinity == 0 {
                    if SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0x11)).is_ok() {
                        collector.hwnds.push(hwnd.0 as isize);
                    }
                }
            }
        }
        BOOL(1)
    }

    impl SelfCaptureExclusionGuard {
        fn acquire() -> Self {
            let mut collector = SelfWindowCollector {
                self_pid: std::process::id(),
                hwnds: Vec::new(),
            };
            unsafe {
                let _ = EnumWindows(
                    Some(collect_self_windows_proc),
                    LPARAM(&mut collector as *mut SelfWindowCollector as isize),
                );
            }
            Self {
                temporarily_protected_hwnds: collector.hwnds,
            }
        }
    }

    impl Drop for SelfCaptureExclusionGuard {
        fn drop(&mut self) {
            if crate::STEALTH_ENABLED.load(Ordering::Relaxed) {
                return;
            }
            unsafe {
                for raw in &self.temporarily_protected_hwnds {
                    let hwnd = HWND(*raw as *mut c_void);
                    let _ = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0));
                }
            }
        }
    }

    fn read_top_down_bgra(hdc: HDC, hbitmap: windows::Win32::Graphics::Gdi::HBITMAP, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // Top-down DIB: no row reversal needed
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default(); 1],
        };

        let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
        let lines = unsafe {
            GetDIBits(
                hdc,
                hbitmap,
                0,
                height,
                Some(buf.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        };
        if lines == 0 {
            return Err("GetDIBits returned 0 scanlines".to_string());
        }
        Ok(buf)
    }

    /// Stage 1: Legacy GDI Screen-DC BitBlt (`SRCCOPY | CAPTUREBLT`).
    fn capture_monitor_dc_bitblt(mon: &Win32MonitorTarget) -> Result<Vec<u8>, String> {
        let _self_guard = SelfCaptureExclusionGuard::acquire();
        let w = mon.phys_width as i32;
        let h = mon.phys_height as i32;
        let rop = ROP_CODE(SRCCOPY.0 | CAPTUREBLT_BITS);

        unsafe {
            let monitor_dc = CreateDCW(
                PCWSTR(mon.sz_device.as_ptr()),
                PCWSTR(mon.sz_device.as_ptr()),
                PCWSTR(ptr::null()),
                None,
            );

            if !monitor_dc.is_invalid() {
                let mem_dc = CreateCompatibleDC(monitor_dc);
                let hbmp = CreateCompatibleBitmap(monitor_dc, w, h);
                let old_obj = SelectObject(mem_dc, hbmp);

                let blt_res = BitBlt(mem_dc, 0, 0, w, h, monitor_dc, 0, 0, rop);
                let pixels_res = if blt_res.is_ok() {
                    read_top_down_bgra(mem_dc, hbmp, mon.phys_width, mon.phys_height)
                } else {
                    Err(format!("BitBlt on monitor DC failed: {:?}", blt_res.err()))
                };

                SelectObject(mem_dc, old_obj);
                let _ = DeleteObject(hbmp);
                let _ = DeleteDC(mem_dc);
                let _ = DeleteDC(monitor_dc);

                if let Ok(pixels) = pixels_res {
                    return Ok(pixels);
                }
            }

            // Fallback: Virtual-screen desktop DC (`GetDC(HWND(0))`)
            let screen_dc = GetDC(HWND::default());
            if screen_dc.is_invalid() {
                return Err("Failed to acquire screen DC".to_string());
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            let hbmp = CreateCompatibleBitmap(screen_dc, w, h);
            let old_obj = SelectObject(mem_dc, hbmp);
            let _ = SetStretchBltMode(mem_dc, STRETCH_HALFTONE);

            let vw = mon.virtual_rect.width().max(1);
            let vh = mon.virtual_rect.height().max(1);
            let blt_ok = if vw == w && vh == h {
                BitBlt(
                    mem_dc,
                    0,
                    0,
                    w,
                    h,
                    screen_dc,
                    mon.virtual_rect.left,
                    mon.virtual_rect.top,
                    rop,
                )
                .is_ok()
            } else {
                StretchBlt(
                    mem_dc,
                    0,
                    0,
                    w,
                    h,
                    screen_dc,
                    mon.virtual_rect.left,
                    mon.virtual_rect.top,
                    vw,
                    vh,
                    rop,
                )
                .as_bool()
            };

            let res = if blt_ok {
                read_top_down_bgra(mem_dc, hbmp, mon.phys_width, mon.phys_height)
            } else {
                Err("Fallback virtual-screen BitBlt/StretchBlt failed".to_string())
            };

            SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(hbmp);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND::default(), screen_dc);
            res
        }
    }

    fn is_window_cloaked(hwnd: HWND) -> bool {
        let mut cloaked: u32 = 0;
        let hr = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED_ATTR,
                (&mut cloaked as *mut u32).cast(),
                mem::size_of::<u32>() as u32,
            )
        };
        hr == 0 && cloaked != 0
    }

    extern "system" fn enum_candidates_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = unsafe { &mut *(lparam.0 as *mut Vec<CandidateWindowMeta>) };
        unsafe {
            let is_visible = IsWindowVisible(hwnd).as_bool();
            if !is_visible {
                return BOOL(1);
            }
            let is_iconic = IsIconic(hwnd).as_bool();
            if is_iconic {
                return BOOL(1);
            }
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            if (ex_style & WS_EX_TRANSPARENT.0) != 0 {
                return BOOL(1);
            }

            let mut rc = RECT::default();
            if GetWindowRect(hwnd, &mut rc).is_err() {
                return BOOL(1);
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            let mut affinity: u32 = 0;
            let _ = GetWindowDisplayAffinity(hwnd, &mut affinity);

            list.push(CandidateWindowMeta {
                hwnd_id: hwnd.0 as usize,
                pid,
                affinity,
                rect: RectI32 {
                    left: rc.left,
                    top: rc.top,
                    right: rc.right,
                    bottom: rc.bottom,
                },
                is_visible,
                is_iconic,
                is_cloaked: is_window_cloaked(hwnd),
            });
        }
        BOOL(1)
    }

    /// Captures a single window's surface into a top-down BGRA buffer of size `(target_w, target_h)`,
    /// trying legacy GDI window-paint methods in order until a non-blank buffer is obtained.
    fn capture_single_window_bgra(
        hwnd: HWND,
        win_virtual_w: u32,
        win_virtual_h: u32,
        target_w: u32,
        target_h: u32,
    ) -> Option<Vec<u8>> {
        if win_virtual_w == 0 || win_virtual_h == 0 || target_w == 0 || target_h == 0 {
            return None;
        }

        unsafe {
            let screen_dc = GetDC(HWND::default());
            if screen_dc.is_invalid() {
                return None;
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            let hbmp = CreateCompatibleBitmap(
                screen_dc,
                win_virtual_w as i32,
                win_virtual_h as i32,
            );
            let old_obj = SelectObject(mem_dc, hbmp);

            let mut captured: Option<Vec<u8>> = None;

            // Attempt 1: PrintWindow with PW_RENDERFULLCONTENT (0x02)
            if PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT_FLAG).as_bool() {
                if let Ok(buf) = read_top_down_bgra(mem_dc, hbmp, win_virtual_w, win_virtual_h) {
                    if is_non_blank_bgra(&buf) {
                        captured = Some(buf);
                    }
                }
            }

            // Attempt 2: PrintWindow with default flags (0)
            if captured.is_none() && PrintWindow(hwnd, mem_dc, 0).as_bool() {
                if let Ok(buf) = read_top_down_bgra(mem_dc, hbmp, win_virtual_w, win_virtual_h) {
                    if is_non_blank_bgra(&buf) {
                        captured = Some(buf);
                    }
                }
            }

            // Attempt 3: WM_PRINT (non-filtering GDI paint message)
            if captured.is_none() {
                let mut result_ptr: usize = 0;
                let sent = SendMessageTimeoutW(
                    hwnd,
                    WM_PRINT_MSG,
                    WPARAM(mem_dc.0 as usize),
                    LPARAM(PRF_COMPOSITE_FLAGS),
                    SMTO_ABORTIFHUNG,
                    150,
                    Some(&mut result_ptr),
                );
                if sent.0 != 0 {
                    if let Ok(buf) = read_top_down_bgra(mem_dc, hbmp, win_virtual_w, win_virtual_h) {
                        if is_non_blank_bgra(&buf) {
                            captured = Some(buf);
                        }
                    }
                }
            }

            // Attempt 4: Direct GetWindowDC + BitBlt(SRCCOPY | CAPTUREBLT)
            if captured.is_none() {
                let win_dc = GetWindowDC(hwnd);
                if !win_dc.is_invalid() {
                    let ok = BitBlt(
                        mem_dc,
                        0,
                        0,
                        win_virtual_w as i32,
                        win_virtual_h as i32,
                        win_dc,
                        0,
                        0,
                        ROP_CODE(SRCCOPY.0 | CAPTUREBLT_BITS),
                    )
                    .is_ok();
                    let _ = ReleaseDC(hwnd, win_dc);
                    if ok {
                        if let Ok(buf) =
                            read_top_down_bgra(mem_dc, hbmp, win_virtual_w, win_virtual_h)
                        {
                            if is_non_blank_bgra(&buf) {
                                captured = Some(buf);
                            }
                        }
                    }
                }
            }

            // If physical DPI scaling differs from logical window rect, scale the captured bitmap
            let final_buf = match captured {
                Some(raw_buf) if target_w != win_virtual_w || target_h != win_virtual_h => {
                    let scaled_dc = CreateCompatibleDC(screen_dc);
                    let scaled_bmp =
                        CreateCompatibleBitmap(screen_dc, target_w as i32, target_h as i32);
                    let old_scaled = SelectObject(scaled_dc, scaled_bmp);
                    let _ = SetStretchBltMode(scaled_dc, STRETCH_HALFTONE);
                    let ok = StretchBlt(
                        scaled_dc,
                        0,
                        0,
                        target_w as i32,
                        target_h as i32,
                        mem_dc,
                        0,
                        0,
                        win_virtual_w as i32,
                        win_virtual_h as i32,
                        SRCCOPY,
                    )
                    .as_bool();
                    let out = if ok {
                        read_top_down_bgra(scaled_dc, scaled_bmp, target_w, target_h)
                            .ok()
                            .unwrap_or(raw_buf)
                    } else {
                        raw_buf
                    };
                    SelectObject(scaled_dc, old_scaled);
                    let _ = DeleteObject(scaled_bmp);
                    let _ = DeleteDC(scaled_dc);
                    Some(out)
                }
                other => other,
            };

            SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(hbmp);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND::default(), screen_dc);

            final_buf
        }
    }

    pub fn get_all_displays_win32() -> Result<Vec<crate::screenshot::DisplayInfo>, String> {
        let monitors = enumerate_monitors_win32()?;
        Ok(monitors
            .into_iter()
            .enumerate()
            .map(|(index, mon)| crate::screenshot::DisplayInfo {
                id: index.to_string(),
                name: format!("Display {}", index + 1),
                width: mon.phys_width,
                height: mon.phys_height,
                is_main: mon.is_primary,
            })
            .collect())
    }

    pub fn capture_display_legacy_gdi(display_id: &str) -> Result<RawRgbaFrame, String> {
        let index: usize = display_id
            .parse()
            .map_err(|_| "Invalid display ID".to_string())?;
        let monitors = enumerate_monitors_win32()?;
        let mon = monitors.get(index).ok_or_else(|| "Display not found".to_string())?;

        // Stage 1: Legacy GDI Screen-DC BitBlt (SRCCOPY | CAPTUREBLT) with self-exclusion
        let mut canvas_bgra = capture_monitor_dc_bitblt(mon)?;

        // Stage 2: Non-filtering Z-order window composition pass for third-party affinity windows
        let mut candidates: Vec<CandidateWindowMeta> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(enum_candidates_proc),
                LPARAM(&mut candidates as *mut Vec<CandidateWindowMeta> as isize),
            );
        }

        let self_pid = std::process::id();
        let selected_hwnds =
            select_windows_to_composite(&candidates, self_pid, mon.virtual_rect);

        for hwnd_id in selected_hwnds {
            if let Some(meta) = candidates.iter().find(|c| c.hwnd_id == hwnd_id) {
                let win_vw = meta.rect.width() as u32;
                let win_vh = meta.rect.height() as u32;
                let phys_rect = scale_rect_to_physical_monitor(
                    meta.rect,
                    mon.virtual_rect,
                    mon.phys_width,
                    mon.phys_height,
                );
                let target_w = phys_rect.width() as u32;
                let target_h = phys_rect.height() as u32;

                let hwnd = HWND(hwnd_id as *mut c_void);
                if let Some(win_bgra) =
                    capture_single_window_bgra(hwnd, win_vw, win_vh, target_w, target_h)
                {
                    composite_bgra_region(
                        &mut canvas_bgra,
                        mon.phys_width,
                        mon.phys_height,
                        (0, 0),
                        &win_bgra,
                        target_w,
                        target_h,
                        (phys_rect.left, phys_rect.top),
                    );
                }
            }
        }

        bgra_to_rgba_in_place(&mut canvas_bgra);

        Ok(RawRgbaFrame {
            width: mon.phys_width,
            height: mon.phys_height,
            rgba: canvas_bgra,
        })
    }
}

#[cfg(target_os = "windows")]
pub use win32_impl::{capture_display_legacy_gdi, get_all_displays_win32};

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

    #[test]
    fn scale_rect_to_physical_monitor_maps_virtual_coords_to_high_dpi_physical_canvas() {
        // Secondary monitor at (-1280, 0)..(0, 720) in logical coords, 1920x1080 physical (150% DPI)
        let mon_virtual = RectI32 { left: -1280, top: 0, right: 0, bottom: 720 };
        let win_virtual = RectI32 { left: -640, top: 180, right: -320, bottom: 540 };
        let scaled = scale_rect_to_physical_monitor(win_virtual, mon_virtual, 1920, 1080);
        assert_eq!(
            scaled,
            RectI32 { left: 960, top: 270, right: 1440, bottom: 810 }
        );
    }
}
