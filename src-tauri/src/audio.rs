// macOS audio capture helper management.
//
// Live transcription (see transcription.rs) captures system audio + microphone through a
// Swift helper (ScreenCaptureKit + AVAudioEngine). This module locates/compiles that helper
// and keeps an optional pre-initialized ("warm") instance while the Audio source is selected.
// On Windows, transcription.rs captures audio directly via WASAPI, so these are no-ops.

// Upper bound passed to the warm helper so an orphaned process can't capture indefinitely.
#[cfg(target_os = "macos")]
const MAX_RECORDING_SECONDS: u64 = 180;

#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    static ref WARM_STATE: std::sync::Mutex<Option<WarmAudioState>> = std::sync::Mutex::new(None);
}

#[cfg(target_os = "macos")]
struct WarmAudioState {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
}

/// Pre-compile the audio recorder helper on app startup (macOS only).
/// Call this from Tauri setup to eliminate first-recording delay.
#[cfg(target_os = "macos")]
pub fn prewarm_audio_recorder() {
    std::thread::spawn(|| {
        println!("🎙️ Pre-compiling audio recorder...");
        match macos::prewarm_helper() {
            Ok(_) => println!("🎙️ Audio recorder compiled successfully"),
            Err(e) => println!("⚠️ Audio recorder compile failed: {}", e),
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn prewarm_audio_recorder() {
    // No pre-warming needed on Windows
}

/// Warm up the audio capture (call when user selects Audio tab).
/// This pre-initializes ScreenCaptureKit so capture starts instantly.
#[cfg(target_os = "macos")]
pub fn warm_audio_capture() -> Result<(), String> {
    macos::warm_audio_capture()
}

#[cfg(not(target_os = "macos"))]
pub fn warm_audio_capture() -> Result<(), String> {
    Ok(()) // No-op on Windows
}

/// Cool down the audio capture (call when user switches away from Audio tab).
#[cfg(target_os = "macos")]
pub fn cooldown_audio_capture() {
    macos::cooldown_audio_capture()
}

#[cfg(not(target_os = "macos"))]
pub fn cooldown_audio_capture() {
    // No-op on Windows
}

#[cfg(target_os = "macos")]
pub(crate) mod macos {
    use super::{WarmAudioState, WARM_STATE, MAX_RECORDING_SECONDS};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::io::Write;

    /// Locate the pre-compiled audio recorder binary.
    /// 1. Bundled inside the .app (production build)
    /// 2. In the resources/ directory next to the executable (dev build)
    /// 3. Fall back to /tmp compile from source (dev convenience)
    pub(crate) fn find_helper_binary() -> Result<PathBuf, String> {
        // 1. Check inside the .app bundle: ../Resources/audio_recorder_bin
        if let Ok(exe) = std::env::current_exe() {
            let bundle_path = exe
                .parent()                     // .app/Contents/MacOS/
                .and_then(|p| p.parent())      // .app/Contents/
                .map(|p| p.join("Resources").join("audio_recorder_bin"));
            if let Some(ref path) = bundle_path {
                if path.exists() {
                    ensure_executable(path);
                    println!("🎙️ Using bundled audio helper: {}", path.display());
                    return Ok(path.clone());
                }
            }
        }

        // 2. Check next to the executable (Tauri dev mode places resources here)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let dev_path = dir.join("audio_recorder_bin");
                if dev_path.exists() {
                    ensure_executable(&dev_path);
                    println!("🎙️ Using dev audio helper: {}", dev_path.display());
                    return Ok(dev_path);
                }
            }
        }

        // 3. Fall back: compile from embedded source (requires Xcode CLI tools — dev only)
        println!("🎙️ Bundled helper not found, compiling from source (dev mode)...");
        compile_helper_from_source()
    }

    /// Ensure the binary has the executable permission bit set.
    /// Tauri's resource bundling may not preserve it.
    fn ensure_executable(path: &PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            let mode = perms.mode();
            if mode & 0o111 == 0 {
                perms.set_mode(mode | 0o755);
                let _ = std::fs::set_permissions(path, perms);
                println!("🎙️ Set executable permission on audio helper");
            }
        }
    }

    fn compile_helper_from_source() -> Result<PathBuf, String> {
        let mut bin = std::env::temp_dir();
        bin.push("cracking_interview_audio_recorder");
        let mut src = std::env::temp_dir();
        src.push("cracking_interview_audio_recorder.swift");

        let current_source = include_str!("../resources/audio_recorder.swift");

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
            .args(["-framework", "Foundation", "-framework", "AVFoundation",
                   "-framework", "CoreMedia", "-framework", "ScreenCaptureKit"])
            .output()
            .map_err(|e| format!("Failed to run xcrun swiftc: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Swift compile failed: {}", stderr.trim()));
        }

        Ok(bin)
    }

    /// Verify the audio helper binary is available (called on app startup).
    pub fn prewarm_helper() -> Result<(), String> {
        find_helper_binary()?;
        Ok(())
    }
    
    /// Warm up audio capture: pre-initialize ScreenCaptureKit in background.
    /// Call this when user selects Audio tab.
    pub fn warm_audio_capture() -> Result<(), String> {
        let mut warm_guard = WARM_STATE.lock().map_err(|_| "Warm state mutex poisoned")?;
        
        // Already warm?
        if warm_guard.is_some() {
            println!("🔥 Audio capture already warm");
            return Ok(());
        }
        
        println!("🔥 Warming up audio capture...");
        let total_start = std::time::Instant::now();
        
        let helper = find_helper_binary()?;
        
        let mut wav_path = std::env::temp_dir();
        wav_path.push("cracking_interview_audio.wav");
        
        // Spawn helper in warm mode
        let mut child = Command::new(helper)
            .arg("--warm")
            .arg("--out")
            .arg(&wav_path)
            .arg("--timeout")
            .arg(MAX_RECORDING_SECONDS.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start audio recorder helper: {e}"))?;
        
        let stdin = child.stdin.take().ok_or("Failed to get stdin")?;
        
        // Wait for WARM_READY signal
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    if line.contains("WARM_READY") {
                        let _ = ready_tx.send(());
                    }
                    println!("🔥 warm-helper: {}", line);
                }
            });
        }
        
        // Wait up to 5 seconds for warm-up
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if ready_rx.try_recv().is_ok() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                // Kill the process and return error
                let _ = child.kill();
                let _ = child.wait();
                return Err("Audio capture warm-up timed out".to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        println!("🔥 Audio capture warm in {:?}", total_start.elapsed());
        
        *warm_guard = Some(WarmAudioState { child, stdin });
        
        Ok(())
    }
    
    /// Cool down audio capture: kill the warm helper process.
    /// Call this when user switches away from Audio tab.
    pub fn cooldown_audio_capture() {
        let mut warm_guard = match WARM_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        
        if let Some(mut state) = warm_guard.take() {
            println!("❄️ Cooling down audio capture...");
            // Send exit command
            let _ = writeln!(state.stdin, "exit");
            let _ = state.child.wait();
            println!("❄️ Audio capture cooled down");
        }
    }
}
