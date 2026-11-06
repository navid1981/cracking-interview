//! Detached Guardian Process Supervisor (`--guardian`) & Single-Instance Arbitration
//! (Scenarios A1d, A2, B2d, B1).

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_FAST_DEATHS: u32 = 5;
const MAX_AAC_PROBES: u32 = 240; // 240 * 30s = 2 hours
const HEALTHY_LIFETIME: Duration = Duration::from_secs(30);
const ULTRA_FAST_LIFETIME: Duration = Duration::from_secs(2);

lazy_static::lazy_static! {
    static ref HELD_GUI_LOCK: Mutex<Option<InstanceLock>> = Mutex::new(None);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RespawnMode {
    Normal,
    AacProbe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RespawnDecision {
    Respawn { delay: Duration, mode: RespawnMode },
    ExitCleanly,
    StandDown { reason: String },
}

#[derive(Debug, Clone, Default)]
pub struct GuardianPolicy {
    pub fast_deaths: u32,
    pub ultra_fast_deaths: u32,
    pub aac_probes: u32,
}

impl GuardianPolicy {
    pub fn decide(
        &mut self,
        child_lifetime: Duration,
        authorized_quit: bool,
        killed_by_os: bool,
    ) -> RespawnDecision {
        // Rule 1: User-authorized quit always wins immediately.
        if authorized_quit {
            return RespawnDecision::ExitCleanly;
        }

        // Rule 2: Healthy long run (>= 30s) resets all failure counters.
        if child_lifetime >= HEALTHY_LIFETIME && !killed_by_os {
            self.fast_deaths = 0;
            self.ultra_fast_deaths = 0;
            self.aac_probes = 0;
            return RespawnDecision::Respawn {
                delay: Duration::from_millis(500),
                mode: RespawnMode::Normal,
            };
        }

        // Rule 3: OS Assessment Mode yield or 3 consecutive < 2s rapid kills -> 30s AAC probe schedule.
        if killed_by_os {
            self.aac_probes += 1;
            if self.aac_probes > MAX_AAC_PROBES {
                return RespawnDecision::StandDown {
                    reason: "Exceeded maximum 2-hour AAC assessment probe window".to_string(),
                };
            }
            return RespawnDecision::Respawn {
                delay: Duration::from_secs(30),
                mode: RespawnMode::AacProbe,
            };
        }

        if child_lifetime < ULTRA_FAST_LIFETIME {
            self.ultra_fast_deaths += 1;
            if self.ultra_fast_deaths >= 3 {
                self.aac_probes += 1;
                if self.aac_probes > MAX_AAC_PROBES {
                    return RespawnDecision::StandDown {
                        reason: "Exceeded maximum AAC rapid-death probe cap".to_string(),
                    };
                }
                return RespawnDecision::Respawn {
                    delay: Duration::from_secs(30),
                    mode: RespawnMode::AacProbe,
                };
            }
        } else {
            self.ultra_fast_deaths = 0;
        }

        // Rule 4: Exponential backoff (1s, 2s, 4s, 8s, 16s) with 5-consecutive-fast-death cap.
        self.fast_deaths += 1;
        if self.fast_deaths > MAX_FAST_DEATHS {
            return RespawnDecision::StandDown {
                reason: format!(
                    "Stopped respawning after {} consecutive fast exits (< 30s)",
                    MAX_FAST_DEATHS
                ),
            };
        }

        let shift = (self.fast_deaths - 1).min(4);
        let backoff_secs = 1u64 << shift;
        RespawnDecision::Respawn {
            delay: Duration::from_secs(backoff_secs),
            mode: RespawnMode::Normal,
        }
    }
}

pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn try_acquire_in_dir(config_dir: &Path, name: &str) -> Option<Self> {
        let _ = fs::create_dir_all(config_dir);
        let path = config_dir.join(name);

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let file = OpenOptions::new()
                .create(true)
                // Lock file only: never truncate (explicit per clippy; the
                // file's contents are irrelevant, its flock is the lock).
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .ok()?;
            let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if rc == 0 {
                Some(InstanceLock { _file: file })
            } else {
                None
            }
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let file = OpenOptions::new()
                .create(true)
                .truncate(false) // Lock file only; contents are irrelevant
                .read(true)
                .write(true)
                .share_mode(0) // Exclusive lock; released by kernel on process exit
                .open(&path)
                .ok()?;
            Some(InstanceLock { _file: file })
        }
    }
}

