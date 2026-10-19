use crate::config::Settings;
use crate::paths::write_atomic;
use crate::profiles::ProfileStore;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "vesper-backup";
pub const VERSION: u32 = 1;
pub const KEEP_AUTOMATIC: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Manual,
    BeforeRestore,
    BeforeImport,
    BeforeReset,
}

impl Reason {
    fn slug(self) -> &'static str {
        match self {
            Reason::Manual => "manual",
            Reason::BeforeRestore => "before-restore",
            Reason::BeforeImport => "before-import",
            Reason::BeforeReset => "before-reset",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub reason: Reason,
    pub settings: Settings,
    pub profiles: ProfileStore,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub reason: Reason,
    pub app_version: String,
    pub profile_count: usize,
    pub size_bytes: u64,
}

impl Bundle {
    pub fn new(reason: Reason, settings: &Settings, profiles: &ProfileStore) -> Self {
        let mut settings = settings.clone();
        settings.last_launch = None;
        Self {
            format: FORMAT.into(),
            version: VERSION,
            created_at: Utc::now(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            reason,
            settings,
            profiles: profiles.clone(),
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| format!("The file isn't valid JSON ({e})."))?;
        if v.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
            return Err("This isn't a Vesper backup.".into());
        }
        let version = v.get("version").and_then(|n| n.as_u64()).unwrap_or(0);
        if version > VERSION as u64 {
            return Err("This backup was made by a newer version of Vesper. Update Vesper to restore it.".into());
        }
        let mut b: Bundle = serde_json::from_value(v).map_err(|e| format!("This backup is damaged ({e})."))?;
        b.settings.sanitize();
        b.profiles.repair();
        Ok(b)
    }

    fn info(&self, id: String, size: u64) -> BackupInfo {
        BackupInfo {
            id,
            created_at: self.created_at,
            reason: self.reason,
            app_version: self.app_version.clone(),
            profile_count: self.profiles.profiles.len(),
            size_bytes: size,
        }
    }
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn file_for(dir: &Path, id: &str) -> Option<PathBuf> {
    is_valid_id(id).then(|| dir.join(format!("{id}.json")))
}

pub fn create(dir: &Path, bundle: &Bundle) -> io::Result<BackupInfo> {
    std::fs::create_dir_all(dir)?;
    let base = format!("{}-{}", bundle.created_at.format("%Y%m%d-%H%M%S"), bundle.reason.slug());
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(format!("{id}.json")).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let json = serde_json::to_vec_pretty(bundle).map_err(io::Error::other)?;
    write_atomic(&dir.join(format!("{id}.json")), &json)?;
    if bundle.reason != Reason::Manual {
        prune(dir, KEEP_AUTOMATIC);
    }
    log::info!("Created backup {id}");
    Ok(bundle.info(id, json.len() as u64))
}

pub fn list(dir: &Path) -> Vec<BackupInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<BackupInfo> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            if path.extension()? != "json" {
                return None;
            }
            let id = path.file_stem()?.to_string_lossy().into_owned();
            let bytes = std::fs::read(&path).ok()?;
            let b = Bundle::parse(&bytes).ok()?;
            Some(b.info(id, bytes.len() as u64))
        })
        .collect();
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    out
}

pub fn load(dir: &Path, id: &str) -> Result<Bundle, String> {
    let path = file_for(dir, id).ok_or("That backup name isn't valid.")?;
    let bytes = std::fs::read(&path).map_err(|_| "That backup no longer exists.".to_string())?;
    Bundle::parse(&bytes)
}

pub fn delete(dir: &Path, id: &str) -> io::Result<()> {
    let path = file_for(dir, id).ok_or_else(|| io::Error::other("invalid backup id"))?;
    std::fs::remove_file(path)
}

pub fn export(dir: &Path, id: &str, dest: &Path) -> Result<(), String> {
    let bundle = load(dir, id)?;
    let json = serde_json::to_vec_pretty(&bundle).map_err(|e| e.to_string())?;
    std::fs::write(dest, json).map_err(|e| e.to_string())
}

