use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod session;
pub mod guardian;

pub static AUTHORIZED_QUIT: AtomicBool = AtomicBool::new(false);
pub static USER_HIDDEN: AtomicBool = AtomicBool::new(false);
pub static KEYBOARD_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static SAVED_X: AtomicI32 = AtomicI32::new(0);
pub static SAVED_Y: AtomicI32 = AtomicI32::new(0);
pub static LAST_USER_INTERACTION_MS: AtomicU64 = AtomicU64::new(0);

const AUTHORIZED_QUIT_FILE: &str = "authorized-quit.json";
const KILLED_BY_OS_FILE: &str = "killed-by-os.json";
const RESILIENCE_CONFIG_FILE: &str = "resilience.json";
const RESILIENCE_LOG_FILE: &str = "resilience.log";
const RESILIENCE_LOG_BACKUP_FILE: &str = "resilience.log.1";
const MAX_LOG_BYTES: u64 = 1_048_576; // 1 MiB
const AUTHORIZED_MARKER_TTL_SECS: u64 = 600; // 10 minutes

lazy_static::lazy_static! {
    static ref CONFIG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ResilienceConfig {
    pub enabled: bool,
    pub yield_to_assessment: bool,
    pub harden_process_dacl: bool,
    pub macos_launch_agent_enabled: bool,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            yield_to_assessment: true,
            harden_process_dacl: false,
            macos_launch_agent_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimestampMarker {
    pub timestamp_secs: u64,
    pub pid: u32,
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn record_user_interaction() {
    LAST_USER_INTERACTION_MS.store(now_unix_ms(), Ordering::Relaxed);
}

/// Atomically writes `bytes` to `target_path` via a sibling `.tmp` file + flush + rename.
pub fn write_atomic(target_path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create_dir_all failed: {}", e))?;
    }
    let file_name = target_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("state");
    let tmp_path = target_path.with_file_name(format!("{}.{}.tmp", file_name, std::process::id()));
    {
        let mut f = fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed to create tmp file {:?}: {}", tmp_path, e))?;
        f.write_all(bytes)
            .map_err(|e| format!("Failed to write tmp file {:?}: {}", tmp_path, e))?;
        f.sync_all()
            .map_err(|e| format!("Failed to sync tmp file {:?}: {}", tmp_path, e))?;
    }
    fs::rename(&tmp_path, target_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "Failed to rename {:?} -> {:?}: {}",
            tmp_path, target_path, e
        )
    })
}

impl ResilienceConfig {
    pub fn load_from_dir(config_dir: &Path) -> Self {
        let path = config_dir.join(RESILIENCE_CONFIG_FILE);
        match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str::<ResilienceConfig>(&raw).unwrap_or_default(),
            Err(_) => ResilienceConfig::default(),
        }
    }

    pub fn should_apply_process_dacl(&self) -> bool {
        self.enabled && self.harden_process_dacl
    }
}

pub fn init_config_dir(dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    clear_authorized_quit_in_dir(&dir);
    if let Ok(mut guard) = CONFIG_DIR.lock() {
        *guard = Some(dir);
    }
}

pub fn get_config_dir() -> Option<PathBuf> {
    if let Ok(guard) = CONFIG_DIR.lock() {
        if let Some(ref d) = *guard {
            return Some(d.clone());
        }
    }
    default_os_config_dir()
}

pub fn default_os_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("com.crackinginterview.app")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(|a| PathBuf::from(a).join("com.crackinginterview.app"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join(".config")
                .join("com.crackinginterview.app")
        })
    }
}

pub fn authorize_quit_in_dir(config_dir: &Path) -> Result<(), String> {
    AUTHORIZED_QUIT.store(true, Ordering::SeqCst);
    let marker = TimestampMarker {
        timestamp_secs: now_unix_secs(),
        pid: std::process::id(),
    };
    let json = serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?;
    write_atomic(&config_dir.join(AUTHORIZED_QUIT_FILE), &json)
}

pub fn quit_was_authorized_in_dir(config_dir: &Path, now_secs: u64) -> bool {
    if AUTHORIZED_QUIT.load(Ordering::SeqCst) {
        return true;
    }
    let path = config_dir.join(AUTHORIZED_QUIT_FILE);
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(marker) = serde_json::from_str::<TimestampMarker>(&raw) {
            let age = now_secs.saturating_sub(marker.timestamp_secs);
            return age <= AUTHORIZED_MARKER_TTL_SECS;
        }
    }
    false
}

pub fn clear_authorized_quit_in_dir(config_dir: &Path) {
    let _ = fs::remove_file(config_dir.join(AUTHORIZED_QUIT_FILE));
}

