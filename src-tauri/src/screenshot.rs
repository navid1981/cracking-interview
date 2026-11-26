// Screen capture module using ScreenCaptureKit (macOS) and screenshots crate (fallback / Windows)

#[cfg(target_os = "macos")]
mod macos_capture {
    use std::path::PathBuf;
    use std::process::Command;

    pub(crate) fn find_helper_binary() -> Result<PathBuf, String> {
        // 1. Check inside the .app bundle: ../Resources/screen_capturer_bin
        if let Ok(exe) = std::env::current_exe() {
            let bundle_path = exe
                .parent()                     // .app/Contents/MacOS/
                .and_then(|p| p.parent())      // .app/Contents/
                .map(|p| p.join("Resources").join("screen_capturer_bin"));
            if let Some(ref path) = bundle_path {
                if path.exists() {
                    ensure_executable(path);
                    return Ok(path.clone());
                }
            }
        }

        // 2. Check next to the executable (Tauri dev mode places resources here)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let dev_path = dir.join("screen_capturer_bin");
                if dev_path.exists() {
                    ensure_executable(&dev_path);
                    return Ok(dev_path);
                }
            }
        }

        // 3. Check resources/ directory relative to current working directory
        let rel_path = PathBuf::from("resources/screen_capturer_bin");
        if rel_path.exists() {
            ensure_executable(&rel_path);
            return Ok(rel_path);
        }

        // 4. Fall back: compile from embedded source (requires Xcode CLI tools — dev only)
        compile_helper_from_source()
    }

    fn ensure_executable(path: &PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            let mode = perms.mode();
            if mode & 0o111 == 0 {
                perms.set_mode(mode | 0o755);
                let _ = std::fs::set_permissions(path, perms);
            }
        }
    }

    fn compile_helper_from_source() -> Result<PathBuf, String> {
        let mut bin = std::env::temp_dir();
        bin.push("cracking_interview_screen_capturer");
        let mut src = std::env::temp_dir();
        src.push("cracking_interview_screen_capturer.swift");

        let current_source = include_str!("../resources/screen_capturer.swift");

        let source_changed = match std::fs::read_to_string(&src) {
            Ok(existing) => existing != current_source,
            Err(_) => true,
        };

        if source_changed {
            std::fs::write(&src, current_source)
                .map_err(|e| format!("Failed to write Swift helper source: {e}"))?;
        }

        if bin.exists() && !source_changed {
            return Ok(bin);
        }

        let output = Command::new("xcrun")
            .args(["swiftc", "-parse-as-library", "-O", "-o"])
            .arg(&bin)
            .arg(&src)
            .args(["-framework", "Foundation", "-framework", "Cocoa",
                   "-framework", "CoreGraphics", "-framework", "ScreenCaptureKit"])
            .output()
            .map_err(|e| format!("Failed to run xcrun swiftc: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Swift compile failed: {}", stderr.trim()));
        }

        Ok(bin)
    }

    pub fn capture_display_sc_kit(
        display_index: usize,
        display_id: Option<u32>,
        is_thumbnail: bool,
    ) -> Result<Vec<u8>, String> {
        let helper = find_helper_binary()?;
        let my_pid = std::process::id();

        let mut temp_file = std::env::temp_dir();
        let fname = format!(
            "ci_cap_{}_{}_{}.jpg",
            display_index,
            if is_thumbnail { "thumb" } else { "full" },
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        temp_file.push(fname);

        let mut cmd = Command::new(&helper);
        cmd.arg("--display-index").arg(display_index.to_string())
            .arg("--out").arg(&temp_file)
            .arg("--exclude-pid").arg(my_pid.to_string())
            .arg("--unhide-hidden");

        if let Some(id) = display_id {
            cmd.arg("--display-id").arg(id.to_string());
        }

        if is_thumbnail {
            cmd.arg("--thumbnail");
        } else {
            cmd.arg("--max-dim").arg("2000");
        }

        let output = cmd.output()
            .map_err(|e| format!("Failed to execute screen capture helper: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = std::fs::remove_file(&temp_file);
            return Err(format!("Screen capture helper failed: {}", stderr.trim()));
        }

        let bytes = std::fs::read(&temp_file)
            .map_err(|e| format!("Failed to read captured image: {}", e))?;

        let _ = std::fs::remove_file(&temp_file);

        if bytes.is_empty() {
            return Err("Captured image file is empty".to_string());
        }

        Ok(bytes)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_main: bool,
}

pub fn get_all_displays() -> Result<Vec<DisplayInfo>, String> {
    let screens = screenshots::Screen::all()
        .map_err(|e| format!("Failed to get displays: {}", e))?;

    let displays: Vec<DisplayInfo> = screens
        .into_iter()
        .enumerate()
        .map(|(index, screen)| {
            DisplayInfo {
                id: index.to_string(),
                name: format!("Display {}", index + 1),
                width: screen.display_info.width,
                height: screen.display_info.height,
                is_main: screen.display_info.is_primary,
            }
        })
        .collect();

    Ok(displays)
}

pub fn capture_display_screenshot(display_id: &str) -> Result<Vec<u8>, String> {
    let index: usize = display_id.parse()
        .map_err(|_| "Invalid display ID")?;

    #[cfg(target_os = "macos")]
    {
        // On macOS, use native ScreenCaptureKit:
        // 1. Temporarily reveals hidden apps so hidden windows are captured (like Zoom without window filtering)
        // 2. Completely excludes our own app window (alwaysOnTop) from the screenshot
        let target_id = screenshots::Screen::all()
            .ok()
            .and_then(|screens| screens.get(index).map(|s| s.display_info.id));

        match macos_capture::capture_display_sc_kit(index, target_id, false) {
            Ok(bytes) => {
                println!("📸 Captured display screenshot via ScreenCaptureKit (hidden apps revealed, app excluded, size: {} bytes)", bytes.len());
                return Ok(bytes);
            }
            Err(e) => {
                println!("⚠️ Native ScreenCaptureKit capture failed ({}), falling back to screenshots crate", e);
            }
        }
    }

    let screens = screenshots::Screen::all()
        .map_err(|e| format!("Failed to get displays: {}", e))?;

    let screen = screens.get(index)
        .ok_or("Display not found")?;

    let image = screen.capture()
        .map_err(|e| format!("Failed to capture: {}", e))?;

    println!("📸 Original: {}x{}", image.width(), image.height());

    // Convert Image to DynamicImage
    use image::{DynamicImage, RgbaImage};
    let rgba = RgbaImage::from_raw(
        image.width(),
        image.height(),
        image.rgba().to_vec()
    ).ok_or("Failed to create image")?;

    let mut dynamic_image = DynamicImage::ImageRgba8(rgba);

    // Resize if too large (max 2000px on longest side to stay under 5MB)
    let max_dimension = 2000;
    if image.width() > max_dimension || image.height() > max_dimension {
        let scale = (max_dimension as f32) / image.width().max(image.height()) as f32;
        let new_width = (image.width() as f32 * scale) as u32;
        let new_height = (image.height() as f32 * scale) as u32;

        println!("📏 Resizing to {}x{} to reduce file size", new_width, new_height);
        dynamic_image = dynamic_image.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
    }

    // Encode as JPEG
    let mut jpeg_bytes = Vec::new();
    dynamic_image
        .write_to(
            &mut std::io::Cursor::new(&mut jpeg_bytes),
            image::ImageFormat::Jpeg
        )
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    println!("✅ Compressed to {} bytes ({:.2} MB)", jpeg_bytes.len(), jpeg_bytes.len() as f64 / 1_000_000.0);

    // If still too large, resize more aggressively
    if jpeg_bytes.len() > 4_500_000 {
        println!("⚠️  Still too large, resizing to 1500px");
        jpeg_bytes.clear();

        let smaller = dynamic_image.resize(1500, 1500, image::imageops::FilterType::Lanczos3);
        smaller
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg_bytes),
                image::ImageFormat::Jpeg
            )
            .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

        println!("✅ Re-compressed to {} bytes ({:.2} MB)", jpeg_bytes.len(), jpeg_bytes.len() as f64 / 1_000_000.0);
    }

    Ok(jpeg_bytes)
}

