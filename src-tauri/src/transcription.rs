// Real-time audio transcription via Deepgram's streaming WebSocket API.
//
// Captures system audio (same as audio.rs), but instead of recording to a file,
// streams PCM chunks to Deepgram and emits live transcript events to the frontend.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio_tungstenite::tungstenite::Message;

lazy_static! {
    static ref TRANSCRIPTION_ACTIVE: AtomicBool = AtomicBool::new(false);
    static ref STOP_SIGNAL: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref FINAL_TRANSCRIPT: Mutex<String> = Mutex::new(String::new());
}

#[derive(Debug, Serialize, Clone)]
pub struct TranscriptEvent {
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    channel: Option<DeepgramChannel>,
    #[allow(dead_code)]
    speech_final: Option<bool>,
    is_final: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Debug, Deserialize)]
struct DeepgramAlternative {
    transcript: String,
}

pub fn is_transcribing() -> bool {
    TRANSCRIPTION_ACTIVE.load(Ordering::Relaxed)
}

pub async fn start_live_transcription(
    app_handle: tauri::AppHandle,
    deepgram_key: String,
    language: String,
) -> Result<(), String> {
    if TRANSCRIPTION_ACTIVE.load(Ordering::Relaxed) {
        return Err("Live transcription is already active".to_string());
    }

    TRANSCRIPTION_ACTIVE.store(true, Ordering::Relaxed);
    STOP_SIGNAL.store(false, Ordering::Relaxed);

    if let Ok(mut t) = FINAL_TRANSCRIPT.lock() {
        t.clear();
    }

    let stop_signal = STOP_SIGNAL.clone();

    tokio::spawn(async move {
        if let Err(e) = run_transcription_session(app_handle.clone(), deepgram_key, language, stop_signal).await {
            println!("[Transcription] Session error: {}", e);
            let _ = app_handle.emit("live_transcript_error", e.clone());
        }
        TRANSCRIPTION_ACTIVE.store(false, Ordering::Relaxed);
        println!("[Transcription] Session ended");
    });

    Ok(())
}

pub fn stop_live_transcription() -> Result<String, String> {
    if !TRANSCRIPTION_ACTIVE.load(Ordering::Relaxed) {
        return Err("No active transcription session".to_string());
    }

    STOP_SIGNAL.store(true, Ordering::Relaxed);

    // Give the async task a moment to finalize
    std::thread::sleep(std::time::Duration::from_millis(300));

    let transcript = FINAL_TRANSCRIPT
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();

    TRANSCRIPTION_ACTIVE.store(false, Ordering::Relaxed);

    Ok(transcript)
}

