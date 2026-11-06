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
    let expected_canvas_len = (canvas_w as usize).saturating_mul(canvas_h as usize).saturating_mul(4);
    let expected_src_len = (src_w as usize).saturating_mul(src_h as usize).saturating_mul(4);
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