#[cfg(unix)]
impl Drop for InstanceLock {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        unsafe {
            let _ = libc::flock(self._file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

/// Entry-point dispatcher called at the very start of `main()`.
///
/// - If `--guardian` is in argv: runs the headless guardian loop and exits.
/// - If `--gui` or `--desktop-worker` is in argv (or dev build without `CI_FORCE_GUARDIAN=1`):
///   acquires the GUI single-instance lock and returns so `main()` starts the Tauri GUI.
/// - Otherwise (production launch from Explorer/Finder/Shortcut): spawns `--guardian` detached
///   outside the parent's process tree and exits `0` immediately.
pub fn handle_early_cli_and_bootstrap() {
    let args: Vec<String> = std::env::args().collect();
    let is_guardian = args.iter().any(|a| a == "--guardian");
    let is_gui = args.iter().any(|a| a == "--gui");
    let is_desktop_worker = args.iter().any(|a| a == "--desktop-worker");

    let config_dir = super::default_os_config_dir().unwrap_or_else(std::env::temp_dir);
    let resilience_cfg = super::ResilienceConfig::load_from_dir(&config_dir);

    if is_guardian {
        run_guardian_loop(config_dir);
        std::process::exit(0);
    }

    let force_guardian = std::env::var("CI_FORCE_GUARDIAN").ok().as_deref() == Some("1");
    let should_bootstrap_guardian =
        resilience_cfg.enabled && !is_gui && !is_desktop_worker && (!cfg!(debug_assertions) || force_guardian);

    // If spawning the guardian failed for any reason, fall through and run the GUI directly.
    if should_bootstrap_guardian && spawn_detached_guardian().is_ok() {
        if should_park_after_guardian_spawn(cfg!(debug_assertions), force_guardian) {
            // `tauri dev` terminates the Vite dev server (BeforeDevCommand) as
            // soon as this app process exits. Exiting here would leave the
            // guardian-spawned GUI navigating to a dead dev URL, which
            // WKWebView/WebView2 paints as a solid white window. In dev the
            // bootstrap process therefore parks (no window, no locks) purely
            // to keep the CLI session — and Vite — alive; the detached
            // guardian supervises the GUI exactly as in production.
            super::log_event_in_dir(
                &config_dir,
                "Dev guardian mode (CI_FORCE_GUARDIAN=1): bootstrap process staying alive so `tauri dev` keeps the Vite dev server running",
            );
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
        std::process::exit(0);
    }

    // Acquire GUI or Desktop-Worker single-instance lock to prevent duplicate windows/hooks.
    if is_desktop_worker {
        let wv2_dir = config_dir.join("WebView2_DeskWorker");
        let _ = fs::create_dir_all(&wv2_dir);
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &wv2_dir);
        // NOTE: WebView2 software rendering is forced via
        // `additionalBrowserArgs` in tauri.conf.json — wry does not read
        // WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, so no env var is set here.
        super::log_event_in_dir(
            &config_dir,
            "[webview-diag] desktop worker started (WebView2 args via tauri.conf additionalBrowserArgs)",
        );
    }
    let lock_name = if is_desktop_worker {
        "desktop-worker.lock"
    } else {
        "gui.lock"
    };
    if let Some(lock) = InstanceLock::try_acquire_in_dir(&config_dir, lock_name) {
        if let Ok(mut guard) = HELD_GUI_LOCK.lock() {
            *guard = Some(lock);
        }
    } else if res_enabled(&resilience_cfg) {
        eprintln!(
            "ℹ️ [guardian] Another CrackingInterview instance already holds {}; exiting duplicate.",
            lock_name
        );
        std::process::exit(0);
    }
}

fn res_enabled(cfg: &super::ResilienceConfig) -> bool {
    cfg.enabled
}

/// Whether the plain-launch bootstrap process must stay alive after spawning
/// the detached guardian instead of exiting. Only true for dev builds with
/// `CI_FORCE_GUARDIAN=1` (i.e. `tauri dev`), where the process exit would
/// make the Tauri CLI kill the Vite dev server the GUI is about to load.
/// Release builds always exit immediately after the guardian is spawned.
pub fn should_park_after_guardian_spawn(debug_build: bool, force_guardian: bool) -> bool {
    debug_build && force_guardian
}

fn spawn_detached_guardian() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.arg("--guardian")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // First try with CREATE_BREAKAWAY_FROM_JOB so Explorer's job object cannot tree-kill us
        cmd.creation_flags(
            DETACHED_PROCESS
                | CREATE_NEW_PROCESS_GROUP
                | CREATE_BREAKAWAY_FROM_JOB
                | CREATE_NO_WINDOW,
        );
        if cmd.spawn().is_ok() {
            return Ok(());
        }

        // Fallback without CREATE_BREAKAWAY_FROM_JOB if the parent job forbids breakaway
        let exe2 = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut fallback = Command::new(exe2);
        fallback
            .arg("--guardian")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
        fallback.spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                let _ = libc::setsid();
                Ok(())
            });
        }
        cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
    }
}