async fn run_transcription_session(
    app_handle: tauri::AppHandle,
    deepgram_key: String,
    language: String,
    stop_signal: Arc<AtomicBool>,
) -> Result<(), String> {
    use tokio_tungstenite::connect_async;

    let lang_param = if language.is_empty() || language == "auto" {
        "multi".to_string()
    } else {
        language
    };

    let url = format!(
        "wss://api.deepgram.com/v1/listen?\
         model=nova-3&language={}&punctuate=true&smart_format=true\
         &interim_results=true&utterance_end_ms=3000&vad_events=true\
         &encoding=linear16&sample_rate=16000&channels=1",
        lang_param
    );

    println!("[Transcription] Connecting to Deepgram: {}", url);

    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(&url)
        .header("Authorization", format!("Bearer {}", deepgram_key))
        .header("Host", "api.deepgram.com")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .body(())
        .map_err(|e| format!("Failed to build WebSocket request: {}", e))?;

    let (ws_stream, _) = connect_async(request)
        .await
        .map_err(|e| format!("Failed to connect to Deepgram: {}", e))?;

    println!("[Transcription] Connected to Deepgram");
    let _ = app_handle.emit("live_transcript_status", "connected");

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Spawn audio capture thread that sends PCM chunks
    let stop_audio = stop_signal.clone();
    let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

    std::thread::spawn(move || {
        if let Err(e) = capture_audio_to_channel(audio_tx, stop_audio) {
            println!("[Transcription] Audio capture error: {}", e);
        }
    });

    // Forward audio chunks to Deepgram WebSocket
    let stop_send = stop_signal.clone();
    let send_handle = tokio::spawn(async move {
        while let Some(chunk) = audio_rx.recv().await {
            if stop_send.load(Ordering::Relaxed) {
                break;
            }
            if ws_sender.send(Message::Binary(chunk)).await.is_err() {
                break;
            }
        }
        // Send close message to Deepgram
        let _ = ws_sender.send(Message::Text(r#"{"type":"CloseStream"}"#.to_string())).await;
    });

    // Receive transcript events from Deepgram
    let stop_recv = stop_signal.clone();
    let app_recv = app_handle.clone();

    while let Some(msg) = ws_receiver.next().await {
        if stop_recv.load(Ordering::Relaxed) {
            break;
        }

        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(resp) = serde_json::from_str::<DeepgramResponse>(&text) {
                    if let Some(channel) = resp.channel {
                        if let Some(alt) = channel.alternatives.first() {
                            let transcript_text = alt.transcript.trim().to_string();
                            if transcript_text.is_empty() {
                                continue;
                            }

                            let is_final = resp.is_final.unwrap_or(false);

                            if is_final {
                                if let Ok(mut t) = FINAL_TRANSCRIPT.lock() {
                                    if !t.is_empty() {
                                        t.push(' ');
                                    }
                                    t.push_str(&transcript_text);
                                }
                            }

                            let event = TranscriptEvent {
                                text: transcript_text,
                                is_final,
                            };
                            let _ = app_recv.emit("live_transcript", &event);
                        }
                    }

                    // Handle UtteranceEnd event (silence detected by Deepgram)
                    if resp.msg_type.as_deref() == Some("UtteranceEnd") {
                        let _ = app_recv.emit("live_transcript_utterance_end", ());
                    }
                }
            }
            Ok(Message::Close(_)) => {
                println!("[Transcription] Deepgram closed connection");
                break;
            }
            Err(e) => {
                println!("[Transcription] WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    send_handle.abort();
    println!("[Transcription] Session cleanup complete");

    Ok(())
}

// Platform-specific audio capture that sends PCM16 mono 16kHz chunks over the channel

#[cfg(target_os = "macos")]
fn capture_audio_to_channel(
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    stop_signal: Arc<AtomicBool>,
) -> Result<(), String> {
    use std::io::{BufRead, BufReader, Read};
    use std::process::{Command, Stdio};

    // Use the same Swift helper but in a special "stream" mode that outputs raw PCM to stdout
    // For now, we use a simpler approach: spawn a small Core Audio capture and pipe PCM
    let swift_src = include_str!("../resources/audio_recorder.swift");
    let mut src_path = std::env::temp_dir();
    src_path.push("cracking_interview_audio_recorder.swift");

    let mut bin_path = std::env::temp_dir();
    bin_path.push("cracking_interview_audio_recorder");

    // Ensure helper is built (reuse existing binary if source unchanged)
    let source_changed = match std::fs::read_to_string(&src_path) {
        Ok(existing) => existing != swift_src,
        Err(_) => true,
    };

    if source_changed || !bin_path.exists() {
        std::fs::write(&src_path, swift_src)
            .map_err(|e| format!("Failed to write Swift source: {}", e))?;

        let output = Command::new("xcrun")
            .args([
                "swiftc", "-parse-as-library", "-O", "-o",
            ])
            .arg(&bin_path)
            .arg(&src_path)
            .args([
                "-framework", "Foundation",
                "-framework", "AVFoundation",
                "-framework", "CoreMedia",
                "-framework", "ScreenCaptureKit",
            ])
            .output()
            .map_err(|e| format!("Failed to compile audio helper: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Swift compile failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    let mut wav_path = std::env::temp_dir();
    wav_path.push("cracking_interview_stream.wav");

    // Spawn audio helper in streaming mode
    let mut child = Command::new(&bin_path)
        .arg("--stream-pcm")
        .arg("--out")
        .arg(&wav_path)
        .arg("--timeout")
        .arg("300")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start audio helper: {}", e))?;

    // Read stderr for status messages
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                println!("[Transcription] audio-helper: {}", line);
            }
        });
    }

    // Read raw PCM from stdout
    if let Some(mut stdout) = child.stdout.take() {
        let mut buf = vec![0u8; 3200]; // 100ms of 16kHz mono 16-bit = 3200 bytes
        loop {
            if stop_signal.load(Ordering::Relaxed) {
                break;
            }

            match stdout.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    // Stop the helper
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = writeln!(stdin, "stop");
    }
    let _ = child.wait();

    // Clean up temp file
    let _ = std::fs::remove_file(&wav_path);

    Ok(())
}

#[cfg(target_os = "windows")]
fn capture_audio_to_channel(
    tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    stop_signal: Arc<AtomicBool>,
) -> Result<(), String> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok();

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("CoCreateInstance failed: {}", e))?;

        let render_device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("GetDefaultAudioEndpoint failed: {}", e))?;

        let loopback_client: IAudioClient = render_device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("Activate loopback failed: {}", e))?;

        let loopback_pwfx = loopback_client
            .GetMixFormat()
            .map_err(|e| format!("GetMixFormat failed: {}", e))?;

        let loopback_wfx: &WAVEFORMATEX = &*loopback_pwfx;
        let src_rate = loopback_wfx.nSamplesPerSec as f64;
        let src_channels = loopback_wfx.nChannels as usize;
        let is_float = loopback_wfx.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
            || loopback_wfx.wFormatTag == WAVE_FORMAT_EXTENSIBLE;

        let hns_buffer: i64 = 1_000_000; // 100ms

        loopback_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                hns_buffer,
                0,
                loopback_wfx,
                None,
            )
            .map_err(|e| format!("Initialize loopback failed: {}", e))?;

        let capture: IAudioCaptureClient = loopback_client
            .GetService()
            .map_err(|e| format!("GetService failed: {}", e))?;

        loopback_client
            .Start()
            .map_err(|e| format!("Start loopback failed: {}", e))?;

        let target_rate = 16000.0f64;
        let volume_boost: f32 = 5.0;
        let mut accumulator: Vec<f32> = Vec::new();

        loop {
            if stop_signal.load(Ordering::Relaxed) {
                break;
            }

            let mut packet_length = capture
                .GetNextPacketSize()
                .map_err(|e| format!("GetNextPacketSize failed: {}", e))?;

            while packet_length != 0 {
                let mut data_ptr: *mut u8 = std::ptr::null_mut();
                let mut num_frames: u32 = 0;
                let mut flags: u32 = 0;

                capture
                    .GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None)
                    .map_err(|e| format!("GetBuffer failed: {}", e))?;

                if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 {
                    for _ in 0..(num_frames as usize) {
                        accumulator.push(0.0);
                    }
                } else if !data_ptr.is_null() {
                    let bytes_per_frame = loopback_wfx.nBlockAlign as usize;
                    let byte_count = num_frames as usize * bytes_per_frame;
                    let slice = std::slice::from_raw_parts(data_ptr, byte_count);

                    if is_float {
                        let floats: &[f32] = std::slice::from_raw_parts(
                            slice.as_ptr() as *const f32,
                            byte_count / std::mem::size_of::<f32>(),
                        );
                        for frame in 0..(num_frames as usize) {
                            let mut sum = 0.0f32;
                            for ch in 0..src_channels {
                                sum += floats[frame * src_channels + ch];
                            }
                            accumulator.push((sum / src_channels as f32) * volume_boost);
                        }
                    } else {
                        let ints: &[i16] = std::slice::from_raw_parts(
                            slice.as_ptr() as *const i16,
                            byte_count / std::mem::size_of::<i16>(),
                        );
                        for frame in 0..(num_frames as usize) {
                            let mut sum = 0.0f32;
                            for ch in 0..src_channels {
                                sum += ints[frame * src_channels + ch] as f32 / i16::MAX as f32;
                            }
                            accumulator.push((sum / src_channels as f32) * volume_boost);
                        }
                    }
                }

                capture.ReleaseBuffer(num_frames).ok();

                packet_length = capture
                    .GetNextPacketSize()
                    .map_err(|e| format!("GetNextPacketSize failed: {}", e))?;
            }

            // Resample accumulated audio to 16kHz and send
            let resampled_len = (accumulator.len() as f64 * target_rate / src_rate) as usize;
            if resampled_len > 0 {
                let mut pcm_bytes: Vec<u8> = Vec::with_capacity(resampled_len * 2);
                for i in 0..resampled_len {
                    let src_idx = (i as f64 * src_rate / target_rate) as usize;
                    let mut sample = if src_idx < accumulator.len() {
                        accumulator[src_idx]
                    } else {
                        0.0
                    };
                    sample = sample.max(-0.95).min(0.95);
                    let pcm = (sample * i16::MAX as f32) as i16;
                    pcm_bytes.extend_from_slice(&pcm.to_le_bytes());
                }

                let consumed = (resampled_len as f64 * src_rate / target_rate) as usize;
                if consumed < accumulator.len() {
                    accumulator.drain(0..consumed);
                } else {
                    accumulator.clear();
                }

                if tx.blocking_send(pcm_bytes).is_err() {
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        loopback_client.Stop().ok();
        CoTaskMemFree(Some(loopback_pwfx as *const _ as *const _));
    }

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn capture_audio_to_channel(
    _tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    _stop_signal: Arc<AtomicBool>,
) -> Result<(), String> {
    Err("Live transcription is only supported on macOS and Windows".to_string())
}
