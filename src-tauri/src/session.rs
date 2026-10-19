use crate::config::{AfterLaunch, FlagMode, LastLaunch, ServerPick};
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
use crate::roblox::watch;
use crate::roblox::{gbs, logs};
use crate::state::{events, lock, AppState, LaunchStatus};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};

const START_TIMEOUT: Duration = Duration::from_secs(60);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);
const VERIFY_TIMEOUT: Duration = Duration::from_secs(60);
const PICK_TIMEOUT: Duration = Duration::from_secs(25);

pub fn begin(app: AppHandle, target: LaunchTarget) -> Result<(), Problem> {
    start(app, target, false)
}

pub fn begin_restart(app: AppHandle, target: LaunchTarget) -> Result<(), Problem> {
    start(app, target, true)
}

fn start(app: AppHandle, target: LaunchTarget, restart: bool) -> Result<(), Problem> {
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
    let first = if restart { "Quitting Roblox" } else { "Checking Roblox" };
    state.set_launch(&app, LaunchStatus::Preparing { step: first.into() });
    std::thread::Builder::new()
        .name("vesper-launch".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            let result =
                if restart { close_roblox(&app).and_then(|()| run(&app, &target)) } else { run(&app, &target) };
            if let Err(problem) = result {
                log::warn!("Launch failed: {problem}");
                record_last(&app, &target, "failed", None);
                state.set_launch(&app, LaunchStatus::Failed { problem });
            }
        })
        .map_err(|e| Problem::internal("Couldn't start the launch.", e))?;
    Ok(())
}

fn close_roblox(app: &AppHandle) -> Result<(), Problem> {
    let state = app.state::<AppState>();
    if process::roblox_pids().is_empty() {
        return Ok(());
    }
    state.hidden_for_roblox.store(false, Ordering::Relaxed);
    if let Err(e) = process::force_quit_players() {
        return Err(Problem::new("roblox.wont_close", "Roblox didn't close")
            .what("Vesper force-quit Roblox so it could start again, but it's still running.")
            .why(if cfg!(target_os = "macos") {
                "macOS didn't let Vesper end it."
            } else {
                "Windows didn't let Vesper end it."
            })
            .fix(if cfg!(target_os = "macos") {
                "Force-quit Roblox from the Apple menu (Option-Command-Esc), then launch again."
            } else {
                "End Roblox in Task Manager, then launch again."
            })
            .details(e)
            .action(ProblemAction::Retry));
    }
    log::info!("Force-quit Roblox for a restart");
    crate::state::roblox_seen(app, &[]);
    std::thread::sleep(Duration::from_millis(1000));
    Ok(())
}

