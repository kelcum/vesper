//! Commands behind the Customize page and the custom window background.
//!
//! Image work (resizing a wheel to 18 sizes) runs on a blocking thread so the
//! window stays responsive. Changes are applied to Roblox straight away when
//! it's closed; otherwise they take effect the next time it starts.

use crate::config::Settings;
use crate::error::{Problem, Result};
use crate::mods::{self, CursorMod, FileMod, ModReport, ModsConfig, PresetInfo, Source, WheelMod};
use crate::state::{events, lock, AppState};
use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Previews {
    pub cursor: Option<String>,
    pub wheel_background: Option<String>,
    pub wheel_ring: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsView {
    pub config: ModsConfig,
    pub cursor_presets: Vec<PresetInfo>,
    pub wheel_presets: Vec<PresetInfo>,
    pub custom_dir: String,
    pub custom_file_count: usize,
    pub last_report: Option<ModReport>,
    pub roblox_running: bool,
    /// True when the last change was applied to Roblox already.
    pub applied_now: bool,
    pub previews: Previews,
}

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The file Vesper prepared for a mod, if any.
fn prepared(state: &AppState, rel: &str) -> Option<String> {
    std::fs::read(state.mod_paths.root.join(rel)).ok().map(|b| data_url(&b))
}

/// Preview of a layer: the prepared file when the mod is on, otherwise Roblox's own art.
fn preview(state: &AppState, rel: &str, modded: bool) -> Option<String> {
    modded.then(|| prepared(state, rel)).flatten().or_else(|| roblox_original(state, rel))
}

fn view(state: &AppState, applied_now: bool) -> ModsView {
    let config = lock(&state.mods).clone();
    let wheel_custom = config.emote_wheel.as_ref().and_then(|w| w.source.as_ref()).is_some();
    let ring_rel = format!(r"{}\Large\SegmentedCircle.png", mods::WHEEL_DIR);
    let bg_rel = format!(r"{}\Large\CircleBackground.png", mods::WHEEL_DIR);
    let previews = Previews {
        cursor: preview(state, mods::CURSOR_FILES[0], config.cursor.is_some()),
        // A custom ring keeps Roblox's background, so fall back per layer.
        wheel_background: preview(state, &bg_rel, wheel_custom),
        wheel_ring: preview(state, &ring_rel, wheel_custom),
    };
    let custom_dir = state.mod_paths.custom_dir();
    ModsView {
        custom_file_count: count_files(&custom_dir),
        custom_dir: crate::redact::display_path(&custom_dir),
        config,
        cursor_presets: mods::presets(mods::CURSOR_PRESETS),
        wheel_presets: mods::presets(mods::WHEEL_PRESETS),
        last_report: lock(&state.last_mod_report).clone(),
        roblox_running: state.roblox_running.load(Ordering::Relaxed),
        applied_now,
        previews,
    }
}

/// Roblox's unmodified file: the backup if Vesper replaced it, otherwise the live one.
fn roblox_original(state: &AppState, rel: &str) -> Option<String> {
    let install = state.preferred_install()?;
    let backup = state.mod_paths.originals.join(&install.version_hash).join(rel);
    let bytes = std::fs::read(backup).ok().or_else(|| std::fs::read(install.dir.join(rel)).ok())?;
    Some(data_url(&bytes))
}

fn count_files(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(std::result::Result::ok)
                .map(|e| if e.path().is_dir() { count_files(&e.path()) } else { 1 })
                .sum()
        })
        .unwrap_or(0)
}

fn problem(what: impl Into<String>) -> Problem {
    Problem::new("mods", "That customisation can't be used")
        .what(what)
        .why("The file didn't pass Vesper's checks.")
        .fix("Choose another file and try again.")
}

/// Saves the config and applies it to Roblox if Roblox is closed.
fn commit(app: &AppHandle, state: &AppState) -> Result<bool> {
    lock(&state.mods)
        .save(&state.mod_paths)
        .map_err(|e| Problem::io("Couldn't save your customisations.", &e))?;
    if state.roblox_running.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let Some(install) = state.preferred_install() else { return Ok(false) };
    state.apply_mods(&install.dir);
    state.redetect(app);
    Ok(true)
}

