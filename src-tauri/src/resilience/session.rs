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

// ---------------------------------------------------------------------------
// Shared prompt state (`prompt-state.json`)
//
// Prompts live in WebView `localStorage`, which is scoped to the WebView2
// profile — and the desktop worker deliberately runs an isolated profile
// (`WebView2_DeskWorker`), so it cannot see the main GUI's selected prompt
// or custom prompt definitions. This file is the cross-process source of
// truth both instances sync against: the worker may read AND write it
// (unlike `session-snapshot.json`, whose geometry it must never touch).
// Field names are camelCase to match the TypeScript prompt model.
// ---------------------------------------------------------------------------

pub const PROMPT_STATE_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROMPT_STATE_BYTES: usize = 524_288; // 512 KiB
const PROMPT_STATE_FILE_NAME: &str = "prompt-state.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomPromptEntry {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub supports_language: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinPromptOverride {
    pub system: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptState {
    pub schema_version: u32,
    /// Monotonic counter assigned by the backend on every save; frontends
    /// use it to detect changes made by the other process.
    pub revision: u64,
    pub updated_at_unix_ms: u64,
    pub selected_template_id: Option<String>,
    pub custom_prompts: Vec<CustomPromptEntry>,
    pub builtin_overrides: std::collections::BTreeMap<String, BuiltinPromptOverride>,
}

impl PromptState {
    pub fn load_from_dir(config_dir: &Path) -> Option<Self> {
        let raw = fs::read_to_string(config_dir.join(PROMPT_STATE_FILE_NAME)).ok()?;
        if raw.len() > MAX_PROMPT_STATE_BYTES {
            return None;
        }
        serde_json::from_str::<PromptState>(&raw).ok()
    }
}

/// Persists `incoming` as the new shared prompt state, assigning the next
/// revision. Last-writer-wins between processes; the revision lets the
/// other instance notice the change on its next poll.
pub fn save_prompt_state_in_dir(
    config_dir: &Path,
    mut incoming: PromptState,
) -> Result<PromptState, String> {
    let next_revision = PromptState::load_from_dir(config_dir)
        .map(|existing| existing.revision.saturating_add(1))
        .unwrap_or(1);
    incoming.schema_version = PROMPT_STATE_SCHEMA_VERSION;
    incoming.revision = next_revision;
    incoming.updated_at_unix_ms = super::now_unix_ms();
    let json = serde_json::to_vec_pretty(&incoming).map_err(|e| e.to_string())?;
    if json.len() > MAX_PROMPT_STATE_BYTES {
        return Err(format!(
            "Prompt state exceeds {} KiB cap",
            MAX_PROMPT_STATE_BYTES / 1024
        ));
    }
    super::write_atomic(&config_dir.join(PROMPT_STATE_FILE_NAME), &json)?;
    Ok(incoming)
}

pub fn load_prompt_state() -> Option<PromptState> {
    let dir = super::get_config_dir()?;
    PromptState::load_from_dir(&dir)
}

pub fn save_prompt_state(incoming: PromptState) -> Result<PromptState, String> {
    let dir = super::get_config_dir().ok_or("Config dir unavailable")?;
    save_prompt_state_in_dir(&dir, incoming)
}

