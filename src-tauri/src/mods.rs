//! Roblox customisations: cursor, emote wheel, font, death sound and custom files.
//!
//! These replace files in Roblox's `content` folder, the same mechanism other
//! launchers have used for years. They change how Roblox looks and sounds on
//! your PC only; nothing is injected and no game code is touched.
//!
//! Safety rules:
//! * Before a Roblox file is replaced, the original is copied to
//!   `cache\originals\<build>\…` and restored when the mod is turned off.
//!
//! Paths below are written Windows-style (`content\fonts\…`) and turned into
//! real paths with [`rel_path`]. On macOS they are relative to
//! `Roblox.app/Contents/Resources`, where Roblox keeps its `content` folder.
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

pub const CURSOR_PRESETS: &[(&str, &str)] =
    &[("dot", "Vesper dot"), ("crosshair", "Crosshair"), ("ring", "Ring"), ("plus", "Plus"), ("arrow", "Arrow")];
/// Cursor size range, in percent of the 64×64 canvas a style is drawn for.
pub const CURSOR_SCALE: (u8, u8) = (30, 150);
pub const WHEEL_PRESETS: &[(&str, &str)] = &[("dusk", "Dusk"), ("glass", "Glass"), ("minimal", "Minimal")];

fn cursor_preset(id: &str) -> Option<&'static [u8]> {
    match id {
        "dot" => Some(include_bytes!("../resources/mods/cursors/dot.png")),
        "crosshair" => Some(include_bytes!("../resources/mods/cursors/crosshair.png")),
        "ring" => Some(include_bytes!("../resources/mods/cursors/ring.png")),
        "plus" => Some(include_bytes!("../resources/mods/cursors/plus.png")),
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

/// Dividers, edge and see-through centre drawn over a picture wheel (1920×1920).
fn photo_overlay(dividers: bool) -> &'static [u8] {
    if dividers {
        include_bytes!("../resources/mods/wheels/photo/SegmentedCircle.png")
    } else {
        include_bytes!("../resources/mods/wheels/photo/SegmentedCircleClean.png")
    }
}