async fn blocking<T: Send + 'static>(app: AppHandle, f: impl FnOnce(&AppHandle) -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app))
        .await
        .map_err(|e| Problem::internal("A background task stopped unexpectedly.", e))?
}

#[tauri::command]
pub fn mods_get(state: State<'_, AppState>) -> ModsView {
    view(&state, false)
}

/// Keeps a copy of a picked image so size or colour changes don't need it picked again.
fn store_source(state: &AppState, kind: &str, path: &Path) -> Result<PathBuf> {
    let dest = state.mod_paths.root.join("sources").join(kind);
    std::fs::create_dir_all(dest.parent().expect("has parent")).map_err(|e| Problem::io("Couldn't keep a copy of the image.", &e))?;
    std::fs::copy(path, &dest).map_err(|e| Problem::io("Couldn't read that image.", &e))?;
    Ok(dest)
}

fn stored_source(state: &AppState, kind: &str) -> Option<PathBuf> {
    let p = state.mod_paths.root.join("sources").join(kind);
    p.is_file().then_some(p)
}

/// `preset` picks a built-in style, `path` a new image. With neither and
/// `reuse`, the previously picked image is rendered again (e.g. at a new size).
#[tauri::command]
pub async fn mods_set_cursor(
    app: AppHandle,
    preset: Option<String>,
    path: Option<PathBuf>,
    scale: Option<u8>,
    reuse: Option<bool>,
) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        let scale = scale.unwrap_or(100).clamp(20, 100);
        let previous = lock(&state.mods).cursor.clone();
        let (source, file) = match (&preset, &path) {
            (Some(id), _) => (Source::Preset { id: id.clone() }, None),
            (None, Some(p)) => (Source::File { name: file_name(p) }, Some(store_source(&state, "cursor", p)?)),
            (None, None) if reuse == Some(true) => match previous.map(|c| c.source) {
                Some(s @ Source::File { .. }) => (s, stored_source(&state, "cursor")),
                Some(s) => (s, None),
                None => return Err(problem("Choose a style or an image.")),
            },
            _ => return Err(problem("Choose a style or an image.")),
        };
        if matches!(source, Source::File { .. }) && file.is_none() {
            return Err(problem("The image you picked earlier is gone. Choose it again."));
        }
        mods::prepare_cursor(&state.mod_paths, &source, file.as_deref(), scale).map_err(problem)?;
        lock(&state.mods).cursor = Some(CursorMod { source, scale });
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

/// With no preset and no path, keeps Roblox's wheel and only changes the accent,
/// unless `reuse` asks to keep the previously picked image.
#[tauri::command]
pub async fn mods_set_wheel(
    app: AppHandle,
    preset: Option<String>,
    path: Option<PathBuf>,
    accent: Option<String>,
    reuse: Option<bool>,
) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        if let Some(a) = &accent {
            mods::parse_hex(a).ok_or_else(|| problem("Colours look like #a596ff."))?;
        }
        let previous = lock(&state.mods).emote_wheel.clone().and_then(|w| w.source);
        let (source, file) = match (&preset, &path) {
            (Some(id), _) => (Some(Source::Preset { id: id.clone() }), None),
            (None, Some(p)) => (Some(Source::File { name: file_name(p) }), Some(store_source(&state, "wheel", p)?)),
            (None, None) if reuse == Some(true) => match previous {
                Some(s @ Source::File { .. }) => (Some(s), stored_source(&state, "wheel")),
                other => (other, None),
            },
            (None, None) => (None, None),
        };
        match &source {
            Some(Source::File { .. }) if file.is_none() => {
                return Err(problem("The image you picked earlier is gone. Choose it again."));
            }
            Some(s) => mods::prepare_wheel(&state.mod_paths, s, file.as_deref()).map_err(problem)?,
            None => mods::clear_prepared(&state.mod_paths, "emoteWheel"),
        }
        let wheel = (source.is_some() || accent.is_some()).then_some(WheelMod { source, accent });
        lock(&state.mods).emote_wheel = wheel;
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

#[tauri::command]
pub async fn mods_set_font(app: AppHandle, path: PathBuf) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        mods::prepare_font(&state.mod_paths, &path).map_err(problem)?;
        lock(&state.mods).font = Some(FileMod { name: file_name(&path) });
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