pub fn authorize_quit(app: &tauri::AppHandle) {
    AUTHORIZED_QUIT.store(true, Ordering::SeqCst);
    let dir = app
        .path()
        .app_config_dir()
        .ok()
        .or_else(get_config_dir);
    if let Some(d) = dir {
        let _ = authorize_quit_in_dir(&d);
        log_event_in_dir(&d, "User authorized application quit");
    }
}

pub fn quit_was_authorized() -> bool {
    if AUTHORIZED_QUIT.load(Ordering::SeqCst) {
        return true;
    }
    if let Some(dir) = get_config_dir() {
        return quit_was_authorized_in_dir(&dir, now_unix_secs());
    }
    false
}

pub fn enabled() -> bool {
    if let Some(dir) = get_config_dir() {
        ResilienceConfig::load_from_dir(&dir).enabled
    } else {
        true
    }
}

pub fn should_yield_to_assessment(app: &tauri::AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        let dir = app
            .path()
            .app_config_dir()
            .ok()
            .or_else(get_config_dir);
        let yield_cfg = dir
            .as_ref()
            .map(|d| ResilienceConfig::load_from_dir(d).yield_to_assessment)
            .unwrap_or(true);
        if !yield_cfg {
            return false;
        }
        // Check if macOS `assessmentagent` is active on the system
        if is_macos_assessment_agent_active() {
            return true;
        }
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        false
    }
}

#[cfg(target_os = "macos")]
pub fn is_macos_assessment_agent_active() -> bool {
    // Check if assessmentagent is actively running under the user's session
    if let Ok(out) = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("assessmentagent")
        .output()
    {
        return out.status.success();
    }
    false
}

pub fn mark_killed_by_os_in_dir(config_dir: &Path) -> Result<(), String> {
    let marker = TimestampMarker {
        timestamp_secs: now_unix_secs(),
        pid: std::process::id(),
    };
    let json = serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?;
    write_atomic(&config_dir.join(KILLED_BY_OS_FILE), &json)
}

pub fn was_killed_by_os_in_dir(config_dir: &Path, now_secs: u64) -> bool {
    let path = config_dir.join(KILLED_BY_OS_FILE);
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(marker) = serde_json::from_str::<TimestampMarker>(&raw) {
            return now_secs.saturating_sub(marker.timestamp_secs) <= 7200; // 2 hours max
        }
    }
    false
}

pub fn clear_killed_by_os_in_dir(config_dir: &Path) {
    let _ = fs::remove_file(config_dir.join(KILLED_BY_OS_FILE));
}

pub fn mark_killed_by_os(app: &tauri::AppHandle) {
    if let Some(dir) = app.path().app_config_dir().ok().or_else(get_config_dir) {
        // Persist latest window geometry into any existing session snapshot before yielding
        if let Some(existing) = session::SessionSnapshot::load_from_dir(&dir) {
            let _ = session::save_snapshot_with_window(app, existing);
        } else {
            let minimal = session::SessionSnapshot {
                schema_version: session::SNAPSHOT_SCHEMA_VERSION,
                saved_at_unix_ms: now_unix_ms(),
                window_rect: None,
                stealth_enabled: crate::STEALTH_ENABLED.load(Ordering::Relaxed),
                user_hidden: USER_HIDDEN.load(Ordering::Relaxed),
                selected_model: None,
                active_template_id: None,
                programming_language: None,
                active_mode: None,
                last_response_markdown: None,
            };
            let _ = session::save_snapshot_with_window(app, minimal);
        }
        let _ = mark_killed_by_os_in_dir(&dir);
        log_event_in_dir(&dir, "Yielding cleanly to OS assessment session");
    }
}

pub fn log_event_in_dir(config_dir: &Path, msg: &str) {
    let _ = fs::create_dir_all(config_dir);
    let log_path = config_dir.join(RESILIENCE_LOG_FILE);
    if let Ok(meta) = fs::metadata(&log_path) {
        if meta.len() >= MAX_LOG_BYTES {
            let backup_path = config_dir.join(RESILIENCE_LOG_BACKUP_FILE);
            let _ = fs::rename(&log_path, &backup_path);
        }
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let ts = chrono::Utc::now().to_rfc3339();
        let _ = writeln!(f, "[{}] [pid={}] {}", ts, std::process::id(), msg);
    }
}

pub fn log_event(msg: &str) {
    println!("🛡️ [resilience] {}", msg);
    if let Some(dir) = get_config_dir() {
        log_event_in_dir(&dir, msg);
    }
}

