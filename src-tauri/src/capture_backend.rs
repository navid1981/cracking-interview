// Selectable screen-capture backend.
//
// - Gdi  : legacy path via the `screenshots` crate (GDI BitBlt on Windows). Default, always available.
// - Dxgi : DXGI Desktop Duplication (Windows only).
// - Wgc  : Windows.Graphics.Capture (Windows 10 1903+ only).
//
// On non-Windows platforms every method resolves to the `screenshots` crate.
// If a Windows-only backend fails at runtime we log and fall back to GDI.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    Gdi = 0,
    Dxgi = 1,
    Wgc = 2,
}

impl CaptureMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            CaptureMethod::Gdi => "gdi",
            CaptureMethod::Dxgi => "dxgi",
            CaptureMethod::Wgc => "wgc",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gdi" => Some(CaptureMethod::Gdi),
            "dxgi" => Some(CaptureMethod::Dxgi),
            "wgc" => Some(CaptureMethod::Wgc),
            _ => None,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => CaptureMethod::Dxgi,
            2 => CaptureMethod::Wgc,
            _ => CaptureMethod::Gdi,
        }
    }
}

static METHOD: AtomicU8 = AtomicU8::new(CaptureMethod::Gdi as u8);

pub fn current() -> CaptureMethod {
    CaptureMethod::from_u8(METHOD.load(Ordering::Relaxed))
}

pub fn set(method: CaptureMethod) {
    METHOD.store(method as u8, Ordering::Relaxed);
}

/// Raw RGBA8 frame.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Capture display `index` (same ordering as `screenshot::get_all_displays`)
/// with the currently selected backend.
pub fn capture_rgba(index: usize) -> Result<Frame, String> {
    #[cfg(target_os = "windows")]
    {
        let method = current();
        let result = match method {
            CaptureMethod::Gdi => None,
            CaptureMethod::Dxgi => Some(crate::capture_win::capture_dxgi(index)),
            CaptureMethod::Wgc => Some(crate::capture_win::capture_wgc(index)),
        };
        match result {
            Some(Ok(frame)) => return Ok(frame),
            Some(Err(e)) => println!(
                "⚠️  {} capture failed ({}); falling back to GDI",
                method.as_str(),
                e
            ),
            None => {}
        }
    }
    capture_gdi(index)
}

/// Legacy implementation (unchanged behaviour): `screenshots` crate.
fn capture_gdi(index: usize) -> Result<Frame, String> {
    let screens = screenshots::Screen::all()
        .map_err(|e| format!("Failed to get displays: {}", e))?;
    let screen = screens.get(index).ok_or("Display not found")?;
    let image = screen
        .capture()
        .map_err(|e| format!("Failed to capture: {}", e))?;
    Ok(Frame {
        width: image.width(),
        height: image.height(),
        rgba: image.rgba().to_vec(),
    })
}
