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
    pub wheel_gradient: Option<String>,
    pub wheel_line: Option<String>,
    pub font: Option<String>,
    pub sky: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsView {
    pub config: ModsConfig,
    pub cursor_presets: Vec<PresetInfo>,
    pub wheel_presets: Vec<PresetInfo>,
    pub sky_presets: Vec<PresetInfo>,
    pub custom_dir: String,
    pub custom_file_count: usize,
    pub cursor_scale: (u8, u8),
    pub max_wheel_dim: u8,
    pub launcher_fonts: Vec<&'static str>,
    pub font_name: Option<String>,
    pub last_report: Option<ModReport>,
    pub roblox_running: bool,
    pub applied_now: bool,
    pub previews: Previews,
}

const MAX_FONT_PREVIEW: usize = 8 * 1024 * 1024;

fn data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn prepared(state: &AppState, rel: &str) -> Option<String> {
    std::fs::read(state.mod_paths.root.join(mods::rel_path(rel))).ok().map(|b| data_url(&b))
}

fn preview(state: &AppState, rel: &str, modded: bool) -> Option<String> {
    modded.then(|| prepared(state, rel)).flatten().or_else(|| roblox_original(state, rel))
}

fn view(state: &AppState, applied_now: bool) -> ModsView {
    let config = lock(&state.mods).clone();
    let wheel_custom = config.emote_wheel.as_ref().and_then(|w| w.source.as_ref()).is_some();
    let layer = |name: &str| format!(r"{}\Large\{name}.png", mods::WHEEL_DIR);
    let accent = config.emote_wheel.as_ref().and_then(|w| w.accent.as_deref()).and_then(mods::parse_hex);
    let see_through = matches!(config.emote_wheel.as_ref().and_then(|w| w.source.as_ref()), Some(Source::File { .. }));
    let cut = see_through.then_some(mods::WHEEL_INNER * 200.0);
    let highlight = |name: &str, cut: Option<f32>| {
        let bytes = roblox_original_bytes(state, &layer(name))?;
        mods::highlight_gradient(&bytes, accent, cut).ok().map(|b| data_url(&b))
    };
    let previews = Previews {
        cursor: preview(state, mods::CURSOR_FILES[0], config.cursor.is_some()),
        wheel_background: preview(state, &layer("CircleBackground"), wheel_custom),
        wheel_ring: preview(state, &layer("SegmentedCircle"), wheel_custom),
        wheel_gradient: highlight("SelectedGradient", cut),
        wheel_line: highlight("SelectedLine", None),
        font: None,
        sky: if config.sky.is_some() {
            std::fs::read(mods::sky_preview(&state.mod_paths)).ok().map(|b| data_url(&b))
        } else {
            roblox_sky_preview(state)
        },
    };
    let font = config
        .font
        .as_ref()
        .and_then(|_| std::fs::read(state.mod_paths.root.join(mods::rel_path(mods::FONT_FILE))).ok());
    let previews = Previews {
        font: font
            .as_deref()
            .filter(|b| b.len() <= MAX_FONT_PREVIEW)
            .map(|b| format!("data:font/ttf;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))),
        ..previews
    };
    let custom_dir = state.mod_paths.custom_dir();
    ModsView {
        font_name: font.as_deref().and_then(mods::font_name),
        custom_file_count: count_files(&custom_dir),
        custom_dir: crate::redact::display_path(&custom_dir),
        cursor_scale: mods::CURSOR_SCALE,
        max_wheel_dim: mods::MAX_WHEEL_DIM,
        launcher_fonts: mods::FONT_LAUNCHERS
            .into_iter()
            .filter(|l| mods::launcher_font(&state.env.local_appdata, l).is_some())
            .collect(),
        config,
        cursor_presets: mods::presets(mods::CURSOR_PRESETS),
        wheel_presets: mods::presets(mods::WHEEL_PRESETS),
        sky_presets: mods::presets(crate::sky::PRESETS),
        last_report: lock(&state.last_mod_report).clone(),
        roblox_running: state.roblox_running.load(Ordering::Relaxed),
        applied_now,
        previews,
    }
}

fn roblox_original_bytes(state: &AppState, rel: &str) -> Option<Vec<u8>> {
    let install = state.preferred_install()?;
    let rel = mods::rel_path(rel);
    let backup = state.mod_paths.originals.join(install.build_id()).join(&rel);
    std::fs::read(backup).ok().or_else(|| std::fs::read(install.content_root().join(&rel)).ok())
}

fn roblox_sky_preview(state: &AppState) -> Option<String> {
    static CACHE: std::sync::Mutex<Option<(String, String)>> = std::sync::Mutex::new(None);
    let version = state.preferred_install()?.version_hash;
    if let Some((v, url)) = lock(&CACHE).as_ref() {
        if *v == version {
            return Some(url.clone());
        }
    }
    let faces: Vec<(&str, image::RgbaImage)> = crate::sky::FACES
        .iter()
        .filter_map(|f| {
            let bytes = roblox_original_bytes(state, &crate::sky::face_rel(f))?;
            Some((*f, crate::sky::decode_dds(&bytes)?))
        })
        .collect();
    if faces.len() < 4 {
        return None;
    }
    let mut png = std::io::Cursor::new(Vec::new());
    crate::sky::preview_strip(&faces, 160).write_to(&mut png, image::ImageFormat::Png).ok()?;
    let url = data_url(png.get_ref());
    *lock(&CACHE) = Some((version, url.clone()));
    Some(url)
}

fn roblox_original(state: &AppState, rel: &str) -> Option<String> {
    roblox_original_bytes(state, rel).map(|b| data_url(&b))
}

fn colour(hex: Option<&str>) -> Result<Option<[u8; 3]>> {
    hex.map(|h| mods::parse_hex(h).ok_or_else(|| problem("Colours look like #a596ff."))).transpose()
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

fn commit(app: &AppHandle, state: &AppState) -> Result<bool> {
    lock(&state.mods).save(&state.mod_paths).map_err(|e| Problem::io("Couldn't save your customisations.", &e))?;
    if state.roblox_running.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let Some(install) = state.preferred_install() else { return Ok(false) };
    state.apply_mods(&install.dir);
    state.redetect(app);
    Ok(true)
}

async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppHandle) -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app))
        .await
        .map_err(|e| Problem::internal("A background task stopped unexpectedly.", e))?
}

