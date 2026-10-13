//! Roblox's frame-rate cap.
//!
//! Since Roblox stopped honouring local frame-rate Fast Flags, the supported
//! way to change the cap is Roblox's own `FramerateCap` setting in
//! `%LOCALAPPDATA%\Roblox\GlobalBasicSettings_<n>.xml` — the same value the
//! in-game "Maximum Frame Rate" option writes.
//!
//! Rules:
//! * Edit only the one value; keep the BOM, line endings and every other byte.
//! * Never write 0 (it breaks Roblox's renderer). -1 means "Roblox default".
//! * Only write while Roblox is closed — Roblox rewrites this file on exit.

use crate::paths::write_atomic;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub const DEFAULT_CAP: i32 = -1;
/// What "Unlimited" writes: a cap no display reaches.
pub const UNLIMITED_CAP: i32 = 9999;
pub const MIN_CUSTOM: i32 = 15;
pub const MAX_CUSTOM: i32 = 1000;

static CAP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(<int name="FramerateCap">)\s*(-?\d+)\s*(</int>)"#).expect("valid regex"));
static FILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^GlobalBasicSettings_(\d+)\.xml$").expect("valid regex"));

/// A frame-rate limit as chosen in a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode", content = "value")]
pub enum FpsCap {
    /// Roblox's own default.
    RobloxDefault,
    /// The refresh rate of the primary display, detected at launch.
    MatchDisplay,
    Fixed(i32),
    Unlimited,
}

impl FpsCap {
    /// The value to write, given the detected display refresh rate.
    pub fn resolve(self, display_hz: Option<u32>) -> Result<i32, String> {
        match self {
            FpsCap::RobloxDefault => Ok(DEFAULT_CAP),
            FpsCap::Unlimited => Ok(UNLIMITED_CAP),
            FpsCap::MatchDisplay => display_hz
                .map(|hz| (hz as i32).clamp(MIN_CUSTOM, MAX_CUSTOM))
                .ok_or_else(|| "Couldn't read the display refresh rate.".into()),
            FpsCap::Fixed(n) => validate_cap(n),
        }
    }
}

pub fn validate_cap(n: i32) -> Result<i32, String> {
    if (MIN_CUSTOM..=MAX_CUSTOM).contains(&n) {
        Ok(n)
    } else {
        Err(format!("Choose a limit between {MIN_CUSTOM} and {MAX_CUSTOM} FPS."))
    }
}

/// Finds the newest GlobalBasicSettings file (Roblox bumps the number occasionally).
pub fn settings_path(roblox_data: &Path) -> Option<PathBuf> {
    std::fs::read_dir(roblox_data)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let n: u32 = FILE_RE.captures(&name)?[1].parse().ok()?;
            Some((n, e.path()))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, p)| p)
}

pub fn read_cap(xml: &str) -> Option<i32> {
    CAP_RE.captures(xml)?[2].parse().ok()
}

/// Returns `xml` with the cap set to `value`, changing nothing else.
pub fn with_cap(xml: &str, value: i32) -> Result<String, String> {
    if value == 0 {
        return Err("A frame-rate cap of 0 breaks Roblox's renderer.".into());
    }
    if CAP_RE.is_match(xml) {
        return Ok(CAP_RE.replace(xml, format!("${{1}}{value}${{3}}")).into_owned());
    }
    // Insert into the UserGameSettings properties, matching the file's style.
    let item =
        xml.find(r#"<Item class="UserGameSettings""#).ok_or("This settings file has no UserGameSettings section.")?;
    let props_rel = xml[item..].find("<Properties>").ok_or("This settings file has no Properties section.")?;
    let props_at = item + props_rel;
    let insert_at = props_at + "<Properties>".len();
    let newline = if xml.contains("\r\n") { "\r\n" } else { "\n" };
    let indent_before = |pos: usize| -> String {
        let start = xml[..pos].rfind('\n').map_or(0, |i| i + 1);
        xml[start..pos].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
    };
    let item_indent = indent_before(item);
    let props_indent = indent_before(props_at);
    let unit = props_indent.strip_prefix(item_indent.as_str()).filter(|u| !u.is_empty()).unwrap_or("  ").to_owned();
    let element = format!(r#"{newline}{props_indent}{unit}<int name="FramerateCap">{value}</int>"#);
    let mut out = String::with_capacity(xml.len() + element.len());
    out.push_str(&xml[..insert_at]);
    out.push_str(&element);
    out.push_str(&xml[insert_at..]);
    Ok(out)
}

#[derive(Debug)]
pub enum CapError {
    NoSettingsFile,
    /// The file is marked read-only, usually by another launcher's "lock settings" option.
    ReadOnly,
    Io(std::io::Error),
    Invalid(String),
}

impl std::fmt::Display for CapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapError::NoSettingsFile => write!(f, "Roblox hasn't created its settings file yet"),
            CapError::ReadOnly => write!(f, "Roblox's settings file is read-only (another tool may have locked it)"),
            CapError::Io(e) => write!(f, "{e}"),
            CapError::Invalid(s) => write!(f, "{s}"),
        }
    }
}

/// Reads the current cap from disk.
pub fn current_cap(roblox_data: &Path) -> Result<Option<i32>, CapError> {
    let path = settings_path(roblox_data).ok_or(CapError::NoSettingsFile)?;
    let text = read_text(&path).map_err(CapError::Io)?;
    Ok(read_cap(&text.body))
}

