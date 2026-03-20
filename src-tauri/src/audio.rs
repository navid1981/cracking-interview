// Audio recording for interview capture.
//
// Records BOTH system audio (loopback - interviewer's voice from Zoom/Teams) AND
// microphone input (candidate's voice), mixed together into a single MP3 file.
//
// Goals:
// - macOS 13+: record via ScreenCaptureKit + AVAudioEngine (Swift helper).
// - Windows: record via WASAPI loopback + WASAPI microphone capture.
// - Automatic 3-minute timeout to prevent excessive recording.
// - MP3 output for good quality with small file size (using mp3lame, no FFmpeg needed).
//
// This module exposes a small state machine:
//   start() -> starts recording
//   stop()  -> stops recording and returns output path

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lazy_static::lazy_static;

// Maximum recording duration (3 minutes)
const MAX_RECORDING_SECONDS: u64 = 180;

lazy_static! {
    static ref REC_STATE: Mutex<Option<RecordingState>> = Mutex::new(None);
}

// Warm state: holds the pre-initialized audio helper (macOS only)
#[cfg(target_os = "macos")]
lazy_static! {
    static ref WARM_STATE: Mutex<Option<WarmAudioState>> = Mutex::new(None);
}

enum RecordingState {
    #[cfg(target_os = "macos")]
    Mac(MacRecordingState),
    #[cfg(target_os = "windows")]
    Windows(WindowsRecordingState),
}

#[cfg(target_os = "macos")]
struct MacRecordingState {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    output_path: PathBuf,
    start_time: Instant,
    is_warm_mode: bool,
}

#[cfg(target_os = "macos")]
struct WarmAudioState {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    output_path: PathBuf,
}

