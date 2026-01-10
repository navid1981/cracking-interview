// Chrome CDP Launcher - Safe zombie cleanup

use std::process::Command;
use std::sync::Mutex;
use tokio::sync::Semaphore;
use std::sync::Arc;

static CDP_CHROME_PID: Mutex<Option<u32>> = Mutex::new(None);

// Prevent concurrent launch attempts
lazy_static::lazy_static! {
    static ref LAUNCH_LOCK: Arc<Semaphore> = Arc::new(Semaphore::new(1));
}

/// Check if Chrome CDP is accessible with real page tabs
pub async fn is_cdp_accessible() -> bool {
    match reqwest::get("http://localhost:9222/json/list").await {
        Ok(response) => {
            if let Ok(tabs) = response.json::<Vec<serde_json::Value>>().await {
                let page_count = tabs.iter()
                    .filter(|t| t["type"].as_str() == Some("page"))
                    .count();
                page_count > 0
            } else {
                false
            }
        }
        Err(_) => false
    }
}

/// Launch Chrome CDP window - doesn't kill running Chrome!
pub async fn launch_chrome_cdp_window() -> Result<String, String> {
    // Acquire lock to prevent concurrent launches
    let _permit = LAUNCH_LOCK.acquire().await.unwrap();
    
    println!("🔒 Launch lock acquired");
    
    // Check if CDP is already accessible
    if is_cdp_accessible().await {
        println!("✅ CDP already has page tabs");
        return Ok("Chrome CDP is already running".to_string());
    }
    
    // CDP not accessible - clean up zombie
    println!("🧹 No page tabs - cleaning zombies...");
    
    match kill_zombie_chrome() {
        Ok(_) => println!("✅ Zombie cleanup complete"),
        Err(e) => println!("⚠️  Zombie cleanup warning: {}", e),
    }
    
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    println!("🚀 Starting Chrome launch...");
    
    #[cfg(target_os = "macos")]
    {
        return launch_chrome_macos().await;
    }
    
    #[cfg(target_os = "windows")]
    {
        return launch_chrome_windows().await;
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("Platform not supported".to_string())
    }
}

/// Kill zombie Chrome ONLY - verify it's actually Chrome before killing!
fn kill_zombie_chrome() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Get PIDs on port 9222
        let output = Command::new("lsof")
            .args(["-ti", ":9222"])
            .output()
            .map_err(|e| e.to_string())?;
        
        let pids = String::from_utf8_lossy(&output.stdout);
        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                // Verify this is actually a Chrome process
                let check_output = Command::new("ps")
                    .args(["-p", &pid.to_string(), "-o", "comm="])
                    .output();
                
                if let Ok(check) = check_output {
                    let process_name = String::from_utf8_lossy(&check.stdout);
                    
                    // Only kill if it's Chrome
                    if process_name.contains("Google Chrome") || process_name.contains("chrome") {
                        println!("🧹 Killing Chrome on port 9222 (PID: {})", pid);
                        Command::new("kill").arg("-9").arg(pid.to_string()).output().ok();
                    } else {
                        println!("⚠️  PID {} on port 9222 is NOT Chrome: '{}' - skipping", pid, process_name.trim());
                    }
                } else {
                    println!("⚠️  Cannot verify PID {} - skipping", pid);
                }
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("netstat")
            .args(["-ano", "|", "findstr", ":9222"])
            .output()
            .map_err(|e| e.to_string())?;
        
        let lines = String::from_utf8_lossy(&output.stdout);
        for line in lines.lines() {
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    // Verify it's Chrome
                    let check = Command::new("tasklist")
                        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV"])
                        .output();
                    
                    if let Ok(output) = check {
                        let list = String::from_utf8_lossy(&output.stdout);
                        if list.to_lowercase().contains("chrome.exe") {
                            Command::new("taskkill")
                                .args(["/F", "/PID", &pid.to_string()])
                                .output()
                                .ok();
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
async fn launch_chrome_macos() -> Result<String, String> {
    println!("🔧 Entering launch_chrome_macos()");
    
    println!("📦 Preparing Command...");
    let child_result = Command::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
        .arg("--remote-debugging-port=9222")
        .arg("--user-data-dir=/tmp/cracking-interview-chrome")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("https://leetcode.com")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    
    println!("⏳ Checking spawn result...");
    
    let child = match child_result {
        Ok(c) => {
            println!("✅ Spawn successful");
            c
        },
        Err(e) => {
            eprintln!("❌ Spawn failed: {}", e);
            return Err(format!("Cannot launch Chrome: {}", e));
        }
    };
    
    println!("🎯 Getting PID...");
    let pid = child.id();
    println!("✅ Got PID: {}", pid);
    
    println!("🔒 Storing PID...");
    {
        let mut guard = CDP_CHROME_PID.lock().unwrap();
        *guard = Some(pid);
    }
    println!("✅ PID stored");
    
    println!("🗑️ Dropping child handle...");
    std::mem::drop(child);
    println!("✅ Child dropped");
    
    println!("⏳ Waiting for CDP...");
    for i in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        if is_cdp_accessible().await {
            println!("✅ CDP ready after {}ms", i * 500);
            return Ok("Chrome CDP opened!".to_string());
        }
    }
    
    println!("❌ Timeout waiting for CDP");
    Err("Timeout after 15s".to_string())
}

#[cfg(target_os = "windows")]
async fn launch_chrome_windows() -> Result<String, String> {
    let chrome_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    
    for path in chrome_paths {
        if !std::path::Path::new(path).exists() {
            continue;
        }
        
        let child = Command::new(path)
            .arg("--remote-debugging-port=9222")
            .arg("--user-data-dir=C:\\Temp\\cracking-interview-chrome")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("https://leetcode.com")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
        
        let pid = child.id();
        
        {
            let mut guard = CDP_CHROME_PID.lock().unwrap();
            *guard = Some(pid);
        }
        
        for i in 0..30 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if is_cdp_accessible().await {
                std::mem::forget(child);
                return Ok("Chrome CDP opened!".to_string());
            }
        }
        
        return Err("Timeout".to_string());
    }
    
    Err("Chrome not found".to_string())
}

/// Get CDP status
pub async fn get_cdp_status() -> String {
    if is_cdp_accessible().await {
        "🟢 CDP Ready".to_string()
    } else {
        "🔴 CDP Not Running".to_string()
    }
}
