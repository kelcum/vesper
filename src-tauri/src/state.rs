//! Application state shared by commands, the tray and background threads.
//!
//! Locks are held briefly and never nested: data is cloned out, worked on,
//! then written back.

use crate::config::{FlagMode, Settings};
use crate::discord::presence::{Presence, PresenceState, PresenceStatus};
use crate::error::{Problem, ProblemAction, Result};
use crate::flags::apply::{self, Ownership};
use crate::flags::verification::VerificationStore;
use crate::mods::{ModLedger, ModPaths, ModReport, ModsConfig};
use crate::paths::AppPaths;
use crate::perf::monitor::MonitorHandle;
use crate::perf::system::SystemInfo;
use crate::profiles::ProfileStore;
use crate::roblox::cdn::{LatestVersion, PublishedValues};
use crate::roblox::detect::{self, DetectEnv, Detection, RobloxInstall};
use crate::roblox::watch::{self, VersionsWatcher};
use crate::updates::UpdateState;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub mod events {
    pub const ROBLOX_RUNNING: &str = "roblox://running";
    pub const DETECTION: &str = "roblox://detection";
    pub const LAUNCH: &str = "launch://status";
    pub const VERIFIED: &str = "launch://verified";
    pub const PERF: &str = "perf://sample";
    pub const DISCORD: &str = "discord://status";
    pub const UPDATE: &str = "update://state";
    pub const PROFILES: &str = "profiles://changed";
    pub const SETTINGS: &str = "settings://changed";
    pub const CLOSE_REQUESTED: &str = "app://close-requested";
    pub const NAVIGATE: &str = "app://navigate";
    pub const ACTIVITY: &str = "roblox://activity";
    pub const PLAYER: &str = "roblox://player";
    pub const HISTORY: &str = "history://changed";
    pub const SHADERS: &str = "shaders://status";
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum LaunchStatus {
    Idle,
    Preparing {
        step: String,
    },
    Starting,
    /// Roblox's own installer is updating Roblox before it opens.
    Updating,
    Running {
        pid: u32,
    },
    Failed {
        problem: Problem,
    },
}

impl LaunchStatus {
    pub fn busy(&self) -> bool {
        matches!(self, LaunchStatus::Preparing { .. } | LaunchStatus::Starting | LaunchStatus::Updating)
    }
}

#[derive(Default)]
pub struct MonitorControl {
    pub handle: Option<MonitorHandle>,
    pub sources: BTreeSet<String>,
}

pub struct AppState {
    pub paths: AppPaths,
    pub env: DetectEnv,
    pub system: SystemInfo,
    pub http: reqwest::Client,
    pub settings: Mutex<Settings>,
    pub profiles: Mutex<ProfileStore>,
    pub ownership: Mutex<Ownership>,
    pub verification: Mutex<VerificationStore>,
    pub detection: Mutex<Detection>,
    pub latest: Mutex<Option<LatestVersion>>,
    pub published: Mutex<Option<PublishedValues>>,
    pub presence: Mutex<Option<Presence>>,
    pub monitor: Mutex<MonitorControl>,
    pub launch: Mutex<LaunchStatus>,
    pub update: Mutex<UpdateState>,
    pub pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
    pub keep_applied: Mutex<Option<VersionsWatcher>>,
    pub notices: Mutex<Vec<String>>,
    pub mod_paths: ModPaths,
    pub mods: Mutex<ModsConfig>,
    pub mod_ledger: Mutex<ModLedger>,
    pub last_mod_report: Mutex<Option<ModReport>>,
    /// The game and server the player is in, from Roblox's log.
    pub activity: Mutex<Option<crate::roblox::activity::Activity>>,
    pub activity_watch: Mutex<Option<crate::roblox::activity::ActivityWatcher>>,
    /// The signed-in Roblox player's id, from Roblox's log.
    pub roblox_user: Mutex<Option<u64>>,
    pub user_info: Mutex<Option<crate::roblox::web::UserInfo>>,
    /// Game names and icons by universe id.
    pub games: Mutex<std::collections::HashMap<u64, crate::roblox::web::GameInfo>>,
    /// Server cities by server address.
    pub locations: Mutex<std::collections::HashMap<String, crate::roblox::web::Location>>,
    pub history: Mutex<crate::history::History>,
    pub server_cache: crate::servers::ServerCache,
    pub shade: Mutex<Option<crate::shade::Overlay>>,
    pub shade_status: Mutex<crate::shade::ShaderStatus>,
    pub roblox_running: AtomicBool,
    /// Vesper hid or minimized itself when it launched Roblox.
    pub hidden_for_roblox: AtomicBool,
    /// Vesper minimized itself for Roblox and should stay on the taskbar (or
    /// Dock), even with "Minimize to tray" on. Cleared when the window is used again.
    pub minimized_for_roblox: AtomicBool,
    pub quitting: AtomicBool,
}

/// Locks a mutex, recovering from poisoning (a panicked thread must not lock the app up).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn load(paths: AppPaths) -> std::result::Result<Self, String> {
        paths.ensure().map_err(|e| format!("Couldn't create Vesper's folders: {e}"))?;
        let env = DetectEnv::system().ok_or(if cfg!(target_os = "macos") {
            "Your home folder isn't set (HOME)"
        } else {
            "%LOCALAPPDATA% isn't set"
        })?;
        let loaded = Settings::load(&paths.settings_file());
        let mut notices = Vec::new();
        if loaded.recovered_from.is_some() {
            notices.push("Your settings file couldn't be read, so Vesper started with defaults. The old file was kept next to the new one.".to_owned());
        }
        let settings = loaded.settings;
        let profiles = ProfileStore::load(&paths.profiles_file());
        let ownership = Ownership::load(&paths.ownership_file());
        let verification = VerificationStore::load(&paths.verification_file());
        let detection = detect::detect(&env, settings.roblox.install_dir.as_deref());
        let mod_paths = ModPaths::new(&paths.config, &paths.cache);
        let mods = ModsConfig::load(&mod_paths);
        let mod_ledger = ModLedger::load(&mod_paths.ledger);
        Ok(Self {
            system: crate::perf::system::info(),
            http: crate::roblox::cdn::http_client(),
            settings: Mutex::new(settings),
            profiles: Mutex::new(profiles),
            ownership: Mutex::new(ownership),
            verification: Mutex::new(verification),
            detection: Mutex::new(detection),
            latest: Mutex::new(None),
            published: Mutex::new(load_published(&paths)),
            presence: Mutex::new(None),
            monitor: Mutex::new(MonitorControl::default()),
            launch: Mutex::new(LaunchStatus::Idle),
            update: Mutex::new(UpdateState::Idle),
            pending_update: Mutex::new(None),
            keep_applied: Mutex::new(None),
            notices: Mutex::new(notices),
            mods: Mutex::new(mods),
            mod_ledger: Mutex::new(mod_ledger),
            mod_paths,
            last_mod_report: Mutex::new(None),
            activity: Mutex::new(None),
            activity_watch: Mutex::new(None),
            roblox_user: Mutex::new(None),
            user_info: Mutex::new(None),
            games: Mutex::new(Default::default()),
            locations: Mutex::new(Default::default()),
            history: Mutex::new(crate::history::History::load(&paths.history_file())),
            server_cache: Default::default(),
            shade: Mutex::new(None),
            shade_status: Mutex::new(crate::shade::ShaderStatus::Off),
            roblox_running: AtomicBool::new(!crate::roblox::process::roblox_pids().is_empty()),
            hidden_for_roblox: AtomicBool::new(false),
            minimized_for_roblox: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
            env,
            paths,
        })
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn save_settings(&self) -> Result<()> {
        let s = self.settings();
        s.save(&self.paths.settings_file()).map_err(|e| Problem::io("Couldn't save your settings.", &e))
    }

    pub fn save_profiles(&self) -> Result<()> {
        let p = lock(&self.profiles).clone();
        p.save(&self.paths.profiles_file()).map_err(|e| Problem::io("Couldn't save your profiles.", &e))
    }

    pub fn save_ownership(&self) {
        if let Err(e) = lock(&self.ownership).save(&self.paths.ownership_file()) {
            log::warn!("Couldn't save the ownership ledger: {e}");
        }
    }

    pub fn save_verification(&self) {
        if let Err(e) = lock(&self.verification).save(&self.paths.verification_file()) {
            log::warn!("Couldn't save verification results: {e}");
        }
    }

    /// Brings Roblox's files in line with the mod settings. `dir` is an install
    /// folder (see `RobloxInstall::dir`). Returns `None` when nothing is
    /// configured and nothing was applied before.
    pub fn apply_mods(&self, dir: &std::path::Path) -> Option<ModReport> {
        let cfg = lock(&self.mods).clone();
        let nothing = cfg == ModsConfig::default() && lock(&self.mod_ledger).files.is_empty();
        if nothing {
            return None;
        }
        match crate::mods::upgrade_picture_wheel(&self.mod_paths, &cfg) {
            Ok(true) => log::info!("Rebuilt the picture emote wheel in the current layout"),
            Ok(false) => {}
            Err(e) => log::warn!("Couldn't rebuild the picture emote wheel: {e}"),
        }
        let report = {
            let mut ledger = lock(&self.mod_ledger);
            crate::mods::apply_build(
                &cfg,
                &self.mod_paths,
                &detect::content_root(dir),
                &detect::build_id(dir),
                &mut ledger,
            )
        };
        if let Err(e) = lock(&self.mod_ledger).save(&self.mod_paths.ledger) {
            log::warn!("Couldn't save the mods ledger: {e}");
        }
        log::info!(
            "Mods: {} written, {} unchanged, {} restored, {} missing, {} left alone (changed by another program), {} errors",
            report.written,
            report.unchanged,
            report.restored,
            report.missing.len(),
            report.skipped_foreign.len(),
            report.errors.len()
        );
        *lock(&self.last_mod_report) = Some(report.clone());
        Some(report)
    }

    pub fn redetect(&self, app: &AppHandle) -> Detection {
        let dir = lock(&self.settings).roblox.install_dir.clone();
        let d = detect::detect(&self.env, dir.as_deref());
        *lock(&self.detection) = d.clone();
        let _ = app.emit(events::DETECTION, &d);
        d
    }

    pub fn preferred_install(&self) -> Option<RobloxInstall> {
        lock(&self.detection).preferred_install().cloned()
    }

    pub fn require_install(&self) -> Result<RobloxInstall> {
        let d = lock(&self.detection).clone();
        if let Some(problem) = &d.override_problem {
            return Err(Problem::new("roblox.custom_folder", "Roblox isn't in the chosen folder")
                .what(problem.clone())
                .why("The folder may have been moved, or Roblox installed an update somewhere else.")
                .fix("Pick the folder again in Settings → Roblox, or switch back to automatic detection.")
                .action(ProblemAction::OpenSettings("roblox".into())));
        }
        d.preferred_install().cloned().ok_or_else(|| {
            let why = if d.store_app_present {
                "Only the Microsoft Store edition of Roblox is installed. Its files are sealed, so launchers can't start or tune it."
            } else {
                if cfg!(target_os = "macos") {
                    "Roblox.app isn't in your Applications folder."
                } else {
                    "Roblox's player isn't installed for this Windows account."
                }
            };
            Problem::new("roblox.not_installed", "Roblox isn't installed")
                .what(if cfg!(target_os = "macos") {
                    "Vesper couldn't find the Roblox player on this Mac."
                } else {
                    "Vesper couldn't find the Roblox player on this PC."
                })
                .why(why)
                .fix(if cfg!(target_os = "macos") {
                    "Install Roblox from roblox.com, start it once, then come back. If Roblox.app is somewhere else, choose it in Settings → Roblox."
                } else {
                    "Install Roblox from roblox.com, start it once, then come back. If it's installed somewhere unusual, choose its folder in Settings → Roblox."
                })
                .action(ProblemAction::OpenUrl("https://www.roblox.com/download".into()))
                .action(ProblemAction::Retry)
        })
    }

    pub fn presence_state(&self) -> PresenceState {
        let s = self.settings();
        let profile = lock(&self.profiles).active().name.clone();
        let game = crate::playing::game_presence(self);
        let join_url = (s.discord.join_button && game.is_some())
            .then(|| lock(&self.activity).as_ref().and_then(|a| a.join_url()))
            .flatten();
        let profile_url = s
            .discord
            .profile_button
            .then(|| (*lock(&self.roblox_user)).map(crate::roblox::web::UserInfo::profile_url))
            .flatten();
        PresenceState {
            roblox_running: self.roblox_running.load(Ordering::Relaxed),
            profile: Some(profile),
            show_profile: s.discord.show_profile,
            show_roblox_status: s.discord.show_roblox_status,
            game,
            join_url,
            profile_url,
        }
    }

    pub fn refresh_presence(&self) {
        let state = self.presence_state();
        let enabled = lock(&self.settings).discord.rich_presence;
        if let Some(p) = lock(&self.presence).as_ref() {
            p.configure(enabled, crate::discord::client_id());
            p.update(state);
        }
    }

    pub fn presence_status(&self) -> PresenceStatus {
        lock(&self.presence).as_ref().map(|p| p.status()).unwrap_or(PresenceStatus::Disabled)
    }

    pub fn set_launch(&self, app: &AppHandle, status: LaunchStatus) {
        *lock(&self.launch) = status.clone();
        let _ = app.emit(events::LAUNCH, &status);
    }

    /// Starts or stops the performance monitor to match who is watching.
    pub fn sync_monitor(&self, app: &AppHandle) {
        let s = self.settings();
        let mut m = lock(&self.monitor);
        let wanted = s.performance.monitoring && !m.sources.is_empty();
        if wanted && m.handle.is_none() {
            let emitter = app.clone();
            m.handle = Some(MonitorHandle::spawn(
                Duration::from_millis(s.performance.sample_ms as u64),
                s.performance.fps_counter,
                move |sample| {
                    let _ = emitter.emit(events::PERF, &sample);
                },
            ));
            log::debug!("Performance monitor started");
        } else if !wanted && m.handle.is_some() {
            m.handle = None; // Drop stops the thread.
            log::debug!("Performance monitor stopped");
        }
    }

    /// Restarts the monitor so changed settings (interval, FPS counter) take effect.
    pub fn restart_monitor(&self, app: &AppHandle) {
        lock(&self.monitor).handle = None;
        self.sync_monitor(app);
    }

    /// Starts or stops the shader overlay: on while it's enabled and Roblox runs.
    pub fn sync_shaders(&self, app: &AppHandle) {
        let s = lock(&self.settings).shaders.clone();
        let wanted = s.enabled && self.roblox_running.load(Ordering::Relaxed);
        let mut slot = lock(&self.shade);
        match (wanted, slot.as_ref()) {
            (true, Some(o)) => o.set_params(s.params),
            (true, None) => {
                let emitter = app.clone();
                *slot = Some(crate::shade::Overlay::start(s.params, crate::shade::roblox_window, move |status| {
                    let state = emitter.state::<AppState>();
                    *lock(&state.shade_status) = status.clone();
                    let _ = emitter.emit(events::SHADERS, &status);
                }));
            }
            (false, Some(_)) => {
                *slot = None;
                *lock(&self.shade_status) = crate::shade::ShaderStatus::Off;
                let _ = app.emit(events::SHADERS, crate::shade::ShaderStatus::Off);
            }
            (false, None) => {}
        }
    }

    /// Starts or stops the Versions-folder watcher to match the flag mode.
    pub fn sync_keep_applied(&self, app: &AppHandle) {
        let mode = lock(&self.settings).roblox.flag_mode;
        let install = self.preferred_install();
        let mut slot = lock(&self.keep_applied);
        if mode == FlagMode::KeepApplied {
            if slot.is_none() {
                let handle = app.clone();
                match watch::watch_updates(&self.env, install.as_ref(), move |dir| apply_to_new_version(&handle, dir)) {
                    Ok(w) => {
                        log::info!("Watching for Roblox updates to keep flags applied");
                        *slot = Some(w);
                    }
                    Err(e) => log::warn!("Couldn't watch for Roblox updates: {e}"),
                }
            }
        } else {
            *slot = None;
        }
    }
}