#[cfg(target_os = "windows")]
struct WindowsRecordingState {
    stop_tx: std::sync::mpsc::Sender<()>,
    join: std::thread::JoinHandle<Result<(), String>>,
    output_path: PathBuf,
    start_time: Instant,
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
/// This pre-initializes ScreenCaptureKit so recording starts instantly.
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

pub fn start_system_audio_recording() -> Result<(), String> {
    let mut guard = REC_STATE.lock().map_err(|_| "Audio recorder mutex poisoned".to_string())?;
    if guard.is_some() {
        return Err("Audio recording is already in progress".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let state = macos::start_macos_recording()?;
        *guard = Some(RecordingState::Mac(state));
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let state = windows::start_windows_recording()?;
        *guard = Some(RecordingState::Windows(state));
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("System audio recording is only supported on macOS and Windows".to_string())
    }
}

pub fn stop_system_audio_recording() -> Result<String, String> {
    let mut guard = REC_STATE.lock().map_err(|_| "Audio recorder mutex poisoned".to_string())?;
    let state = guard.take().ok_or("Audio recording is not running")?;

    match state {
        #[cfg(target_os = "macos")]
        RecordingState::Mac(s) => macos::stop_macos_recording(s),
        #[cfg(target_os = "windows")]
        RecordingState::Windows(s) => windows::stop_windows_recording(s),
        #[allow(unreachable_patterns)]
        _ => Err("Unsupported platform recording state".to_string()),
    }
}

pub fn is_recording() -> bool {
    #[allow(unused_mut)]  // mut needed on macOS only
    let mut guard = match REC_STATE.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };

    #[cfg(target_os = "macos")]
    {
        if let Some(RecordingState::Mac(state)) = guard.as_mut() {
            // Check if timeout reached
            if state.start_time.elapsed() > Duration::from_secs(MAX_RECORDING_SECONDS) {
                println!("🎙️ Audio recording timeout reached ({} seconds)", MAX_RECORDING_SECONDS);
                // Stop via SIGTERM
                #[link(name = "c")]
                extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                const SIGTERM: i32 = 15;
                let pid = state.child.id() as i32;
                unsafe {
                    let _ = kill(pid, SIGTERM);
                }
            }
            
            if let Ok(Some(_status)) = state.child.try_wait() {
                *guard = None;
                return false;
            }
            return true;
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(RecordingState::Windows(state)) = guard.as_ref() {
            // Check if timeout reached
            if state.start_time.elapsed() > Duration::from_secs(MAX_RECORDING_SECONDS) {
                println!("🎙️ Audio recording timeout reached ({} seconds)", MAX_RECORDING_SECONDS);
                let _ = state.stop_tx.send(());
            }
        }
    }

    guard.is_some()
}

/// Get the current recording duration in seconds
#[allow(dead_code)]
pub fn get_recording_duration_secs() -> Option<u64> {
    let guard = REC_STATE.lock().ok()?;
    
    match guard.as_ref()? {
        #[cfg(target_os = "macos")]
        RecordingState::Mac(state) => Some(state.start_time.elapsed().as_secs()),
        #[cfg(target_os = "windows")]
        RecordingState::Windows(state) => Some(state.start_time.elapsed().as_secs()),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// Get remaining recording time in seconds before auto-timeout
#[allow(dead_code)]
pub fn get_remaining_recording_secs() -> Option<u64> {
    let elapsed = get_recording_duration_secs()?;
    Some(MAX_RECORDING_SECONDS.saturating_sub(elapsed))
}

// ============================================================================
// MP3 ENCODING (using mp3lame-encoder crate)
// ============================================================================

/// Convert a WAV file to MP3 using mp3lame-encoder
/// This crate provides high-level LAME bindings
pub fn convert_wav_to_mp3(wav_path: &PathBuf, mp3_path: &PathBuf) -> Result<(), String> {
    use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm, MonoPcm};
    use std::io::Write;
    use std::mem::MaybeUninit;
    
    println!("🎙️ Converting WAV to MP3 using mp3lame-encoder...");
    
    // Read WAV file
    let mut wav_reader = hound::WavReader::open(wav_path)
        .map_err(|e| format!("Failed to open WAV file: {e}"))?;
    
    let spec = wav_reader.spec();
    println!("🎙️ WAV spec: {} Hz, {} channels, {} bits", 
             spec.sample_rate, spec.channels, spec.bits_per_sample);
    
    // Read all samples based on bit depth
    let samples: Vec<i16> = if spec.bits_per_sample == 32 {
        // Float32 WAV - convert to i16
        wav_reader.samples::<f32>()
            .filter_map(|s| s.ok())
            .map(|s| (s * 32767.0) as i16)
            .collect()
    } else {
        // Int16 WAV
        wav_reader.samples::<i16>()
            .filter_map(|s| s.ok())
            .collect()
    };
    
    if samples.is_empty() {
        return Err("WAV file contains no audio samples".to_string());
    }
    
    // For mono, samples count = frames. For stereo, samples count = frames * 2
    let frame_count = if spec.channels == 1 { samples.len() } else { samples.len() / 2 };
    println!("🎙️ Encoding {} frames ({} samples) to MP3...", frame_count, samples.len());
    
    // Build encoder with optimal settings for speech
    let mut mp3_encoder = Builder::new()
        .ok_or("Failed to create LAME encoder")?;
    
    // Set input sample rate
    mp3_encoder.set_sample_rate(spec.sample_rate)
        .map_err(|e| format!("Failed to set sample rate: {:?}", e))?;
    
    // Set number of channels
    mp3_encoder.set_num_channels(spec.channels as u8)
        .map_err(|e| format!("Failed to set channels: {:?}", e))?;
    
    mp3_encoder.set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| format!("Failed to set quality: {:?}", e))?;
    
    // Use 128kbps for better voice quality
    mp3_encoder.set_brate(mp3lame_encoder::Bitrate::Kbps128)
        .map_err(|e| format!("Failed to set bitrate: {:?}", e))?;
    
    let mut encoder = mp3_encoder.build()
        .map_err(|e| format!("Failed to build encoder: {:?}", e))?;
    
    println!("🎙️ Encoder built: input rate={} Hz, channels={}", spec.sample_rate, spec.channels);
    
    // Create output file
    let mut mp3_file = std::fs::File::create(mp3_path)
        .map_err(|e| format!("Failed to create MP3 file: {e}"))?;
    
    // Allocate MP3 buffer (LAME recommends 1.25 * num_samples + 7200)
    let mp3_buffer_size = (samples.len() as f64 * 1.25) as usize + 7200;
    let mut mp3_buffer: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); mp3_buffer_size];
    
    // Encode based on channel count
    let encoded_size = if spec.channels == 1 {
        // Mono - use MonoPcm for single channel
        let input = MonoPcm(&samples);
        encoder.encode(input, &mut mp3_buffer)
            .map_err(|e| format!("Encode error: {:?}", e))?
    } else {
        // Stereo - interleaved format (L,R,L,R,...)
        let input = InterleavedPcm(&samples);
        encoder.encode(input, &mut mp3_buffer)
            .map_err(|e| format!("Encode error: {:?}", e))?
    };
    
