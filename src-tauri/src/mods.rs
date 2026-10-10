//! Roblox customisations: cursor, emote wheel, font, death sound and custom files.
//!
//! These replace files in Roblox's `content` folder, the same mechanism other
//! launchers have used for years. They change how Roblox looks and sounds on
//! your PC only; nothing is injected and no game code is touched.
//!
//! Safety rules:
//! * Before a Roblox file is replaced, the original is copied to
//!   `cache\originals\<version>\…` and restored when the mod is turned off.
//! * Texture and sound mods only replace files that exist in the current
//!   Roblox build. If Roblox removes or renames one, the mod is skipped and
//!   reported instead of leaving stray files behind.
//! * A ledger records the hash of every file written, so a file someone else
//!   changed afterwards is never overwritten or deleted silently.

use crate::paths::write_atomic;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub const CURSOR_FILES: [&str; 2] = [
    r"content\textures\Cursors\KeyboardMouse\ArrowCursor.png",
    r"content\textures\Cursors\KeyboardMouse\ArrowFarCursor.png",
];
pub const CURSOR_SIZE: u32 = 64;
pub const DEATH_SOUNDS: [&str; 2] = [r"content\sounds\ouch.ogg", r"content\sounds\oof.ogg"];
pub const FONT_FILE: &str = r"content\fonts\CustomFont.ttf";
pub const FONT_FAMILIES: &str = r"content\fonts\families";
pub const FONT_ASSET: &str = "rbxasset://fonts/CustomFont.ttf";
pub const WHEEL_DIR: &str = r"content\textures\ui\Emotes";
/// Emote wheel size classes and the diameter of their @1x circle.
pub const WHEEL_SIZES: [(&str, u32); 3] = [("Small", 300), ("Large", 400), ("TenFoot", 640)];
pub const SCALES: [(&str, u32); 3] = [("", 1), ("@2x", 2), ("@3x", 3)];
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_FONT_BYTES: u64 = 30 * 1024 * 1024;
pub const MAX_SOUND_BYTES: u64 = 10 * 1024 * 1024;

pub const CURSOR_PRESETS: &[(&str, &str)] = &[("dot", "Vesper dot"), ("crosshair", "Crosshair"), ("arrow", "Arrow")];
pub const WHEEL_PRESETS: &[(&str, &str)] = &[("dusk", "Dusk"), ("glass", "Glass"), ("minimal", "Minimal")];

fn cursor_preset(id: &str) -> Option<&'static [u8]> {
    match id {
        "dot" => Some(include_bytes!("../resources/mods/cursors/dot.png")),
        "crosshair" => Some(include_bytes!("../resources/mods/cursors/crosshair.png")),
        "arrow" => Some(include_bytes!("../resources/mods/cursors/arrow.png")),
        _ => None,
    }
}

/// (CircleBackground, SegmentedCircle) masters at 1920×1920.
fn wheel_preset(id: &str) -> Option<(&'static [u8], &'static [u8])> {
    match id {
        "dusk" => Some((
            include_bytes!("../resources/mods/wheels/dusk/CircleBackground.png"),
            include_bytes!("../resources/mods/wheels/dusk/SegmentedCircle.png"),
        )),
        "glass" => Some((
            include_bytes!("../resources/mods/wheels/glass/CircleBackground.png"),
            include_bytes!("../resources/mods/wheels/glass/SegmentedCircle.png"),
        )),
        "minimal" => Some((
            include_bytes!("../resources/mods/wheels/minimal/CircleBackground.png"),
            include_bytes!("../resources/mods/wheels/minimal/SegmentedCircle.png"),
        )),
        _ => None,
    }
}

