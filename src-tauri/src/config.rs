//! Vesper's settings.
//!
//! Stored as `settings.json` in the config folder. Missing fields take their
//! defaults, so older files keep working after updates. A file that can't be
//! parsed is moved aside (never deleted) and defaults are used, with a notice
//! shown to the user.

use crate::paths::write_atomic;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

pub const SCHEMA: u32 = 1;
pub const UI_SCALES: &[u16] = &[90, 100, 110, 125, 150];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CloseBehavior {
    /// Keep running in the system tray.
    #[default]
    Tray,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AfterLaunch {
    Stay,
    #[default]
    Minimize,
    Tray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Transparency {
    /// Windows Mica material behind translucent panels.
    #[default]
    Full,
    /// Solid window, lightly translucent panels.
    Reduced,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
    /// Follow Windows' "Animation effects" setting.
    #[default]
    System,
    Full,
    Reduced,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FlagMode {
    /// Write the active profile's flags each time Vesper launches Roblox.
    #[default]
    ApplyOnLaunch,
    /// Also re-apply them when Roblox installs an update while Vesper runs.
    KeepApplied,
    /// Never touch ClientAppSettings.json.
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    pub start_with_windows: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub close_behavior: CloseBehavior,
    pub launch_roblox_on_start: bool,
    pub after_launch: AfterLaunch,
    /// Shown once, the first time the window is closed to the tray.
    pub tray_hint_shown: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            start_minimized: true,
            minimize_to_tray: false,
            close_behavior: CloseBehavior::Tray,
            launch_roblox_on_start: false,
            after_launch: AfterLaunch::Minimize,
            tray_hint_shown: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    pub theme: Theme,
    pub transparency: Transparency,
    pub motion: Motion,
    pub density: Density,
    /// UI scale in percent (see [`UI_SCALES`]).
    pub scale: u16,
    /// File name of a custom window background in the config folder.
    pub background: Option<String>,
    /// How much the background is darkened behind panels, 0-90 %.
    pub background_dim: u8,
    /// Background blur in pixels, 0-40.
    pub background_blur: u8,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            transparency: Transparency::Full,
            motion: Motion::System,
            density: Density::Comfortable,
            scale: 100,
            background: None,
            background_dim: 55,
            background_blur: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Performance {
    /// Sample system and Roblox metrics while the Performance page or dashboard is visible.
    pub monitoring: bool,
    pub sample_ms: u32,
    /// Try to measure FPS from Direct3D present events (needs extra Windows permissions).
    pub fps_counter: bool,
}

impl Default for Performance {
    fn default() -> Self {
        Self { monitoring: true, sample_ms: 1000, fps_counter: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RobloxSettings {
    /// A folder chosen by the user, overriding automatic detection.
    pub install_dir: Option<PathBuf>,
    pub flag_mode: FlagMode,
    /// Apply the profile's frame-rate limit to Roblox's own setting.
    pub apply_fps_cap: bool,
    /// Read Roblox's log after launch to confirm which flags it accepted.
    pub verify_after_launch: bool,
    /// Compare the installed version with Roblox's latest release.
    pub check_roblox_updates: bool,
}

impl Default for RobloxSettings {
    fn default() -> Self {
        Self {
            install_dir: None,
            flag_mode: FlagMode::ApplyOnLaunch,
            apply_fps_cap: true,
            verify_after_launch: true,
            check_roblox_updates: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DiscordSettings {
    pub rich_presence: bool,
    /// Mention the active profile ("Competitive profile").
    pub show_profile: bool,
    /// Say whether Roblox is running.
    pub show_roblox_status: bool,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self { rich_presence: false, show_profile: true, show_roblox_status: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateSettings {
    pub auto_check: bool,
    pub channel: UpdateChannel,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { auto_check: true, channel: UpdateChannel::Stable }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Advanced {
    pub log_level: String,
}

impl Default for Advanced {
    fn default() -> Self {
        Self { log_level: "info".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastLaunch {
    pub at: DateTime<Utc>,
    pub roblox_version: Option<String>,
    pub profile: String,
    pub target: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub schema: u32,
    pub first_run_complete: bool,
    pub general: General,
    pub appearance: Appearance,
    pub performance: Performance,
    pub roblox: RobloxSettings,
    pub discord: DiscordSettings,
    pub updates: UpdateSettings,
    pub advanced: Advanced,
    pub last_launch: Option<LastLaunch>,
    /// Roblox's FramerateCap before Vesper first changed it, for restoring on reset/uninstall.
    pub original_fps_cap: Option<i32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            first_run_complete: false,
            general: General::default(),
            appearance: Appearance::default(),
            performance: Performance::default(),
            roblox: RobloxSettings::default(),
            discord: DiscordSettings::default(),
            updates: UpdateSettings::default(),
            advanced: Advanced::default(),
            last_launch: None,
            original_fps_cap: None,
        }
    }
}

#[derive(Debug)]
pub struct Loaded {
    pub settings: Settings,
    /// Set when the previous file was unreadable and was moved here.
    pub recovered_from: Option<PathBuf>,
}

impl Settings {
    /// Corrects out-of-range values instead of rejecting the whole file.
    pub fn sanitize(&mut self) {
        if !UI_SCALES.contains(&self.appearance.scale) {
            self.appearance.scale = 100;
        }
        self.performance.sample_ms = self.performance.sample_ms.clamp(500, 5000);
        self.appearance.background_dim = self.appearance.background_dim.min(90);
        self.appearance.background_blur = self.appearance.background_blur.min(40);
        if let Some(b) = &self.appearance.background {
            // Only a bare file name inside the config folder is allowed.
            if b.contains(['/', '\\']) || b.contains("..") || !b.starts_with("background.") {
                self.appearance.background = None;
            }
        }
        if !["error", "warn", "info", "debug", "trace"].contains(&self.advanced.log_level.as_str()) {
            self.advanced.log_level = "info".into();
        }
        if let Some(dir) = &self.roblox.install_dir {
            if !dir.is_absolute() {
                self.roblox.install_dir = None;
            }
        }
        self.schema = SCHEMA;
    }

    pub fn load(path: &Path) -> Loaded {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Loaded { settings: Settings::default(), recovered_from: None },
        };
        match serde_json::from_slice::<Settings>(&bytes) {
            Ok(mut s) => {
                s.sanitize();
                Loaded { settings: s, recovered_from: None }
            }
            Err(e) => {
                let aside =
                    path.with_file_name(format!("settings.unreadable-{}.json", Utc::now().format("%Y%m%d-%H%M%S")));
                let moved = std::fs::rename(path, &aside).is_ok();
                log::warn!("settings.json was unreadable ({e}); using defaults");
                Loaded { settings: Settings::default(), recovered_from: moved.then_some(aside) }
            }
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        write_atomic(path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let l = Settings::load(&dir.path().join("settings.json"));
        assert_eq!(l.settings, Settings::default());
        assert!(l.recovered_from.is_none());
    }

    #[test]
    fn round_trip_persists_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = Settings::default();
        s.discord.rich_presence = true;
        s.appearance.theme = Theme::Light;
        s.roblox.flag_mode = FlagMode::KeepApplied;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).settings, s);
    }

    #[test]
    fn partial_and_older_files_fill_in_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"discord": {"richPresence": true}, "unknownFutureField": 1}"#).unwrap();
        let s = Settings::load(&path).settings;
        assert!(s.discord.rich_presence);
        assert!(s.discord.show_profile, "missing nested field takes default");
        assert_eq!(s.general, General::default());
    }

    #[test]
    fn corrupt_file_is_moved_aside_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let l = Settings::load(&path);
        assert_eq!(l.settings, Settings::default());
        let aside = l.recovered_from.unwrap();
        assert_eq!(std::fs::read_to_string(aside).unwrap(), "{ this is not json");
        assert!(!path.exists());
    }

    #[test]
    fn out_of_range_values_are_corrected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"appearance": {"scale": 333}, "performance": {"sampleMs": 5}, "advanced": {"logLevel": "loud"}, "roblox": {"installDir": "relative\\path"}}"#,
        )
        .unwrap();
        let s = Settings::load(&path).settings;
        assert_eq!(s.appearance.scale, 100);
        assert_eq!(s.performance.sample_ms, 500);
        assert_eq!(s.advanced.log_level, "info");
        assert!(s.roblox.install_dir.is_none());
    }
}