/// Writes the active profile's flags into a version folder Roblox just created.
pub fn apply_to_new_version(app: &AppHandle, dir: &std::path::Path) {
    let state = app.state::<AppState>();
    state.apply_mods(dir);
    if lock(&state.settings).roblox.flag_mode == FlagMode::Off {
        return;
    }
    let flags = lock(&state.profiles).active().flags.clone();
    if flags.is_empty() || crate::flags::validate::normalize(&flags).is_err() {
        return;
    }
    let result = {
        let mut ledger = lock(&state.ownership);
        apply::apply(dir, &flags, &mut ledger, &state.paths.roblox_backups())
    };
    match result {
        Ok(o) => log::info!("Applied {} flags to the updated Roblox ({:?})", o.flag_count, o.action),
        Err(e) => log::warn!("Couldn't apply flags to the updated Roblox: {e}"),
    }
    state.save_ownership();
    state.redetect(app);
}

fn load_published(paths: &AppPaths) -> Option<PublishedValues> {
    let bytes = std::fs::read(paths.cache.join("published.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save_published(paths: &AppPaths, p: &PublishedValues) {
    if let Ok(json) = serde_json::to_vec_pretty(p) {
        let _ = crate::paths::write_atomic(&paths.cache.join("published.json"), &json);
    }
}

/// Watches for Roblox starting and stopping (including launches from the website).
/// A process snapshot costs about a millisecond and opens no handles.
/// Brings Vesper back (or quits) once Roblox has stayed closed for a moment;
/// Roblox briefly restarts itself when it updates.
fn after_roblox_closed(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        let state = app.state::<AppState>();
        if !crate::roblox::process::roblox_pids().is_empty() {
            state.hidden_for_roblox.store(true, Ordering::Relaxed);
            return;
        }
        let behaviour = lock(&state.settings).general.when_roblox_closes;
        log::info!("Roblox closed; Vesper will {behaviour:?}");
        match behaviour {
            crate::config::WhenRobloxCloses::Show => crate::tray::show_main(&app),
            crate::config::WhenRobloxCloses::Stay => {}
            crate::config::WhenRobloxCloses::Quit => {
                // Never close a window someone is using.
                let in_use = app.get_webview_window("main").is_some_and(|w| {
                    w.is_visible().unwrap_or(false)
                        && !w.is_minimized().unwrap_or(false)
                        && w.is_focused().unwrap_or(false)
                });
                if !in_use {
                    crate::tray::quit(&app);
                }
            }
        }
    });
}

