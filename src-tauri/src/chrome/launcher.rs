// Chrome Auto-Launcher with CDP
// Automatically kills existing Chrome and launches with debugging enabled

use std::process::Command;

/// Check if Chrome CDP is accessible
pub async fn is_cdp_accessible() -> bool {
    reqwest::get("http://localhost:9222/json/version")
        .await
        .is_ok()
}

/// Kill all Chrome processes
pub fn kill_chrome() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("pkill")
            .arg("-9")
            .arg("Google Chrome")
            .output()
            .map_err(|e| format!("Failed to kill Chrome: {}", e))?;
        
        // Wait a moment for processes to die
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    
    #[cfg(target_os = "windows")]
    {
        Command::new("taskkill")
            .args(["/F", "/IM", "chrome.exe"])
            .output()
            .ok(); // Ignore errors if Chrome not running
        
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    
    Ok(())
}

/// Launch Chrome with CDP enabled
pub fn launch_chrome_with_cdp() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let chrome_path = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
        
        Command::new(chrome_path)
            .arg("--remote-debugging-port=9222")
            .arg("--user-data-dir=/tmp/cracking-interview-chrome")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .spawn()
            .map_err(|e| format!("Failed to launch Chrome: {}", e))?;
    }
    
    #[cfg(target_os = "windows")]
    {
        // Try common Chrome paths on Windows
        let chrome_paths = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Users\%USERNAME%\AppData\Local\Google\Chrome\Application\chrome.exe",
        ];
        
        let mut launched = false;
        for path in chrome_paths {
            if std::path::Path::new(path).exists() {
                Command::new(path)
                    .arg("--remote-debugging-port=9222")
                    .arg("--user-data-dir=C:\\Temp\\cracking-interview-chrome")
                    .arg("--no-first-run")
                    .arg("--no-default-browser-check")
                    .spawn()
                    .ok();
                launched = true;
                break;
            }
        }
        
        if !launched {
            return Err("Chrome not found on this system. Please install Google Chrome.".to_string());
        }
    }
    
    Ok(())
}

/// Smart Chrome startup - auto-kills and relaunches if needed
pub async fn ensure_chrome_cdp() -> Result<String, String> {
    // Check if CDP already accessible
    if is_cdp_accessible().await {
        return Ok("Chrome CDP already running".to_string());
    }
    
    // Not accessible - kill existing Chrome
    println!("🔄 Restarting Chrome with CDP...");
    kill_chrome()?;
    
    // Launch with CDP
    launch_chrome_with_cdp()?;
    
    // Wait for Chrome to start (max 10 seconds)
    for i in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        if is_cdp_accessible().await {
            println!("✅ Chrome CDP ready after {}ms", i * 500);
            return Ok("Chrome launched successfully with CDP".to_string());
        }
    }
    
    Err("Chrome started but CDP not accessible after 10 seconds".to_string())
}

/// Get Chrome CDP status (for UI display)
pub async fn get_cdp_status() -> String {
    if is_cdp_accessible().await {
        if let Ok(response) = reqwest::get("http://localhost:9222/json/version").await {
            if let Ok(text) = response.text().await {
                return format!("✅ Connected\n{}", text);
            }
        }
        "✅ Connected (details unavailable)".to_string()
    } else {
        "❌ Not connected\n\nClick 'Start Chrome CDP' to launch".to_string()
    }
}