/// Writes the cap. Returns the previous value.
pub fn set_cap(roblox_data: &Path, value: i32) -> Result<Option<i32>, CapError> {
    let path = settings_path(roblox_data).ok_or(CapError::NoSettingsFile)?;
    let text = read_text(&path).map_err(CapError::Io)?;
    let previous = read_cap(&text.body);
    if previous == Some(value) {
        return Ok(previous);
    }
    let updated = with_cap(&text.body, value).map_err(CapError::Invalid)?;
    let mut bytes = Vec::with_capacity(updated.len() + 3);
    if text.bom {
        bytes.extend_from_slice(b"\xEF\xBB\xBF");
    }
    bytes.extend_from_slice(updated.as_bytes());

    // Fishstrap and others can mark the file read-only; respect that choice.
    if is_read_only(&path) {
        return Err(CapError::ReadOnly);
    }
    write_atomic(&path, &bytes).map_err(CapError::Io)?;
    log::info!("Set Roblox FramerateCap {previous:?} -> {value}");
    Ok(previous)
}

/// Whether the settings file is locked against changes. On macOS this also
/// covers Finder's "Locked" checkbox, which isn't a permission bit.
pub fn is_read_only(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        const UF_IMMUTABLE: u32 = 0x0000_0002;
        if meta.st_flags() & UF_IMMUTABLE != 0 {
            return true;
        }
    }
    meta.permissions().readonly()
}

struct Text {
    bom: bool,
    body: String,
}

fn read_text(path: &Path) -> std::io::Result<Text> {
    let bytes = std::fs::read(path)?;
    let (bom, body) = match bytes.strip_prefix(b"\xEF\xBB\xBF") {
        Some(rest) => (true, rest),
        None => (false, bytes.as_slice()),
    };
    let body = String::from_utf8(body.to_vec()).map_err(std::io::Error::other)?;
    Ok(Text { bom, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<roblox version=\"4\">\n  <External>null</External>\n  <Item class=\"UserGameSettings\" referent=\"RBX1\">\n    <Properties>\n      <string name=\"DefaultCameraID\">{DefaultDeviceGuid}</string>\n      <int name=\"FramerateCap\">9999</int>\n      <bool name=\"Fullscreen\">false</bool>\n    </Properties>\n  </Item>\n</roblox>\n";

    #[test]
    fn reads_and_replaces_only_the_value() {
        assert_eq!(read_cap(SAMPLE), Some(9999));
        let out = with_cap(SAMPLE, 144).unwrap();
        assert_eq!(read_cap(&out), Some(144));
        assert_eq!(out.replace(">144<", ">9999<"), SAMPLE, "nothing else changed");
    }

    #[test]
    fn inserts_missing_element_with_matching_indent() {
        let without = SAMPLE.replace("      <int name=\"FramerateCap\">9999</int>\n", "");
        let out = with_cap(&without, 60).unwrap();
        assert_eq!(read_cap(&out), Some(60));
        assert!(out.contains("    <Properties>\n      <int name=\"FramerateCap\">60</int>\n"));
    }

    #[test]
    fn zero_is_refused() {
        assert!(with_cap(SAMPLE, 0).is_err());
    }

    #[test]
    fn crlf_files_keep_crlf() {
        let crlf = SAMPLE.replace("      <int name=\"FramerateCap\">9999</int>\n", "").replace('\n', "\r\n");
        let out = with_cap(&crlf, 75).unwrap();
        assert!(!out.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn resolves_caps() {
        assert_eq!(FpsCap::RobloxDefault.resolve(None).unwrap(), -1);
        assert_eq!(FpsCap::Unlimited.resolve(None).unwrap(), 9999);
        assert_eq!(FpsCap::MatchDisplay.resolve(Some(165)).unwrap(), 165);
        assert!(FpsCap::MatchDisplay.resolve(None).is_err());
        assert_eq!(FpsCap::Fixed(240).resolve(None).unwrap(), 240);
        assert!(FpsCap::Fixed(5).resolve(None).is_err());
        assert!(FpsCap::Fixed(5000).resolve(None).is_err());
    }

    #[test]
    fn file_round_trip_preserves_bom_and_picks_highest_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice(SAMPLE.as_bytes());
        std::fs::write(dir.path().join("GlobalBasicSettings_13.xml"), &bytes).unwrap();
        std::fs::write(dir.path().join("GlobalBasicSettings_12.xml"), SAMPLE).unwrap();
        std::fs::write(dir.path().join("GlobalBasicSettings_13_Studio.xml"), SAMPLE).unwrap();

        assert_eq!(settings_path(dir.path()).unwrap().file_name().unwrap(), "GlobalBasicSettings_13.xml");
        let prev = set_cap(dir.path(), 120).unwrap();
        assert_eq!(prev, Some(9999));
        let after = std::fs::read(dir.path().join("GlobalBasicSettings_13.xml")).unwrap();
        assert!(after.starts_with(b"\xEF\xBB\xBF"));
        assert_eq!(current_cap(dir.path()).unwrap(), Some(120));
        let studio = std::fs::read_to_string(dir.path().join("GlobalBasicSettings_13_Studio.xml")).unwrap();
        assert_eq!(read_cap(&studio), Some(9999), "Studio settings untouched");
    }

    #[test]
    fn missing_file_and_read_only_file_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(set_cap(dir.path(), 60), Err(CapError::NoSettingsFile)));
        let p = dir.path().join("GlobalBasicSettings_13.xml");
        std::fs::write(&p, SAMPLE).unwrap();
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm.clone()).unwrap();
        assert!(is_read_only(&p));
        assert!(matches!(set_cap(dir.path(), 60), Err(CapError::ReadOnly)));
        assert_eq!(set_cap(dir.path(), 9999).unwrap(), Some(9999), "an unchanged value needs no write");
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        std::fs::set_permissions(&p, perm).unwrap();
    }
}
