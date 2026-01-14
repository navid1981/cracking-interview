// Screen capture module using screenshots crate

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
    
    // On first capture, this triggers macOS permission dialog
    #[cfg(target_os = "macos")]
    {
        // Trigger permission prompt by attempting capture
        println!("⚠️  If thumbnails show only wallpaper, grant Screen Recording permission:");
        println!("   System Settings → Privacy & Security → Screen Recording");
        println!("   Enable 'cracking-interview' then restart app");
    }
    
    let index: usize = display_id.parse()
        .map_err(|_| "Invalid display ID")?;
    
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
    
    // Resize to thumbnail (160x120)
    let thumbnail = dynamic_image.resize(
        160,
        120,
        image::imageops::FilterType::Lanczos3
    );
    
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
    use std::process::Command;
    
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