/// Captures current native window rect and merges it into the snapshot before saving.
pub fn save_snapshot_with_window(
    app: &tauri::AppHandle,
    mut snapshot: SessionSnapshot,
) -> Result<(), String> {
    if should_skip_snapshot_write(running_as_desktop_worker()) {
        return Ok(());
    }
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

/// Updates the existing `session-snapshot.json` on disk with the latest window geometry and `USER_HIDDEN` state.
pub fn update_window_state_in_snapshot(app: &tauri::AppHandle) {
    if should_skip_snapshot_write(running_as_desktop_worker()) {
        return;
    }
    let Some(dir) = super::get_config_dir() else {
        return;
    };
    let mut rect_opt: Option<WindowRect> = None;
    if let Some(win) = app.get_webview_window("main") {
        if let (Ok(pos), Ok(size)) = (win.outer_position(), win.inner_size()) {
            if pos.x > -10000 && pos.y > -10000 {
                super::record_onscreen_position_if_valid(pos.x, pos.y);
                rect_opt = Some(WindowRect {
                    x: pos.x,
                    y: pos.y,
                    width: size.width,
                    height: size.height,
                });
            }
        }
    }
    let stealth = crate::STEALTH_ENABLED.load(Ordering::Relaxed);
    let hidden = super::USER_HIDDEN.load(Ordering::Relaxed);
    let _ = update_window_state_in_snapshot_dir(&dir, rect_opt, stealth, hidden);
}

pub fn update_window_state_in_snapshot_dir(
    config_dir: &Path,
    rect: Option<WindowRect>,
    stealth_enabled: bool,
    user_hidden: bool,
) -> Result<(), String> {
    let mut snap = SessionSnapshot::load_from_dir(config_dir).unwrap_or(SessionSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        saved_at_unix_ms: super::now_unix_ms(),
        window_rect: None,
        stealth_enabled,
        user_hidden,
        selected_model: None,
        active_template_id: None,
        programming_language: None,
        active_mode: None,
        last_response_markdown: None,
    });
    snap.saved_at_unix_ms = super::now_unix_ms();
    if let Some(r) = rect {
        snap.window_rect = Some(r);
    }
    snap.stealth_enabled = stealth_enabled;
    snap.user_hidden = user_hidden;
    snap.save_to_dir(config_dir)
}

pub fn load_snapshot_if_restore_mode() -> Option<SessionSnapshot> {
    let is_restore = std::env::args().any(|a| a == "--restore-session");
    if !is_restore {
        return None;
    }
    let dir = super::get_config_dir()?;
    SessionSnapshot::load_from_dir(&dir)
}

/// Whether this process must not write the session snapshot. Desktop
/// workers run a throwaway full-screen window on a foreign desktop; if
/// they persisted their geometry/flags into the shared snapshot, every
/// later restore (including the real GUI's) would inherit them.
pub fn should_skip_snapshot_write(is_desktop_worker: bool) -> bool {
    is_desktop_worker
}

fn running_as_desktop_worker() -> bool {
    std::env::args().any(|a| a == "--desktop-worker")
}

/// Clamps a saved window rect onto the currently connected monitors.
///
/// A snapshot saved while an external/above-arranged display was connected
/// would otherwise restore the window entirely off-screen after that display
/// is disconnected (invisible window that never composites). If the rect's
/// center is inside any connected monitor the geometry is left untouched;
/// otherwise the rect is clamped inside the first (primary) monitor. A rect
/// larger than its monitor (e.g. poisoned by a full-screen desktop worker)
/// is shrunk to fit.
pub fn clamp_rect_to_monitors(rect: &WindowRect, monitors: &[WindowRect]) -> WindowRect {
    if monitors.is_empty() {
        return rect.clone();
    }
    let cx = rect.x + rect.width as i32 / 2;
    let cy = rect.y + rect.height as i32 / 2;
    let containing = monitors.iter().find(|m| {
        cx >= m.x && cx < m.x + m.width as i32 && cy >= m.y && cy < m.y + m.height as i32
    });
    let m = containing.cloned().unwrap_or_else(|| monitors[0].clone());
    let width = rect.width.min(m.width);
    let height = rect.height.min(m.height);
    if containing.is_some() && width == rect.width && height == rect.height {
        return rect.clone();
    }
    let max_x = m.x + (m.width as i32 - width as i32).max(0);
    let max_y = m.y + (m.height as i32 - height as i32).max(0);
    WindowRect {
        x: rect.x.clamp(m.x, max_x),
        y: rect.y.clamp(m.y, max_y),
        width,
        height,
    }
}

