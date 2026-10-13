//! Commands the interface can call. Every failure is a [`Problem`].

use crate::backup::{self, BackupInfo, Bundle, Reason};
use crate::cleanup::{self, CleanupSummary};
use crate::config::{FlagMode, Settings};
use crate::diagnostics;
use crate::discord::presence::PresenceStatus;
use crate::error::{Problem, ProblemAction, Result};
use crate::flags::apply;
use crate::flags::catalog::Catalog;
use crate::flags::model::FlagSet;
use crate::flags::validate::{validate, Issue};
use crate::flags::verification::VerificationStore;
use crate::logging::{self, LogEntry};
use crate::perf::system::SystemInfo;
use crate::profiles::{Profile, ProfileError, ProfilePatch, ProfileStore};
use crate::roblox::cdn::{self, LatestVersion, PublishedValues};
use crate::roblox::deeplink::{self, LaunchTarget};
use crate::roblox::detect::{self, Detection};
use crate::roblox::gbs;
use crate::state::{events, lock, save_published, AppState, LaunchStatus};
use crate::updates::{self, UpdateState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

fn profile_problem(e: ProfileError) -> Problem {
    let title = match e {
        ProfileError::NotFound => "Profile not found",
        ProfileError::BuiltinNotDeletable => "Built-in profiles stay",
        ProfileError::InvalidImport(_) => "That file can't be imported",
        _ => "That change can't be made",
    };
    Problem::new("profiles", title)
        .what(e.to_string())
        .why("The profile didn't pass validation.")
        .fix("Adjust the value and try again.")
}

fn after_profiles_change(app: &AppHandle, state: &AppState) -> Result<ProfileStore> {
    state.save_profiles()?;
    let store = lock(&state.profiles).clone();
    let _ = app.emit(events::PROFILES, &store);
    state.refresh_presence();
    crate::tray::refresh(app);
    Ok(store)
}

// ------------------------------------------------------------------ bootstrap

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub version: String,
    pub portable: bool,
    pub settings: Settings,
    pub profiles: ProfileStore,
    pub detection: Detection,
    pub catalog: Catalog,
    pub verification: VerificationStore,
    pub system: SystemInfo,
    pub presence: PresenceStatus,
    pub discord_configured: bool,
    pub update: UpdateState,
    pub launch: LaunchStatus,
    pub roblox_running: bool,
    pub latest_roblox: Option<LatestVersion>,
    pub published: Option<PublishedValues>,
    pub notices: Vec<String>,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap> {
    Ok(Bootstrap {
        version: env!("CARGO_PKG_VERSION").into(),
        portable: state.paths.portable,
        settings: state.settings(),
        profiles: lock(&state.profiles).clone(),
        detection: lock(&state.detection).clone(),
        catalog: Catalog::embedded().clone(),
        verification: lock(&state.verification).clone(),
        system: state.system.clone(),
        presence: state.presence_status(),
        discord_configured: crate::discord::client_id().is_some(),
        update: lock(&state.update).clone(),
        launch: lock(&state.launch).clone(),
        roblox_running: state.roblox_running.load(Ordering::Relaxed),
        latest_roblox: lock(&state.latest).clone(),
        published: lock(&state.published).clone(),
        notices: std::mem::take(&mut *lock(&state.notices)),
    })
}

#[tauri::command]
pub fn notices_take(state: State<'_, AppState>) -> Vec<String> {
    std::mem::take(&mut *lock(&state.notices))
}

// ------------------------------------------------------------------ settings

fn merge(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                merge(b.entry(k).or_insert(Value::Null), v);
            }
        }
        (slot, v) => *slot = v,
    }
}