#[tauri::command]
pub fn mods_get(state: State<'_, AppState>) -> ModsView {
    view(&state, false)
}

fn store_source(state: &AppState, kind: &str, path: &Path) -> Result<PathBuf> {
    let dest = state.mod_paths.root.join("sources").join(kind);
    std::fs::create_dir_all(dest.parent().expect("has parent"))
        .map_err(|e| Problem::io("Couldn't keep a copy of the image.", &e))?;
    std::fs::copy(path, &dest).map_err(|e| Problem::io("Couldn't read that image.", &e))?;
    Ok(dest)
}

fn stored_source(state: &AppState, kind: &str) -> Option<PathBuf> {
    let p = state.mod_paths.root.join("sources").join(kind);
    p.is_file().then_some(p)
}

#[tauri::command]
pub async fn mods_set_cursor(
    app: AppHandle,
    preset: Option<String>,
    path: Option<PathBuf>,
    scale: Option<u8>,
    color: Option<String>,
    reuse: Option<bool>,
    shift_lock: Option<bool>,
) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        let scale = scale.unwrap_or(100).clamp(mods::CURSOR_SCALE.0, mods::CURSOR_SCALE.1);
        let rgb = colour(color.as_deref())?;
        let previous = lock(&state.mods).cursor.clone();
        let shift_lock = shift_lock.or(previous.as_ref().map(|c| c.shift_lock)).unwrap_or(true);
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
        let (rgb, color) = if matches!(source, Source::Preset { .. }) { (rgb, color) } else { (None, None) };
        mods::prepare_cursor(&state.mod_paths, &source, file.as_deref(), scale, rgb).map_err(problem)?;
        lock(&state.mods).cursor = Some(CursorMod { source, scale, color, shift_lock });
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn mods_set_wheel(
    app: AppHandle,
    preset: Option<String>,
    path: Option<PathBuf>,
    accent: Option<String>,
    dim: Option<u8>,
    dividers: Option<bool>,
    reuse: Option<bool>,
) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        colour(accent.as_deref())?;
        let current = lock(&state.mods).emote_wheel.clone();
        let mut look = current.as_ref().map(|w| w.look).unwrap_or_default();
        if let Some(d) = dim {
            look.dim = d.min(mods::MAX_WHEEL_DIM);
        }
        if let Some(d) = dividers {
            look.dividers = d;
        }
        let previous = current.and_then(|w| w.source);
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
            Some(s) => mods::prepare_wheel(&state.mod_paths, s, file.as_deref(), look).map_err(problem)?,
            None => mods::clear_prepared(&state.mod_paths, "emoteWheel"),
        }
        let wheel = (source.is_some() || accent.is_some()).then_some(WheelMod { source, accent, look });
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
pub async fn mods_use_launcher_font(app: AppHandle, launcher: String) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        let file = mods::launcher_font(&state.env.local_appdata, &launcher)
            .ok_or_else(|| problem(format!("{launcher} doesn't have a custom font set.")))?;
        mods::import_font(&state.mod_paths, &mut lock(&state.mods), &file, format!("{launcher}'s font"))
            .map_err(problem)?;
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