/// Restores window geometry and `user_hidden` state on startup when launched with `--restore-session`.
pub fn apply_restored_window_rect(app: &tauri::AppHandle) {
    let Some(snap) = load_snapshot_if_restore_mode() else {
        return;
    };
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if let Some(rect) = snap.window_rect.clone() {
        let monitors: Vec<WindowRect> = win
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| WindowRect {
                x: m.position().x,
                y: m.position().y,
                width: m.size().width,
                height: m.size().height,
            })
            .collect();
        let clamped = clamp_rect_to_monitors(&rect, &monitors);
        if clamped != rect {
            super::log_event(&format!(
                "Saved window rect {}x{}@({}, {}) adjusted for current monitors to {}x{}@({}, {})",
                rect.width, rect.height, rect.x, rect.y,
                clamped.width, clamped.height, clamped.x, clamped.y
            ));
        }
        if rect.x > -10000 && rect.y > -10000 {
            super::record_onscreen_position_if_valid(clamped.x, clamped.y);
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: clamped.x,
                y: clamped.y,
            }));
        }
        if clamped.width >= 200 && clamped.height >= 200 {
            let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: clamped.width,
                height: clamped.height,
            }));
        }
    }
    if snap.user_hidden {
        super::USER_HIDDEN.store(true, Ordering::SeqCst);
        #[cfg(target_os = "windows")]
        if crate::STEALTH_ENABLED.load(Ordering::Relaxed) {
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: -30000,
                y: -30000,
            }));
            super::log_event("Restored user_hidden offscreen state from session-snapshot.json");
            return;
        }
        let _ = win.hide();
        super::log_event("Restored user_hidden window state from session-snapshot.json");
    } else if let Some(rect) = snap.window_rect {
        super::log_event(&format!(
            "Restored window geometry from session-snapshot.json (saved rect {}x{}@{},{})",
            rect.width, rect.height, rect.x, rect.y
        ));
    } else {
        super::log_event("Restored window geometry from session-snapshot.json");
    }
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
    fn test_clamp_rect_keeps_window_on_connected_monitor() {
        let primary = WindowRect { x: 0, y: 0, width: 1920, height: 1080 };
        let above = WindowRect { x: -443, y: -1080, width: 1920, height: 1080 };
        let rect = WindowRect { x: -443, y: -1012, width: 550, height: 900 };
        // Second display still connected -> geometry untouched.
        assert_eq!(clamp_rect_to_monitors(&rect, &[primary.clone(), above]), rect);
        // No monitors known -> untouched.
        assert_eq!(clamp_rect_to_monitors(&rect, &[]), rect);
        let _ = primary;
    }

    #[test]
    fn test_clamp_rect_shrinks_oversized_window_to_monitor() {
        let primary = WindowRect { x: 0, y: 0, width: 1920, height: 1080 };
        // A poisoned snapshot (e.g. written by a full-screen desktop worker)
        // must not restore a window larger than the display.
        let rect = WindowRect { x: -9, y: -9, width: 1938, height: 1098 };
        let clamped = clamp_rect_to_monitors(&rect, &[primary]);
        assert!(clamped.width <= 1920 && clamped.height <= 1080, "size clamped: {:?}", clamped);
        assert!(clamped.x >= 0 && clamped.y >= 0, "origin clamped: {:?}", clamped);
    }

    fn sample_prompt_state() -> PromptState {
        PromptState {
            schema_version: PROMPT_STATE_SCHEMA_VERSION,
            revision: 0,
            updated_at_unix_ms: 0,
            selected_template_id: Some("custom_123".to_string()),
            custom_prompts: vec![CustomPromptEntry {
                id: "custom_123".to_string(),
                name: "My Prompt".to_string(),
                system_prompt: "You are helpful".to_string(),
                user_prompt: "Solve {CONTENT}".to_string(),
                supports_language: true,
            }],
            builtin_overrides: std::collections::BTreeMap::from([(
                "algorithm_optimal".to_string(),
                BuiltinPromptOverride {
                    system: Some("Custom system".to_string()),
                    user: None,
                },
            )]),
        }
    }

    #[test]
    fn test_prompt_state_round_trip_and_revision_increments() {
        let dir = temp_test_dir("promptstate");
        assert!(PromptState::load_from_dir(&dir).is_none());

        let saved1 = save_prompt_state_in_dir(&dir, sample_prompt_state()).unwrap();
        assert_eq!(saved1.revision, 1);
        let loaded = PromptState::load_from_dir(&dir).unwrap();
        assert_eq!(loaded.selected_template_id.as_deref(), Some("custom_123"));
        assert_eq!(loaded.custom_prompts.len(), 1);
        assert_eq!(loaded.custom_prompts[0].system_prompt, "You are helpful");
        assert_eq!(
            loaded.builtin_overrides["algorithm_optimal"].system.as_deref(),
            Some("Custom system")
        );

        let saved2 = save_prompt_state_in_dir(&dir, sample_prompt_state()).unwrap();
        assert_eq!(saved2.revision, 2, "revision must increase on every save");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_prompt_state_corrupt_file_returns_none() {
        let dir = temp_test_dir("promptcorrupt");
        fs::write(dir.join("prompt-state.json"), "{not json").unwrap();
        assert!(PromptState::load_from_dir(&dir).is_none());
        // A save over a corrupt file restarts revision numbering cleanly.
        let saved = save_prompt_state_in_dir(&dir, sample_prompt_state()).unwrap();
        assert_eq!(saved.revision, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_desktop_worker_never_writes_snapshot() {
        assert!(should_skip_snapshot_write(true));
        assert!(!should_skip_snapshot_write(false));
    }

    #[test]
    fn test_clamp_rect_pulls_offscreen_window_onto_primary() {
        let primary = WindowRect { x: 0, y: 0, width: 1920, height: 1080 };
        // Saved when a display sat above the primary; that display is gone.
        let rect = WindowRect { x: -443, y: -1012, width: 550, height: 900 };
        let clamped = clamp_rect_to_monitors(&rect, &[primary]);
        assert!(clamped.x >= 0 && clamped.x + clamped.width as i32 <= 1920, "x clamped: {:?}", clamped);
        assert!(clamped.y >= 0 && clamped.y + clamped.height as i32 <= 1080, "y clamped: {:?}", clamped);
        assert_eq!((clamped.width, clamped.height), (550, 900));
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

    #[test]
    fn test_update_window_state_preserves_ui_fields_and_updates_user_hidden() {
        let dir = temp_test_dir("win_update");
        let initial = SessionSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            saved_at_unix_ms: 1000,
            window_rect: Some(WindowRect {
                x: 10,
                y: 20,
                width: 550,
                height: 900,
            }),
            stealth_enabled: true,
            user_hidden: false,
            selected_model: Some("gemini-pro".to_string()),
            active_template_id: Some("system_design".to_string()),
            programming_language: Some("Python".to_string()),
            active_mode: Some("screenshot".to_string()),
            last_response_markdown: Some("existing answer".to_string()),
        };
        initial.save_to_dir(&dir).unwrap();

        update_window_state_in_snapshot_dir(
            &dir,
            Some(WindowRect {
                x: 300,
                y: 400,
                width: 600,
                height: 850,
            }),
            true,
            true,
        )
        .unwrap();

        let updated = SessionSnapshot::load_from_dir(&dir).unwrap();
        assert!(updated.user_hidden);
        assert_eq!(
            updated.window_rect,
            Some(WindowRect {
                x: 300,
                y: 400,
                width: 600,
                height: 850,
            })
        );
        // Preserves UI fields
        assert_eq!(updated.selected_model.as_deref(), Some("gemini-pro"));
        assert_eq!(updated.last_response_markdown.as_deref(), Some("existing answer"));

        let _ = fs::remove_dir_all(&dir);
    }
}