// ------------------------------------------------------------------ configuration

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Source {
    Preset {
        id: String,
    },
    /// A file the user picked; `name` is its original file name, for display.
    File {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorMod {
    pub source: Source,
    /// Size in percent: of the 64×64 canvas for an image, of the drawn size for a style.
    #[serde(default = "full_size")]
    pub scale: u8,
    /// Tint for a built-in style as `#rrggbb`; `None` keeps its own colours.
    #[serde(default)]
    pub color: Option<String>,
}

fn full_size() -> u8 {
    100
}

/// How a picture wheel is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WheelLook {
    /// How much the picture is darkened so emotes and numbers stay readable, in percent.
    pub dim: u8,
    /// Draw Roblox-style dividers between the eight slots.
    pub dividers: bool,
}

impl Default for WheelLook {
    fn default() -> Self {
        Self { dim: 30, dividers: true }
    }
}

pub const MAX_WHEEL_DIM: u8 = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelMod {
    /// `None` keeps Roblox's own wheel and only recolours the highlight.
    pub source: Option<Source>,
    /// Highlight colour as `#rrggbb`; `None` keeps Roblox's white.
    pub accent: Option<String>,
    /// Only used for a picture.
    #[serde(default)]
    pub look: WheelLook,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMod {
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SkyLayout {
    /// One 360° picture.
    #[default]
    Panorama,
    /// Six pictures, one per side.
    Faces,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyMod {
    pub source: Source,
    #[serde(default)]
    pub layout: SkyLayout,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModsConfig {
    pub cursor: Option<CursorMod>,
    pub emote_wheel: Option<WheelMod>,
    pub font: Option<FileMod>,
    pub death_sound: Option<FileMod>,
    pub sky: Option<SkyMod>,
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
        self.root.join(rel_path(rel))
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

/// Fits `img` inside a box of `scale_pct` of a `size`×`size` canvas, centred
/// (Roblox draws cursor images centred on the mouse position). Above 100 %
/// the edges are cropped.
pub fn fit_centered(img: &DynamicImage, size: u32, scale_pct: u8) -> RgbaImage {
    let pct = scale_pct.clamp(CURSOR_SCALE.0, CURSOR_SCALE.1) as f32;
    let box_size = ((size as f32) * pct / 100.0).round().max(1.0) as u32;
    let fitted = img.resize(box_size, box_size, FilterType::Lanczos3).to_rgba8();
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let x = (size as i64 - fitted.width() as i64) / 2;
    let y = (size as i64 - fitted.height() as i64) / 2;
    image::imageops::overlay(&mut canvas, &fitted, x, y);
    canvas
}

/// Multiplies every pixel by `rgb`: white becomes `rgb`, dark outlines stay dark.
pub fn tint(img: &mut RgbaImage, rgb: [u8; 3]) {
    for p in img.pixels_mut() {
        for c in 0..3 {
            p[c] = ((p[c] as u32 * rgb[c] as u32 + 127) / 255) as u8;
        }
    }
}

/// The picture as a round, darkened disc filling a `side`×`side` square,
/// with an anti-aliased edge.
pub fn photo_disc(img: &DynamicImage, side: u32, dim_pct: u8) -> RgbaImage {
    let mut disc = square(img, side);
    let keep = 1.0 - dim_pct.min(MAX_WHEEL_DIM) as f32 / 100.0;
    let c = side as f32 / 2.0;
    for (x, y, p) in disc.enumerate_pixels_mut() {
        let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
        let coverage = (c - d).clamp(0.0, 1.0);
        for ch in 0..3 {
            p[ch] = (p[ch] as f32 * keep).round() as u8;
        }
        p[3] = (p[3] as f32 * coverage).round() as u8;
    }
    disc
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

/// Radius of the wheel's centre disc as a share of the wheel's radius (Roblox's layout).
pub const WHEEL_INNER: f32 = 0.465;

/// Roblox's slot highlight, optionally recoloured. Its point sits at the wheel's
/// centre and is brightest there; Roblox's solid centre disc normally hides that
/// part. With a see-through centre (a picture wheel), `cut_radius` removes it so
/// no white tip shows over the picture.
pub fn highlight_gradient(
    original: &[u8],
    accent: Option<[u8; 3]>,
    cut_radius: Option<f32>,
) -> Result<Vec<u8>, String> {
    let recoloured;
    let bytes = match accent {
        Some(rgb) => {
            recoloured = recolor(original, rgb)?;
            &recoloured[..]
        }
        None => original,
    };
    let Some(r) = cut_radius else { return Ok(bytes.to_vec()) };
    let mut img = image::load_from_memory(bytes).map_err(|e| e.to_string())?.to_rgba8();
    let apex_y = img.height() as f32 / 2.0;
    for (x, y, p) in img.enumerate_pixels_mut() {
        let d = ((x as f32 + 0.5).powi(2) + (y as f32 + 0.5 - apex_y).powi(2)).sqrt();
        let keep = ((d - r) / 2.0).clamp(0.0, 1.0);
        p[3] = (p[3] as f32 * keep).round() as u8;
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

/// Prepares the cursor files from a preset or an image. `color` tints a preset.
pub fn prepare_cursor(
    p: &ModPaths,
    source: &Source,
    file: Option<&Path>,
    scale: u8,
    color: Option<[u8; 3]>,
) -> Result<(), String> {
    let bytes = match source {
        Source::Preset { id } => {
            let art = cursor_preset(id).ok_or("Unknown cursor style.")?;
            if scale == 100 && color.is_none() {
                art.to_vec()
            } else {
                let img = image::load_from_memory(art).map_err(|e| e.to_string())?;
                let mut out = if scale == 100 { img.to_rgba8() } else { fit_centered(&img, CURSOR_SIZE, scale) };
                if let Some(rgb) = color {
                    tint(&mut out, rgb);
                }
                png_bytes(&out)?
            }
        }
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
///
/// A picture is cut into a round, darkened disc (Roblox draws a square image as
/// a square, covering the wheel) with dividers and a see-through centre drawn
/// over it, and goes into the ring layer: Roblox draws that layer exactly as
/// given, while it doesn't show a picture put in the background layer. Roblox's
/// own background stays underneath.
pub fn prepare_wheel(p: &ModPaths, source: &Source, file: Option<&Path>, look: WheelLook) -> Result<(), String> {
    let decode = |b: &[u8]| image::load_from_memory(b).map_err(|e| e.to_string());
    enum Art {
        Preset { background: DynamicImage, ring: DynamicImage },
        Picture { picture: DynamicImage, overlay: DynamicImage },
    }
    let art = match source {
        Source::Preset { id } => {
            let (bg, ring) = wheel_preset(id).ok_or("Unknown emote wheel style.")?;
            Art::Preset { background: decode(bg)?, ring: decode(ring)? }
        }
        Source::File { .. } => Art::Picture {
            picture: load_image(file.ok_or("Choose an image.")?)?,
            overlay: decode(photo_overlay(look.dividers))?,
        },
    };
    let _ = std::fs::remove_dir_all(p.prepared(WHEEL_DIR));
    let write = |size: &str, layer: &str, suffix: &str, img: &RgbaImage| {
        write_atomic(&p.prepared(&wheel_rel(size, layer, suffix)), &png_bytes(img)?).map_err(|e| e.to_string())
    };
    for (size, base) in WHEEL_SIZES {
        for (suffix, mul) in SCALES {
            let side = base * mul;
            match &art {
                Art::Preset { background, ring } => {
                    write(size, "CircleBackground", suffix, &square(background, side))?;
                    write(size, "SegmentedCircle", suffix, &square(ring, side))?;
                }
                Art::Picture { picture, overlay } => {
                    let mut ring = photo_disc(picture, side, look.dim);
                    image::imageops::overlay(&mut ring, &square(overlay, side), 0, 0);
                    write(size, "SegmentedCircle", suffix, &ring)?;
                }
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
    let head = std::fs::File::open(file).and_then(|mut f| {
        let mut b = [0u8; 4];
        std::io::Read::read_exact(&mut f, &mut b).map(|_| b)
    });
    if matches!(head.as_ref().map(|b| &b[..]), Ok(b"wOFF" | b"wOF2")) {
        return Err("That's a web font (.woff). Roblox needs the .ttf or .otf version of the font.".into());
    }
    let bytes = check_header(
        file,
        MAX_FONT_BYTES,
        |h| h == [0x00, 0x01, 0x00, 0x00] || h == b"OTTO" || h == b"ttcf" || h == b"true",
        "font (.ttf, .otf or .ttc)",
    )?;
    write_atomic(&p.prepared(FONT_FILE), &bytes).map_err(|e| e.to_string())
}

/// The name a font file gives itself ("Poppins ExtraBold"), from its `name` table.
pub fn font_name(bytes: &[u8]) -> Option<String> {
    let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize);
    let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    // A collection lists its fonts; the first one is the one Roblox uses.
    let font = if bytes.starts_with(b"ttcf") { u32_at(12)? } else { 0 };
    let tables = u16_at(font + 4)?;
    let name = (0..tables).map(|i| font + 12 + i * 16).find(|&rec| bytes.get(rec..rec + 4) == Some(b"name"))?;
    let table = u32_at(name + 8)?;
    let count = u16_at(table + 2)?;
    let strings = table + u16_at(table + 4)?;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = table + 6 + i * 12;
        let (platform, language, id) = (u16_at(rec)?, u16_at(rec + 4)?, u16_at(rec + 6)?);
        let (len, offset) = (u16_at(rec + 8)?, u16_at(rec + 10)?);
        // Full name first, then the family name; Windows English, then any language.
        let rank = match (id, platform, language) {
            (4, 3, 0x409) => 0,
            (4, 3, _) => 1,
            (1, 3, 0x409) => 2,
            (1, 3, _) => 3,
            (4, 1, _) => 4,
            (1, 1, _) => 5,
            _ => continue,
        };
        if best.as_ref().is_some_and(|(r, _)| *r <= rank) {
            continue;
        }
        let Some(raw) = bytes.get(strings + offset..strings + offset + len) else { continue };
        let text = if platform == 3 {
            let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
            String::from_utf16_lossy(&units)
        } else {
            raw.iter().map(|&b| b as char).collect()
        };
        let text = text.trim().to_owned();
        if !text.is_empty() {
            best = Some((rank, text));
        }
    }
    best.map(|(_, name)| name.chars().take(80).collect())
}

/// Rebuilds a picture wheel prepared by an older Vesper, which put the picture in
/// the background layer Roblox doesn't show. Returns true if it rebuilt it.
pub fn upgrade_picture_wheel(p: &ModPaths, cfg: &ModsConfig) -> Result<bool, String> {
    let Some(w) = &cfg.emote_wheel else { return Ok(false) };
    let Some(source @ Source::File { .. }) = &w.source else { return Ok(false) };
    let old_layout = p.prepared(&wheel_rel("Large", "CircleBackground", "")).is_file();
    let picture = p.root.join("sources").join("wheel");
    if !old_layout || !picture.is_file() {
        return Ok(false);
    }
    prepare_wheel(p, source, Some(&picture), w.look)?;
    Ok(true)
}

/// Launchers that keep a custom Roblox font where Vesper can bring it over from.
pub const FONT_LAUNCHERS: [&str; 2] = ["Fishstrap", "Bloxstrap"];

/// The custom font a launcher applies to Roblox, if it has one.
pub fn launcher_font(local_appdata: &Path, launcher: &str) -> Option<PathBuf> {
    FONT_LAUNCHERS
        .contains(&launcher)
        .then(|| local_appdata.join(launcher).join("Modifications").join(rel_path(FONT_FILE)))
        .filter(|p| p.is_file())
}

/// Uses a font file: prepares it and records it in the configuration.
pub fn import_font(p: &ModPaths, cfg: &mut ModsConfig, file: &Path, name: String) -> Result<(), String> {
    prepare_font(p, file)?;
    cfg.font = Some(FileMod { name });
    Ok(())
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

/// Where the preview of the prepared sky is kept.
pub fn sky_preview(p: &ModPaths) -> PathBuf {
    p.root.join("previews").join("sky.png")
}

/// Encodes six faces the way Roblox's own sky is stored, plus a preview.
pub fn prepare_sky(p: &ModPaths, faces: &[(&str, RgbaImage)]) -> Result<(), String> {
    if faces.len() != crate::sky::FACES.len() {
        return Err("A skybox needs all six sides.".into());
    }
    let encoded: Vec<(String, Vec<u8>)> = std::thread::scope(|s| {
        let jobs: Vec<_> = faces
            .iter()
            .map(|(face, img)| s.spawn(move || (crate::sky::face_rel(face), crate::sky::encode_dds(img))))
            .collect();
        jobs.into_iter().filter_map(|j| j.join().ok()).collect()
    });
    for (rel, bytes) in &encoded {
        write_atomic(&p.prepared(rel), bytes).map_err(|e| format!("Couldn't save the sky: {e}"))?;
    }
    let strip = crate::sky::preview_strip(faces, 160);
    let preview = sky_preview(p);
    std::fs::create_dir_all(preview.parent().expect("has parent")).map_err(|e| e.to_string())?;
    write_atomic(&preview, &png_bytes(&strip)?).map_err(|e| format!("Couldn't save the sky preview: {e}"))
}

/// Removes a mod's prepared files.
pub fn clear_prepared(p: &ModPaths, kind: &str) {
    let rels: Vec<String> = match kind {
        "sky" => {
            let _ = std::fs::remove_file(sky_preview(p));
            crate::sky::FACES.iter().map(|f| crate::sky::face_rel(f)).collect()
        }
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
    /// The file's exact path. Ledger keys are lowercased for lookup, but
    /// writing through a lowercased path would rename the file on NTFS.
    #[serde(default)]
    pub path: Option<PathBuf>,
    /// The Roblox build the file was written for (see `detect::build_id`).
    #[serde(default)]
    pub build: Option<String>,
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

/// Ledger key for a file. Windows paths are case-insensitive, so they are compared lowercased.
fn key(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.into_owned()
    }
}

/// Turns a Windows-style relative path (`content\sounds\oof.ogg`) into a path for this system.
pub fn rel_path(rel: &str) -> PathBuf {
    rel.split(['\\', '/']).filter(|c| !c.is_empty()).collect()
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

fn desired_files(
    cfg: &ModsConfig,
    p: &ModPaths,
    install: &Path,
    originals: &Path,
) -> (BTreeMap<String, Desired>, Vec<String>) {
    let mut out = BTreeMap::new();
    let mut errors = Vec::new();
    let mut add_prepared =
        |rel: &str, kind: &'static str, create: bool, out: &mut BTreeMap<String, Desired>| match std::fs::read(
            p.prepared(rel),
        ) {
            Ok(bytes) => {
                out.insert(rel.to_owned(), Desired { kind, bytes, create });
            }
            Err(_) => errors.push(format!("{kind}: prepared file {rel} is missing; choose the {kind} again.")),
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
        let accent = w.accent.as_deref().and_then(parse_hex);
        let see_through = matches!(w.source, Some(Source::File { .. }));
        for (size, base) in WHEEL_SIZES {
            for (suffix, mul) in SCALES {
                let cut = see_through.then_some(WHEEL_INNER * (base * mul) as f32 / 2.0);
                for layer in ["SelectedGradient", "SelectedLine"] {
                    let (rgb, cut) = if layer == "SelectedGradient" { (accent, cut) } else { (accent, None) };
                    if rgb.is_none() && cut.is_none() {
                        continue;
                    }
                    let rel = wheel_rel(size, layer, suffix);
                    // Start from Roblox's own shape: the backup if we already replaced it.
                    let source = original_for(install, &rel, originals);
                    if let Some(bytes) = source.and_then(|b| highlight_gradient(&b, rgb, cut).ok()) {
                        out.insert(rel, Desired { kind: "emoteWheel", bytes, create: false });
                    }
                }
            }
        }
    }
    if cfg.sky.is_some() {
        for face in crate::sky::FACES {
            add_prepared(&crate::sky::face_rel(face), "sky", false, &mut out);
        }
    }
    if cfg.font.is_some() {
        add_prepared(FONT_FILE, "font", true, &mut out);
        let families = install.join(rel_path(FONT_FAMILIES));
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
/// `originals` is the backup folder of this build.
fn original_for(install: &Path, rel: &str, originals: &Path) -> Option<Vec<u8>> {
    std::fs::read(originals.join(rel_path(rel))).ok().or_else(|| std::fs::read(install.join(rel_path(rel))).ok())
}

/// Points every face of a font family manifest at CustomFont.ttf.
///
/// Only the asset ids change: the rest of Roblox's file (key order, layout)
/// stays as Roblox wrote it, as Bloxstrap and Fishstrap leave it.
pub fn rewrite_family(original: &[u8]) -> Option<Vec<u8>> {
    const KEY: &str = "\"assetId\"";
    let text = std::str::from_utf8(original).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(KEY) {
        let tail = &rest[at + KEY.len()..];
        let colon = tail.find(|c: char| !c.is_whitespace())?;
        if !tail[colon..].starts_with(':') {
            return None;
        }
        let value_at = colon + 1 + tail[colon + 1..].find(|c: char| !c.is_whitespace())?;
        let value = tail[value_at..].strip_prefix('"')?;
        let end = closing_quote(value)?;
        out.push_str(&rest[..at + KEY.len() + value_at]);
        out.push('"');
        out.push_str(FONT_ASSET);
        out.push('"');
        rest = &value[end + 1..];
    }
    out.push_str(rest);
    // The result must still be a manifest with every face on the custom font.
    let v: serde_json::Value = serde_json::from_str(out.trim_start_matches('\u{feff}')).ok()?;
    let faces = v.get("faces")?.as_array()?;
    let all = faces.iter().all(|f| f.get("assetId").and_then(|a| a.as_str()) == Some(FONT_ASSET));
    (!faces.is_empty() && all).then(|| out.into_bytes())
}

/// Index of the quote that ends a JSON string (the opening quote already consumed).
fn closing_quote(s: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some(i),
            _ => {}
        }
    }
    None
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

/// Brings a Windows version folder in line with `cfg`: writes enabled mods, restores disabled ones.
pub fn apply(cfg: &ModsConfig, p: &ModPaths, install: &Path, ledger: &mut ModLedger) -> ModReport {
    let Some(build) = install.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return ModReport::default();
    };
    apply_build(cfg, p, install, &build, ledger)
}

/// Brings the Roblox files under `install` (the folder holding `content`) in
/// line with `cfg`. `build` names the Roblox build, so backups of different
/// builds stay apart (see `detect::build_id`).
pub fn apply_build(cfg: &ModsConfig, p: &ModPaths, install: &Path, build: &str, ledger: &mut ModLedger) -> ModReport {
    let mut report = ModReport::default();
    let originals = p.originals.join(build);
    let (desired, errors) = desired_files(cfg, p, install, &originals);
    report.errors = errors;

    for (rel, want) in &desired {
        let target = install.join(rel_path(rel));
        let k = key(&target);
        let current = std::fs::read(&target).ok();
        if current.is_none() && !want.create {
            report.missing.push(rel.clone());
            continue;
        }
        // On macOS Roblox updates in place: a file written for an older build
        // that no longer holds Vesper's content is the new build's original,
        // and is backed up afresh.
        let ours = ledger.files.get(&k).filter(|o| {
            o.build.as_deref().is_none_or(|b| b == build) || current.as_ref().is_some_and(|c| sha(c) == o.sha256)
        });
        if let (Some(cur), Some(entry)) = (&current, ours) {
            if sha(cur) == sha(&want.bytes) {
                report.unchanged += 1;
                continue;
            }
            if sha(cur) != entry.sha256 && !is_original(entry, cur) {
                // Changed by something else after Vesper wrote it.
                report.skipped_foreign.push(rel.clone());
                continue;
            }
            // Otherwise it's Vesper's file, or Roblox's own file is back (a repair,
            // a reset or another tool put it back); writing again loses nothing.
        }
        let mut original = ours.and_then(|o| o.original.clone());
        if ours.is_none() {
            if let Some(cur) = &current {
                if sha(cur) == sha(&want.bytes) {
                    report.unchanged += 1;
                    continue;
                }
                let backup = originals.join(rel_path(rel));
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
                ledger.files.insert(
                    k,
                    AppliedFile {
                        kind: want.kind.into(),
                        sha256: sha(&want.bytes),
                        original,
                        path: Some(target.clone()),
                        build: Some(build.to_owned()),
                    },
                );
                report.written += 1;
            }
            Err(e) => report.errors.push(format!("Couldn't write {rel}: {e}")),
        }
    }

    // Undo files from mods that are no longer wanted in this install.
    // The separator matters: "version-a" must not match "version-ab".
    let prefix = format!("{}{}", key(install), std::path::MAIN_SEPARATOR);
    let stale: Vec<String> = ledger
        .files
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .filter(|k| !desired.keys().any(|rel| key(&install.join(rel_path(rel))) == **k))
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

/// Is `current` exactly the Roblox file Vesper backed up before replacing it?
fn is_original(entry: &AppliedFile, current: &[u8]) -> bool {
    entry.original.as_deref().and_then(|b| std::fs::read(b).ok()).is_some_and(|o| sha(&o) == sha(current))
}

/// Restores one file. `Ok(false)` if it was changed by something else and left alone.
fn restore_one(k: &str, ledger: &mut ModLedger) -> Result<bool, String> {
    let Some(entry) = ledger.files.get(k).cloned() else { return Ok(true) };
    // Restore through the exact path; the lowercased key would rename the file.
    let target = entry.path.clone().unwrap_or_else(|| PathBuf::from(k));
    match std::fs::read(&target) {
        Ok(cur) if is_original(&entry, &cur) => {
            // Already back to Roblox's file.
            ledger.files.remove(k);
            return Ok(true);
        }
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
            let p = install.join(rel_path(rel));
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
        std::fs::read(e.install.join(rel_path(rel))).unwrap()
    }

    #[test]
    fn custom_cursor_is_fitted_centred_and_restored() {
        let e = env();
        let src = e.mods.root.join("pick.png");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&src, png(200, 100, [255, 0, 0, 255])).unwrap();
        prepare_cursor(&e.mods, &Source::File { name: "pick.png".into() }, Some(&src), 50, None).unwrap();

        let cfg = ModsConfig {
            cursor: Some(CursorMod { source: Source::File { name: "pick.png".into() }, scale: 50, color: None }),
            ..Default::default()
        };
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
    fn sky_replaces_robloxs_faces_and_restores_them() {
        let e = env();
        for face in crate::sky::FACES {
            let p = e.install.join(crate::sky::face_rel(face));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, format!("original {face}")).unwrap();
        }
        let faces = crate::sky::render(64, &crate::sky::preset_color("sunset").unwrap());
        prepare_sky(&e.mods, &faces).unwrap();
        assert!(sky_preview(&e.mods).is_file());
        let cfg = ModsConfig {
            sky: Some(SkyMod { source: Source::Preset { id: "sunset".into() }, layout: SkyLayout::Panorama }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 6, "{r:?}");
        let ft = read(&e, &crate::sky::face_rel("ft"));
        assert!(crate::sky::decode_dds(&ft).is_some(), "a DXT1 texture like Roblox's");
        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(off.restored, 6);
        assert_eq!(read(&e, &crate::sky::face_rel("up")), b"original up");
        clear_prepared(&e.mods, "sky");
        assert!(!sky_preview(&e.mods).exists());
    }

    #[test]
    fn preset_wheel_writes_every_size_and_accent_recolours_highlight() {
        let e = env();
        prepare_wheel(&e.mods, &Source::Preset { id: "dusk".into() }, None, WheelLook::default()).unwrap();
        let cfg = ModsConfig {
            emote_wheel: Some(WheelMod {
                source: Some(Source::Preset { id: "dusk".into() }),
                accent: Some("#a596ff".into()),
                look: WheelLook::default(),
            }),
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
            emote_wheel: Some(WheelMod {
                source: Some(Source::Preset { id: "dusk".into() }),
                accent: Some("#ff0000".into()),
                look: WheelLook::default(),
            }),
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
    fn picture_wheel_is_round_dimmed_and_has_dividers() {
        let e = env();
        let src = e.mods.root.join("me.png");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&src, png(300, 200, [200, 100, 50, 255])).unwrap();
        let source = Source::File { name: "me.png".into() };
        prepare_wheel(&e.mods, &source, Some(&src), WheelLook { dim: 50, dividers: true }).unwrap();

        assert!(
            !e.mods.prepared(&wheel_rel("Large", "CircleBackground", "")).exists(),
            "Roblox's own background stays; it doesn't show pictures"
        );
        let ring_rel = wheel_rel("Large", "SegmentedCircle", "");
        let ring = image::load_from_memory(&std::fs::read(e.mods.prepared(&ring_rel)).unwrap()).unwrap().to_rgba8();
        assert_eq!(ring.dimensions(), (400, 400));
        assert_eq!(ring.get_pixel(1, 1)[3], 0, "corners are outside the circle");
        assert_eq!(ring.get_pixel(399, 0)[3], 0);
        // Mid-slot, between the centre and the edge: the picture, darkened by half.
        let slot = ring.get_pixel(200, 50);
        assert_eq!((slot[0], slot[1], slot[2], slot[3]), (100, 50, 25, 255), "picture darkened by half");
        let centre = ring.get_pixel(200, 200);
        assert_eq!(centre[3], 255);
        assert!(centre[0] < 80, "centre darkened further so text stays readable: {centre:?}");
        // A divider at 22.5°, half-way out.
        let a = 22.5f32.to_radians();
        let (x, y) = (200.0 + a.sin() * 140.0, 200.0 - a.cos() * 140.0);
        let divider = ring.get_pixel(x as u32, y as u32);
        assert!(divider[1] > 90, "a light divider over the darkened picture: {divider:?}");

        prepare_wheel(&e.mods, &source, Some(&src), WheelLook { dim: 0, dividers: false }).unwrap();
        let ring = image::load_from_memory(&std::fs::read(e.mods.prepared(&ring_rel)).unwrap()).unwrap().to_rgba8();
        let plain = ring.get_pixel(x as u32, y as u32);
        assert_eq!((plain[0], plain[1], plain[2]), (200, 100, 50), "no divider, picture not darkened");

        let cfg = ModsConfig {
            emote_wheel: Some(WheelMod { source: Some(source), accent: None, look: WheelLook::default() }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 18, "ring and trimmed highlight for every size and scale: {r:?}");
        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(off.restored, 18);
    }

    #[test]
    fn files_put_back_to_robloxs_original_are_taken_over_again() {
        let e = env();
        prepare_cursor(&e.mods, &Source::Preset { id: "dot".into() }, None, 100, None).unwrap();
        let cfg = ModsConfig {
            cursor: Some(CursorMod { source: Source::Preset { id: "dot".into() }, scale: 100, color: None }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        assert_eq!(apply(&cfg, &e.mods, &e.install, &mut ledger).written, 2);

        // Something (a repair, a reset, another tool) puts Roblox's own file back.
        let target = e.install.join(rel_path(CURSOR_FILES[0]));
        std::fs::write(&target, png(64, 64, [255, 255, 255, 255])).unwrap();
        let again = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!((again.written, again.skipped_foreign.len()), (1, 0), "{again:?}");
        assert_eq!(std::fs::read(&target).unwrap(), cursor_preset("dot").unwrap());

        // Anything else is still left alone.
        std::fs::write(&target, b"someone else's cursor").unwrap();
        let foreign = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(foreign.skipped_foreign, vec![CURSOR_FILES[0].to_owned()]);

        // Restoring a file that is already Roblox's counts as restored, not foreign.
        std::fs::write(&target, png(64, 64, [255, 255, 255, 255])).unwrap();
        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!((off.restored, off.skipped_foreign.len()), (2, 0), "{off:?}");
        assert!(ledger.files.is_empty());
    }

    #[test]
    fn highlight_point_is_trimmed_for_see_through_centres() {
        let original = png(100, 60, [255, 255, 255, 255]);
        let cut = highlight_gradient(&original, Some([255, 0, 0]), Some(40.0)).unwrap();
        let img = image::load_from_memory(&cut).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(5, 30)[3], 0, "point removed");
        assert_eq!(img.get_pixel(80, 30).0, [255, 0, 0, 255], "outer part kept and recoloured");
        assert_eq!(highlight_gradient(&original, None, None).unwrap(), original);
    }

    #[test]
    fn preset_cursors_can_be_tinted_and_resized() {
        let e = env();
        let style = Source::Preset { id: "plus".into() };
        prepare_cursor(&e.mods, &style, None, 100, None).unwrap();
        let plain = std::fs::read(e.mods.prepared(CURSOR_FILES[0])).unwrap();
        assert_eq!(plain, cursor_preset("plus").unwrap(), "unchanged style is written as drawn");

        prepare_cursor(&e.mods, &style, None, 100, Some([255, 0, 0])).unwrap();
        let red = image::load_from_memory(&std::fs::read(e.mods.prepared(CURSOR_FILES[0])).unwrap()).unwrap();
        let px = red.to_rgba8().get_pixel(32, 26).0;
        assert!(px[0] > 200 && px[1] < 40 && px[2] < 40, "white arm is now red: {px:?}");

        prepare_cursor(&e.mods, &style, None, 50, None).unwrap();
        let small = image::load_from_memory(&std::fs::read(e.mods.prepared(CURSOR_FILES[0])).unwrap()).unwrap();
        let small = small.to_rgba8();
        assert_eq!(small.dimensions(), (64, 64));
        assert_eq!(small.get_pixel(32, 21)[3], 0, "arm end moved inwards");
        assert!(small.get_pixel(32, 28)[3] > 0);
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
        assert!(e.install.join(rel_path(FONT_FILE)).is_file());
        let fam = String::from_utf8(read(&e, &format!(r"{FONT_FAMILIES}\BuilderSans.json"))).unwrap();
        assert!(fam.contains(FONT_ASSET));
        assert!(!fam.contains("BuilderSans-Regular.otf"));

        apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert!(!e.install.join(rel_path(FONT_FILE)).exists(), "created file removed");
        let fam = String::from_utf8(read(&e, &format!(r"{FONT_FAMILIES}\BuilderSans.json"))).unwrap();
        assert!(fam.contains("BuilderSans-Regular.otf"), "family restored");
    }

    fn font_with_names(names: &[(u16, u16, u16, &str)]) -> Vec<u8> {
        let (mut records, mut strings) = (Vec::new(), Vec::new());
        for &(platform, language, id, text) in names {
            let encoded: Vec<u8> = if platform == 3 {
                text.encode_utf16().flat_map(u16::to_be_bytes).collect()
            } else {
                text.bytes().collect()
            };
            for v in [platform, u16::from(platform == 3), language, id, encoded.len() as u16, strings.len() as u16] {
                records.extend(v.to_be_bytes());
            }
            strings.extend(encoded);
        }
        let mut table = Vec::new();
        for v in [0u16, names.len() as u16, (6 + records.len()) as u16] {
            table.extend(v.to_be_bytes());
        }
        table.extend(records);
        table.extend(strings);
        let mut font = vec![0, 1, 0, 0];
        for v in [1u16, 16, 0, 0] {
            font.extend(v.to_be_bytes());
        }
        font.extend(b"name");
        for v in [0u32, 28, table.len() as u32] {
            font.extend(v.to_be_bytes());
        }
        font.extend(table);
        font
    }

    #[test]
    fn font_names_are_read_from_the_file() {
        let font =
            font_with_names(&[(1, 0, 4, "Mac Name"), (3, 0x407, 4, "Deutsch"), (3, 0x409, 4, "Poppins ExtraBold")]);
        assert_eq!(font_name(&font).as_deref(), Some("Poppins ExtraBold"));
        let family_only = font_with_names(&[(3, 0x409, 1, "Poppins"), (3, 0x409, 2, "Bold")]);
        assert_eq!(font_name(&family_only).as_deref(), Some("Poppins"));
        // A collection: a 16-byte header pointing at its first font. Offsets
        // inside a collection count from the start of the file.
        let mut face = font_with_names(&[(3, 0x409, 4, "First Face")]);
        face[20..24].copy_from_slice(&(28u32 + 16).to_be_bytes());
        let mut collection = b"ttcf\0\x01\0\0\0\0\0\x01\0\0\0\x10".to_vec();
        collection.extend(face);
        assert_eq!(font_name(&collection).as_deref(), Some("First Face"));
        assert_eq!(font_name(b"\0\x01\0\0garbage"), None);
        assert_eq!(font_name(&[]), None);
    }

    #[test]
    fn font_families_keep_roblox_layout() {
        let original = "{\n\t\"name\": \"Builder Sans\",\n\t\"faces\": [\n\t\t{\n\t\t\t\"name\": \"Thin\",\n\t\t\t\"weight\": 100,\n\t\t\t\"style\": \"normal\",\n\t\t\t\"assetId\": \"rbxassetid://16480735997\"\n\t\t},\n\t\t{\n\t\t\t\"name\": \"Regular\",\n\t\t\t\"weight\": 400,\n\t\t\t\"style\": \"normal\",\n\t\t\t\"assetId\" : \"rbxasset://fonts/Builder\\\"Sans.otf\"\n\t\t}\n\t]\n}\n";
        let out = String::from_utf8(rewrite_family(original.as_bytes()).unwrap()).unwrap();
        let expected = original
            .replace("rbxassetid://16480735997", "rbxasset://fonts/CustomFont.ttf")
            .replace("rbxasset://fonts/Builder\\\"Sans.otf", "rbxasset://fonts/CustomFont.ttf");
        assert_eq!(out, expected, "only the asset ids change");
        assert!(rewrite_family(br#"{"name":"x","faces":[{"name":"Regular"}]}"#).is_none(), "faces without a file");
        assert!(rewrite_family(br#"{"name":"assetId","faces":[]}"#).is_none());
        assert!(rewrite_family(b"not json").is_none());
    }

    #[test]
    fn picture_wheels_from_older_versions_are_rebuilt() {
        let e = env();
        let source = Source::File { name: "me.png".into() };
        let cfg = ModsConfig {
            emote_wheel: Some(WheelMod { source: Some(source.clone()), accent: None, look: WheelLook::default() }),
            ..Default::default()
        };
        // What an older Vesper left: the picture in the background layer.
        let old_bg = e.mods.prepared(&wheel_rel("Large", "CircleBackground", ""));
        std::fs::create_dir_all(old_bg.parent().unwrap()).unwrap();
        std::fs::write(&old_bg, png(400, 400, [9, 9, 9, 255])).unwrap();
        let picture = e.mods.root.join("sources").join("wheel");
        std::fs::create_dir_all(picture.parent().unwrap()).unwrap();
        std::fs::write(&picture, png(300, 300, [200, 100, 50, 255])).unwrap();

        assert!(upgrade_picture_wheel(&e.mods, &cfg).unwrap());
        assert!(!old_bg.exists(), "the background layer goes back to Roblox's");
        let ring = std::fs::read(e.mods.prepared(&wheel_rel("Large", "SegmentedCircle", ""))).unwrap();
        let ring = image::load_from_memory(&ring).unwrap().to_rgba8();
        assert_eq!(ring.get_pixel(200, 50)[3], 255, "the picture is in the ring layer");
        assert!(!upgrade_picture_wheel(&e.mods, &cfg).unwrap(), "only once");
    }

    #[test]
    fn a_launchers_font_is_found_and_imported() {
        let e = env();
        let local = e.mods.root.parent().unwrap().join("local");
        assert!(launcher_font(&local, "Fishstrap").is_none());
        let font = local.join("Fishstrap").join("Modifications").join(rel_path(FONT_FILE));
        std::fs::create_dir_all(font.parent().unwrap()).unwrap();
        std::fs::write(&font, [0x00, 0x01, 0x00, 0x00, 1, 2, 3]).unwrap();
        assert_eq!(launcher_font(&local, "Fishstrap"), Some(font.clone()));
        assert!(launcher_font(&local, "SomethingElse").is_none(), "only known launchers");

        let mut cfg = ModsConfig::default();
        import_font(&e.mods, &mut cfg, &font, "Fishstrap's font".into()).unwrap();
        assert_eq!(cfg.font, Some(FileMod { name: "Fishstrap's font".into() }));
        assert_eq!(std::fs::read(e.mods.prepared(FONT_FILE)).unwrap(), std::fs::read(&font).unwrap());
    }

    #[test]
    fn invalid_files_are_rejected() {
        let e = env();
        std::fs::create_dir_all(&e.mods.root).unwrap();
        let bad = e.mods.root.join("bad.ttf");
        std::fs::write(&bad, b"<html>").unwrap();
        assert!(prepare_font(&e.mods, &bad).is_err());
        assert!(prepare_sound(&e.mods, &bad).is_err());
        assert!(prepare_cursor(&e.mods, &Source::File { name: "x".into() }, Some(&bad), 100, None).is_err());
        assert!(prepare_wheel(&e.mods, &Source::Preset { id: "nope".into() }, None, WheelLook::default()).is_err());
        let ogg = e.mods.root.join("s.ogg");
        std::fs::write(&ogg, b"OggS....").unwrap();
        assert!(prepare_sound(&e.mods, &ogg).is_ok());
        let woff = e.mods.root.join("web.ttf");
        std::fs::write(&woff, b"wOF2....").unwrap();
        assert!(prepare_font(&e.mods, &woff).unwrap_err().contains("web font"));
    }

    #[test]
    fn missing_roblox_files_are_reported_not_created() {
        let e = env();
        for rel in DEATH_SOUNDS {
            std::fs::remove_file(e.install.join(rel_path(rel))).unwrap();
        }
        let s = e.mods.root.join("s.ogg");
        std::fs::create_dir_all(&e.mods.root).unwrap();
        std::fs::write(&s, b"OggSmine").unwrap();
        prepare_sound(&e.mods, &s).unwrap();
        let cfg = ModsConfig { death_sound: Some(FileMod { name: "s.ogg".into() }), ..Default::default() };
        let r = apply(&cfg, &e.mods, &e.install, &mut ModLedger::default());
        assert_eq!(r.written, 0);
        assert_eq!(r.missing.len(), 2);
        assert!(!e.install.join(rel_path(DEATH_SOUNDS[0])).exists());
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
        std::fs::write(e.install.join(rel_path(DEATH_SOUNDS[0])), b"OggSsomeone-else").unwrap();
        let off = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(off.restored, 1, "oof.ogg restored");
        assert_eq!(off.skipped_foreign.len(), 1, "ouch.ogg kept");
        assert_eq!(read(&e, DEATH_SOUNDS[0]), b"OggSsomeone-else");
    }

    #[test]
    fn custom_files_mirror_roblox_layout_and_restore_all_works() {
        let e = env();
        let custom = e.mods.custom_dir().join(rel_path(r"content\textures\new.png"));
        std::fs::create_dir_all(custom.parent().unwrap()).unwrap();
        std::fs::write(&custom, b"mine").unwrap();
        let cfg = ModsConfig { custom_files: true, ..Default::default() };
        let mut ledger = ModLedger::default();
        let r = apply(&cfg, &e.mods, &e.install, &mut ledger);
        assert_eq!(r.written, 1);
        assert_eq!(read(&e, r"content\textures\new.png"), b"mine");
        let all = restore_all(&mut ledger);
        assert_eq!(all.restored, 1);
        assert!(!e.install.join(rel_path(r"content\textures\new.png")).exists());
    }

    #[test]
    fn similarly_named_installs_are_kept_apart() {
        let e = env();
        let sibling = e.install.with_file_name("version-test2");
        let rel = CURSOR_FILES[0];
        std::fs::create_dir_all(sibling.join(rel_path(rel)).parent().unwrap()).unwrap();
        std::fs::write(sibling.join(rel_path(rel)), png(64, 64, [1, 1, 1, 255])).unwrap();
        prepare_cursor(&e.mods, &Source::Preset { id: "dot".into() }, None, 100, None).unwrap();
        let cfg = ModsConfig {
            cursor: Some(CursorMod { source: Source::Preset { id: "dot".into() }, scale: 100, color: None }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        apply(&cfg, &e.mods, &sibling, &mut ledger);
        // Turning mods off for the first install must not touch the sibling.
        let r = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(r.restored, 0);
        assert_ne!(
            std::fs::read(sibling.join(rel_path(rel))).unwrap(),
            png(64, 64, [1, 1, 1, 255]),
            "sibling keeps its mod"
        );
    }

    /// Regression: restoring through the lowercased ledger key renamed
    /// "ArrowCursor.png" to "arrowcursor.png" on a real Roblox install.
    #[test]
    fn restore_keeps_original_file_name_case() {
        let e = env();
        prepare_cursor(&e.mods, &Source::Preset { id: "dot".into() }, None, 100, None).unwrap();
        let cfg = ModsConfig {
            cursor: Some(CursorMod { source: Source::Preset { id: "dot".into() }, scale: 100, color: None }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        apply(&cfg, &e.mods, &e.install, &mut ledger);
        let r = apply(&ModsConfig::default(), &e.mods, &e.install, &mut ledger);
        assert_eq!(r.restored, 2);
        let dir = e.install.join(rel_path(r"content\textures\Cursors\KeyboardMouse"));
        let mut names: Vec<String> =
            std::fs::read_dir(dir).unwrap().map(|x| x.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, vec!["ArrowCursor.png", "ArrowFarCursor.png"]);
    }

    /// macOS: Roblox replaces its files in place when it updates.
    #[test]
    fn files_replaced_by_an_in_place_update_are_taken_over_again() {
        let e = env();
        prepare_cursor(&e.mods, &Source::Preset { id: "dot".into() }, None, 100, None).unwrap();
        let cfg = ModsConfig {
            cursor: Some(CursorMod { source: Source::Preset { id: "dot".into() }, scale: 100, color: None }),
            ..Default::default()
        };
        let mut ledger = ModLedger::default();
        assert_eq!(apply_build(&cfg, &e.mods, &e.install, "mac-0.1", &mut ledger).written, 2);

        let updated = png(64, 64, [9, 9, 9, 255]);
        for rel in CURSOR_FILES {
            std::fs::write(e.install.join(rel_path(rel)), &updated).unwrap();
        }
        let same = apply_build(&cfg, &e.mods, &e.install, "mac-0.1", &mut ledger);
        assert_eq!(same.skipped_foreign.len(), 2, "same build: something else changed them");

        let next = apply_build(&cfg, &e.mods, &e.install, "mac-0.2", &mut ledger);
        assert_eq!(next.written, 2, "a new build's files are taken over");
        let off = apply_build(&ModsConfig::default(), &e.mods, &e.install, "mac-0.2", &mut ledger);
        assert_eq!(off.restored, 2);
        assert_eq!(read(&e, CURSOR_FILES[0]), updated, "the new build's own cursor is put back");
    }

    #[test]
    fn relative_paths_use_this_systems_separator() {
        let p = rel_path(r"content\fonts\CustomFont.ttf");
        assert_eq!(p, Path::new("content").join("fonts").join("CustomFont.ttf"));
        assert_eq!(rel_path("content/sounds/oof.ogg"), Path::new("content").join("sounds").join("oof.ogg"));
    }

    #[test]
    fn hex_colours_parse() {
        assert_eq!(parse_hex("#A596FF"), Some([0xa5, 0x96, 0xff]));
        assert_eq!(parse_hex("a596ff"), Some([0xa5, 0x96, 0xff]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
    }
}
