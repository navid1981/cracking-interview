// Windows-only capture backends: DXGI Desktop Duplication and Windows.Graphics.Capture.

use crate::capture_backend::Frame;
use std::time::{Duration, Instant};
use windows::core::{factory, Interface};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{BOOL, HMODULE, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

fn e2s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

unsafe extern "system" fn monitor_cb(hmon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let list = &mut *(data.0 as *mut Vec<HMONITOR>);
    list.push(hmon);
    BOOL(1)
}

/// Monitor handle for display `index` (EnumDisplayMonitors order, same as the `screenshots` crate).
fn monitor_for_index(index: usize) -> Result<HMONITOR, String> {
    let mut list: Vec<HMONITOR> = Vec::new();
    let ok = unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(monitor_cb),
            LPARAM(&mut list as *mut _ as isize),
        )
    };
    if !ok.as_bool() {
        return Err("EnumDisplayMonitors failed".into());
    }
    list.get(index).copied().ok_or_else(|| "Display not found".to_string())
}

fn create_device(
    adapter: Option<&IDXGIAdapter>,
) -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    let driver = if adapter.is_some() {
        D3D_DRIVER_TYPE_UNKNOWN
    } else {
        D3D_DRIVER_TYPE_HARDWARE
    };
    let mut device: Option<ID3D11Device> = None;
    let mut ctx: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            adapter,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut ctx),
        )
    }
    .map_err(|e| format!("D3D11CreateDevice: {}", e))?;
    Ok((
        device.ok_or("D3D11 device missing")?,
        ctx.ok_or("D3D11 context missing")?,
    ))
}

/// Copy a GPU texture into CPU memory as RGBA8.
fn read_texture(
    device: &ID3D11Device,
    ctx: &ID3D11DeviceContext,
    tex: &ID3D11Texture2D,
) -> Result<Frame, String> {
    unsafe {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        tex.GetDesc(&mut desc);
        if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
            return Err(format!("unsupported pixel format {}", desc.Format.0));
        }
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;

        let mut staging: Option<ID3D11Texture2D> = None;
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .map_err(|e| format!("CreateTexture2D: {}", e))?;
        let staging = staging.ok_or("staging texture missing")?;
        ctx.CopyResource(&staging, tex);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| format!("Map: {}", e))?;

        let (w, h) = (desc.Width as usize, desc.Height as usize);
        let pitch = mapped.RowPitch as usize;
        let src = std::slice::from_raw_parts(mapped.pData as *const u8, pitch * h);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            let row = &src[y * pitch..y * pitch + w * 4];
            let dst = &mut rgba[y * w * 4..(y + 1) * w * 4];
            for x in 0..w {
                dst[x * 4] = row[x * 4 + 2];
                dst[x * 4 + 1] = row[x * 4 + 1];
                dst[x * 4 + 2] = row[x * 4];
                dst[x * 4 + 3] = 255;
            }
        }
        ctx.Unmap(&staging, 0);

        Ok(Frame { width: desc.Width, height: desc.Height, rgba })
    }
}

// ---------------------------------------------------------------------------
// DXGI Desktop Duplication
// ---------------------------------------------------------------------------

pub fn capture_dxgi(index: usize) -> Result<Frame, String> {
    let target = monitor_for_index(index)?;
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(e2s)?;
        let mut ai = 0;
        while let Ok(adapter) = factory.EnumAdapters1(ai) {
            ai += 1;
            let mut oi = 0;
            while let Ok(output) = adapter.EnumOutputs(oi) {
                oi += 1;
                let desc = output.GetDesc().map_err(e2s)?;
                if desc.Monitor != target {
                    continue;
                }
                let base_adapter: IDXGIAdapter = adapter.cast().map_err(e2s)?;
                let (device, ctx) = create_device(Some(&base_adapter))?;
                let output1: IDXGIOutput1 = output.cast().map_err(e2s)?;
                return duplicate_frame(&device, &ctx, &output1);
            }
        }
    }
    Err("No DXGI output matches the selected display".into())
}

fn duplicate_frame(
    device: &ID3D11Device,
    ctx: &ID3D11DeviceContext,
    output: &IDXGIOutput1,
) -> Result<Frame, String> {
    unsafe {
        let dup = output
            .DuplicateOutput(device)
            .map_err(|e| format!("DuplicateOutput: {}", e))?;

        let deadline = Instant::now() + Duration::from_millis(1500);
        let mut best: Option<Frame> = None;
        loop {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut res: Option<IDXGIResource> = None;
            match dup.AcquireNextFrame(100, &mut info, &mut res) {
                Ok(()) => {
                    let converted = res
                        .ok_or_else(|| "no desktop resource".to_string())
                        .and_then(|r| r.cast::<ID3D11Texture2D>().map_err(e2s))
                        .and_then(|t| read_texture(device, ctx, &t));
                    let _ = dup.ReleaseFrame();
                    let frame = converted?;
                    // Frames with LastPresentTime == 0 only carry cursor updates; prefer a real present.
                    if info.LastPresentTime != 0 {
                        return Ok(frame);
                    }
                    best = Some(frame);
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {}
                Err(e) => return Err(format!("AcquireNextFrame: {}", e)),
            }
            if Instant::now() >= deadline {
                return best.ok_or_else(|| "timed out waiting for a desktop frame".to_string());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Windows.Graphics.Capture
// ---------------------------------------------------------------------------

pub fn capture_wgc(index: usize) -> Result<Frame, String> {
    let hmon = monitor_for_index(index)?;
    unsafe {
        // WinRT needs an initialised apartment on this worker thread. Ignore "already initialised".
        let _ = RoInitialize(RO_INIT_MULTITHREADED);

        if !GraphicsCaptureSession::IsSupported().map_err(e2s)? {
            return Err("Windows.Graphics.Capture is not supported on this system".into());
        }

        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(e2s)?;
        let item: GraphicsCaptureItem = interop
            .CreateForMonitor(hmon)
            .map_err(|e| format!("CreateForMonitor: {}", e))?;

        let (device, ctx) = create_device(None)?;
        let dxgi_device: IDXGIDevice = device.cast().map_err(e2s)?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device).map_err(e2s)?;
        let d3d_device: IDirect3DDevice = inspectable.cast().map_err(e2s)?;

        let size = item.Size().map_err(e2s)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            size,
        )
        .map_err(|e| format!("CreateFreeThreaded: {}", e))?;
        let session = pool.CreateCaptureSession(&item).map_err(e2s)?;
        // Best effort: hide cursor and the yellow capture border (border opt-out needs Win11 / consent).
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().map_err(|e| format!("StartCapture: {}", e))?;

        let deadline = Instant::now() + Duration::from_millis(3000);
        let result = loop {
            if let Ok(frame) = pool.TryGetNextFrame() {
                let r = (|| -> Result<Frame, String> {
                    let surface = frame.Surface().map_err(e2s)?;
                    let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(e2s)?;
                    let tex: ID3D11Texture2D = access.GetInterface().map_err(e2s)?;
                    read_texture(&device, &ctx, &tex)
                })();
                let _ = frame.Close();
                break r;
            }
            if Instant::now() >= deadline {
                break Err("timed out waiting for a capture frame".to_string());
            }
            std::thread::sleep(Duration::from_millis(5));
        };

        let _ = session.Close();
        let _ = pool.Close();
        result
    }
}