pub fn validate_external_http_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err("Only http:// and https:// URLs are allowed".to_string());
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || matches!(c, '"' | '\'' | '`' | '|' | ';' | '<' | '>' | '\r' | '\n' | ' '))
    {
        return Err("URL contains unsafe characters".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn extract_exe_from_command_template(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix('"') {
        let end = rest.find('"')?;
        let exe = &rest[..end];
        if exe.to_lowercase().ends_with(".exe") {
            return Some(exe.to_string());
        }
    }
    let token = trimmed.split_whitespace().next()?;
    if token.to_lowercase().ends_with(".exe") {
        Some(token.to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopSwitchAction {
    NoOp,
    SpawnWorker { desktop_name: String },
    StandByForDefault { reason: String },
    ReturnToDefault { terminate_worker: bool },
}

#[derive(Debug, Clone, Default)]
pub struct DesktopSwitchState {
    pub active_custom_desktop: Option<String>,
    pub worker_spawned_for: Option<String>,
    pub blocked_desktop: Option<String>,
    pub access_denied_logged: bool,
}

impl DesktopSwitchState {
    pub fn on_poll(
        &mut self,
        observed_desktop: Option<&str>,
        worker_alive: bool,
    ) -> DesktopSwitchAction {
        match observed_desktop {
            None => {
                // OpenInputDesktop failed (e.g., restrictive kiosk DACL or UAC secure desktop)
                if !self.access_denied_logged {
                    self.access_denied_logged = true;
                    return DesktopSwitchAction::StandByForDefault {
                        reason: "OpenInputDesktop denied access to active desktop; standing by for return to Default".to_string(),
                    };
                }
                DesktopSwitchAction::NoOp
            }
            Some(name) if name.eq_ignore_ascii_case("Default") => {
                let was_Away = self.active_custom_desktop.is_some() || self.access_denied_logged;
                self.active_custom_desktop = None;
                self.worker_spawned_for = None;
                self.blocked_desktop = None;
                self.access_denied_logged = false;
                if was_Away {
                    DesktopSwitchAction::ReturnToDefault {
                        terminate_worker: worker_alive,
                    }
                } else {
                    DesktopSwitchAction::NoOp
                }
            }
            Some(name)
                if name.eq_ignore_ascii_case("Winlogon")
                    || name.eq_ignore_ascii_case("Disconnect") =>
            {
                // System lock screen or UAC prompt — never spawn onto Winlogon
                DesktopSwitchAction::NoOp
            }
            Some(custom_desk) => {
                self.access_denied_logged = false;
                self.active_custom_desktop = Some(custom_desk.to_string());

                if self.blocked_desktop.as_deref() == Some(custom_desk) {
                    return DesktopSwitchAction::NoOp;
                }

                if self.worker_spawned_for.as_deref() == Some(custom_desk) {
                    if worker_alive {
                        return DesktopSwitchAction::NoOp;
                    }
                    // Worker exited prematurely on custom desktop -> mark blocked for this session
                    self.blocked_desktop = Some(custom_desk.to_string());
                    return DesktopSwitchAction::StandByForDefault {
                        reason: format!(
                            "Desktop worker on '{}' exited or was denied window creation; standing by for return to Default",
                            custom_desk
                        ),
                    };
                }

                self.worker_spawned_for = Some(custom_desk.to_string());
                DesktopSwitchAction::SpawnWorker {
                    desktop_name: custom_desk.to_string(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ci_resilience_test_{}_{}_{}",
            label,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_authorize_quit_round_trip_and_expiry() {
        let dir = temp_test_dir("auth_quit");
        AUTHORIZED_QUIT.store(false, Ordering::SeqCst);

        // Missing marker and atomic false -> false
        assert!(!quit_was_authorized_in_dir(&dir, 1_000));

        // Authorize quit -> sets atomic and writes marker
        authorize_quit_in_dir(&dir).expect("authorize_quit_in_dir should succeed");
        assert!(AUTHORIZED_QUIT.load(Ordering::SeqCst));
        assert!(dir.join("authorized-quit.json").exists());

        // Reset in-memory atomic to simulate separate guardian process reading disk marker
        AUTHORIZED_QUIT.store(false, Ordering::SeqCst);
        let now_secs = now_unix_secs();
        assert!(quit_was_authorized_in_dir(&dir, now_secs + 10));

        // Stale marker (> 600s old) -> false
        assert!(!quit_was_authorized_in_dir(&dir, now_secs + 601));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_resilience_config_defaults_and_overrides() {
        let dir = temp_test_dir("cfg");

        // Missing file -> defaults
        let cfg = ResilienceConfig::load_from_dir(&dir);
        assert_eq!(cfg, ResilienceConfig::default());

        // Corrupt JSON -> defaults without panic
        fs::write(dir.join("resilience.json"), b"{not valid json").unwrap();
        let cfg_corrupt = ResilienceConfig::load_from_dir(&dir);
        assert_eq!(cfg_corrupt, ResilienceConfig::default());

        // Explicit kill-switch override
        fs::write(
            dir.join("resilience.json"),
            r#"{"enabled": false, "yield_to_assessment": false, "harden_process_dacl": true}"#,
        )
        .unwrap();
        let cfg_custom = ResilienceConfig::load_from_dir(&dir);
        assert!(!cfg_custom.enabled);
        assert!(!cfg_custom.yield_to_assessment);
        assert!(cfg_custom.harden_process_dacl);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_external_http_url_rejects_unsafe_schemes_and_preserves_query() {
        let valid = "https://checkout.stripe.com/c/pay/cs_live_123?foo=1&bar=2";
        assert_eq!(validate_external_http_url(valid).unwrap(), valid);
        assert!(validate_external_http_url("http://127.0.0.1:1420/test").is_ok());

        // Reject non-http(s) and command-injection attempts
        assert!(validate_external_http_url("file:///C:/Windows/System32/cmd.exe").is_err());
        assert!(validate_external_http_url("javascript:alert(1)").is_err());
        assert!(validate_external_http_url("https://example.com\" & calc.exe").is_err());
        assert!(validate_external_http_url("").is_err());

        // Extract executable from Windows registry shell\open\command template
        let reg_cmd = r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --single-argument %1"#;
        assert_eq!(
            extract_exe_from_command_template(reg_cmd).as_deref(),
            Some(r#"C:\Program Files\Google\Chrome\Application\chrome.exe"#)
        );
    }

    #[test]
    fn test_desktop_switch_state_machine_spawn_block_and_return() {
        let mut st = DesktopSwitchState::default();

        // Staying on Default -> NoOp
        assert_eq!(st.on_poll(Some("Default"), false), DesktopSwitchAction::NoOp);

        // Switch to ProctorSimDesk -> SpawnWorker once
        assert_eq!(
            st.on_poll(Some("ProctorSimDesk"), false),
            DesktopSwitchAction::SpawnWorker {
                desktop_name: "ProctorSimDesk".to_string()
            }
        );

        // Next poll while worker is alive -> NoOp
        assert_eq!(
            st.on_poll(Some("ProctorSimDesk"), true),
            DesktopSwitchAction::NoOp
        );

        // If worker dies while still on ProctorSimDesk -> StandByForDefault (no infinite respawn loop)
        match st.on_poll(Some("ProctorSimDesk"), false) {
            DesktopSwitchAction::StandByForDefault { .. } => {}
            other => panic!("Expected StandByForDefault when worker dies on custom desktop, got {:?}", other),
        }
        assert_eq!(
            st.on_poll(Some("ProctorSimDesk"), false),
            DesktopSwitchAction::NoOp
        );

        // Return to Default -> ReturnToDefault
        assert_eq!(
            st.on_poll(Some("Default"), false),
            DesktopSwitchAction::ReturnToDefault {
                terminate_worker: false
            }
        );

        // OpenInputDesktop access denied (None) -> StandByForDefault once, then NoOp until Default returns
        match st.on_poll(None, false) {
            DesktopSwitchAction::StandByForDefault { .. } => {}
            other => panic!("Expected StandByForDefault on access denied, got {:?}", other),
        }
        assert_eq!(st.on_poll(None, false), DesktopSwitchAction::NoOp);
        assert!(
            st.on_poll(Some("Default"), false)
                == DesktopSwitchAction::ReturnToDefault {
                    terminate_worker: false
                }
        );
    }

    #[test]
    fn test_should_apply_process_dacl_requires_enabled_and_flag() {
        let default_cfg = ResilienceConfig::default();
        assert!(!default_cfg.should_apply_process_dacl());

        let flag_only_but_disabled = ResilienceConfig {
            enabled: false,
            yield_to_assessment: true,
            harden_process_dacl: true,
            macos_launch_agent_enabled: false,
        };
        assert!(!flag_only_but_disabled.should_apply_process_dacl());

        let both_on = ResilienceConfig {
            enabled: true,
            yield_to_assessment: true,
            harden_process_dacl: true,
            macos_launch_agent_enabled: false,
        };
        assert!(both_on.should_apply_process_dacl());
    }
}
