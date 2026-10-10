//! Profiles: named bundles of Fast Flags, a frame-rate limit and process options.
//!
//! Built-in profiles only use allowlisted flags whose behaviour the catalog
//! marks as established. Their descriptions say what they change, never what
//! they gain: Vesper doesn't promise frame rates.

use crate::flags::catalog::{Behavior, Catalog};
use crate::flags::model::{json_to_flag_string, FlagSet};
use crate::flags::validate::{has_errors, normalize, validate, Issue};
use crate::paths::write_atomic;
use crate::roblox::gbs::FpsCap;
use crate::roblox::launch::Priority;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

pub const EXPORT_FORMAT: &str = "vesper-profile";
pub const MAX_PROFILES: usize = 50;
pub const MAX_NAME: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub enum BuiltinId {
    Competitive,
    Balanced,
    Quality,
    Potato,
    Custom,
}

impl BuiltinId {
    pub const ALL: [BuiltinId; 5] =
        [BuiltinId::Competitive, BuiltinId::Balanced, BuiltinId::Quality, BuiltinId::Potato, BuiltinId::Custom];

    pub fn id(self) -> &'static str {
        match self {
            BuiltinId::Competitive => "competitive",
            BuiltinId::Balanced => "balanced",
            BuiltinId::Quality => "quality",
            BuiltinId::Potato => "potato",
            BuiltinId::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub builtin: Option<BuiltinId>,
    pub flags: FlagSet,
    /// `None` leaves Roblox's own frame-rate setting alone.
    pub fps_cap: Option<FpsCap>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub affinity: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn flags(pairs: &[(&str, &str)]) -> FlagSet {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

pub fn builtin(id: BuiltinId) -> Profile {
    let now = Utc::now();
    let (name, description, f, fps_cap, priority) = match id {
        BuiltinId::Competitive => (
            "Competitive",
            "Uncapped frame rate, above-normal process priority, no multisampling and no terrain grass. \
             Alt+Enter switches to exclusive fullscreen.",
            flags(&[
                ("FFlagHandleAltEnterFullscreenManually", "False"),
                ("FIntDebugForceMSAASamples", "1"),
                ("FIntFRMMinGrassDistance", "0"),
                ("FIntFRMMaxGrassDistance", "0"),
            ]),
            Some(FpsCap::Unlimited),
            Priority::AboveNormal,
        ),
        BuiltinId::Balanced => (
            "Balanced",
            "Frame rate capped to your display's refresh rate. Everything else stays at Roblox's defaults, \
             apart from allowing exclusive fullscreen with Alt+Enter.",
            flags(&[("FFlagHandleAltEnterFullscreenManually", "False")]),
            Some(FpsCap::MatchDisplay),
            Priority::Normal,
        ),
        BuiltinId::Quality => (
            "Quality",
            "Full-resolution rendering on scaled displays, 4× multisampling and the highest texture quality, \
             capped to your display's refresh rate.",
            flags(&[
                ("DFFlagDisableDPIScale", "True"),
                ("FIntDebugForceMSAASamples", "4"),
                ("DFFlagTextureQualityOverrideEnabled", "True"),
                ("DFIntTextureQualityOverride", "3"),
            ]),
            Some(FpsCap::MatchDisplay),
            Priority::Normal,
        ),
        BuiltinId::Potato => (
            "Potato",
            "For older PCs: lowest graphics quality level, lowest textures, no multisampling, no grass and a \
             plain gray sky, capped at 60 FPS.",
            flags(&[
                ("DFIntDebugFRMQualityLevelOverride", "1"),
                ("DFFlagTextureQualityOverrideEnabled", "True"),
                ("DFIntTextureQualityOverride", "0"),
                ("FIntDebugForceMSAASamples", "1"),
                ("FIntFRMMinGrassDistance", "0"),
                ("FIntFRMMaxGrassDistance", "0"),
                ("FFlagDebugSkyGray", "True"),
            ]),
            Some(FpsCap::Fixed(60)),
            Priority::Normal,
        ),
        BuiltinId::Custom => (
            "Custom",
            "Starts empty: Roblox's defaults with nothing changed. Add any flags and options you like.",
            FlagSet::new(),
            None,
            Priority::Normal,
        ),
    };
    Profile {
        id: id.id().into(),
        name: name.into(),
        description: description.into(),
        builtin: Some(id),
        flags: f,
        fps_cap,
        priority,
        affinity: None,
        created_at: now,
        updated_at: now,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStore {
    pub active: String,
    pub default: String,
    pub profiles: Vec<Profile>,
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self {
            active: BuiltinId::Balanced.id().into(),
            default: BuiltinId::Balanced.id().into(),
            profiles: BuiltinId::ALL.iter().map(|b| builtin(*b)).collect(),
        }
    }
}

/// Changes to apply to a profile. `None` fields are left as they are.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub flags: Option<FlagSet>,
    /// `Some(None)` clears the frame-rate limit.
    #[serde(default, with = "double_option")]
    pub fps_cap: Option<Option<FpsCap>>,
    pub priority: Option<Priority>,
    #[serde(default, with = "double_option")]
    pub affinity: Option<Option<u64>>,
}

mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExport {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub flags: FlagSet,
    pub fps_cap: Option<FpsCap>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub affinity: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub profile: Profile,
    /// Validation notes for the imported flags (for example, flags Roblox ignores).
    pub issues: Vec<Issue>,
}

#[derive(Debug, PartialEq)]
pub enum ProfileError {
    NotFound,
    BuiltinNotDeletable,
    LastProfile,
    TooMany,
    InvalidName(String),
    InvalidFlags(Vec<String>),
    InvalidImport(String),
    Invalid(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::NotFound => write!(f, "That profile no longer exists."),
            ProfileError::BuiltinNotDeletable => write!(f, "Built-in profiles can be reset, but not deleted."),
            ProfileError::LastProfile => write!(f, "At least one profile has to remain."),
            ProfileError::TooMany => write!(f, "You can have up to {MAX_PROFILES} profiles."),
            ProfileError::InvalidName(s) | ProfileError::InvalidImport(s) | ProfileError::Invalid(s) => {
                write!(f, "{s}")
            }
            ProfileError::InvalidFlags(e) => write!(f, "{}", e.join(" ")),
        }
    }
}

fn clean_name(name: &str) -> Result<String, ProfileError> {
    let n = name.trim();
    if n.is_empty() {
        return Err(ProfileError::InvalidName("Give the profile a name.".into()));
    }
    if n.chars().count() > MAX_NAME {
        return Err(ProfileError::InvalidName(format!("Profile names are at most {MAX_NAME} characters.")));
    }
    if n.chars().any(char::is_control) {
        return Err(ProfileError::InvalidName("Profile names can't contain control characters.".into()));
    }
    Ok(n.to_owned())
}

fn check_flags(f: &FlagSet) -> Result<FlagSet, ProfileError> {
    normalize(f).map_err(|issues| {
        ProfileError::InvalidFlags(issues.into_iter().map(|i| format!("{}: {}", i.flag, i.message)).collect())
    })
}

fn check_options(fps: Option<FpsCap>, affinity: Option<u64>) -> Result<(), ProfileError> {
    if let Some(FpsCap::Fixed(n)) = fps {
        crate::roblox::gbs::validate_cap(n).map_err(ProfileError::Invalid)?;
    }
    if let Some(mask) = affinity {
        crate::roblox::launch::validate_affinity(mask).map_err(ProfileError::Invalid)?;
    }
    Ok(())
}

impl ProfileStore {
    pub fn load(path: &Path) -> Self {
        let mut store: ProfileStore = std::fs::read(path)
            .ok()
            .and_then(|b| match serde_json::from_slice(&b) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::warn!("profiles.json was unreadable ({e}); restoring built-in profiles");
                    let _ = std::fs::rename(
                        path,
                        path.with_extension(format!("unreadable-{}.json", Utc::now().format("%Y%m%d-%H%M%S"))),
                    );
                    None
                }
            })
            .unwrap_or_default();
        store.repair();
        store
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self).map_err(io::Error::other)?)
    }

    /// Restores missing built-ins and fixes dangling active/default ids.
    pub fn repair(&mut self) {
        for b in BuiltinId::ALL {
            if !self.profiles.iter().any(|p| p.builtin == Some(b)) {
                self.profiles.push(builtin(b));
            }
        }
        if self.get(&self.default).is_none() {
            self.default = BuiltinId::Balanced.id().into();
        }
        if self.get(&self.active).is_none() {
            self.active = self.default.clone();
        }
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Profile, ProfileError> {
        self.profiles.iter_mut().find(|p| p.id == id).ok_or(ProfileError::NotFound)
    }

    pub fn active(&self) -> &Profile {
        self.get(&self.active).or_else(|| self.profiles.first()).expect("repair keeps at least one profile")
    }

    pub fn set_active(&mut self, id: &str) -> Result<(), ProfileError> {
        self.get(id).ok_or(ProfileError::NotFound)?;
        self.active = id.to_owned();
        Ok(())
    }

    pub fn set_default(&mut self, id: &str) -> Result<(), ProfileError> {
        self.get(id).ok_or(ProfileError::NotFound)?;
        self.default = id.to_owned();
        Ok(())
    }

    fn unique_name(&self, base: &str) -> String {
        if !self.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(base)) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base} ({n})"))
            .find(|candidate| !self.profiles.iter().any(|p| p.name.eq_ignore_ascii_case(candidate)))
            .expect("an unused name exists")
    }

    fn insert_custom(&mut self, mut p: Profile) -> Result<Profile, ProfileError> {
        if self.profiles.len() >= MAX_PROFILES {
            return Err(ProfileError::TooMany);
        }
        p.id = format!("p-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        p.builtin = None;
        p.name = self.unique_name(&p.name);
        let now = Utc::now();
        p.created_at = now;
        p.updated_at = now;
        self.profiles.push(p.clone());
        Ok(p)
    }

    /// Creates a new profile, copying `from` if given.
    pub fn create(&mut self, name: &str, from: Option<&str>) -> Result<Profile, ProfileError> {
        let name = clean_name(name)?;
        let base = match from {
            Some(id) => self.get(id).ok_or(ProfileError::NotFound)?.clone(),
            None => builtin(BuiltinId::Custom),
        };
        let description = if from.is_some() { base.description.clone() } else { String::new() };
        self.insert_custom(Profile { name, description, ..base })
    }

    pub fn duplicate(&mut self, id: &str) -> Result<Profile, ProfileError> {
        let src = self.get(id).ok_or(ProfileError::NotFound)?.clone();
        let name = format!("{} copy", src.name);
        let name: String = name.chars().take(MAX_NAME).collect();
        self.insert_custom(Profile { name, ..src })
    }

    pub fn update(&mut self, id: &str, patch: ProfilePatch) -> Result<Profile, ProfileError> {
        let new_name = patch.name.as_deref().map(clean_name).transpose()?;
        let new_flags = patch.flags.as_ref().map(check_flags).transpose()?;
        let current = self.get(id).ok_or(ProfileError::NotFound)?;
        check_options(patch.fps_cap.unwrap_or(current.fps_cap), patch.affinity.unwrap_or(current.affinity))?;
        if let Some(n) = &new_name {
            if self.profiles.iter().any(|p| p.id != id && p.name.eq_ignore_ascii_case(n)) {
                return Err(ProfileError::InvalidName("Another profile already has that name.".into()));
            }
        }
        let p = self.get_mut(id)?;
        if let Some(n) = new_name {
            p.name = n;
        }
        if let Some(d) = patch.description {
            p.description = d.chars().take(400).collect();
        }
        if let Some(f) = new_flags {
            p.flags = f;
        }
        if let Some(c) = patch.fps_cap {
            p.fps_cap = c;
        }
        if let Some(pr) = patch.priority {
            p.priority = pr;
        }
        if let Some(a) = patch.affinity {
            p.affinity = a;
        }
        p.updated_at = Utc::now();
        Ok(p.clone())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), ProfileError> {
        let p = self.get(id).ok_or(ProfileError::NotFound)?;
        if p.builtin.is_some() {
            return Err(ProfileError::BuiltinNotDeletable);
        }
        if self.profiles.len() <= 1 {
            return Err(ProfileError::LastProfile);
        }
        self.profiles.retain(|p| p.id != id);
        if self.default == id {
            self.default = BuiltinId::Balanced.id().into();
        }
        if self.active == id {
            self.active = self.default.clone();
        }
        Ok(())
    }

    /// Restores a built-in profile to its shipped settings.
    pub fn reset(&mut self, id: &str) -> Result<Profile, ProfileError> {
        let b = self.get(id).ok_or(ProfileError::NotFound)?.builtin.ok_or_else(|| {
            ProfileError::Invalid("Only built-in profiles can be reset. Delete or edit this one instead.".into())
        })?;
        let fresh = builtin(b);
        let p = self.get_mut(id)?;
        let created = p.created_at;
        *p = Profile { created_at: created, ..fresh };
        Ok(p.clone())
    }

    pub fn export(&self, id: &str) -> Result<String, ProfileError> {
        let p = self.get(id).ok_or(ProfileError::NotFound)?;
        let e = ProfileExport {
            format: EXPORT_FORMAT.into(),
            version: 1,
            name: p.name.clone(),
            description: p.description.clone(),
            flags: p.flags.clone(),
            fps_cap: p.fps_cap,
            priority: p.priority,
            affinity: p.affinity,
        };
        serde_json::to_string_pretty(&e).map_err(|e| ProfileError::Invalid(e.to_string()))
    }

    /// Imports a Vesper profile export, or a plain ClientAppSettings.json-style object.
    pub fn import(&mut self, json: &str, fallback_name: &str, catalog: &Catalog) -> Result<ImportResult, ProfileError> {
        if json.len() > 512 * 1024 {
            return Err(ProfileError::InvalidImport("That file is too large to be a profile.".into()));
        }
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| ProfileError::InvalidImport(format!("The file isn't valid JSON ({e}).")))?;
        let obj = value.as_object().ok_or_else(|| ProfileError::InvalidImport("Expected a JSON object.".into()))?;

        let mut profile = builtin(BuiltinId::Custom);
        if obj.get("format").and_then(|f| f.as_str()) == Some(EXPORT_FORMAT) {
            let e: ProfileExport = serde_json::from_value(value.clone())
                .map_err(|e| ProfileError::InvalidImport(format!("This Vesper profile is damaged ({e}).")))?;
            if e.version > 1 {
                return Err(ProfileError::InvalidImport(
                    "This profile was made by a newer version of Vesper. Update Vesper to import it.".into(),
                ));
            }
            profile.name = clean_name(&e.name)?;
            profile.description = e.description.chars().take(400).collect();
            profile.flags = e.flags;
            profile.fps_cap = e.fps_cap;
            profile.priority = e.priority;
            profile.affinity = e.affinity;
        } else {
            let mut f = BTreeMap::new();
            for (k, v) in obj {
                let s = json_to_flag_string(v).ok_or_else(|| {
                    ProfileError::InvalidImport(format!("{k} has a value that isn't text, a number or true/false."))
                })?;
                f.insert(k.clone(), s);
            }
            profile.name = clean_name(fallback_name).unwrap_or_else(|_| "Imported".into());
            profile.description = "Imported from a Fast Flags file.".into();
            profile.flags = f;
        }
        check_options(profile.fps_cap, profile.affinity)?;
        profile.flags = check_flags(&profile.flags)?;
        let issues = validate(&profile.flags, catalog);
        if has_errors(&issues) {
            log::info!("Imported profile has conflicts the user must resolve before launching");
        }
        let profile = self.insert_custom(profile)?;
        Ok(ImportResult { profile, issues })
    }
}

