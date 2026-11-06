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
}