/// Applies a partial settings object, e.g. `{"discord": {"richPresence": true}}`.
#[tauri::command]
pub fn settings_update(app: AppHandle, state: State<'_, AppState>, patch: Value) -> Result<Settings> {
    let old = state.settings();
    let mut json = serde_json::to_value(&old).map_err(|e| Problem::internal("Couldn't read settings.", e))?;
    merge(&mut json, patch);
    let mut new: Settings = serde_json::from_value(json).map_err(|e| {
        Problem::invalid("One of the settings has an unexpected value.", "Reload Vesper and try again.").details(e)
    })?;
    new.sanitize();
    // Fields the interface can't change directly.
    new.last_launch = old.last_launch.clone();
    new.original_fps_cap = old.original_fps_cap;
    if let Some(dir) = &new.roblox.install_dir {
        if old.roblox.install_dir.as_ref() != Some(dir) {
            detect::resolve_custom(dir).map_err(|e| {
                Problem::new("roblox.custom_folder", "Roblox isn't in that folder")
                    .what(e)
                    .why(if cfg!(target_os = "macos") {
                        "Choose Roblox.app, or the folder that holds it."
                    } else {
                        "The folder needs to contain RobloxPlayerBeta.exe, or version folders that do."
                    })
                    .fix("Choose the Roblox Versions folder or a version-… folder inside it.")
            })?;
        }
    }
    *lock(&state.settings) = new.clone();
    state.save_settings()?;

    if old.general.start_with_windows != new.general.start_with_windows {
        use tauri_plugin_autostart::ManagerExt;
        let r = if new.general.start_with_windows { app.autolaunch().enable() } else { app.autolaunch().disable() };
        if let Err(e) = r {
            log::warn!("Couldn't change starting at sign-in: {e}");
        }
    }
    if old.advanced.log_level != new.advanced.log_level {
        logging::set_level(logging::parse_level(&new.advanced.log_level));
    }
    if old.discord != new.discord {
        state.refresh_presence();
    }
    if old.roblox.flag_mode != new.roblox.flag_mode {
        state.sync_keep_applied(&app);
    }
    if old.roblox.install_dir != new.roblox.install_dir {
        state.redetect(&app);
    }
    if old.performance != new.performance {
        state.restart_monitor(&app);
    }
    let _ = app.emit(events::SETTINGS, &new);
    Ok(new)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstRunChoices {
    pub appearance: crate::config::Appearance,
    pub profile: String,
    pub rich_presence: bool,
}

#[tauri::command]
pub fn first_run_finish(app: AppHandle, state: State<'_, AppState>, choices: FirstRunChoices) -> Result<Bootstrap> {
    {
        let mut s = lock(&state.settings);
        s.appearance = choices.appearance;
        s.discord.rich_presence = choices.rich_presence;
        s.first_run_complete = true;
        s.sanitize();
    }
    state.save_settings()?;
    {
        let mut p = lock(&state.profiles);
        p.set_active(&choices.profile).map_err(profile_problem)?;
        p.set_default(&choices.profile).map_err(profile_problem)?;
    }
    after_profiles_change(&app, &state)?;
    bootstrap(state)
}

// ------------------------------------------------------------------ roblox

#[tauri::command]
pub fn roblox_detect(app: AppHandle, state: State<'_, AppState>) -> Detection {
    state.redetect(&app)
}

#[tauri::command]
pub fn roblox_parse_target(input: String) -> Result<LaunchTarget> {
    deeplink::parse(&input)
        .map_err(|e| Problem::invalid(e, "Paste a Roblox game link, a place ID, or leave it empty to open Roblox."))
}

#[tauri::command]
pub fn roblox_launch(app: AppHandle, input: Option<String>) -> Result<()> {
    let target = roblox_parse_target(input.unwrap_or_default())?;
    crate::session::begin(app, target)
}

#[tauri::command]
pub fn roblox_dismiss_launch(app: AppHandle, state: State<'_, AppState>) {
    let mut l = lock(&state.launch);
    if matches!(*l, LaunchStatus::Failed { .. }) {
        *l = LaunchStatus::Idle;
        let _ = app.emit(events::LAUNCH, &*l);
    }
}

#[tauri::command]
pub async fn roblox_latest(state: State<'_, AppState>, refresh: bool) -> Result<Option<LatestVersion>> {
    let cached = lock(&state.latest).clone();
    let fresh =
        cached.as_ref().is_some_and(|c| chrono::Utc::now().signed_duration_since(c.fetched_at).num_minutes() < 10);
    if fresh && !refresh {
        return Ok(cached);
    }
    match cdn::latest_version(&state.http).await {
        Ok(v) => {
            *lock(&state.latest) = Some(v.clone());
            Ok(Some(v))
        }
        Err(e) => {
            log::info!("Couldn't check Roblox's latest version: {e}");
            Ok(cached)
        }
    }
}

// ------------------------------------------------------------------ flags

#[tauri::command]
pub fn flags_validate(flags: FlagSet) -> Vec<Issue> {
    validate(&flags, Catalog::embedded())
}

#[tauri::command]
pub async fn flags_published(state: State<'_, AppState>, refresh: bool) -> Result<Option<PublishedValues>> {
    let cached = lock(&state.published).clone();
    let fresh =
        cached.as_ref().is_some_and(|c| chrono::Utc::now().signed_duration_since(c.fetched_at).num_hours() < 12);
    if fresh && !refresh {
        return Ok(cached);
    }
    let names: Vec<&str> = Catalog::embedded().flags.iter().map(|f| f.name.as_str()).collect();
    match cdn::published_values(&state.http, &names).await {
        Ok(v) => {
            save_published(&state.paths, &v);
            *lock(&state.published) = Some(v.clone());
            Ok(Some(v))
        }
        Err(e) => {
            log::info!("Couldn't fetch Roblox's published flag values: {e}");
            Ok(cached)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFlags {
    pub file: Option<String>,
    pub flags: Option<FlagSet>,
    pub error: Option<String>,
    pub written_by_vesper: bool,
}

/// What's in the Roblox install's ClientAppSettings.json right now.
#[tauri::command]
pub fn flags_installed(state: State<'_, AppState>) -> InstalledFlags {
    let Some(install) = state.preferred_install() else {
        return InstalledFlags { file: None, flags: None, error: None, written_by_vesper: false };
    };
    let path = apply::settings_file(&install.dir);
    let bytes = std::fs::read(&path).ok();
    let owned = bytes.as_ref().is_some_and(|b| lock(&state.ownership).owns(&path, b));
    let (flags, error) = match apply::read_current(&install.dir) {
        None => (None, None),
        Some(Ok(f)) => (Some(f), None),
        Some(Err(e)) => (None, Some(e)),
    };
    InstalledFlags { file: Some(crate::redact::display_path(&path)), flags, error, written_by_vesper: owned }
}

/// Recovery: removes the flags Vesper wrote from every Roblox folder.
#[tauri::command]
pub fn flags_remove_applied(app: AppHandle, state: State<'_, AppState>, force: bool) -> CleanupSummary {
    let summary = {
        let mut ledger = lock(&state.ownership);
        // Also cover the current install if it has a file Vesper never recorded.
        if force {
            if let Some(i) = state.preferred_install() {
                let _ = apply::remove(&i.dir, &mut ledger, true, &state.paths.roblox_backups());
            }
        }
        cleanup::remove_all_flags(&mut ledger, force, &state.paths.roblox_backups())
    };
    state.save_ownership();
    state.redetect(&app);
    log::info!("Reset Fast Flags: {summary:?}");
    summary
}

// ------------------------------------------------------------------ profiles

#[tauri::command]
pub fn profiles_get(state: State<'_, AppState>) -> ProfileStore {
    lock(&state.profiles).clone()
}

#[tauri::command]
pub fn profiles_set_active(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<ProfileStore> {
    lock(&state.profiles).set_active(&id).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_set_default(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<ProfileStore> {
    lock(&state.profiles).set_default(&id).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_create(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    from: Option<String>,
) -> Result<ProfileStore> {
    lock(&state.profiles).create(&name, from.as_deref()).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_duplicate(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<ProfileStore> {
    lock(&state.profiles).duplicate(&id).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    patch: ProfilePatch,
) -> Result<ProfileStore> {
    lock(&state.profiles).update(&id, patch).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<ProfileStore> {
    lock(&state.profiles).delete(&id).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_reset(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<ProfileStore> {
    lock(&state.profiles).reset(&id).map_err(profile_problem)?;
    after_profiles_change(&app, &state)
}

#[tauri::command]
pub fn profiles_export(state: State<'_, AppState>, id: String, path: PathBuf) -> Result<()> {
    let json = lock(&state.profiles).export(&id).map_err(profile_problem)?;
    std::fs::write(&path, json).map_err(|e| Problem::io("Couldn't save the profile file.", &e))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub store: ProfileStore,
    pub profile: Profile,
    pub issues: Vec<Issue>,
}

#[tauri::command]
pub fn profiles_import(app: AppHandle, state: State<'_, AppState>, path: PathBuf) -> Result<ImportOutcome> {
    let text = std::fs::read_to_string(&path).map_err(|e| Problem::io("Couldn't read that file.", &e))?;
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported".into());
    let r = lock(&state.profiles).import(&text, &name, Catalog::embedded()).map_err(profile_problem)?;
    let store = after_profiles_change(&app, &state)?;
    Ok(ImportOutcome { store, profile: r.profile, issues: r.issues })
}

// ------------------------------------------------------------------ frame-rate cap

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FpsCapStatus {
    pub current: Option<i32>,
    pub original: Option<i32>,
    pub display_hz: Option<u32>,
    pub error: Option<String>,
    /// Roblox's settings file is read-only (often another launcher's lock).
    pub locked: bool,
}

fn fps_status(state: &AppState) -> FpsCapStatus {
    let (current, error) = match gbs::current_cap(&state.env.roblox_data()) {
        Ok(c) => (c, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let locked = gbs::settings_path(&state.env.roblox_data()).is_some_and(|p| gbs::is_read_only(&p));
    FpsCapStatus {
        current,
        original: lock(&state.settings).original_fps_cap,
        display_hz: state.system.display_hz,
        error,
        locked,
    }
}

#[tauri::command]
pub fn fps_cap_status(state: State<'_, AppState>) -> FpsCapStatus {
    fps_status(&state)
}

/// Puts Roblox's frame-rate setting back to what it was before Vesper changed it.
#[tauri::command]
pub fn fps_cap_restore(state: State<'_, AppState>) -> Result<FpsCapStatus> {
    if state.roblox_running.load(Ordering::Relaxed) {
        return Err(Problem::new("fps.roblox_running", "Close Roblox first")
            .what("Roblox is running.")
            .why("Roblox saves its settings when it closes and would overwrite the change.")
            .fix("Close Roblox, then try again."));
    }
    let original = lock(&state.settings).original_fps_cap.unwrap_or(gbs::DEFAULT_CAP);
    match gbs::set_cap(&state.env.roblox_data(), original) {
        // Nothing to put back: Roblox will create the file with its own default.
        Ok(_) | Err(gbs::CapError::NoSettingsFile) => {}
        Err(gbs::CapError::ReadOnly) => {
            return Err(Problem::new("fps.locked", "Roblox's settings file is locked")
                .what("Vesper couldn't put Roblox's frame-rate setting back because the file is read-only.")
                .why("Another launcher's \"lock settings\" option usually does this. Vesper leaves that lock alone.")
                .fix("Turn the lock off in the launcher that set it, then try again."));
        }
        Err(gbs::CapError::Io(e)) => {
            return Err(Problem::io("Couldn't restore Roblox's frame-rate setting.", &e).action(ProblemAction::Retry));
        }
        Err(e) => return Err(Problem::internal("Couldn't restore Roblox's frame-rate setting.", e)),
    }
    lock(&state.settings).original_fps_cap = None;
    state.save_settings()?;
    Ok(fps_status(&state))
}

// ------------------------------------------------------------------ performance

/// The interface reports which views need live data; sampling stops when none do.
#[tauri::command]
pub fn perf_watch(app: AppHandle, state: State<'_, AppState>, source: String, active: bool) {
    {
        let mut m = lock(&state.monitor);
        if active {
            m.sources.insert(source);
        } else {
            m.sources.remove(&source);
        }
    }
    state.sync_monitor(&app);
}

#[tauri::command]
pub fn system_info(state: State<'_, AppState>) -> SystemInfo {
    state.system.clone()
}

// ------------------------------------------------------------------ backups & recovery

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub settings: Settings,
    pub profiles: ProfileStore,
    pub safety_backup: BackupInfo,
}

fn snapshot_bundle(state: &AppState, reason: Reason) -> Bundle {
    Bundle::new(reason, &state.settings(), &lock(&state.profiles))
}

fn safety_backup(state: &AppState, reason: Reason) -> Result<BackupInfo> {
    backup::create(&state.paths.backups, &snapshot_bundle(state, reason))
        .map_err(|e| Problem::io("Couldn't create a safety backup, so nothing was changed.", &e))
}

fn restore_bundle(app: &AppHandle, state: &AppState, bundle: Bundle, reason: Reason) -> Result<RestoreOutcome> {
    let safety = safety_backup(state, reason)?;
    let mut settings = bundle.settings;
    {
        let current = lock(&state.settings);
        // Keep machine history and the remembered original cap from this PC.
        settings.last_launch = current.last_launch.clone();
        settings.original_fps_cap = current.original_fps_cap;
    }
    *lock(&state.settings) = settings.clone();
    *lock(&state.profiles) = bundle.profiles;
    state.save_settings()?;
    let profiles = after_profiles_change(app, state)?;
    state.sync_keep_applied(app);
    state.restart_monitor(app);
    state.redetect(app);
    let _ = app.emit(events::SETTINGS, &settings);
    Ok(RestoreOutcome { settings, profiles, safety_backup: safety })
}

#[tauri::command]
pub fn backups_list(state: State<'_, AppState>) -> Vec<BackupInfo> {
    backup::list(&state.paths.backups)
}

#[tauri::command]
pub fn backups_create(state: State<'_, AppState>) -> Result<BackupInfo> {
    backup::create(&state.paths.backups, &snapshot_bundle(&state, Reason::Manual))
        .map_err(|e| Problem::io("Couldn't create the backup.", &e))
}

#[tauri::command]
pub fn backups_restore(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<RestoreOutcome> {
    let bundle = backup::load(&state.paths.backups, &id).map_err(|e| {
        Problem::new("backup.unreadable", "That backup can't be restored").what(e).fix("Choose another backup.")
    })?;
    restore_bundle(&app, &state, bundle, Reason::BeforeRestore)
}

#[tauri::command]
pub fn backups_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    backup::delete(&state.paths.backups, &id).map_err(|e| Problem::io("Couldn't delete the backup.", &e))
}

#[tauri::command]
pub fn backups_export(state: State<'_, AppState>, id: String, path: PathBuf) -> Result<()> {
    backup::export(&state.paths.backups, &id, &path).map_err(|e| {
        Problem::new("backup.export", "Couldn't export the backup").what(e).fix("Choose another location.")
    })
}

#[tauri::command]
pub fn backups_import(app: AppHandle, state: State<'_, AppState>, path: PathBuf) -> Result<RestoreOutcome> {
    let bytes = std::fs::read(&path).map_err(|e| Problem::io("Couldn't read that file.", &e))?;
    let bundle = Bundle::parse(&bytes).map_err(|e| {
        Problem::new("backup.invalid", "That file can't be restored")
            .what(e)
            .fix("Choose a .json backup created by Vesper.")
    })?;
    restore_bundle(&app, &state, bundle, Reason::BeforeImport)
}

#[tauri::command]
pub fn recovery_reset_settings(app: AppHandle, state: State<'_, AppState>) -> Result<RestoreOutcome> {
    let mut fresh = Settings { first_run_complete: true, ..Settings::default() };
    fresh.general.tray_hint_shown = lock(&state.settings).general.tray_hint_shown;
    let bundle = Bundle::new(Reason::BeforeReset, &fresh, &ProfileStore::default());
    restore_bundle(&app, &state, bundle, Reason::BeforeReset)
}

// ------------------------------------------------------------------ folders, links, diagnostics

#[tauri::command]
pub fn open_folder(app: AppHandle, state: State<'_, AppState>, kind: String) -> Result<()> {
    let path = match kind.as_str() {
        "config" => state.paths.config.clone(),
        "logs" => state.paths.logs.clone(),
        "backups" => state.paths.backups.clone(),
        "roblox" => state.preferred_install().map(|i| i.dir).ok_or_else(|| {
            Problem::new("roblox.not_installed", "Roblox isn't installed").what("There's no Roblox folder to open.")
        })?,
        "robloxLogs" => state.env.roblox_logs(),
        _ => return Err(Problem::invalid("Unknown folder.", "This is a bug in Vesper.")),
    };
    std::fs::create_dir_all(&path).ok();
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| Problem::internal("Couldn't open the folder.", e))
}

const ALLOWED_HOSTS: &[&str] = &[
    "xan.gripe",
    "github.com",
    "www.roblox.com",
    "roblox.com",
    "create.roblox.com",
    "devforum.roblox.com",
    "clientsettingscdn.roblox.com",
    "discord.com",
    "learn.microsoft.com",
    "support.microsoft.com",
    // Free fonts for the Customize page.
    "fonts.google.com",
    "www.fontshare.com",
];

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<()> {
    let parsed =
        url::Url::parse(&url).map_err(|_| Problem::invalid("That link isn't valid.", "This is a bug in Vesper."))?;
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https" || !ALLOWED_HOSTS.contains(&host) {
        return Err(Problem::invalid("Vesper only opens links to sites it knows.", "This is a bug in Vesper."));
    }
    app.opener()
        .open_url(parsed.as_str(), None::<&str>)
        .map_err(|e| Problem::internal("Couldn't open your browser.", e))
}

#[tauri::command]
pub fn diagnostics_text(state: State<'_, AppState>) -> String {
    let settings = state.settings();
    let detection = lock(&state.detection).clone();
    let profile = lock(&state.profiles).active().clone();
    let install = detection.preferred_install().cloned();
    let snapshot = diagnostics::Snapshot {
        app_version: env!("CARGO_PKG_VERSION").into(),
        portable: state.paths.portable,
        os: state.system.os.clone(),
        cpu: state.system.cpu.clone(),
        logical_processors: state.system.logical_processors,
        memory_total: state.system.memory_total,
        gpus: state.system.gpus.iter().map(|g| g.name.clone()).collect(),
        display_hz: state.system.display_hz,
        roblox: install.as_ref().map(|i| diagnostics::RobloxFacts {
            version: i.version.clone(),
            version_hash: i.version_hash.clone(),
            location: i.location.clone(),
            source: format!("{:?}", i.source),
            registered: i.registered,
        }),
        other_installs: detection.installs.len().saturating_sub(1),
        latest_roblox: lock(&state.latest).as_ref().map(|l| format!("{} ({})", l.version, l.version_hash)),
        protocol_owner: detection.protocol.as_ref().map(|p| p.owner.clone()),
        roblox_running: state.roblox_running.load(Ordering::Relaxed),
        profile: profile.name.clone(),
        flags: profile.flags.clone(),
        fps_cap: profile.fps_cap.map(|c| format!("{c:?}")).unwrap_or_else(|| "Not changed".into()),
        current_frame_cap: gbs::current_cap(&state.env.roblox_data()).ok().flatten(),
        flag_mode: format!("{:?}", settings.roblox.flag_mode),
        last_report: lock(&state.verification).last_report.clone(),
        discord: format!(
            "{:?}{}",
            state.presence_status(),
            if crate::discord::client_id().is_some() { "" } else { " (no application ID in this build)" }
        ),
        updates: lock(&state.update).describe(),
        log_tail: logging::tail(40),
    };
    diagnostics::render(&snapshot)
}

#[tauri::command]
pub fn logs_tail(limit: usize) -> Vec<LogEntry> {
    logging::tail(limit.min(400))
}

// ------------------------------------------------------------------ discord

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordInfo {
    pub status: PresenceStatus,
    pub configured: bool,
}

#[tauri::command]
pub fn discord_status(state: State<'_, AppState>) -> DiscordInfo {
    DiscordInfo { status: state.presence_status(), configured: crate::discord::client_id().is_some() }
}

// ------------------------------------------------------------------ updates

fn set_update(app: &AppHandle, state: &AppState, s: UpdateState) {
    *lock(&state.update) = s.clone();
    let _ = app.emit(events::UPDATE, &s);
}

#[tauri::command]
pub async fn updates_check(app: AppHandle, state: State<'_, AppState>) -> Result<UpdateState> {
    use tauri_plugin_updater::UpdaterExt;
    set_update(&app, &state, UpdateState::Checking);
    let channel = lock(&state.settings).updates.channel;
    let url = url::Url::parse(updates::endpoint(channel)).map_err(|e| Problem::internal("Bad update URL.", e))?;
    let result =
        match app.updater_builder().endpoints(vec![url]).and_then(|b| b.timeout(Duration::from_secs(20)).build()) {
            Ok(updater) => updater.check().await.map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
    let next = match result {
        Ok(Some(update)) => {
            let s = UpdateState::Available {
                version: update.version.clone(),
                notes: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
            };
            *lock(&state.pending_update) = Some(update);
            s
        }
        Ok(None) => UpdateState::UpToDate { checked_at: chrono::Utc::now() },
        Err(e) => {
            log::info!("Update check failed: {e}");
            UpdateState::Failed { message: updates::friendly_error(&e) }
        }
    };
    set_update(&app, &state, next.clone());
    Ok(next)
}

/// Downloads, verifies and installs the pending update, then restarts Vesper.
#[tauri::command]
pub async fn updates_install(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let update = lock(&state.pending_update).take().ok_or_else(|| {
        Problem::new("update.none", "No update to install")
            .what("Check for updates first.")
            .fix("Press “Check for updates”.")
    })?;
    let mut downloaded = 0u64;
    let progress_app = app.clone();
    let finish_app = app.clone();
    let result = update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let s = UpdateState::Downloading { downloaded, total };
                let _ = progress_app.emit(events::UPDATE, &s);
            },
            move || {
                let _ = finish_app.emit(events::UPDATE, &UpdateState::Installing);
            },
        )
        .await;
    match result {
        Ok(()) => {
            log::info!("Update installed; restarting");
            app.restart();
        }
        Err(e) => {
            let message = updates::friendly_error(&e.to_string());
            log::warn!("Update failed: {e}");
            set_update(&app, &state, UpdateState::Failed { message: message.clone() });
            Err(Problem::new("update.failed", "The update didn't install")
                .what(message)
                .why("Nothing was changed; Vesper is still on the current version.")
                .fix("Try again later, or download the installer from xan.gripe/vesper.")
                .action(ProblemAction::OpenUrl("https://xan.gripe/vesper".into())))
        }
    }
}

// ------------------------------------------------------------------ window

#[tauri::command]
pub fn app_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn app_quit(app: AppHandle) {
    crate::tray::quit(&app);
}

/// Called once the interface has rendered, so the window never flashes empty.
#[tauri::command]
pub fn app_ready(app: AppHandle, state: State<'_, AppState>) {
    let hidden_start = std::env::args().any(|a| a == "--autostart") && state.settings().general.start_minimized;
    if !hidden_start {
        crate::tray::show_main(&app);
    }
    if state.settings().roblox.flag_mode == FlagMode::KeepApplied {
        state.sync_keep_applied(&app);
    }
    // "Launch Roblox when Vesper starts": once per run, not on every window reload.
    static LAUNCHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if state.settings().general.launch_roblox_on_start
        && !state.roblox_running.load(Ordering::Relaxed)
        && !LAUNCHED.swap(true, Ordering::Relaxed)
    {
        if let Err(problem) = crate::session::begin(app.clone(), LaunchTarget::App) {
            state.set_launch(&app, LaunchStatus::Failed { problem });
        }
    }
}
