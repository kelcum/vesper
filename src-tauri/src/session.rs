//! One launch of Roblox, from preparing files to confirming what Roblox accepted.
//!
//! Runs on a background thread and reports progress through `launch://status`
//! events, so the window never blocks.

use crate::config::{AfterLaunch, FlagMode, LastLaunch};
use crate::error::{Problem, ProblemAction};
use crate::flags::apply;
use crate::flags::model::FlagSet;
use crate::flags::validate::{has_errors, validate};
use crate::flags::verification::LaunchReport;
use crate::flags::Catalog;
use crate::roblox::deeplink::{launch_argument, LaunchTarget};
use crate::roblox::detect::RobloxInstall;
use crate::roblox::launch::{spawn, SpawnRequest};
use crate::roblox::process::{self, INSTALLER_PROCESS, PLAYER_PROCESS};
use crate::roblox::watch::VersionsWatcher;
use crate::roblox::{gbs, logs};
use crate::state::{events, lock, AppState, LaunchStatus};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};

const START_TIMEOUT: Duration = Duration::from_secs(60);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);
const VERIFY_TIMEOUT: Duration = Duration::from_secs(60);

/// Starts a launch in the background. Returns an error only if one is already running.
pub fn begin(app: AppHandle, target: LaunchTarget) -> Result<(), Problem> {
    let state = app.state::<AppState>();
    {
        let current = lock(&state.launch);
        if current.busy() {
            return Err(Problem::new("launch.busy", "Roblox is already launching")
                .what("A launch is still in progress.")
                .why("Vesper launches one Roblox window at a time.")
                .fix("Wait a moment for the current launch to finish."));
        }
    }
    state.set_launch(&app, LaunchStatus::Preparing { step: "Checking Roblox".into() });
    std::thread::Builder::new()
        .name("vesper-launch".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            if let Err(problem) = run(&app, &target) {
                log::warn!("Launch failed: {problem}");
                record_last(&app, &target, "failed", None);
                state.set_launch(&app, LaunchStatus::Failed { problem });
            }
        })
        .map_err(|e| Problem::internal("Couldn't start the launch.", e))?;
    Ok(())
}

fn step(app: &AppHandle, s: &str) {
    app.state::<AppState>().set_launch(app, LaunchStatus::Preparing { step: s.into() });
}