    // Write encoded data
    if encoded_size > 0 {
        // Safety: encoder wrote `encoded_size` bytes
        let encoded_slice = unsafe {
            std::slice::from_raw_parts(mp3_buffer.as_ptr() as *const u8, encoded_size)
        };
        mp3_file.write_all(encoded_slice)
            .map_err(|e| format!("Write error: {e}"))?;
    }
    
    // Flush remaining data
    let flush_size = encoder.flush::<FlushNoGap>(&mut mp3_buffer)
        .map_err(|e| format!("Flush error: {:?}", e))?;
    
    if flush_size > 0 {
        let flush_slice = unsafe {
            std::slice::from_raw_parts(mp3_buffer.as_ptr() as *const u8, flush_size)
        };
        mp3_file.write_all(flush_slice)
            .map_err(|e| format!("Write error: {e}"))?;
    }
    
    // Verify output
    let mp3_size = std::fs::metadata(mp3_path)
        .map(|m| m.len())
        .unwrap_or(0);
    
    if mp3_size < 100 {
        return Err("MP3 encoding failed - output file too small".to_string());
    }
    
    println!("🎙️ MP3 conversion complete: {} bytes", mp3_size);
    
    // Keep WAV file for debugging - user can compare WAV vs MP3
    println!("🎙️ WAV file kept at: {}", wav_path.display());
    // let _ = std::fs::remove_file(wav_path);
    
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) mod macos {
    use super::{MacRecordingState, WarmAudioState, WARM_STATE, MAX_RECORDING_SECONDS, convert_wav_to_mp3};
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::io::Write;
    use std::time::Instant;

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
    /// Call this when user selects Audio tab for instant recording.
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
        
        let mut output_path = std::env::temp_dir();
        output_path.push("cracking_interview_audio.mp3");
        
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
        
        *warm_guard = Some(WarmAudioState {
            child,
            stdin,
            output_path,
        });
        
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

    pub fn start_macos_recording() -> Result<MacRecordingState, String> {
        let total_start = std::time::Instant::now();
        
        // Check if we have a warm helper ready
        let mut warm_guard = WARM_STATE.lock().map_err(|_| "Warm state mutex poisoned")?;
        
        if let Some(mut warm_state) = warm_guard.take() {
            // Use the warm helper - instant start!
            println!("🔥 Using warm audio capture - instant start!");
            
            // Send "start" command to begin recording
            writeln!(warm_state.stdin, "start")
                .map_err(|e| format!("Failed to send start command: {e}"))?;
            
            println!("🎙️ [TIMING] Instant start (warm): {:?}", total_start.elapsed());
            println!("🔊 Recording started (system audio). Max duration: {} seconds", MAX_RECORDING_SECONDS);
            
            return Ok(MacRecordingState {
                child: warm_state.child,
                stdin: Some(warm_state.stdin),
                output_path: warm_state.output_path,
                start_time: Instant::now(),
                is_warm_mode: true,
            });
        }
        
        drop(warm_guard); // Release lock before slow path
        
        // Cold start - spawn new helper
        println!("🎙️ Cold start (no warm helper available)");
        
        let helper = find_helper_binary()?;
        println!("🎙️ [TIMING] Helper ready: {:?}", total_start.elapsed());
        
        // Output WAV first, will convert to MP3 after recording
        let mut wav_path = std::env::temp_dir();
        wav_path.push("cracking_interview_audio.wav");
        
        let mut output_path = std::env::temp_dir();
        output_path.push("cracking_interview_audio.mp3");

        // Spawn helper with timeout argument - outputs WAV (we'll convert to MP3 later)
        let spawn_start = std::time::Instant::now();
        let mut child = Command::new(helper)
            .arg("--out")
            .arg(&wav_path)  // Output WAV
            .arg("--timeout")
            .arg(MAX_RECORDING_SECONDS.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start audio recorder helper: {e}"))?;
        println!("🎙️ [TIMING] Process spawned: {:?}", spawn_start.elapsed());

        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let wait_start = std::time::Instant::now();
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    if line.contains("AudioRecorder capture started.") {
                        let _ = started_tx.send(());
                    }
                    println!("🎙️ audio-helper: {}", line);
                }
            });
        }

        let start_deadline = std::time::Instant::now() + std::time::Duration::from_secs(14);
        loop {
            if started_rx.try_recv().is_ok() {
                println!("🎙️ [TIMING] Capture started signal received: {:?}", wait_start.elapsed());
                break;
            }

            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("Failed to check recorder status: {e}"))?
            {
                return Err(format!(
                    "Audio recorder failed to start (status={}). Check macOS Screen Recording and Microphone permissions and try again.",
                    status
                ));
            }

            if std::time::Instant::now() >= start_deadline {
                #[link(name = "c")]
                extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                const SIGTERM: i32 = 15;
                let pid = child.id() as i32;
                unsafe {
                    let _ = kill(pid, SIGTERM);
                }
                let _ = child.wait();
                return Err("Audio recorder did not start capture in time. Try again, and ensure both Screen Recording and Microphone permissions are granted in System Settings.".to_string());
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        
        println!("🎙️ [TIMING] Total startup: {:?}", total_start.elapsed());

        println!("🔊 Recording started (system audio). Max duration: {} seconds", MAX_RECORDING_SECONDS);
        
        Ok(MacRecordingState { 
            child, 
            stdin: None,
            output_path,
            start_time: Instant::now(),
            is_warm_mode: false,
        })
    }

    pub fn stop_macos_recording(mut state: MacRecordingState) -> Result<String, String> {
        if state.is_warm_mode {
            // Warm mode: send "stop" command via stdin
            if let Some(mut stdin) = state.stdin.take() {
                let _ = writeln!(stdin, "stop");
            }
        } else {
            // Legacy mode: send SIGTERM
            #[link(name = "c")]
            extern "C" {
                fn kill(pid: i32, sig: i32) -> i32;
            }
            const SIGTERM: i32 = 15;

            let pid = state.child.id() as i32;
            unsafe {
                let _ = kill(pid, SIGTERM);
            }
        }

        let _ = state.child.wait();

        // WAV path (what Swift helper outputs)
        let wav_path = state.output_path.with_extension("wav");
        
        // Wait for WAV file to appear
        for _ in 0..100 {
            if wav_path.exists() {
                if let Ok(meta) = std::fs::metadata(&wav_path) {
                    if meta.len() > 1000 {
                        println!(
                            "🎙️ audio-helper: WAV file exists ({} bytes) at {}",
                            meta.len(),
                            wav_path.display()
                        );
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        
        if !wav_path.exists() {
            return Err(format!(
                "Audio file was not created at {}. Recording may have been too short or macOS permissions prevented capture.",
                wav_path.display()
            ));
        }
        
        // Convert WAV to MP3 (smaller file, faster upload)
        convert_wav_to_mp3(&wav_path, &state.output_path)?;
        
        // Delete WAV file after successful conversion
        let _ = std::fs::remove_file(&wav_path);
        
        Ok(state.output_path.to_str().ok_or("Invalid output path")?.to_string())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{WindowsRecordingState, MAX_RECORDING_SECONDS, convert_wav_to_mp3};
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    fn sample_format_to_hound_spec(_wfx: &WAVEFORMATEX) -> Result<hound::WavSpec, String> {
        // Output as mono 16-bit PCM at 16kHz for optimal speech quality and small size
        Ok(hound::WavSpec {
            channels: 1,  // Mono
            sample_rate: 16000,  // 16kHz (Gemini's native rate)
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        })
    }

    pub fn start_windows_recording() -> Result<WindowsRecordingState, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok();
        }

        let mut output_path = std::env::temp_dir();
        output_path.push("cracking_interview_audio.mp3");

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let output_path_clone = output_path.clone();

        let join = thread::spawn(move || capture_loop(output_path_clone, stop_rx));

        println!("🔊 Recording started (system audio). Max duration: {} seconds", MAX_RECORDING_SECONDS);

        Ok(WindowsRecordingState { 
            stop_tx, 
            join, 
            output_path,
            start_time: Instant::now(),
        })
    }

    pub fn stop_windows_recording(state: WindowsRecordingState) -> Result<String, String> {
        let _ = state.stop_tx.send(());
        let res = state.join.join().map_err(|_| "Audio recording thread panicked".to_string())?;
        res?;
        
        // Prefer MP3 (smaller file, faster upload)
        if state.output_path.exists() {
            return Ok(state.output_path.to_str().ok_or("Invalid output path")?.to_string());
        }
        
        // Fallback to WAV if MP3 doesn't exist
        let wav_path = state.output_path.with_extension("wav");
        if wav_path.exists() {
            return Ok(wav_path.to_str().ok_or("Invalid output path")?.to_string());
        }
        
        Err(format!("Audio file was not created at {}", state.output_path.display()))
    }

    fn capture_loop(output_path: PathBuf, stop_rx: mpsc::Receiver<()>) -> Result<(), String> {
        let wav_path = output_path.with_extension("wav");
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(MAX_RECORDING_SECONDS);
        
        unsafe {
            // ===== SETUP LOOPBACK (SYSTEM AUDIO ONLY) =====
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {e}"))?;

            let render_device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| format!("GetDefaultAudioEndpoint(render) failed: {e}"))?;

            let loopback_client: IAudioClient = render_device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Activate(IAudioClient) for loopback failed: {e}"))?;

            let loopback_pwfx = loopback_client
                .GetMixFormat()
                .map_err(|e| format!("GetMixFormat(loopback) failed: {e}"))?;

            let loopback_wfx: &WAVEFORMATEX = &*loopback_pwfx;
            
            // Copy packed struct fields to avoid unaligned reference errors
            let sample_rate = loopback_wfx.nSamplesPerSec;
            let channels = loopback_wfx.nChannels;
            println!("🎙️ System audio format: {}Hz {}ch", sample_rate, channels);

            let spec = sample_format_to_hound_spec(loopback_wfx)?;

            // Buffer duration in 100ns units (100ms)
            let hns_buffer_duration: i64 = 1_000_000;

            // Initialize loopback client
            loopback_client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    hns_buffer_duration,
                    0,
                    loopback_wfx,
                    None,
                )
                .map_err(|e| format!("IAudioClient::Initialize(loopback) failed: {e}"))?;

            let loopback_capture: IAudioCaptureClient = loopback_client
                .GetService()
                .map_err(|e| format!("GetService(loopback) failed: {e}"))?;

            loopback_client
                .Start()
                .map_err(|e| format!("IAudioClient::Start(loopback) failed: {e}"))?;

            let mut writer = hound::WavWriter::create(&wav_path, spec)
                .map_err(|e| format!("Failed to create WAV writer: {e}"))?;

            // Sample rate conversion
            let loopback_rate = loopback_wfx.nSamplesPerSec as f64;
            let target_rate = spec.sample_rate as f64;
            
            let loopback_is_float = loopback_wfx.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
                || loopback_wfx.wFormatTag == WAVE_FORMAT_EXTENSIBLE;

            let loopback_channels = loopback_wfx.nChannels as usize;

            // Accumulator buffer
            let mut loopback_samples: Vec<f32> = Vec::new();
            
            // Volume boost (similar to macOS)
            let volume_boost: f32 = 5.0;
            
            loop {
                // Check stop signal
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                
                // Check timeout
                if start_time.elapsed() >= max_duration {
                    println!("🎙️ Recording timeout reached ({} seconds)", MAX_RECORDING_SECONDS);
                    break;
                }

                // ===== CAPTURE SYSTEM AUDIO =====
                let mut packet_length = loopback_capture
                    .GetNextPacketSize()
                    .map_err(|e| format!("GetNextPacketSize(loopback) failed: {e}"))?;

                while packet_length != 0 {
                    let mut data_ptr: *mut u8 = std::ptr::null_mut();
                    let mut num_frames: u32 = 0;
                    let mut flags: u32 = 0;

                    loopback_capture
                        .GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)
                        .map_err(|e| format!("GetBuffer(loopback) failed: {e}"))?;

                    if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 {
                        // Silent - add zeros
                        for _ in 0..(num_frames as usize) {
                            loopback_samples.push(0.0);
                        }
                    } else if !data_ptr.is_null() {
                        let bytes_per_frame = loopback_wfx.nBlockAlign as usize;
                        let byte_count = num_frames as usize * bytes_per_frame;
                        let slice = std::slice::from_raw_parts(data_ptr, byte_count);

                        if loopback_is_float {
                            let floats: &[f32] = std::slice::from_raw_parts(
                                slice.as_ptr() as *const f32,
                                byte_count / std::mem::size_of::<f32>(),
                            );
                            // Mix channels to mono with volume boost
                            for frame in 0..(num_frames as usize) {
                                let mut sum = 0.0f32;
                                for ch in 0..loopback_channels {
                                    sum += floats[frame * loopback_channels + ch];
                                }
                                let mono = (sum / loopback_channels as f32) * volume_boost;
                                loopback_samples.push(mono);
                            }
                        } else {
                            let ints: &[i16] = std::slice::from_raw_parts(
                                slice.as_ptr() as *const i16,
                                byte_count / std::mem::size_of::<i16>(),
                            );
                            for frame in 0..(num_frames as usize) {
                                let mut sum = 0.0f32;
                                for ch in 0..loopback_channels {
                                    sum += ints[frame * loopback_channels + ch] as f32 / i16::MAX as f32;
                                }
                                let mono = (sum / loopback_channels as f32) * volume_boost;
                                loopback_samples.push(mono);
                            }
                        }
                    }

                    loopback_capture
                        .ReleaseBuffer(num_frames)
                        .map_err(|e| format!("ReleaseBuffer(loopback) failed: {e}"))?;

                    packet_length = loopback_capture
                        .GetNextPacketSize()
                        .map_err(|e| format!("GetNextPacketSize(loopback) failed: {e}"))?;
                }

                // ===== RESAMPLE AND WRITE =====
                let resampled_len = (loopback_samples.len() as f64 * target_rate / loopback_rate) as usize;
                
                if resampled_len > 0 {
                    for i in 0..resampled_len {
                        // Linear interpolation for resampling
                        let src_idx = (i as f64 * loopback_rate / target_rate) as usize;
                        
                        let mut sample = if src_idx < loopback_samples.len() {
                            loopback_samples[src_idx]
                        } else {
                            0.0
                        };
                        
                        // Soft clipping (same as macOS)
                        if sample > 0.9 { sample = 0.9 + (sample - 0.9) * 0.2; }
                        else if sample < -0.9 { sample = -0.9 + (sample + 0.9) * 0.2; }
                        sample = sample.max(-0.95).min(0.95);
                        
                        let pcm = (sample * i16::MAX as f32) as i16;
                        writer.write_sample::<i16>(pcm).map_err(|e| format!("WAV write failed: {e}"))?;
                    }
                    
                    // Remove consumed samples
                    let consumed = (resampled_len as f64 * loopback_rate / target_rate) as usize;
                    if consumed < loopback_samples.len() {
                        loopback_samples.drain(0..consumed);
                    } else {
                        loopback_samples.clear();
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(20));
            }

            loopback_client.Stop().ok();
            writer.finalize().map_err(|e| format!("Finalize WAV failed: {e}"))?;

            CoTaskMemFree(Some(loopback_pwfx as *const _ as *const _));
        }

        // Convert WAV to MP3 using mp3lame (no FFmpeg needed)
        convert_wav_to_mp3(&wav_path, &output_path)?;
        
        // Delete WAV file after successful conversion
        let _ = std::fs::remove_file(&wav_path);

        Ok(())
    }
}