fn run_guardian_loop(config_dir: PathBuf) {
    // Disguise macOS process as prohibited (headless daemon invisible to Dock & NSWorkspace GUI sweeps)
    #[cfg(target_os = "macos")]
    {
        crate::set_macos_activation_policy(2);
        // SEB's sweep can target this PID by bundle ID; ignore SIGTERM so the
        // supervisor survives (authorized quit works via the marker file).
        super::macos::install_guardian_signal_guard();
    }

    let Some(_guardian_lock) = InstanceLock::try_acquire_in_dir(&config_dir, "guardian.lock") else {
        return;
    };

    super::clear_authorized_quit_in_dir(&config_dir);
    super::clear_killed_by_os_in_dir(&config_dir);
    super::log_event_in_dir(&config_dir, "Guardian supervisor started");

    // Start Windows input-desktop monitor (Scenario A3) inside the windowless guardian process
    #[cfg(target_os = "windows")]
    super::windows::start_desktop_switch_monitor(config_dir.clone());

    let mut policy = GuardianPolicy::default();
    let mut is_first_spawn = true;

    loop {
        if !super::ResilienceConfig::load_from_dir(&config_dir).enabled {
            super::log_event_in_dir(&config_dir, "Resilience disabled in config; guardian exiting");
            break;
        }

        let spawn_start = Instant::now();
        let mut child = match spawn_gui_child(!is_first_spawn) {
            Ok(c) => c,
            Err(e) => {
                super::log_event_in_dir(
                    &config_dir,
                    &format!("Guardian failed to spawn GUI child: {}", e),
                );
                break;
            }
        };
        is_first_spawn = false;

        wait_for_child_exit(&mut child, &config_dir);
        let lifetime = spawn_start.elapsed();

        let now_secs = super::now_unix_secs();
        let authorized = super::quit_was_authorized_in_dir(&config_dir, now_secs);
        let killed_by_os = super::was_killed_by_os_in_dir(&config_dir, now_secs);

        match policy.decide(lifetime, authorized, killed_by_os) {
            RespawnDecision::ExitCleanly => {
                super::clear_authorized_quit_in_dir(&config_dir);
                super::clear_killed_by_os_in_dir(&config_dir);
                super::log_event_in_dir(
                    &config_dir,
                    "Guardian observed authorized quit; exiting cleanly",
                );
                break;
            }
            RespawnDecision::StandDown { reason } => {
                super::log_event_in_dir(&config_dir, &format!("Guardian standing down: {}", reason));
                break;
            }
            RespawnDecision::Respawn { delay, mode } => {
                if mode == RespawnMode::Normal {
                    super::clear_killed_by_os_in_dir(&config_dir);
                }
                super::log_event_in_dir(
                    &config_dir,
                    &format!(
                        "GUI exited after {:?} (mode={:?}); respawning in {:?}",
                        lifetime, mode, delay
                    ),
                );
                if sleep_interruptible_by_quit(&config_dir, delay) {
                    super::clear_authorized_quit_in_dir(&config_dir);
                    super::log_event_in_dir(
                        &config_dir,
                        "Guardian observed authorized quit during backoff; exiting",
                    );
                    break;
                }
            }
        }
    }
}

fn spawn_gui_child(restore_session: bool) -> Result<Child, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.arg("--gui");
    if restore_session {
        cmd.arg("--restore-session");
    }
    cmd.spawn().map_err(|e| e.to_string())
}

fn sleep_interruptible_by_quit(config_dir: &Path, total: Duration) -> bool {
    let start = Instant::now();
    let step = Duration::from_millis(250);
    while start.elapsed() < total {
        if super::quit_was_authorized_in_dir(config_dir, super::now_unix_secs()) {
            return true;
        }
        let rem = total.saturating_sub(start.elapsed());
        std::thread::sleep(rem.min(step));
    }
    super::quit_was_authorized_in_dir(config_dir, super::now_unix_secs())
}

#[cfg(target_os = "macos")]
fn wait_for_child_exit(child: &mut Child, _config_dir: &Path) {
    let pid = child.id() as libc::pid_t;
    unsafe {
        let kq = libc::kqueue();
        if kq >= 0 {
            let mut kev: libc::kevent = std::mem::zeroed();
            kev.ident = pid as usize;
            kev.filter = libc::EVFILT_PROC;
            kev.flags = libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT;
            kev.fflags = libc::NOTE_EXIT;

            let mut out: libc::kevent = std::mem::zeroed();
            let rc = libc::kevent(kq, &kev, 1, &mut out, 1, std::ptr::null());
            let _ = libc::close(kq);
            if rc >= 0 {
                let _ = child.wait();
                return;
            }
        }
    }
    let _ = child.wait();
}