fn run(app: &AppHandle, target: &LaunchTarget) -> Result<(), Problem> {
    let state = app.state::<AppState>();

    if let LaunchTarget::ShareLink { url } = target {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|e| Problem::internal("Couldn't open the share link in your browser.", e))?;
        state.set_launch(app, LaunchStatus::Idle);
        return Ok(());
    }

    state.redetect(app);
    let install = state.require_install()?;
    let settings = state.settings();
    let profile = lock(&state.profiles).active().clone();
    let roblox_running = !process::roblox_pids().is_empty();
    let launched_at = SystemTime::now();

    // 1. Fast Flags
    let mut expected = FlagSet::new();
    if settings.roblox.flag_mode != FlagMode::Off {
        step(app, "Applying Fast Flags");
        let issues = validate(&profile.flags, Catalog::embedded());
        if has_errors(&issues) {
            let list: Vec<String> = issues
                .iter()
                .filter(|i| i.severity == crate::flags::validate::Severity::Error)
                .map(|i| format!("{}: {}", i.flag, i.message))
                .collect();
            return Err(Problem::new("flags.invalid", "This profile has conflicting flags")
                .what(format!("“{}” can't be applied as it is.", profile.name))
                .why(list.join(" "))
                .fix("Open Fast Flags and resolve the highlighted conflicts, then launch again.")
                .action(ProblemAction::OpenSettings("flags".into())));
        }
        let outcome = {
            let mut ledger = lock(&state.ownership);
            apply::apply(&install.dir, &profile.flags, &mut ledger, &state.paths.roblox_backups())
        };
        state.save_ownership();
        match outcome {
            Ok(o) => {
                if o.action == apply::ApplyAction::LeftForeignFile {
                    log::info!("Left a ClientAppSettings.json Vesper didn't write in place");
                }
                if let Some(b) = o.backup {
                    lock(&state.notices).push(format!(
                        "Roblox already had a Fast Flags file. Vesper kept a copy at {b} before replacing it."
                    ));
                }
                expected = profile.flags.clone();
            }
            Err(e) => {
                return Err(Problem::io("Vesper couldn't write Fast Flags into the Roblox folder.", &e)
                    .fix("Close Roblox if it's open and try again. Security software can also block writes to the Roblox folder.")
                    .action(ProblemAction::Retry));
            }
        }
    }

    // 2. Customisations (cursor, emote wheel, font, sounds). Roblox reads them at startup.
    if !roblox_running {
        if let Some(r) = state.apply_mods(&install.dir) {
            if !r.missing.is_empty() {
                lock(&state.notices).push(format!(
                    "This Roblox version doesn't have {} file(s) a customisation replaces, so they were skipped. Roblox may have moved them.",
                    r.missing.len()
                ));
            }
            for e in r.errors.iter().take(3) {
                lock(&state.notices).push(e.clone());
            }
        }
    }

    // 3. Frame-rate limit (Roblox's own setting; only while Roblox is closed).
    if settings.roblox.apply_fps_cap && !roblox_running {
        if let Some(cap) = profile.fps_cap {
            step(app, "Setting the frame-rate limit");
            match cap.resolve(state.system.display_hz) {
                Ok(value) => match gbs::set_cap(&state.env.roblox_data(), value) {
                    Ok(previous) => {
                        let mut s = lock(&state.settings);
                        if s.original_fps_cap.is_none() {
                            s.original_fps_cap = Some(previous.unwrap_or(gbs::DEFAULT_CAP));
                            drop(s);
                            let _ = state.save_settings();
                        }
                    }
                    Err(gbs::CapError::NoSettingsFile) => {
                        log::info!(
                            "Roblox hasn't created its settings file yet; the limit applies from the next launch"
                        );
                    }
                    Err(gbs::CapError::Invalid(reason)) if reason.contains("read-only") => {
                        log::warn!("Couldn't set the frame-rate limit: {reason}");
                        lock(&state.notices).push(
                            "The frame-rate limit wasn't applied: Roblox's settings file is locked read-only, usually by \
                             another launcher's \"lock settings\" option. Vesper leaves that lock alone; turn it off there to \
                             let Vesper set the limit."
                                .into(),
                        );
                    }
                    Err(e) => {
                        log::warn!("Couldn't set the frame-rate limit: {e}");
                        lock(&state.notices).push(format!("The frame-rate limit wasn't applied: {e}."));
                    }
                },
                Err(e) => log::warn!("Frame-rate limit skipped: {e}"),
            }
        }
    }

    // 4. Start Roblox.
    state.set_launch(app, LaunchStatus::Starting);
    // If Roblox updates itself during this launch, carry the flags into the new folder.
    let has_mods = *lock(&state.mods) != crate::mods::ModsConfig::default();
    let temp_watch = (settings.roblox.flag_mode != FlagMode::KeepApplied && (!expected.is_empty() || has_mods))
        .then(|| {
            let handle = app.clone();
            VersionsWatcher::start(&state.env.roblox_data().join("Versions"), move |dir| {
                crate::state::apply_to_new_version(&handle, dir)
            })
            .ok()
        })
        .flatten();

    let request = SpawnRequest {
        exe: install.exe(),
        working_dir: install.dir.clone(),
        argument: launch_argument(target),
        priority: profile.priority,
        affinity: profile.affinity,
    };
    let spawned = spawn(&request).map_err(|e| spawn_problem(&install, &e))?;
    log::info!(
        "Started Roblox {} (pid {}, priority {:?})",
        install.version.as_deref().unwrap_or("?"),
        spawned.pid,
        profile.priority
    );
    if let Some(err) = &spawned.affinity_error {
        lock(&state.notices).push(format!(
            "Windows didn't allow changing which processors Roblox uses ({err}). Roblox started normally."
        ));
    }

    // 5. Wait for the player window process (Roblox may update itself first).
    let pid = wait_for_player(app, spawned.pid)?;
    state.roblox_running.store(true, Ordering::Relaxed);
    let _ = app.emit(events::ROBLOX_RUNNING, serde_json::json!({ "running": true, "pids": [pid] }));
    state.set_launch(app, LaunchStatus::Running { pid });
    state.refresh_presence();
    crate::tray::refresh(app);
    after_launch(app, settings.general.after_launch);

    // 6. Confirm with Roblox's own log. Read the log of the client that is
    // actually running: if Roblox updated itself, the first client's short
    // log (written before it handed off to the updater) must be ignored.
    let since = process::start_time(pid).map_or(launched_at, |t| t.max(launched_at));
    let version = if settings.roblox.verify_after_launch { verify(app, since, &expected) } else { None };
    drop(temp_watch);
    record_last(app, target, "started", version);
    state.redetect(app);
    Ok(())
}