/// True if every flag in every built-in profile is allowlisted with established behaviour.
pub fn builtins_use_established_flags(catalog: &Catalog) -> Result<(), String> {
    for b in BuiltinId::ALL {
        for name in builtin(b).flags.keys() {
            match catalog.get(name) {
                Some(info) if info.behavior == Behavior::Established => {}
                Some(_) => return Err(format!("{b:?} uses {name}, whose behaviour is uncertain")),
                None => return Err(format!("{b:?} uses {name}, which isn't allowlisted")),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_honest_and_valid() {
        let c = Catalog::embedded();
        builtins_use_established_flags(c).unwrap();
        for b in BuiltinId::ALL {
            let p = builtin(b);
            let issues = validate(&p.flags, c);
            assert!(issues.is_empty(), "{b:?}: {issues:?}");
            let text = p.description.to_lowercase();
            for banned in ["boost", "faster", "more fps", "increase", "gain", "improve"] {
                assert!(!text.contains(banned), "{b:?} description makes a performance claim: {banned}");
            }
        }
    }

    #[test]
    fn default_store_has_builtins_and_balanced_active() {
        let s = ProfileStore::default();
        assert_eq!(s.profiles.len(), 5);
        assert_eq!(s.active().name, "Balanced");
    }

    #[test]
    fn create_duplicate_rename_delete() {
        let mut s = ProfileStore::default();
        let p = s.duplicate("competitive").unwrap();
        assert_eq!(p.name, "Competitive copy");
        assert_eq!(p.flags, builtin(BuiltinId::Competitive).flags);
        assert!(p.builtin.is_none());

        let again = s.duplicate("competitive").unwrap();
        assert_eq!(again.name, "Competitive copy (2)");

        let renamed = s.update(&p.id, ProfilePatch { name: Some("  Ranked  ".into()), ..Default::default() }).unwrap();
        assert_eq!(renamed.name, "Ranked");
        assert!(
            s.update(&again.id, ProfilePatch { name: Some("ranked".into()), ..Default::default() }).is_err(),
            "names are unique, case-insensitively"
        );

        s.set_active(&p.id).unwrap();
        s.set_default(&p.id).unwrap();
        s.delete(&p.id).unwrap();
        assert_eq!(s.active, "balanced", "active falls back to default");
        assert_eq!(s.default, "balanced");
        assert!(s.get(&p.id).is_none());
    }

    #[test]
    fn builtins_cannot_be_deleted_but_can_be_reset() {
        let mut s = ProfileStore::default();
        assert_eq!(s.delete("potato"), Err(ProfileError::BuiltinNotDeletable));
        s.update("potato", ProfilePatch { name: Some("Toaster".into()), fps_cap: Some(None), ..Default::default() })
            .unwrap();
        let reset = s.reset("potato").unwrap();
        assert_eq!(reset.name, "Potato");
        assert_eq!(reset.fps_cap, Some(FpsCap::Fixed(60)));
        let custom = s.create("Mine", None).unwrap();
        assert!(s.reset(&custom.id).is_err());
    }

    #[test]
    fn invalid_updates_are_rejected_without_partial_changes() {
        let mut s = ProfileStore::default();
        let before = s.get("balanced").unwrap().clone();
        let mut bad = FlagSet::new();
        bad.insert("FIntDebugForceMSAASamples".into(), "lots".into());
        let r = s
            .update("balanced", ProfilePatch { name: Some("New name".into()), flags: Some(bad), ..Default::default() });
        assert!(matches!(r, Err(ProfileError::InvalidFlags(_))));
        assert_eq!(s.get("balanced").unwrap(), &before);
        assert!(s
            .update("balanced", ProfilePatch { fps_cap: Some(Some(FpsCap::Fixed(2))), ..Default::default() })
            .is_err());
        assert!(s.update("balanced", ProfilePatch { affinity: Some(Some(0)), ..Default::default() }).is_err());
    }

    #[test]
    fn flag_values_are_normalised_on_update() {
        let mut s = ProfileStore::default();
        let mut f = FlagSet::new();
        f.insert("FFlagDebugSkyGray".into(), "true".into());
        let p = s.update("custom", ProfilePatch { flags: Some(f), ..Default::default() }).unwrap();
        assert_eq!(p.flags["FFlagDebugSkyGray"], "True");
    }

    #[test]
    fn export_import_round_trip() {
        let mut s = ProfileStore::default();
        let json = s.export("quality").unwrap();
        let r = s.import(&json, "ignored", Catalog::embedded()).unwrap();
        assert_eq!(r.profile.name, "Quality (2)");
        assert_eq!(r.profile.flags, builtin(BuiltinId::Quality).flags);
        assert_eq!(r.profile.fps_cap, Some(FpsCap::MatchDisplay));
        assert!(r.issues.is_empty());
    }

    #[test]
    fn plain_flag_files_import_with_warnings() {
        let mut s = ProfileStore::default();
        let r = s
            .import(
                r#"{"DFIntTaskSchedulerTargetFps": 240, "FFlagDebugSkyGray": true}"#,
                "old-bloxstrap-flags",
                Catalog::embedded(),
            )
            .unwrap();
        assert_eq!(r.profile.name, "old-bloxstrap-flags");
        assert_eq!(r.profile.flags["FFlagDebugSkyGray"], "True");
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].code, "not_allowlisted");
    }

    #[test]
    fn bad_imports_are_rejected_cleanly() {
        let mut s = ProfileStore::default();
        let c = Catalog::embedded();
        assert!(s.import("[1,2]", "x", c).is_err());
        assert!(s.import("not json", "x", c).is_err());
        assert!(s.import(r#"{"FFlagX": [1]}"#, "x", c).is_err());
        assert!(s.import(r#"{"Not a flag": "1"}"#, "x", c).is_err());
        assert!(s
            .import(
                r#"{"format":"vesper-profile","version":9,"name":"x","description":"","flags":{},"fpsCap":null}"#,
                "x",
                c
            )
            .is_err());
        assert_eq!(s.profiles.len(), 5, "nothing was added");
    }

    #[test]
    fn repair_restores_missing_builtins_and_ids() {
        let mut s = ProfileStore { active: "gone".into(), default: "gone".into(), profiles: vec![] };
        s.repair();
        assert_eq!(s.profiles.len(), 5);
        assert_eq!(s.active, "balanced");
    }

    #[test]
    fn store_round_trips_and_corrupt_file_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let mut s = ProfileStore::default();
        s.create("Mine", Some("potato")).unwrap();
        s.save(&path).unwrap();
        assert_eq!(ProfileStore::load(&path), s);

        std::fs::write(&path, "garbage").unwrap();
        let recovered = ProfileStore::load(&path);
        assert_eq!(recovered.profiles.len(), 5);
        let kept: Vec<_> = std::fs::read_dir(dir.path()).unwrap().filter_map(Result::ok).collect();
        assert!(kept.iter().any(|e| e.file_name().to_string_lossy().contains("unreadable")));
    }
}