#[tauri::command]
pub async fn mods_set_sound(app: AppHandle, path: PathBuf) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        mods::prepare_sound(&state.mod_paths, &path).map_err(problem)?;
        lock(&state.mods).death_sound = Some(FileMod { name: file_name(&path) });
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

#[tauri::command]
pub async fn mods_set_custom(app: AppHandle, enabled: bool) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        std::fs::create_dir_all(state.mod_paths.custom_dir()).ok();
        lock(&state.mods).custom_files = enabled;
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

/// Turns one customisation off and restores Roblox's original files.
#[tauri::command]
pub async fn mods_clear(app: AppHandle, kind: String) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        {
            let mut m = lock(&state.mods);
            match kind.as_str() {
                "cursor" => m.cursor = None,
                "emoteWheel" => m.emote_wheel = None,
                "font" => m.font = None,
                "deathSound" => m.death_sound = None,
                "customFiles" => m.custom_files = false,
                "all" => *m = ModsConfig::default(),
                _ => return Err(Problem::invalid("Unknown customisation.", "This is a bug in Vesper.")),
            }
        }
        if kind == "all" {
            for k in ["cursor", "emoteWheel", "font", "deathSound"] {
                mods::clear_prepared(&state.mod_paths, k);
            }
        } else {
            mods::clear_prepared(&state.mod_paths, &kind);
        }
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

#[tauri::command]
pub fn mods_open_custom_folder(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = state.mod_paths.custom_dir();
    std::fs::create_dir_all(&dir).map_err(|e| Problem::io("Couldn't create the custom files folder.", &e))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| Problem::internal("Couldn't open the folder.", e))
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().chars().take(80).collect()).unwrap_or_else(|| "file".into())
}

// ------------------------------------------------------------------ window background

const MAX_BACKGROUND_SOURCE: u64 = 40 * 1024 * 1024;
const BACKGROUND_MAX_SIDE: u32 = 2560;

/// Stores a downscaled JPEG copy, so a huge photo never slows the window down.
#[tauri::command]
pub async fn appearance_set_background(app: AppHandle, path: PathBuf) -> Result<Settings> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        let meta = std::fs::metadata(&path).map_err(|e| Problem::io("Couldn't read that image.", &e))?;
        if meta.len() > MAX_BACKGROUND_SOURCE {
            return Err(problem("Background images can be at most 40 MB."));
        }
        let img = image::ImageReader::open(&path)
            .and_then(|r| r.with_guessed_format())
            .map_err(|e| Problem::io("Couldn't read that image.", &e))?
            .decode()
            .map_err(|_| problem("That isn't a PNG, JPEG or WebP image Vesper can read."))?;
        let img = if img.width() > BACKGROUND_MAX_SIDE || img.height() > BACKGROUND_MAX_SIDE {
            img.resize(BACKGROUND_MAX_SIDE, BACKGROUND_MAX_SIDE, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img.to_rgb8())
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .map_err(|e| Problem::internal("Couldn't convert the image.", e))?;
        let name = "background.jpg";
        crate::paths::write_atomic(&state.paths.config.join(name), jpeg.get_ref())
            .map_err(|e| Problem::io("Couldn't save the background.", &e))?;
        lock(&state.settings).appearance.background = Some(name.into());
        state.save_settings()?;
        let s = state.settings();
        let _ = app.emit(events::SETTINGS, &s);
        Ok(s)
    })
    .await
}

#[tauri::command]
pub fn appearance_clear_background(app: AppHandle, state: State<'_, AppState>) -> Result<Settings> {
    if let Some(name) = lock(&state.settings).appearance.background.take() {
        let _ = std::fs::remove_file(state.paths.config.join(name));
    }
    state.save_settings()?;
    let s = state.settings();
    let _ = app.emit(events::SETTINGS, &s);
    Ok(s)
}

#[tauri::command]
pub fn appearance_background_data(state: State<'_, AppState>) -> Option<String> {
    let name = lock(&state.settings).appearance.background.clone()?;
    let bytes = std::fs::read(state.paths.config.join(name)).ok()?;
    Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}