pub fn prune(dir: &Path, keep: usize) {
    let automatic: Vec<BackupInfo> = list(dir).into_iter().filter(|b| b.reason != Reason::Manual).collect();
    for old in automatic.iter().skip(keep) {
        let _ = delete(dir, &old.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(reason: Reason) -> Bundle {
        Bundle::new(reason, &Settings::default(), &ProfileStore::default())
    }

    #[test]
    fn create_list_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings::default();
        settings.discord.rich_presence = true;
        let mut profiles = ProfileStore::default();
        profiles.create("Mine", Some("competitive")).unwrap();
        let b = Bundle::new(Reason::Manual, &settings, &profiles);
        let info = create(dir.path(), &b).unwrap();

        let listed = list(dir.path());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, info.id);
        assert_eq!(listed[0].profile_count, 6);

        let loaded = load(dir.path(), &info.id).unwrap();
        assert!(loaded.settings.discord.rich_presence);
        assert_eq!(loaded.profiles, profiles);
    }

    #[test]
    fn same_second_backups_get_distinct_ids() {
        let dir = tempfile::tempdir().unwrap();
        let b = bundle(Reason::Manual);
        let a = create(dir.path(), &b).unwrap();
        let c = create(dir.path(), &b).unwrap();
        assert_ne!(a.id, c.id);
        assert_eq!(list(dir.path()).len(), 2);
    }

    #[test]
    fn automatic_backups_are_pruned_but_manual_ones_kept() {
        let dir = tempfile::tempdir().unwrap();
        let manual = create(dir.path(), &bundle(Reason::Manual)).unwrap();
        for _ in 0..(KEEP_AUTOMATIC + 5) {
            create(dir.path(), &bundle(Reason::BeforeRestore)).unwrap();
        }
        let all = list(dir.path());
        assert_eq!(all.iter().filter(|b| b.reason != Reason::Manual).count(), KEEP_AUTOMATIC);
        assert!(all.iter().any(|b| b.id == manual.id));
    }

    #[test]
    fn foreign_newer_and_damaged_files_are_rejected() {
        assert!(Bundle::parse(b"{\"some\": \"json\"}").unwrap_err().contains("isn't a Vesper backup"));
        assert!(Bundle::parse(b"nope").is_err());
        let newer = br#"{"format":"vesper-backup","version":99}"#;
        assert!(Bundle::parse(newer).unwrap_err().contains("newer version"));
        let damaged = br#"{"format":"vesper-backup","version":1,"settings":5}"#;
        assert!(Bundle::parse(damaged).unwrap_err().contains("damaged"));
    }

    #[test]
    fn unreadable_files_are_skipped_and_ids_are_sanitised() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("broken.json"), "x").unwrap();
        create(dir.path(), &bundle(Reason::Manual)).unwrap();
        assert_eq!(list(dir.path()).len(), 1);
        assert!(load(dir.path(), "../../settings").is_err());
        assert!(delete(dir.path(), "..\\x").is_err());
    }

    #[test]
    fn last_launch_is_not_backed_up() {
        let s = Settings {
            last_launch: Some(crate::config::LastLaunch {
                at: Utc::now(),
                roblox_version: None,
                profile: "x".into(),
                target: "app".into(),
                outcome: "started".into(),
            }),
            ..Settings::default()
        };
        assert!(Bundle::new(Reason::Manual, &s, &ProfileStore::default()).settings.last_launch.is_none());
    }

    #[test]
    fn export_writes_a_restorable_copy() {
        let dir = tempfile::tempdir().unwrap();
        let info = create(dir.path(), &bundle(Reason::Manual)).unwrap();
        let dest = dir.path().join("elsewhere.vesperbackup");
        export(dir.path(), &info.id, &dest).unwrap();
        assert!(Bundle::parse(&std::fs::read(dest).unwrap()).is_ok());
    }
}