pub fn import_font_cli(arg: Option<&str>) -> i32 {
    let fail = |msg: &str| {
        eprintln!("{msg}");
        log::warn!("Font import: {msg}");
        log::logger().flush();
        1
    };
    let Ok(paths) = crate::paths::AppPaths::resolve() else { return 1 };
    crate::logging::init(&paths.logs, log::LevelFilter::Info);
    let Some(arg) = arg else { return fail("Usage: Vesper --import-font <font file | Fishstrap | Bloxstrap>") };
    let running = crate::roblox::process::snapshot()
        .map(|all| crate::roblox::process::named(&all, crate::roblox::process::VESPER_PROCESS).count())
        .unwrap_or(0);
    if running > 1 {
        return fail("Close Vesper first, then try again.");
    }
    let local = crate::roblox::detect::DetectEnv::system().map(|e| e.local_appdata).unwrap_or_default();
    let (file, name) = match mods::launcher_font(&local, arg) {
        Some(f) => (f, format!("{arg}'s font")),
        None => (PathBuf::from(arg), file_name(Path::new(arg))),
    };
    let mod_paths = mods::ModPaths::new(&paths.config, &paths.cache);
    let mut cfg = ModsConfig::load(&mod_paths);
    if let Err(e) = mods::import_font(&mod_paths, &mut cfg, &file, name.clone()) {
        return fail(&e);
    }
    if let Err(e) = cfg.save(&mod_paths) {
        return fail(&e.to_string());
    }
    log::info!("Font set to {name}; it's applied the next time Vesper starts Roblox");
    log::logger().flush();
    0
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

#[tauri::command]
pub async fn mods_set_sky(
    app: AppHandle,
    preset: Option<String>,
    panorama: Option<PathBuf>,
    faces: Option<Vec<PathBuf>>,
) -> Result<ModsView> {
    blocking(app, move |app| {
        let state = app.state::<AppState>();
        let (built, sky) = if let Some(id) = preset {
            let colour = crate::sky::preset_color(&id).ok_or_else(|| problem("That sky style doesn't exist."))?;
            let faces = crate::sky::render(crate::sky::FACE_SIZE, &colour);
            (faces, mods::SkyMod { source: Source::Preset { id }, layout: mods::SkyLayout::Panorama })
        } else if let Some(path) = panorama {
            let faces = crate::sky::faces_from_panorama(&path).map_err(problem)?;
            (faces, mods::SkyMod { source: Source::File { name: file_name(&path) }, layout: mods::SkyLayout::Panorama })
        } else if let Some(paths) = faces {
            let faces = crate::sky::faces_from_files(&paths).map_err(problem)?;
            let name = paths
                .first()
                .and_then(|p| p.parent())
                .and_then(|d| d.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Six pictures".into());
            (faces, mods::SkyMod { source: Source::File { name }, layout: mods::SkyLayout::Faces })
        } else {
            return Err(Problem::invalid("Choose a sky.", "Pick a style or a picture."));
        };
        mods::prepare_sky(&state.mod_paths, &built).map_err(problem)?;
        lock(&state.mods).sky = Some(sky);
        let applied = commit(app, &state)?;
        Ok(view(&state, applied))
    })
    .await
}

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
                "sky" => m.sky = None,
                "customFiles" => m.custom_files = false,
                "all" => *m = ModsConfig::default(),
                _ => return Err(Problem::invalid("Unknown customisation.", "This is a bug in Vesper.")),
            }
        }
        if kind == "all" {
            for k in ["cursor", "emoteWheel", "font", "deathSound", "sky"] {
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

#[tauri::command]
pub fn mods_reveal(app: AppHandle, state: State<'_, AppState>, kind: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let install = state.preferred_install().ok_or_else(|| {
        Problem::new("roblox.not_installed", "Roblox isn't installed").what("There are no Roblox files to show.")
    })?;
    let rel = match kind.as_str() {
        "cursor" => mods::CURSOR_FILES[0].to_owned(),
        "emoteWheel" => format!(r"{}\Large\SegmentedCircle.png", mods::WHEEL_DIR),
        "font" => mods::FONT_FILE.to_owned(),
        "deathSound" => mods::DEATH_SOUNDS[1].to_owned(),
        "sky" => crate::sky::face_rel("ft"),
        _ => return Err(Problem::invalid("Unknown customisation.", "This is a bug in Vesper.")),
    };
    let root = install.content_root();
    let mut path = root.join(mods::rel_path(&rel));
    while !path.exists() && path != root && path.pop() {}
    let opened = if path.is_file() {
        app.opener().reveal_item_in_dir(&path)
    } else {
        app.opener().open_path(path.to_string_lossy(), None::<&str>)
    };
    opened.map_err(|e| Problem::internal("Couldn't open the folder.", e))
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().chars().take(80).collect()).unwrap_or_else(|| "file".into())
}

const MAX_BACKGROUND_SOURCE: u64 = 40 * 1024 * 1024;
const BACKGROUND_MAX_SIDE: u32 = 2560;

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