/// Detect audio MIME type from file bytes
pub fn detect_audio_mime_type(audio_data: &[u8]) -> Result<&'static str, String> {
    // MP3: starts with ID3 tag or sync bytes (0xFF 0xFB, 0xFF 0xFA, 0xFF 0xF3, etc.)
    if audio_data.len() >= 3 {
        if &audio_data[0..3] == b"ID3" {
            return Ok("audio/mp3");
        }
        if audio_data[0] == 0xFF && (audio_data[1] & 0xE0) == 0xE0 {
            return Ok("audio/mp3");
        }
    }
    
    // WAV: "RIFF"..."WAVE"
    if audio_data.len() >= 12 {
        if &audio_data[0..4] == b"RIFF" && &audio_data[8..12] == b"WAVE" {
            return Ok("audio/wav");
        }
    }
    
    // OGG: "OggS"
    if audio_data.len() >= 4 && &audio_data[0..4] == b"OggS" {
        return Ok("audio/ogg");
    }
    
    // FLAC: "fLaC"
    if audio_data.len() >= 4 && &audio_data[0..4] == b"fLaC" {
        return Ok("audio/flac");
    }
    
    // AAC/M4A: starts with "ftyp" at offset 4
    if audio_data.len() >= 8 && &audio_data[4..8] == b"ftyp" {
        return Ok("audio/m4a");
    }
    
    // Default to WAV if unknown
    Ok("audio/wav")
}
