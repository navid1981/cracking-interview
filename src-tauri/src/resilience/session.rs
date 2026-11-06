//! Atomic lifecycle ledger (`run-state.json`) and bounded non-sensitive session snapshot (`session-snapshot.json`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::Manager;

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: usize = 262_144; // 256 KiB
pub const MAX_RESPONSE_MARKDOWN_BYTES: usize = 65_536; // 64 KiB
const SNAPSHOT_FILE_NAME: &str = "session-snapshot.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub schema_version: u32,
    pub saved_at_unix_ms: u64,
    pub window_rect: Option<WindowRect>,
    pub stealth_enabled: bool,
    pub user_hidden: bool,
    pub selected_model: Option<String>,
    pub active_template_id: Option<String>,
    pub programming_language: Option<String>,
    pub active_mode: Option<String>,
    pub last_response_markdown: Option<String>,
}

fn truncate_utf8_to_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl SessionSnapshot {
    pub fn sanitized(&self) -> Self {
        let mut copy = self.clone();
        copy.schema_version = SNAPSHOT_SCHEMA_VERSION;
        if copy.saved_at_unix_ms == 0 {
            copy.saved_at_unix_ms = super::now_unix_ms();
        }
        if let Some(ref md) = copy.last_response_markdown {
            copy.last_response_markdown = Some(truncate_utf8_to_bytes(md, MAX_RESPONSE_MARKDOWN_BYTES));
        }
        copy
    }

    pub fn save_to_dir(&self, config_dir: &Path) -> Result<(), String> {
        let mut cleaned = self.sanitized();
        let mut bytes = serde_json::to_vec_pretty(&cleaned).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            cleaned.last_response_markdown = None;
            bytes = serde_json::to_vec_pretty(&cleaned).map_err(|e| e.to_string())?;
        }
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(format!(
                "SessionSnapshot exceeds {} byte cap ({} bytes)",
                MAX_SNAPSHOT_BYTES,
                bytes.len()
            ));
        }
        super::write_atomic(&config_dir.join(SNAPSHOT_FILE_NAME), &bytes)
    }

    pub fn load_from_dir(config_dir: &Path) -> Option<Self> {
        let path = config_dir.join(SNAPSHOT_FILE_NAME);
        let raw = fs::read(&path).ok()?;
        if raw.len() > MAX_SNAPSHOT_BYTES {
            quarantine_corrupt_file(&path);
            return None;
        }
        match serde_json::from_slice::<SessionSnapshot>(&raw) {
            Ok(snap) if snap.schema_version == SNAPSHOT_SCHEMA_VERSION => Some(snap.sanitized()),
            _ => {
                quarantine_corrupt_file(&path);
                None
            }
        }
    }
}

fn quarantine_corrupt_file(path: &Path) {
    let ts = super::now_unix_ms();
    let quarantine_name = format!("session-snapshot.corrupt-{}.json", ts);
    let quarantine_path = path.with_file_name(quarantine_name);
    let _ = fs::rename(path, quarantine_path);
}

/// Captures current native window rect and merges it into the snapshot before saving.
pub fn save_snapshot_with_window(
    app: &tauri::AppHandle,
    mut snapshot: SessionSnapshot,
) -> Result<(), String> {
    let dir = super::get_config_dir().ok_or("Config dir unavailable")?;
    if snapshot.window_rect.is_none() {
        if let Some(win) = app.get_webview_window("main") {
            if let (Ok(pos), Ok(size)) = (win.outer_position(), win.inner_size()) {
                // Only record on-screen positions (ignore -30000 offscreen hide coords)
                if pos.x > -10000 && pos.y > -10000 {
                    snapshot.window_rect = Some(WindowRect {
                        x: pos.x,
                        y: pos.y,
                        width: size.width,
                        height: size.height,
                    });
                }
            }
        }
    }
    snapshot.stealth_enabled = crate::STEALTH_ENABLED.load(Ordering::Relaxed);
    snapshot.user_hidden = super::USER_HIDDEN.load(Ordering::Relaxed);
    snapshot.save_to_dir(&dir)
}

pub fn load_snapshot_if_restore_mode() -> Option<SessionSnapshot> {
    let is_restore = std::env::args().any(|a| a == "--restore-session");
    if !is_restore {
        return None;
    }
    let dir = super::get_config_dir()?;
    SessionSnapshot::load_from_dir(&dir)
}

/// Restores window geometry on startup when launched with `--restore-session`.
pub fn apply_restored_window_rect(app: &tauri::AppHandle) {
    let Some(snap) = load_snapshot_if_restore_mode() else {
        return;
    };
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if let Some(rect) = snap.window_rect {
        if rect.x > -10000 && rect.y > -10000 {
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: rect.x,
                y: rect.y,
            }));
        }
        if rect.width >= 200 && rect.height >= 200 {
            let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: rect.width,
                height: rect.height,
            }));
        }
    }
    super::log_event("Restored window geometry from session-snapshot.json");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ci_session_test_{}_{}_{}",
            label,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_session_snapshot_atomic_round_trip() {
        let dir = temp_test_dir("roundtrip");
        let snap = SessionSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            saved_at_unix_ms: 123_456_789,
            window_rect: Some(WindowRect {
                x: 120,
                y: 80,
                width: 550,
                height: 900,
            }),
            stealth_enabled: true,
            user_hidden: false,
            selected_model: Some("gemini-2.5-pro".to_string()),
            active_template_id: Some("algorithm_optimal".to_string()),
            programming_language: Some("Java".to_string()),
            active_mode: Some("text".to_string()),
            last_response_markdown: Some(
                "## Optimal Solution\n```java\nclass Solution {}\n```".to_string(),
            ),
        };

        snap.save_to_dir(&dir).expect("save_to_dir should succeed");
        let loaded =
            SessionSnapshot::load_from_dir(&dir).expect("load_from_dir should return Some");
        assert_eq!(loaded, snap);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_snapshot_is_quarantined_and_returns_none() {
        let dir = temp_test_dir("corrupt");
        let snap_path = dir.join("session-snapshot.json");
        fs::write(&snap_path, b"{truncated_json: [1, 2").unwrap();

        let loaded = SessionSnapshot::load_from_dir(&dir);
        assert!(loaded.is_none());
        assert!(!snap_path.exists(), "corrupt file should be moved aside");

        let quarantined = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("session-snapshot.corrupt-")
            });
        assert!(
            quarantined,
            "corrupt file should be quarantined with timestamp"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_truncates_oversized_response_and_enforces_256k_cap() {
        let dir = temp_test_dir("sizecap");
        let huge_markdown = "A".repeat(120_000); // > 64 KiB
        let snap = SessionSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            saved_at_unix_ms: 999,
            window_rect: None,
            stealth_enabled: true,
            user_hidden: false,
            selected_model: Some("model".to_string()),
            active_template_id: None,
            programming_language: None,
            active_mode: None,
            last_response_markdown: Some(huge_markdown),
        };

        snap.save_to_dir(&dir)
            .expect("save_to_dir should succeed after truncating markdown");
        let loaded = SessionSnapshot::load_from_dir(&dir).expect("should load truncated snapshot");
        let md = loaded.last_response_markdown.unwrap();
        assert!(md.len() <= MAX_RESPONSE_MARKDOWN_BYTES);

        let raw_bytes = fs::read(dir.join("session-snapshot.json")).unwrap();
        assert!(raw_bytes.len() <= MAX_SNAPSHOT_BYTES);

        let _ = fs::remove_dir_all(&dir);
    }
}