pub fn capture_display_thumbnail(display_id: &str) -> Result<Vec<u8>, String> {
    println!("📸 Capturing thumbnail for display: {}", display_id);

    let index: usize = display_id.parse()
        .map_err(|_| "Invalid display ID")?;

    #[cfg(target_os = "macos")]
    {
        let target_id = screenshots::Screen::all()
            .ok()
            .and_then(|screens| screens.get(index).map(|s| s.display_info.id));

        match macos_capture::capture_display_sc_kit(index, target_id, true) {
            Ok(bytes) => {
                println!("✅ Thumbnail captured via ScreenCaptureKit ({} bytes)", bytes.len());
                return Ok(bytes);
            }
            Err(e) => {
                println!("⚠️ ScreenCaptureKit thumbnail failed ({}), falling back to screenshots crate", e);
            }
        }
    }

    // On first capture, this triggers macOS permission dialog
    #[cfg(target_os = "macos")]
    {
        // Trigger permission prompt by attempting capture
        println!("⚠️  If thumbnails show only wallpaper, grant Screen Recording permission:");
        println!("   System Settings → Privacy & Security → Screen Recording");
        println!("   Enable 'cracking-interview' then restart app");
    }

    let screens = screenshots::Screen::all()
        .map_err(|e| format!("Failed to get displays: {}", e))?;

    println!("📊 Total screens available: {}", screens.len());

    let screen = screens.get(index)
        .ok_or("Display not found")?;

    println!("📸 Capturing screen {} ({}x{})", index, screen.display_info.width, screen.display_info.height);

    let image = screen.capture()
        .map_err(|e| format!("Failed to capture: {}", e))?;

    println!("✅ Image captured: {}x{} pixels", image.width(), image.height());

    // Create RGBA image
    use image::{DynamicImage, RgbaImage};
    let rgba = RgbaImage::from_raw(
        image.width(),
        image.height(),
        image.rgba().to_vec()
    ).ok_or("Failed to create image")?;

    let dynamic_image = DynamicImage::ImageRgba8(rgba);

    // Resize to thumbnail (160x120). `thumbnail()` is ~7x faster than a Lanczos3 resize on a
    // 4K frame (0.5s vs 3.6s in a debug build), which kept it under the UI's 5s timeout.
    let thumbnail = dynamic_image.thumbnail(160, 120);

    println!("✅ Thumbnail resized to 160x120");

    // Convert to JPEG bytes (smaller file size)
    let mut jpeg_bytes = Vec::new();
    thumbnail
        .write_to(&mut std::io::Cursor::new(&mut jpeg_bytes), image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    println!("✅ Thumbnail encoded, size: {} bytes", jpeg_bytes.len());

    Ok(jpeg_bytes)
}
// Screen Recording Permission Manager for macOS

#[cfg(target_os = "macos")]
pub fn request_screen_recording_permission() -> bool {
    println!("🔐 Checking Screen Recording permission...");
    
    // Try to capture a screen to trigger permission dialog
    match screenshots::Screen::all() {
        Ok(screens) => {
            if let Some(screen) = screens.first() {
                match screen.capture() {
                    Ok(image) => {
                        // Check if we got actual content or just blank/wallpaper
                        let pixels = image.rgba();
                        println!("✅ Screen capture working - {} bytes captured", pixels.len());
                        
                        // If we got data, permission is likely granted
                        if pixels.len() > 1000 {
                            println!("✅ Screen Recording permission appears to be granted");
                            return true;
                        }
                    }
                    Err(e) => {
                        println!("❌ Screen capture failed: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to access screens: {}", e);
        }
    }
    
    // Show instructions to user
    println!("");
    println!("⚠️  ========================================");
    println!("⚠️  SCREEN RECORDING PERMISSION REQUIRED");
    println!("⚠️  ========================================");
    println!("");
    println!("📋 To grant permission:");
    println!("   1. Open System Settings");
    println!("   2. Go to: Privacy & Security → Screen Recording");
    println!("   3. Click the (+) button");
    println!("   4. Navigate to:");
    println!("      /Users/nsalehvaziri/cracking-interview/src-tauri/target/debug/cracking-interview");
    println!("   5. Add the app and enable the checkbox");
    println!("   6. Restart this app");
    println!("");
    println!("🔍 Or run this command to open System Settings:");
    println!("   open 'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture'");
    println!("");
    
    false
}

#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording_permission() -> bool {
    true // Not needed on other platforms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_displays() {
        let displays = get_all_displays().expect("Failed to get displays");
        assert!(!displays.is_empty(), "Expected at least one display");
    }

    #[test]
    fn test_capture_display_screenshot_and_thumbnail() {
        let displays = get_all_displays().expect("Failed to get displays");
        if !displays.is_empty() {
            let screenshot = capture_display_screenshot("0");
            assert!(screenshot.is_ok(), "Screenshot failed: {:?}", screenshot.err());
            let bytes = screenshot.unwrap();
            assert!(!bytes.is_empty(), "Screenshot bytes empty");
            // Check JPEG magic bytes: 0xFF, 0xD8, 0xFF
            assert_eq!(&bytes[0..3], &[0xFF, 0xD8, 0xFF], "Invalid JPEG header");

            let thumb = capture_display_thumbnail("0");
            assert!(thumb.is_ok(), "Thumbnail failed: {:?}", thumb.err());
            let thumb_bytes = thumb.unwrap();
            assert!(!thumb_bytes.is_empty(), "Thumbnail bytes empty");
            assert_eq!(&thumb_bytes[0..3], &[0xFF, 0xD8, 0xFF], "Invalid JPEG header for thumbnail");
        }
    }
}