#[cfg(not(target_os = "macos"))]
fn wait_for_child_exit(child: &mut Child, _config_dir: &Path) {
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ci_guardian_test_{}_{}_{}",
            label,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_tauri_conf_forces_webview2_software_rendering() {
        // wry ignores WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS; the only honored
        // channel is the Tauri window config. On a kiosk-created desktop
        // (SEB), WebView2 GPU compositing can fail (DOM alive, window white),
        // so the config must force software rendering AND preserve wry's
        // default --disable-features list (setting the field replaces it).
        let conf = include_str!("../../tauri.conf.json");
        assert!(
            conf.contains("\"additionalBrowserArgs\""),
            "tauri.conf.json must set additionalBrowserArgs"
        );
        assert!(conf.contains("--disable-gpu"), "must disable GPU: {conf}");
        assert!(
            conf.contains("msWebOOUI,msPdfOOUI,msSmartScreenProtection"),
            "must preserve wry default disabled features: {conf}"
        );
    }

    #[test]
    fn test_guardian_policy_authorized_quit_always_exits() {
        let mut policy = GuardianPolicy::default();
        assert_eq!(
            policy.decide(Duration::from_secs(100), true, false),
            RespawnDecision::ExitCleanly
        );
        assert_eq!(
            policy.decide(Duration::from_millis(200), true, true),
            RespawnDecision::ExitCleanly
        );
    }

    #[test]
    fn test_guardian_policy_healthy_run_resets_counters_and_respawns_fast() {
        let mut policy = GuardianPolicy {
            fast_deaths: 4,
            ultra_fast_deaths: 2,
            aac_probes: 3,
        };
        let decision = policy.decide(Duration::from_secs(45), false, false);
        assert_eq!(
            decision,
            RespawnDecision::Respawn {
                delay: Duration::from_millis(500),
                mode: RespawnMode::Normal
            }
        );
        assert_eq!(policy.fast_deaths, 0);
        assert_eq!(policy.ultra_fast_deaths, 0);
        assert_eq!(policy.aac_probes, 0);
    }

    #[test]
    fn test_guardian_policy_crash_loop_exponential_backoff_and_cap() {
        let mut policy = GuardianPolicy::default();
        let expected_secs = [1u64, 2, 4, 8, 16];
        for &secs in &expected_secs {
            let d = policy.decide(Duration::from_secs(5), false, false);
            assert_eq!(
                d,
                RespawnDecision::Respawn {
                    delay: Duration::from_secs(secs),
                    mode: RespawnMode::Normal
                }
            );
        }
        // 6th consecutive fast death -> StandDown
        match policy.decide(Duration::from_secs(5), false, false) {
            RespawnDecision::StandDown { .. } => {}
            other => panic!("Expected StandDown on 6th fast death, got {:?}", other),
        }
    }

    #[test]
    fn test_guardian_policy_aac_probe_mode_on_rapid_deaths_or_os_marker() {
        let mut policy = GuardianPolicy::default();
        // Explicit killed_by_os marker immediately enters 30s AAC probe mode
        assert_eq!(
            policy.decide(Duration::from_secs(10), false, true),
            RespawnDecision::Respawn {
                delay: Duration::from_secs(30),
                mode: RespawnMode::AacProbe
            }
        );

        // 3 consecutive < 2s deaths also enter 30s AAC probe mode
        let mut policy2 = GuardianPolicy::default();
        let _ = policy2.decide(Duration::from_millis(500), false, false);
        let _ = policy2.decide(Duration::from_millis(500), false, false);
        let third = policy2.decide(Duration::from_millis(500), false, false);
        assert_eq!(
            third,
            RespawnDecision::Respawn {
                delay: Duration::from_secs(30),
                mode: RespawnMode::AacProbe
            }
        );
    }

    #[test]
    fn test_park_after_guardian_spawn_only_in_dev_forced_mode() {
        // `tauri dev` kills the Vite dev server as soon as the app process
        // exits, so a dev+forced bootstrap must park instead of exiting or
        // the guardian-spawned GUI loads a dead URL and paints white.
        assert!(should_park_after_guardian_spawn(true, true));
        // Release launches always exit immediately (production model).
        assert!(!should_park_after_guardian_spawn(false, true));
        assert!(!should_park_after_guardian_spawn(false, false));
        // Debug without the force flag never bootstraps a guardian at all.
        assert!(!should_park_after_guardian_spawn(true, false));
    }

    #[test]
    fn test_single_instance_lock_prevents_duplicate_acquisition() {
        let dir = temp_test_dir("lock");
        let lock1 = InstanceLock::try_acquire_in_dir(&dir, "gui.lock");
        assert!(lock1.is_some(), "First lock acquisition should succeed");

        let lock2 = InstanceLock::try_acquire_in_dir(&dir, "gui.lock");
        assert!(
            lock2.is_none(),
            "Second lock acquisition while first is held should return None"
        );

        drop(lock1);
        let lock3 = InstanceLock::try_acquire_in_dir(&dir, "gui.lock");
        assert!(
            lock3.is_some(),
            "Lock acquisition after dropping first lock should succeed"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