fn choose_server(app: &AppHandle, target: &LaunchTarget) -> LaunchTarget {
    let LaunchTarget::Experience { place_id, game_instance_id: None, link_code: None, launch_data } = target else {
        return target.clone();
    };
    let state = app.state::<AppState>();
    let prefs = state.settings().servers;
    if prefs.pick == ServerPick::Off {
        return target.clone();
    }
    let label = crate::servers::region_label(&prefs.region);
    step(
        app,
        &if prefs.pick == ServerPick::Region {
            format!("Finding a server in {label}")
        } else {
            "Finding the lowest-ping server".into()
        },
    );
    let result = tauri::async_runtime::block_on(async {
        tokio::time::timeout(
            PICK_TIMEOUT,
            crate::servers::pick(
                &state.http,
                &state.server_cache,
                &state.paths.datacenters_file(),
                *place_id,
                prefs.pick,
                &prefs.region,
            ),
        )
        .await
        .unwrap_or_else(|_| Err("finding a server took too long".into()))
    });
    match result {
        Ok(p) => {
            let ping = p.ms.map(|ms| format!(" (about {ms} ms)")).unwrap_or_default();
            log::info!("Picked a server in {}{ping}, {}/{} players", p.city, p.playing, p.max_players);
            lock(&state.notices).push(if p.fallback {
                format!("No servers in {label} right now, so Vesper picked the lowest-ping one: {}{ping}.", p.city)
            } else {
                format!("Joining a server in {}{ping}.", p.city)
            });
            LaunchTarget::Experience {
                place_id: *place_id,
                game_instance_id: Some(p.server_id),
                link_code: None,
                launch_data: launch_data.clone(),
            }
        }
        Err(e) => {
            log::info!("Server picking skipped: {e}");
            lock(&state.notices).push(format!("Vesper couldn't pick a server ({e}), so Roblox chose one as usual."));
            target.clone()
        }
    }
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
    if !process::roblox_pids().is_empty() {
        return Err(Problem::new("roblox.already_running", "Roblox is already open")
            .what("Roblox runs one copy at a time, so a second launch wouldn't start anything.")
            .why("Roblox keeps a single window open.")
            .fix("Use “Restart Roblox” on Home: Vesper closes Roblox and launches it again, into the same server if you like."));
    }
    let launched_at = SystemTime::now();

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
            Err(e) if cfg!(target_os = "macos") && e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(mac_app_blocked(&e));
            }
            Err(e) => {
                return Err(Problem::io("Vesper couldn't write Fast Flags into the Roblox folder.", &e)
                    .fix("Close Roblox if it's open and try again. Security software can also block writes to the Roblox folder.")
                    .action(ProblemAction::Retry));
            }
        }
    }

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

    if settings.roblox.apply_fps_cap {
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
                    Err(gbs::CapError::ReadOnly) => {
                        log::warn!("Couldn't set the frame-rate limit: {}", gbs::CapError::ReadOnly);
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

    let picked = choose_server(app, target);

    state.set_launch(app, LaunchStatus::Starting);
    let has_mods = *lock(&state.mods) != crate::mods::ModsConfig::default();
    let temp_watch = (settings.roblox.flag_mode != FlagMode::KeepApplied && (!expected.is_empty() || has_mods))
        .then(|| {
            let handle = app.clone();
            watch::watch_updates(&state.env, Some(&install), move |dir| {
                crate::state::apply_to_new_version(&handle, dir)
            })
            .ok()
        })
        .flatten();

    let request = SpawnRequest {
        exe: install.exe(),
        working_dir: install.dir.clone(),
        argument: launch_argument(&picked),
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
        let os = if cfg!(target_os = "macos") { "macOS" } else { "Windows" };
        lock(&state.notices)
            .push(format!("{os} didn't allow changing which processors Roblox uses ({err}). Roblox started normally."));
    }
    if let Some(err) = &spawned.priority_error {
        lock(&state.notices)
            .push(format!("Roblox's priority wasn't changed ({err}). Roblox started at normal priority."));
    }

    let pid = wait_for_player(app, spawned.pid)?;
    crate::state::roblox_seen(app, &[pid]);
    state.set_launch(app, LaunchStatus::Running { pid });
    state.refresh_presence();
    crate::tray::refresh(app);
    after_launch(app, settings.general.after_launch);

    let since = process::start_time(pid).map_or(launched_at, |t| t.max(launched_at));
    let version = if settings.roblox.verify_after_launch { verify(app, since, &expected) } else { None };
    drop(temp_watch);
    record_last(app, target, "started", version);
    state.redetect(app);
    Ok(())
}

fn mac_app_blocked(e: &std::io::Error) -> Problem {
    Problem::new("roblox.mac_app_management", "macOS blocked Vesper from changing Roblox")
        .what("Vesper couldn't write Fast Flags into Roblox.app.")
        .why("macOS only lets apps change other apps when you allow it under App Management.")
        .fix("Open System Settings → Privacy & Security → App Management, turn on Vesper, then launch again. If Roblox.app belongs to another user, reinstall it from roblox.com while signed in as yourself.")
        .details(e)
        .action(ProblemAction::Retry)
}

fn spawn_problem(install: &RobloxInstall, e: &std::io::Error) -> Problem {
    let mac = cfg!(target_os = "macos");
    match e.kind() {
        std::io::ErrorKind::NotFound => Problem::new("roblox.missing_exe", "Roblox's files are missing")
            .what(if mac {
                format!("Roblox wasn't found at {}.", install.location)
            } else {
                format!("{} wasn't found in {}.", crate::roblox::detect::PLAYER_EXE, install.location)
            })
            .why("Roblox may have been updated or uninstalled since Vesper last looked.")
            .fix(if mac {
                "Open Roblox once from your Applications folder so it can repair itself, then try again."
            } else {
                "Open Roblox once from the Start menu so it can repair itself, then try again."
            })
            .action(ProblemAction::Retry),
        _ if mac => Problem::new("roblox.mac_open_failed", "macOS couldn't open Roblox")
            .what("macOS refused to start Roblox.")
            .why("Roblox may be damaged, or macOS may be waiting for you to confirm opening it.")
            .fix("Open Roblox once from your Applications folder. If macOS asks, allow it, then try again. If that fails, reinstall Roblox from roblox.com.")
            .details(e)
            .action(ProblemAction::Retry)
            .action(ProblemAction::OpenUrl("https://www.roblox.com/download".into())),
        std::io::ErrorKind::PermissionDenied => Problem::new("roblox.blocked", "Windows blocked Roblox from starting")
            .what("Windows refused to start Roblox.")
            .why("Security software or a Windows policy may be blocking RobloxPlayerBeta.exe.")
            .fix("Check your antivirus quarantine and allow Roblox, then try again.")
            .details(e)
            .action(ProblemAction::Retry),
        _ => Problem::io("Roblox couldn't be started.", e),
    }
}

fn installer_running(all: &[process::ProcessSample]) -> bool {
    process::named(all, INSTALLER_PROCESS).next().is_some()
}

fn closed_immediately() -> Problem {
    Problem::new("roblox.closed_immediately", "Roblox closed right after starting")
        .what("Roblox opened and then closed on its own.")
        .why("A Fast Flag, a recent Roblox update or a graphics driver issue can make Roblox exit at startup.")
        .fix(if cfg!(target_os = "macos") {
            "Try the Balanced profile or turn off Fast Flags in Settings → Roblox. If it keeps happening, reinstall Roblox from roblox.com."
        } else {
            "Try the Balanced profile or turn off Fast Flags in Settings → Roblox. If it keeps happening, update your graphics driver."
        })
        .action(ProblemAction::OpenSettings("recovery".into()))
        .action(ProblemAction::Retry)
}

fn wait_for_player(app: &AppHandle, spawned: u32) -> Result<u32, Problem> {
    let state = app.state::<AppState>();
    let started = Instant::now();
    let direct = spawned != 0;
    let mut updating = false;
    let mut seen_spawned = false;
    let mut first_player: Option<(u32, Instant)> = None;
    let mut installer_gone_since: Option<Instant> = None;
    loop {
        let all = process::snapshot().unwrap_or_default();
        let players: Vec<u32> = process::named(&all, PLAYER_PROCESS).map(|p| p.pid).collect();
        if players.contains(&spawned) {
            seen_spawned = true;
        }
        if !direct && !updating {
            match (players.first(), first_player) {
                (Some(&p), None) => first_player = Some((p, Instant::now())),
                (Some(&p), Some((_, since))) if since.elapsed() > Duration::from_secs(3) => return Ok(p),
                (None, Some(_)) if process::named(&all, INSTALLER_PROCESS).next().is_none() => {
                    return Err(closed_immediately())
                }
                _ => {}
            }
        }
        if !updating && seen_spawned && players.contains(&spawned) && started.elapsed() > Duration::from_secs(3) {
            return Ok(spawned);
        }
        if updating {
            if let Some(p) = players.iter().copied().find(|p| *p != spawned && (direct || !installer_running(&all))) {
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
        if direct && spawned_gone && !installer && !updating && started.elapsed() > Duration::from_secs(2) {
            if let Some(p) = players.first() {
                return Ok(*p);
            }
            if seen_spawned || started.elapsed() > Duration::from_secs(5) {
                return Err(closed_immediately());
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
        let state = app.state::<AppState>();
        let hidden = match behaviour {
            AfterLaunch::Stay => false,
            AfterLaunch::Minimize => {
                state.minimized_for_roblox.store(true, Ordering::Relaxed);
                w.minimize().is_ok()
            }
            AfterLaunch::Tray => w.hide().is_ok(),
            AfterLaunch::Hidden => {
                let hidden = w.hide().is_ok();
                if hidden {
                    crate::tray::set_shown(app, false);
                }
                hidden
            }
        };
        if hidden {
            state.hidden_for_roblox.store(true, Ordering::Relaxed);
        }
    }
}

fn verify(app: &AppHandle, launched_at: SystemTime, expected: &FlagSet) -> Option<String> {
    let state = app.state::<AppState>();
    let dir = state.env.roblox_logs();
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
