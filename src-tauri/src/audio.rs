// System audio (loopback) recording.
//
// Goals:
// - macOS 13+: record system audio via ScreenCaptureKit (implemented in a small Swift helper we spawn).
// - Windows: record system audio via WASAPI loopback (native Rust).
//
// This module exposes a small state machine:
//   start() -> starts recording
//   stop()  -> stops recording and returns output path

use std::path::PathBuf;
use std::sync::Mutex;

use lazy_static::lazy_static;

lazy_static! {
    static ref REC_STATE: Mutex<Option<RecordingState>> = Mutex::new(None);
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
    output_path: PathBuf,
}

#[cfg(target_os = "windows")]
struct WindowsRecordingState {
    stop_tx: std::sync::mpsc::Sender<()>,
    join: std::thread::JoinHandle<Result<(), String>>,
    output_path: PathBuf,
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
    let mut guard = match REC_STATE.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };

    // On macOS, the helper can exit early (e.g., startCapture timeout/error). Treat that as not recording.
    #[cfg(target_os = "macos")]
    {
        if let Some(RecordingState::Mac(state)) = guard.as_mut() {
            if let Ok(Some(_status)) = state.child.try_wait() {
                *guard = None;
                return false;
            }
            return true;
        }
    }

    guard.is_some()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::MacRecordingState;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    const SWIFT_HELPER_NAME: &str = "cracking_interview_audio_recorder";

    fn swift_source() -> &'static str {
        include_str!("../resources/audio_recorder.swift")
    }

    fn helper_paths() -> (PathBuf, PathBuf) {
        // Put compiled helper in temp dir. For dev this is sufficient and avoids bundling complexity.
        // If you want to ship this, we should compile it during build and bundle it as a sidecar.
        let mut bin = std::env::temp_dir();
        bin.push(SWIFT_HELPER_NAME);

        let mut src = std::env::temp_dir();
        src.push(format!("{SWIFT_HELPER_NAME}.swift"));

        (bin, src)
    }

    fn ensure_helper_built() -> Result<PathBuf, String> {
        let (bin, src) = helper_paths();

        // Always write the current Swift source. Rebuild the helper when the source changes.
        std::fs::write(&src, swift_source())
            .map_err(|e| format!("Failed to write Swift helper source: {e}"))?;

        let needs_rebuild = match (std::fs::metadata(&bin), std::fs::metadata(&src)) {
            (Ok(bin_meta), Ok(src_meta)) => match (bin_meta.modified(), src_meta.modified()) {
                (Ok(bin_mtime), Ok(src_mtime)) => src_mtime > bin_mtime,
                _ => true,
            },
            _ => true,
        };

        if bin.exists() && !needs_rebuild {
            return Ok(bin);
        }

        // Compile with ScreenCaptureKit + AVFoundation. Requires Xcode CLT.
        let output = Command::new("xcrun")
            .arg("swiftc")
            .arg("-parse-as-library")
            .arg("-O")
            .arg("-o")
            .arg(&bin)
            .arg(&src)
            .arg("-framework")
            .arg("Foundation")
            .arg("-framework")
            .arg("AVFoundation")
            .arg("-framework")
            .arg("CoreMedia")
            .arg("-framework")
            .arg("ScreenCaptureKit")
            .output()
            .map_err(|e| {
                format!(
                    "Failed to run `xcrun swiftc` (needed for macOS 13+ system-audio capture). Install Xcode Command Line Tools.\nUnderlying error: {e}"
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let details = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else if !stdout.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                "No compiler output captured.".to_string()
            };
            return Err(format!(
                "Failed to compile ScreenCaptureKit audio helper (xcrun swiftc). Ensure Xcode Command Line Tools are installed.\n\nswiftc output:\n{}",
                details
            ));
        }

        Ok(bin)
    }

    pub fn start_macos_recording() -> Result<MacRecordingState, String> {
        let helper = ensure_helper_built()?;
        let mut output_path = std::env::temp_dir();
        output_path.push("cracking_interview_system_audio.wav");

        // Spawn helper; it records until terminated.
        // Pipe stderr so we can surface helper diagnostics in the Rust logs.
        let mut child = Command::new(helper)
            .arg("--out")
            .arg(&output_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start audio recorder helper: {e}"))?;

        // Stream helper stderr to our stdout for easier debugging.
        // Also wait for an explicit "capture started" line before we return success,
        // otherwise a quick Stop can terminate before buffers arrive (resulting in a header-only WAV).
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
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

        // Wait briefly for capture-started (helper has its own internal timeout too).
        let start_deadline = std::time::Instant::now() + std::time::Duration::from_secs(14);
        loop {
            if started_rx.try_recv().is_ok() {
                break;
            }

            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("Failed to check recorder status: {e}"))?
            {
                return Err(format!(
                    "Audio recorder failed to start (status={}). Check macOS Screen Recording permission and try again.",
                    status
                ));
            }

            if std::time::Instant::now() >= start_deadline {
                // Ensure we don't leave a stuck helper running (it can block future starts and
                // keep producing header-only WAV files if the user stops quickly).
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
                return Err("Audio recorder did not start capture in time. Try again, and ensure system audio is playing. If it still fails, macOS ScreenCaptureKit may be blocked or unstable on this machine.".to_string());
            }

            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        Ok(MacRecordingState { child, output_path })
    }

    pub fn stop_macos_recording(mut state: MacRecordingState) -> Result<String, String> {
        // Politely terminate; helper traps SIGTERM and finalizes the WAV.
        // IMPORTANT: `Child::kill()` sends SIGKILL on Unix, which prevents the helper from flushing/closing the file.
        // We explicitly send SIGTERM, then wait for exit, then wait for the output file to exist.

        #[link(name = "c")]
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }
        const SIGTERM: i32 = 15;

        let pid = state.child.id() as i32;
        unsafe {
            let _ = kill(pid, SIGTERM);
        }

        // Wait for helper to exit (best-effort).
        let _ = state.child.wait();

        // Wait briefly for file to appear.
        for _ in 0..50 {
            if state.output_path.exists() {
                if let Ok(meta) = std::fs::metadata(&state.output_path) {
                    println!(
                        "🎙️ audio-helper: output file exists ({} bytes) at {}",
                        meta.len(),
                        state.output_path.display()
                    );
                }
                return Ok(state
                    .output_path
                    .to_str()
                    .ok_or("Invalid output path")?
                    .to_string());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        Err(format!(
            "Audio file was not created at {}. Recording may have been too short or macOS permissions prevented capture.",
            state.output_path.display()
        ))
    }
}

/// Transcribe audio from a WAV file
/// TODO: Install libvosk native library and uncomment vosk dependency in Cargo.toml to enable local transcription
/// For now, returns a placeholder message - audio will be sent to AI models that support it
pub fn transcribe_audio(wav_path: &str) -> Result<String, String> {
    println!("[Transcribe] Audio file recorded at: {}", wav_path);
    
    // Check if file exists and has content
    let metadata = std::fs::metadata(wav_path)
        .map_err(|e| format!("Cannot read audio file: {}", e))?;
    
    if metadata.len() < 1000 {
        return Err("Audio file is too small - recording may have failed".to_string());
    }
    
    println!("[Transcribe] Audio file size: {} bytes", metadata.len());
    
    // Return path for now - frontend will handle sending to AI
    // When vosk is installed, this will do local transcription
    Err(format!("LOCAL_TRANSCRIPTION_DISABLED:{}", wav_path))
}

#[cfg(target_os = "windows")]
mod windows {
    use super::WindowsRecordingState;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;

    use windows::core::Interface;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    fn sample_format_to_hound_spec(wfx: &WAVEFORMATEX) -> Result<hound::WavSpec, String> {
        let channels = wfx.nChannels as u16;
        let sample_rate = wfx.nSamplesPerSec;

        // We write 16-bit PCM (converting if necessary).
        Ok(hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        })
    }

    pub fn start_windows_recording() -> Result<WindowsRecordingState, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).map_err(|e| format!("CoInitializeEx failed: {e}"))?;
        }

        let mut output_path = std::env::temp_dir();
        output_path.push("cracking_interview_system_audio.wav");

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let output_path_clone = output_path.clone();

        let join = thread::spawn(move || capture_loop(output_path_clone, stop_rx));

        Ok(WindowsRecordingState { stop_tx, join, output_path })
    }

    pub fn stop_windows_recording(state: WindowsRecordingState) -> Result<String, String> {
        let _ = state.stop_tx.send(());
        let res = state.join.join().map_err(|_| "Audio recording thread panicked".to_string())?;
        res?;
        Ok(state
            .output_path
            .to_str()
            .ok_or("Invalid output path")?
            .to_string())
    }

    fn capture_loop(output_path: PathBuf, stop_rx: mpsc::Receiver<()>) -> Result<(), String> {
        unsafe {
            // Get default render device (system output).
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance(MMDeviceEnumerator) failed: {e}"))?;

            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| format!("GetDefaultAudioEndpoint failed: {e}"))?;

            let audio_client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Activate(IAudioClient) failed: {e}"))?;

            let pwfx = audio_client
                .GetMixFormat()
                .map_err(|e| format!("GetMixFormat failed: {e}"))?;

            let wfx: &WAVEFORMATEX = &*pwfx;
            let spec = sample_format_to_hound_spec(wfx)?;

            // 100ms buffer duration.
            let hns_buffer_duration: i64 = 10_000_000; // 1s in 100ns units => 0.1s = 1_000_000; but we use 1s? Keep stable.
            let hns_buffer_duration: i64 = 1_000_000;

            audio_client
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    AUDCLNT_STREAMFLAGS_LOOPBACK,
                    hns_buffer_duration,
                    0,
                    wfx,
                    None,
                )
                .map_err(|e| format!("IAudioClient::Initialize failed: {e}"))?;

            let capture_client: IAudioCaptureClient = audio_client
                .GetService()
                .map_err(|e| format!("GetService(IAudioCaptureClient) failed: {e}"))?;

            audio_client
                .Start()
                .map_err(|e| format!("IAudioClient::Start failed: {e}"))?;

            let mut writer = hound::WavWriter::create(&output_path, spec)
                .map_err(|e| format!("Failed to create WAV writer: {e}"))?;

            // Determine source sample format. Mix format is often float32.
            let is_float = wfx.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
                || (wfx.wFormatTag == WAVE_FORMAT_EXTENSIBLE
                    && {
                        // If extensible, we assume float32; robust parsing is more work.
                        true
                    });

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let mut packet_length: u32 = 0;
                capture_client
                    .GetNextPacketSize(&mut packet_length)
                    .map_err(|e| format!("GetNextPacketSize failed: {e}"))?;

                while packet_length != 0 {
                    let mut data_ptr: *mut u8 = std::ptr::null_mut();
                    let mut num_frames: u32 = 0;
                    let mut flags: u32 = 0;

                    capture_client
                        .GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)
                        .map_err(|e| format!("GetBuffer failed: {e}"))?;

                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 != 0 {
                        // Write silence frames.
                        let channels = wfx.nChannels as usize;
                        let samples = num_frames as usize * channels;
                        for _ in 0..samples {
                            writer.write_sample::<i16>(0).map_err(|e| format!("WAV write failed: {e}"))?;
                        }
                    } else if !data_ptr.is_null() {
                        let channels = wfx.nChannels as usize;
                        let bytes_per_frame = wfx.nBlockAlign as usize;
                        let byte_count = num_frames as usize * bytes_per_frame;
                        let slice = std::slice::from_raw_parts(data_ptr, byte_count);

                        if is_float {
                            // Interpret as f32 interleaved.
                            let floats: &[f32] = std::slice::from_raw_parts(
                                slice.as_ptr() as *const f32,
                                (byte_count / std::mem::size_of::<f32>()),
                            );
                            for &s in floats.iter().take(num_frames as usize * channels) {
                                let clamped = s.max(-1.0).min(1.0);
                                let pcm = (clamped * i16::MAX as f32) as i16;
                                writer.write_sample::<i16>(pcm).map_err(|e| format!("WAV write failed: {e}"))?;
                            }
                        } else {
                            // Assume i16 PCM interleaved.
                            let ints: &[i16] = std::slice::from_raw_parts(
                                slice.as_ptr() as *const i16,
                                (byte_count / std::mem::size_of::<i16>()),
                            );
                            for &s in ints.iter().take(num_frames as usize * channels) {
                                writer.write_sample::<i16>(s).map_err(|e| format!("WAV write failed: {e}"))?;
                            }
                        }
                    }

                    capture_client
                        .ReleaseBuffer(num_frames)
                        .map_err(|e| format!("ReleaseBuffer failed: {e}"))?;

                    capture_client
                        .GetNextPacketSize(&mut packet_length)
                        .map_err(|e| format!("GetNextPacketSize failed: {e}"))?;
                }

                std::thread::sleep(std::time::Duration::from_millis(20));
            }

            audio_client.Stop().ok();
            writer.finalize().map_err(|e| format!("Finalize WAV failed: {e}"))?;

            CoTaskMemFree(Some(pwfx as *const _ as *const _));
        }

        Ok(())
    }
}


