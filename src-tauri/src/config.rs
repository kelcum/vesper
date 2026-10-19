use crate::paths::write_atomic;
pub use crate::servers::ServerPick;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

pub const SCHEMA: u32 = 1;
pub const UI_SCALES: &[u16] = &[90, 100, 110, 125, 150];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CloseBehavior {
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
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum WhenRobloxCloses {
    #[default]
    Show,
    Stay,
    Quit,
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
    #[default]
    Full,
    Reduced,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Motion {
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
    #[default]
    ApplyOnLaunch,
    KeepApplied,
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
    pub when_roblox_closes: WhenRobloxCloses,
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
            when_roblox_closes: WhenRobloxCloses::Show,
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
    pub scale: u16,
    pub background: Option<String>,
    pub background_dim: u8,
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
    pub monitoring: bool,
    pub sample_ms: u32,
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
    pub install_dir: Option<PathBuf>,
    pub flag_mode: FlagMode,
    pub apply_fps_cap: bool,
    pub verify_after_launch: bool,
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
    pub show_profile: bool,
    pub show_roblox_status: bool,
    #[serde(default)]
    pub show_game: bool,
    pub show_play_time: bool,
    pub join_button: bool,
    pub profile_button: bool,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            rich_presence: false,
            show_profile: true,
            show_roblox_status: true,
            show_game: true,
            show_play_time: true,
            join_button: false,
            profile_button: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerSettings {
    pub pick: ServerPick,
    pub region: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self { pick: ServerPick::Off, region: "us-east".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShaderSettings {
    pub enabled: bool,
    pub preset: Option<String>,
    pub params: crate::shade::ShaderParams,
}

impl Default for ShaderSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: Some("cinematic".into()),
            params: crate::shade::preset("cinematic").unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GamesSettings {
    pub history: bool,
    pub locate_servers: bool,
}

impl Default for GamesSettings {
    fn default() -> Self {
        Self { history: true, locate_servers: false }
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
    pub servers: ServerSettings,
    pub games: GamesSettings,
    pub shaders: ShaderSettings,
    pub updates: UpdateSettings,
    pub advanced: Advanced,
    pub last_launch: Option<LastLaunch>,
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
            servers: ServerSettings::default(),
            games: GamesSettings::default(),
            shaders: ShaderSettings::default(),
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
    pub recovered_from: Option<PathBuf>,
}

impl Settings {
    pub fn sanitize(&mut self) {
        if !UI_SCALES.contains(&self.appearance.scale) {
            self.appearance.scale = 100;
        }
        self.performance.sample_ms = self.performance.sample_ms.clamp(500, 5000);
        self.appearance.background_dim = self.appearance.background_dim.min(90);
        self.appearance.background_blur = self.appearance.background_blur.min(40);
        if let Some(b) = &self.appearance.background {
            if b.contains(['/', '\\']) || b.contains("..") || !b.starts_with("background.") {
                self.appearance.background = None;
            }
        }
        self.shaders.params = self.shaders.params.clamped();
        if self.shaders.preset.as_deref().is_some_and(|p| crate::shade::preset(p).is_none()) {
            self.shaders.preset = None;
        }
        let region = &self.servers.region;
        let valid_region = crate::servers::REGIONS.iter().any(|r| r.id == region)
            || (region.starts_with("city:") && region.len() <= 80 && region.matches(':').count() == 2);
        if !valid_region {
            self.servers.region = ServerSettings::default().region;
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
    fn settings_from_before_the_roblox_closes_option_bring_vesper_back() {
        let g: General = serde_json::from_str(r#"{"afterLaunch":"tray","closeBehavior":"tray"}"#).unwrap();
        assert_eq!(g.after_launch, AfterLaunch::Tray);
        assert_eq!(g.when_roblox_closes, WhenRobloxCloses::Show);
        let g: General = serde_json::from_str(r#"{"whenRobloxCloses":"quit"}"#).unwrap();
        assert_eq!(g.when_roblox_closes, WhenRobloxCloses::Quit);
        let g: General = serde_json::from_str(r#"{"afterLaunch":"hidden"}"#).unwrap();
        assert_eq!(g.after_launch, AfterLaunch::Hidden);
    }

    #[test]
    fn earlier_discord_settings_dont_start_sharing_the_game() {
        assert!(DiscordSettings::default().show_game, "new installs show it");
        let d: DiscordSettings = serde_json::from_str(r#"{"richPresence":true,"showProfile":true}"#).unwrap();
        assert!(d.rich_presence && !d.show_game, "0.2 settings had no game in the presence");
        assert!(d.show_play_time, "other new options keep their defaults");
    }

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