// ------------------------------------------------------------------ configuration

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Source {
    Preset { id: String },
    /// A file the user picked; `name` is its original file name, for display.
    File { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorMod {
    pub source: Source,
    /// Size of a custom image inside the 64×64 cursor, in percent.
    pub scale: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelMod {
    /// `None` keeps Roblox's own wheel and only recolours the highlight.
    pub source: Option<Source>,
    /// Highlight colour as `#rrggbb`; `None` keeps Roblox's white.
    pub accent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMod {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModsConfig {
    pub cursor: Option<CursorMod>,
    pub emote_wheel: Option<WheelMod>,
    pub font: Option<FileMod>,
    pub death_sound: Option<FileMod>,
    /// Apply everything in the custom files folder.
    pub custom_files: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub id: &'static str,
    pub label: &'static str,
}

pub fn presets(list: &[(&'static str, &'static str)]) -> Vec<PresetInfo> {
    list.iter().map(|(id, label)| PresetInfo { id, label }).collect()
}

/// Where Vesper keeps prepared mod files.
#[derive(Debug, Clone)]
pub struct ModPaths {
    pub root: PathBuf,
    pub originals: PathBuf,
    pub ledger: PathBuf,
}

impl ModPaths {
    pub fn new(config: &Path, cache: &Path) -> Self {
        Self { root: config.join("mods"), originals: cache.join("originals"), ledger: config.join("mods-applied.json") }
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("mods.json")
    }
    pub fn custom_dir(&self) -> PathBuf {
        self.root.join("custom")
    }
    fn prepared(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl ModsConfig {
    pub fn load(p: &ModPaths) -> Self {
        std::fs::read(p.config_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save(&self, p: &ModPaths) -> std::io::Result<()> {
        write_atomic(&p.config_file(), &serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)
    }
}

// ------------------------------------------------------------------ preparing files

fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

fn load_image(path: &Path) -> Result<DynamicImage, String> {
    let meta = std::fs::metadata(path).map_err(|_| "That file can't be read.".to_string())?;
    if meta.len() > MAX_IMAGE_BYTES {
        return Err("Images can be at most 20 MB.".into());
    }
    image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|_| "That file can't be read.".to_string())?
        .decode()
        .map_err(|_| "That isn't a PNG, JPEG or WebP image Vesper can read.".to_string())
}

/// Fits `img` inside a `size`×`size` canvas at `scale_pct`, centred
/// (Roblox draws cursor images centred on the mouse position).
pub fn fit_centered(img: &DynamicImage, size: u32, scale_pct: u8) -> RgbaImage {
    let box_size = ((size as f32) * (scale_pct.clamp(20, 100) as f32) / 100.0).round().max(1.0) as u32;
    let fitted = img.resize(box_size, box_size, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let x = (size - fitted.width()) / 2;
    let y = (size - fitted.height()) / 2;
    image::imageops::overlay(&mut canvas, &fitted, x as i64, y as i64);
    canvas
}

/// Centre-crops to a square, then resizes to `side`.
pub fn square(img: &DynamicImage, side: u32) -> RgbaImage {
    let s = img.width().min(img.height());
    let cropped = img.crop_imm((img.width() - s) / 2, (img.height() - s) / 2, s, s);
    cropped.resize_exact(side, side, FilterType::Lanczos3).to_rgba8()
}

/// Keeps the alpha of `original` and paints every pixel `rgb`.
pub fn recolor(original: &[u8], rgb: [u8; 3]) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(original).map_err(|e| e.to_string())?.to_rgba8();
    for p in img.pixels_mut() {
        // The originals are white with varying alpha; scale by their brightness
        // so soft edges stay soft.
        let lum = (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3;
        *p = Rgba([
            ((rgb[0] as u32 * lum) / 255) as u8,
            ((rgb[1] as u32 * lum) / 255) as u8,
            ((rgb[2] as u32 * lum) / 255) as u8,
            p[3],
        ]);
    }
    png_bytes(&img)
}

pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').unwrap_or(s);
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn wheel_rel(size: &str, layer: &str, scale: &str) -> String {
    format!(r"{WHEEL_DIR}\{size}\{layer}{scale}.png")
}

/// Prepares the cursor files from a preset or an image.
pub fn prepare_cursor(p: &ModPaths, source: &Source, file: Option<&Path>, scale: u8) -> Result<(), String> {
    let bytes = match source {
        Source::Preset { id } => cursor_preset(id).ok_or("Unknown cursor style.")?.to_vec(),
        Source::File { .. } => {
            let img = load_image(file.ok_or("Choose an image.")?)?;
            png_bytes(&fit_centered(&img, CURSOR_SIZE, scale))?
        }
    };
    for rel in CURSOR_FILES {
        write_atomic(&p.prepared(rel), &bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Prepares emote wheel layers for every size class and scale.
pub fn prepare_wheel(p: &ModPaths, source: &Source, file: Option<&Path>) -> Result<(), String> {
    let (background, ring): (Option<DynamicImage>, DynamicImage) = match source {
        Source::Preset { id } => {
            let (bg, ring) = wheel_preset(id).ok_or("Unknown emote wheel style.")?;
            (
                Some(image::load_from_memory(bg).map_err(|e| e.to_string())?),
                image::load_from_memory(ring).map_err(|e| e.to_string())?,
            )
        }
        Source::File { .. } => (None, load_image(file.ok_or("Choose an image.")?)?),
    };
    // Clear layers from a previous choice so a custom ring keeps Roblox's background.
    let _ = std::fs::remove_dir_all(p.prepared(WHEEL_DIR));
    for (size, base) in WHEEL_SIZES {
        for (suffix, mul) in SCALES {
            let side = base * mul;
            write_atomic(&p.prepared(&wheel_rel(size, "SegmentedCircle", suffix)), &png_bytes(&square(&ring, side))?)
                .map_err(|e| e.to_string())?;
            if let Some(bg) = &background {
                write_atomic(&p.prepared(&wheel_rel(size, "CircleBackground", suffix)), &png_bytes(&square(bg, side))?)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn check_header(path: &Path, max: u64, ok: impl Fn(&[u8]) -> bool, what: &str) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|_| "That file can't be read.".to_string())?;
    if meta.len() > max {
        return Err(format!("{what} files can be at most {} MB.", max / 1024 / 1024));
    }
    let bytes = std::fs::read(path).map_err(|_| "That file can't be read.".to_string())?;
    if bytes.len() < 4 || !ok(&bytes[..4]) {
        return Err(format!("That doesn't look like a valid {what} file."));
    }
    Ok(bytes)
}

pub fn prepare_font(p: &ModPaths, file: &Path) -> Result<(), String> {
    let bytes = check_header(
        file,
        MAX_FONT_BYTES,
        |h| h == [0x00, 0x01, 0x00, 0x00] || h == b"OTTO" || h == b"ttcf" || h == b"true",
        "font (.ttf, .otf or .ttc)",
    )?;
    write_atomic(&p.prepared(FONT_FILE), &bytes).map_err(|e| e.to_string())
}

pub fn prepare_sound(p: &ModPaths, file: &Path) -> Result<(), String> {
    let bytes = check_header(
        file,
        MAX_SOUND_BYTES,
        |h| h == b"OggS" || &h[..3] == b"ID3" || (h[0] == 0xFF && h[1] & 0xE0 == 0xE0),
        "sound (.ogg or .mp3)",
    )?;
    for rel in DEATH_SOUNDS {
        write_atomic(&p.prepared(rel), &bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Removes a mod's prepared files.
pub fn clear_prepared(p: &ModPaths, kind: &str) {
    let rels: Vec<String> = match kind {
        "cursor" => CURSOR_FILES.iter().map(|s| s.to_string()).collect(),
        "font" => vec![FONT_FILE.to_string()],
        "deathSound" => DEATH_SOUNDS.iter().map(|s| s.to_string()).collect(),
        "emoteWheel" => {
            let _ = std::fs::remove_dir_all(p.prepared(WHEEL_DIR));
            vec![]
        }
        _ => vec![],
    };
    for rel in rels {
        let _ = std::fs::remove_file(p.prepared(&rel));
    }
}

// ------------------------------------------------------------------ applying to Roblox

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppliedFile {
    pub kind: String,
    pub sha256: String,
    /// Backup of the Roblox file this replaced, if there was one.
    pub original: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModLedger {
    pub files: BTreeMap<String, AppliedFile>,
}

impl ModLedger {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)
    }
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModReport {
    pub written: usize,
    pub unchanged: usize,
    pub restored: usize,
    /// Files this Roblox build doesn't have, so the mod was skipped.
    pub missing: Vec<String>,
    /// Files changed by something else since Vesper wrote them; left alone.
    pub skipped_foreign: Vec<String>,
    pub errors: Vec<String>,
}

struct Desired {
    kind: &'static str,
    bytes: Vec<u8>,
    /// May the target be created if Roblox doesn't have it?
    create: bool,
}

fn desired_files(cfg: &ModsConfig, p: &ModPaths, install: &Path, originals: &Path) -> (BTreeMap<String, Desired>, Vec<String>) {
    let mut out = BTreeMap::new();
    let mut errors = Vec::new();
    let mut add_prepared = |rel: &str, kind: &'static str, create: bool, out: &mut BTreeMap<String, Desired>| {
        match std::fs::read(p.prepared(rel)) {
            Ok(bytes) => {
                out.insert(rel.to_owned(), Desired { kind, bytes, create });
            }
            Err(_) => errors.push(format!("{kind}: prepared file {rel} is missing; choose the {kind} again.")),
        }
    };
    if cfg.cursor.is_some() {
        for rel in CURSOR_FILES {
            add_prepared(rel, "cursor", false, &mut out);
        }
    }
    if cfg.death_sound.is_some() {
        for rel in DEATH_SOUNDS {
            add_prepared(rel, "deathSound", false, &mut out);
        }
    }
    if let Some(w) = &cfg.emote_wheel {
        if w.source.is_some() {
            for (size, _) in WHEEL_SIZES {
                for (suffix, _) in SCALES {
                    for layer in ["SegmentedCircle", "CircleBackground"] {
                        let rel = wheel_rel(size, layer, suffix);
                        if p.prepared(&rel).is_file() {
                            add_prepared(&rel, "emoteWheel", false, &mut out);
                        }
                    }
                }
            }
        }
        if let Some(rgb) = w.accent.as_deref().and_then(parse_hex) {
            for (size, _) in WHEEL_SIZES {
                for (suffix, _) in SCALES {
                    for layer in ["SelectedGradient", "SelectedLine"] {
                        let rel = wheel_rel(size, layer, suffix);
                        // Recolour Roblox's own shape: from the backup if we already replaced it.
                        let source = original_for(install, &rel, originals);
                        if let Some(bytes) = source.and_then(|b| recolor(&b, rgb).ok()) {
                            out.insert(rel, Desired { kind: "emoteWheel", bytes, create: false });
                        }
                    }
                }
            }
        }
    }
    if cfg.font.is_some() {
        add_prepared(FONT_FILE, "font", true, &mut out);
        let families = install.join(FONT_FAMILIES);
        if let Ok(rd) = std::fs::read_dir(&families) {
            for e in rd.filter_map(Result::ok) {
                let name = e.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".json") {
                    continue;
                }
                let rel = format!(r"{FONT_FAMILIES}\{name}");
                let Some(original) = original_for(install, &rel, originals) else { continue };
                if let Some(bytes) = rewrite_family(&original) {
                    out.insert(rel, Desired { kind: "font", bytes, create: false });
                }
            }
        }
    }
    if cfg.custom_files {
        let root = p.custom_dir();
        for file in walk(&root) {
            if let Ok(rel) = file.strip_prefix(&root) {
                let rel = rel.to_string_lossy().replace('/', "\\");
                if let Ok(bytes) = std::fs::read(&file) {
                    out.insert(rel, Desired { kind: "custom", bytes, create: true });
                }
            }
        }
    }
    (out, errors)
}

/// The unmodified Roblox file: the backup if Vesper replaced it, otherwise the live file.
fn original_for(install: &Path, rel: &str, originals: &Path) -> Option<Vec<u8>> {
    let version = install.file_name()?.to_string_lossy().into_owned();
    std::fs::read(originals.join(&version).join(rel)).ok().or_else(|| std::fs::read(install.join(rel)).ok())
}

/// Points every face of a font family manifest at CustomFont.ttf.
pub fn rewrite_family(original: &[u8]) -> Option<Vec<u8>> {
    let mut v: serde_json::Value = serde_json::from_slice(original).ok()?;
    let faces = v.get_mut("faces")?.as_array_mut()?;
    for f in faces.iter_mut() {
        if let Some(obj) = f.as_object_mut() {
            obj.insert("assetId".into(), serde_json::Value::String(FONT_ASSET.into()));
        }
    }
    serde_json::to_vec(&v).ok()
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.filter_map(Result::ok) {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.is_file() {
            out.push(p);
        }
    }
    out
}

/// Brings `install` in line with `cfg`: writes enabled mods, restores disabled ones.
pub fn apply(cfg: &ModsConfig, p: &ModPaths, install: &Path, ledger: &mut ModLedger) -> ModReport {
    let mut report = ModReport::default();
    let Some(version) = install.file_name().map(|n| n.to_string_lossy().into_owned()) else { return report };
    let originals = p.originals.join(&version);
    let (desired, errors) = desired_files(cfg, p, install, &p.originals);
    report.errors = errors;

    for (rel, want) in &desired {
        let target = install.join(rel);
        let k = key(&target);
        let current = std::fs::read(&target).ok();
        if current.is_none() && !want.create {
            report.missing.push(rel.clone());
            continue;
        }
        let ours = ledger.files.get(&k);
        if let (Some(cur), Some(entry)) = (&current, ours) {
            if sha(cur) == sha(&want.bytes) {
                report.unchanged += 1;
                continue;
            }
            if sha(cur) != entry.sha256 {
                // Changed by something else after Vesper wrote it.
                report.skipped_foreign.push(rel.clone());
                continue;
            }
        }
        let mut original = ours.and_then(|o| o.original.clone());
        if ours.is_none() {
            if let Some(cur) = &current {
                if sha(cur) == sha(&want.bytes) {
                    report.unchanged += 1;
                    continue;
                }
                let backup = originals.join(rel);
                if !backup.exists() {
                    if let Err(e) = write_atomic(&backup, cur) {
                        report.errors.push(format!("Couldn't back up {rel}: {e}"));
                        continue;
                    }
                }
                original = Some(backup);
            }
        }
        match write_atomic(&target, &want.bytes) {
            Ok(()) => {
                ledger.files.insert(k, AppliedFile { kind: want.kind.into(), sha256: sha(&want.bytes), original });
                report.written += 1;
            }
            Err(e) => report.errors.push(format!("Couldn't write {rel}: {e}")),
        }
    }

    // Undo files from mods that are no longer wanted in this install.
    // The separator matters: "version-a" must not match "version-ab".
    let prefix = format!("{}\\", key(install));
    let stale: Vec<String> = ledger
        .files
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .filter(|k| !desired.keys().any(|rel| key(&install.join(rel)) == **k))
        .cloned()
        .collect();
    for k in stale {
        match restore_one(&k, ledger) {
            Ok(true) => report.restored += 1,
            Ok(false) => report.skipped_foreign.push(k.trim_start_matches(&prefix).to_owned()),
            Err(e) => report.errors.push(e),
        }
    }
    report
}

/// Restores one file. `Ok(false)` if it was changed by something else and left alone.
fn restore_one(k: &str, ledger: &mut ModLedger) -> Result<bool, String> {
    let Some(entry) = ledger.files.get(k).cloned() else { return Ok(true) };
    let target = PathBuf::from(k);
    match std::fs::read(&target) {
        Ok(cur) if sha(&cur) != entry.sha256 => {
            ledger.files.remove(k);
            return Ok(false);
        }
        Err(_) => {
            ledger.files.remove(k);
            return Ok(true);
        }
        _ => {}
    }
    let result = match &entry.original {
        Some(backup) => std::fs::read(backup)
            .map_err(|e| format!("The original of {} is missing: {e}", target.display()))
            .and_then(|b| write_atomic(&target, &b).map_err(|e| e.to_string())),
        None => std::fs::remove_file(&target).map_err(|e| e.to_string()),
    };
    result.map_err(|e| crate::redact::redact(&e))?;
    ledger.files.remove(k);
    Ok(true)
}

/// Restores every file Vesper replaced, in every Roblox folder (uninstall / reset).
pub fn restore_all(ledger: &mut ModLedger) -> ModReport {
    let mut report = ModReport::default();
    for k in ledger.files.keys().cloned().collect::<Vec<_>>() {
        match restore_one(&k, ledger) {
            Ok(true) => report.restored += 1,
            Ok(false) => report.skipped_foreign.push(k),
            Err(e) => report.errors.push(e),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Env {
        _dir: tempfile::TempDir,
        mods: ModPaths,
        install: PathBuf,
    }

    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        png_bytes(&RgbaImage::from_pixel(w, h, Rgba(rgba))).unwrap()
    }

    /// A fake Roblox version folder with the files mods touch.
    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("Versions").join("version-test");
        let w = |rel: &str, b: &[u8]| {
            let p = install.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        };
        for rel in CURSOR_FILES {
            w(rel, &png(64, 64, [255, 255, 255, 255]));
        }
        for rel in DEATH_SOUNDS {
            w(rel, b"OggS-original");
        }
        for (size, base) in WHEEL_SIZES {
            for (suffix, mul) in SCALES {
                for layer in ["CircleBackground", "SegmentedCircle", "SelectedGradient", "SelectedLine"] {
                    w(&wheel_rel(size, layer, suffix), &png(base * mul / 10, base * mul / 10, [255, 255, 255, 128]));
                }
            }
        }
        w(&format!(r"{FONT_FAMILIES}\BuilderSans.json"), br#"{"name":"Builder Sans","faces":[{"name":"Regular","weight":400,"style":"normal","assetId":"rbxasset://fonts/BuilderSans-Regular.otf"}]}"#);
        let mods = ModPaths::new(&dir.path().join("config"), &dir.path().join("cache"));
        Env { _dir: dir, mods, install }
    }

    fn read(e: &Env, rel: &str) -> Vec<u8> {
        std::fs::read(e.install.join(rel)).unwrap()
    }

    #[test]
    fn custom_cursor_is_fitted_centred_and_restored() {
        let e = env();
        let src = e.mods.root.join("pick.png");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&src, png(200, 100, [255, 0, 0, 255])).unwrap();
        prepare_cursor(&e.mods, &Source::File { name: "pick.png".into() }, Some(&src), 50).unwrap();

        let cfg = ModsConfig { cursor: Some(CursorMod { source: Source::File { name: "pick.png".into() }, scale: 50 }), ..Default::default() };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 2, "{r:?}");
        let img = image::load_from_memory(&read(&e, CURSOR_FILES[0])).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (64, 64));
        assert_eq!(img.get_pixel(32, 32)[0], 255, "red in the centre");
        assert_eq!(img.get_pixel(2, 2)[3], 0, "transparent outside the scaled area");

        let again = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(again.written, 0);
        assert_eq!(again.unchanged, 2);

        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(off.restored, 2);
        assert_eq!(read(&e, CURSOR_FILES[0]), png(64, 64, [255, 255, 255, 255]), "original is back");
        assert!(ledger.files.is_empty());
    }

    #[test]
    fn preset_wheel_writes_every_size_and_accent_recolours_highlight() {
        let e = env();
        prepare_wheel(&e.mods, &Source::Preset { id: "dusk".into() }, None).unwrap();
        let cfg = ModsConfig {
            emote_wheel: Some(WheelMod { source: Some(Source::Preset { id: "dusk".into() }), accent: Some("#a596ff".into()) }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 36, "{r:?}");
        let ring = image::load_from_memory(&read(&e, &wheel_rel("TenFoot", "SegmentedCircle", "@3x"))).unwrap();
        assert_eq!(ring.width(), 1920);
        let hl = image::load_from_memory(&read(&e, &wheel_rel("Large", "SelectedLine", ""))).unwrap().to_rgba8();
        let px = hl.get_pixel(0, 0);
        assert_eq!((px[0], px[1], px[2], px[3]), (0xa5, 0x96, 0xff, 128), "colour changed, alpha kept");

        // Changing only the accent recolours from the backed-up original, not from our own output.
        let cfg2 = ModsConfig {
            emote_wheel: Some(WheelMod { source: Some(Source::Preset { id: "dusk".into() }), accent: Some("#ff0000".into()) }),
            ..Default::default()
        };
        apply(&cfg2, &e.mods, &e.install, &mut ledger);
        let hl = image::load_from_memory(&read(&e, &wheel_rel("Large", "SelectedLine", ""))).unwrap().to_rgba8();
        let px = hl.get_pixel(0, 0);
        assert_eq!([px[0], px[1], px[2]], [255, 0, 0]);

        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(off.restored, 36);
    }

    #[test]
    fn font_mod_adds_font_and_rewrites_families() {
        let e = env();
        let font = e.mods.root.join("f.ttf");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&font, [0u8, 1, 0, 0, 9, 9, 9]).unwrap();
        prepare_font(&e.mods, &font).unwrap();
        let cfg = ModsConfig { font: Some(FileMod { name: "f.ttf".into() }), ..Default::default() };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 2, "{r:?}");
        assert!(e.install.join(FONT_FILE).is_file());
        let fam = String::from_utf8(read(&e, &format!(r"{FONT_FAMILIES}\BuilderSans.json"))).unwrap();
        assert!(fam.contains(FONT_ASSET));
        assert!(!fam.contains("BuilderSans-Regular.otf"));

        apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert!(!e.install.join(FONT_FILE).exists(), "created file removed");
        let fam = String::from_utf8(read(&e, &format!(r"{FONT_FAMILIES}\BuilderSans.json"))).unwrap();
        assert!(fam.contains("BuilderSans-Regular.otf"), "family restored");
    }

    #[test]
    fn invalid_files_are_rejected() {
        let e = env();
        std::fs::create_dir_all(&e.mods.root).unwrap();
        let bad = e.mods.root.join("bad.ttf");
        std::fs::write(&bad, b"<html>").unwrap();
        assert!(prepare_font(&e.mods, &bad).is_err());
        assert!(prepare_sound(&e.mods, &bad).is_err());
        assert!(prepare_cursor(&e.mods, &Source::File { name: "x".into() }, Some(&bad), 100).is_err());
        assert!(prepare_wheel(&e.mods, &Source::Preset { id: "nope".into() }, None).is_err());
        let ogg = e.mods.root.join("s.ogg");
        std::fs::write(&ogg, b"OggS....").unwrap();
        assert!(prepare_sound(&e.mods, &ogg).is_ok());
    }

    #[test]
    fn missing_roblox_files_are_reported_not_created() {
        let e = env();
        for rel in DEATH_SOUNDS {
            std::fs::remove_file(e.install.join(rel)).unwrap();
        }
        let s = e.mods.root.join("s.ogg");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&s, b"OggSmine").unwrap();
        prepare_sound(&e.mods, &s).unwrap();
        let cfg = ModsConfig { death_sound: Some(FileMod { name: "s.ogg".into() }), ..Default::default() };
        let r = apply(&cfg, &e.mods, &e.install, &mut ModLedger::default());
        assert_eq!(r.written, 0);
        assert_eq!(r.missing.len(), 2);
        assert!(!e.install.join(DEATH_SOUNDS[0]).exists());
    }

    #[test]
    fn files_changed_by_others_are_left_alone() {
        let e = env();
        let s = e.mods.root.join("s.ogg");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&s, b"OggSmine").unwrap();
        prepare_sound(&e.mods, &s).unwrap();
        let cfg = ModsConfig { death_sound: Some(FileMod { name: "s.ogg".into() }), ..Default::default() };
        let mut ledger = ModLedger::default();
        apply(&cfg, &e.mods, &e.install, &mut ledger);
        std::fs::write(e.install.join(DEATH_SOUNDS[0]), b"OggSsomeone-else").unwrap();
        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(off.restored, 1, "oof.ogg restored");
        assert_eq!(off.skipped_foreign.len(), 1, "ouch.ogg kept");
        assert_eq!(read(&e, DEATH_SOUNDS[0]), b"OggSsomeone-else");
    }

    #[test]
    fn custom_files_mirror_roblox_layout_and_restore_all_works() {
        let e = env();
        let custom = e.mods.custom_dir().join(r"content\textures\new.png");
        std::fs::create_dir_all(custom.parent().unwrap()).unwrap();
        std::fs::write(&custom, b"mine").unwrap();
        let cfg = ModsConfig { custom_files: true, ..Default::default() };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 1);
        assert_eq!(read(&e, r"content\textures\new.png"), b"mine");
        let all = restore_all(&mut ledger);
        assert_eq!(all.restored, 1);
        assert!(!e.install.join(r"content\textures\new.png").exists());
    }

    #[test]
    fn similarly_named_installs_are_kept_apart() {
        let e = env();
        let sibling = e.install.with_file_name("version-test2");
        let rel = CURSOR_FILES[0];
        std::fs::create_dir_all(sibling.join(rel).parent().unwrap()).unwrap();
        std::fs::write(sibling.join(rel), png(64, 64, [1, 1, 1, 255])).unwrap();
        prepare_cursor(&e.mods, &Source::Preset { id: "dot".into() }, None, 100).unwrap();
        let cfg = ModsConfig {
            cursor: Some(CursorMod { source: Source::Preset { id: "dot".into() }, scale: 100 }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        apply(&cfg, &e.mods, &sibling, &mut ledger);
        // Turning mods off for the first install must not touch the sibling.
        let r = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(r.restored, 0);
        assert_ne!(std::fs::read(sibling.join(rel)).unwrap(), png(64, 64, [1, 1, 1, 255]), "sibling keeps its mod");
    }

    #[test]
    fn hex_colours_parse() {
        assert_eq!(parse_hex("#A596FF"), Some([0xa5, 0x96, 0xff]));
        assert_eq!(parse_hex("a596ff"), Some([0xa5, 0x96, 0xff]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
    }
}