pub fn spawn_roblox_watcher(app: AppHandle) {
    std::thread::Builder::new()
        .name("vesper-roblox".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_millis(2500));
            let state = app.state::<AppState>();
            if state.quitting.load(Ordering::Relaxed) {
                return;
            }
            let pids = crate::roblox::process::roblox_pids();
            let running = !pids.is_empty();
            if state.roblox_running.swap(running, Ordering::Relaxed) != running {
                log::info!("Roblox {}", if running { "started" } else { "closed" });
                let _ = app.emit(events::ROBLOX_RUNNING, serde_json::json!({ "running": running, "pids": pids }));
                if running {
                    crate::playing::start(&app);
                } else {
                    crate::playing::stop(&app);
                }
                state.sync_shaders(&app);
                state.refresh_presence();
                if !running {
                    let mut l = lock(&state.launch);
                    if matches!(*l, LaunchStatus::Running { .. }) {
                        *l = LaunchStatus::Idle;
                        let _ = app.emit(events::LAUNCH, &*l);
                    }
                    drop(l);
                    if state.hidden_for_roblox.swap(false, Ordering::Relaxed) {
                        after_roblox_closed(app.clone());
                    }
                }
                crate::tray::refresh(&app);
            }
        })
        .expect("spawn roblox watcher");
}