fn spawn_problem(install: &RobloxInstall, e: &std::io::Error) -> Problem {
    match e.kind() {
        std::io::ErrorKind::NotFound => Problem::new("roblox.missing_exe", "Roblox's files are missing")
            .what(format!("{} wasn't found in {}.", crate::roblox::detect::PLAYER_EXE, install.location))
            .why("Roblox may have been updated or uninstalled since Vesper last looked.")
            .fix("Open Roblox once from the Start menu so it can repair itself, then try again.")
            .action(ProblemAction::Retry),
        std::io::ErrorKind::PermissionDenied => Problem::new("roblox.blocked", "Windows blocked Roblox from starting")
            .what("Windows refused to start Roblox.")
            .why("Security software or a Windows policy may be blocking RobloxPlayerBeta.exe.")
            .fix("Check your antivirus quarantine and allow Roblox, then try again.")
            .details(e)
            .action(ProblemAction::Retry),
        _ => Problem::io("Roblox couldn't be started.", e),
    }
}

fn wait_for_player(app: &AppHandle, spawned: u32) -> Result<u32, Problem> {
    let state = app.state::<AppState>();
    let started = Instant::now();
    let mut updating = false;
    let mut seen_spawned = false;
    let mut installer_gone_since: Option<Instant> = None;
    loop {
        let all = process::snapshot().unwrap_or_default();
        let players: Vec<u32> = process::named(&all, PLAYER_PROCESS).map(|p| p.pid).collect();
        if players.contains(&spawned) {
            seen_spawned = true;
        }
        // Give the client a moment so a crash on startup is caught here.
        if !updating && seen_spawned && players.contains(&spawned) && started.elapsed() > Duration::from_secs(3) {
            return Ok(spawned);
        }
        if updating {
            if let Some(p) = players.iter().copied().find(|p| *p != spawned) {
                return Ok(p);
            }
        }
        let installer = process::named(&all, INSTALLER_PROCESS).next().is_some();
        if installer && !updating {
            updating = true;
            log::info!("Roblox is updating itself before opening");
            state.set_launch(app, LaunchStatus::Updating);
        }
        if updating {
            if installer || !players.is_empty() {
                installer_gone_since = None;
            } else if installer_gone_since.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(15) {
                return Err(Problem::new("roblox.update_incomplete", "Roblox's update didn't finish")
                    .what("Roblox's installer closed without opening Roblox.")
                    .why("The update may have been cancelled, or the download failed.")
                    .fix(
                        "Launch again to let Roblox retry its update. If that fails, reinstall Roblox from roblox.com.",
                    )
                    .action(ProblemAction::Retry)
                    .action(ProblemAction::OpenUrl("https://www.roblox.com/download".into())));
            }
        }
        let spawned_gone = !players.contains(&spawned);
        if spawned_gone && !installer && !updating && started.elapsed() > Duration::from_secs(2) {
            // The spawned process handed off to another one.
            if let Some(p) = players.first() {
                return Ok(*p);
            }
            if seen_spawned || started.elapsed() > Duration::from_secs(5) {
                return Err(Problem::new("roblox.closed_immediately", "Roblox closed right after starting")
                    .what("Roblox opened and then closed on its own.")
                    .why("A Fast Flag, a recent Roblox update or a graphics driver issue can make Roblox exit at startup.")
                    .fix("Try the Balanced profile or turn off Fast Flags in Settings → Roblox. If it keeps happening, update your graphics driver.")
                    .action(ProblemAction::OpenSettings("recovery".into()))
                    .action(ProblemAction::Retry));
            }
        }
        let limit = if updating { UPDATE_TIMEOUT } else { START_TIMEOUT };
        if started.elapsed() > limit {
            return Err(Problem::new("roblox.didnt_start", "Roblox didn't open")
                .what(format!("Roblox didn't start within {} seconds.", limit.as_secs()))
                .why("Roblox may be stuck updating, or blocked by security software.")
                .fix("Check whether a Roblox window or installer is waiting in the background, then try again.")
                .action(ProblemAction::Retry));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn after_launch(app: &AppHandle, behaviour: AfterLaunch) {
    if let Some(w) = app.get_webview_window("main") {
        match behaviour {
            AfterLaunch::Stay => {}
            AfterLaunch::Minimize => {
                let _ = w.minimize();
            }
            AfterLaunch::Tray => {
                let _ = w.hide();
            }
        }
    }
}

/// Reads Roblox's log until it has reported on the flags, then records the verdicts.
fn verify(app: &AppHandle, launched_at: SystemTime, expected: &FlagSet) -> Option<String> {
    let state = app.state::<AppState>();
    let dir = state.env.roblox_data().join("logs");
    let started = Instant::now();
    let mut last: Option<(Option<String>, logs::FlagLoad)> = None;
    while started.elapsed() < VERIFY_TIMEOUT {
        if let Some(path) = logs::newest_player_log_since(&dir, launched_at) {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if let Ok(text) = std::fs::read_to_string(&path) {
                let load = logs::parse_flag_load(&text);
                let done = load.finished;
                last = Some((logs::version_from_log_name(&name), load));
                if done {
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let (version, load) = last?;
    if expected.is_empty() {
        return version;
    }
    let verdicts = logs::verdicts(expected, &load);
    let report = LaunchReport::from_verdicts(version.clone(), &verdicts, load.finished);
    log::info!(
        "Roblox {} accepted {} flag(s), denied {}, didn't read {}",
        version.as_deref().unwrap_or("?"),
        report.accepted.len(),
        report.denied.len(),
        report.not_read.len()
    );
    lock(&state.verification).record(report.clone(), &verdicts);
    state.save_verification();
    let _ = app.emit(events::VERIFIED, &report);
    version
}

fn record_last(app: &AppHandle, target: &LaunchTarget, outcome: &str, version: Option<String>) {
    let state = app.state::<AppState>();
    let profile = lock(&state.profiles).active().name.clone();
    let target = match target {
        LaunchTarget::App => "Roblox app".to_owned(),
        LaunchTarget::Experience { place_id, .. } => format!("Experience {place_id}"),
        LaunchTarget::ShareLink { .. } => "Share link".to_owned(),
    };
    lock(&state.settings).last_launch =
        Some(LastLaunch { at: chrono::Utc::now(), roblox_version: version, profile, target, outcome: outcome.into() });
    let _ = state.save_settings();
    let _ = app.emit(events::SETTINGS, &state.settings());
}
