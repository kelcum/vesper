//! Writing flags into a Roblox installation.
//!
//! Roblox reads `<version folder>\ClientSettings\ClientAppSettings.json`.
//! Vesper records a SHA-256 of every file it writes in an ownership ledger:
//! a file is only removed or silently replaced when its content still matches
//! what Vesper wrote. Anything else (hand-edited, written by another tool)
//! is backed up before being replaced, and never deleted.

use super::model::FlagSet;
use crate::paths::write_atomic;
use crate::redact::display_path;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

pub const SETTINGS_DIR: &str = "ClientSettings";
pub const SETTINGS_FILE: &str = "ClientAppSettings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedFile {
    pub sha256: String,
    pub written_at: DateTime<Utc>,
    pub flag_count: usize,
}

/// Ledger of Roblox files written by Vesper.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ownership {
    pub files: BTreeMap<String, OwnedFile>,
}

/// Ledger key for a file. Windows paths are case-insensitive, so they are
/// compared lowercased; elsewhere the exact path is kept, since it is also
/// how the file is found again.
fn key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.into_owned()
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

impl Ownership {
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                log::warn!("Ownership ledger is unreadable ({e}); starting a new one");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        write_atomic(path, &json)
    }

    pub fn owns(&self, file: &Path, content: &[u8]) -> bool {
        self.files.get(&key(file)).is_some_and(|o| o.sha256 == sha256(content))
    }

    fn record(&mut self, file: &Path, content: &[u8], flag_count: usize) {
        self.files.insert(key(file), OwnedFile { sha256: sha256(content), written_at: Utc::now(), flag_count });
    }

    fn forget(&mut self, file: &Path) {
        self.files.remove(&key(file));
    }

    /// Paths of every file Vesper believes it wrote.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.files.keys().map(PathBuf::from).collect()
    }
}

pub fn settings_file(version_dir: &Path) -> PathBuf {
    version_dir.join(SETTINGS_DIR).join(SETTINGS_FILE)
}

/// Serialises flags the way Roblox expects: a flat JSON object of strings.
pub fn render(flags: &FlagSet) -> String {
    let mut s = serde_json::to_string_pretty(flags).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyAction {
    /// Flags were written.
    Written,
    /// The file already contained exactly these flags.
    Unchanged,
    /// Vesper's file was removed (no flags to apply).
    Removed,
    /// There was no file to remove.
    NothingToRemove,
    /// A file not written by Vesper exists and was left alone.
    LeftForeignFile,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub file: String,
    pub action: ApplyAction,
    pub flag_count: usize,
    /// Where a foreign file was copied before it was replaced.
    pub backup: Option<String>,
}

fn backup_foreign(target: &Path, content: &[u8], version_dir: &Path, backup_dir: &Path) -> io::Result<PathBuf> {
    let version = version_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "unknown".into());
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let dest = backup_dir.join(format!("ClientAppSettings-{version}-{stamp}.json"));
    std::fs::create_dir_all(backup_dir)?;
    std::fs::write(&dest, content)?;
    log::info!("Backed up existing {} to {}", display_path(target), display_path(&dest));
    Ok(dest)
}

/// Writes `flags` into `version_dir`. An empty set removes Vesper's file.
pub fn apply(
    version_dir: &Path,
    flags: &FlagSet,
    ledger: &mut Ownership,
    backup_dir: &Path,
) -> io::Result<ApplyOutcome> {
    let target = settings_file(version_dir);
    let existing = std::fs::read(&target).ok();

    if flags.is_empty() {
        return remove_inner(&target, existing, ledger, false, version_dir, backup_dir);
    }

    let content = render(flags);
    let mut backup = None;

    if let Some(bytes) = &existing {
        if bytes.as_slice() == content.as_bytes() {
            ledger.record(&target, bytes, flags.len());
            return Ok(ApplyOutcome {
                file: display_path(&target),
                action: ApplyAction::Unchanged,
                flag_count: flags.len(),
                backup: None,
            });
        }
        if !ledger.owns(&target, bytes) {
            backup = Some(display_path(&backup_foreign(&target, bytes, version_dir, backup_dir)?));
        }
    }

    write_atomic(&target, content.as_bytes())?;
    ledger.record(&target, content.as_bytes(), flags.len());
    log::info!("Wrote {} flags to {}", flags.len(), display_path(&target));
    Ok(ApplyOutcome { file: display_path(&target), action: ApplyAction::Written, flag_count: flags.len(), backup })
}

/// Removes Vesper's flags file from `version_dir`.
/// With `force`, a foreign file is backed up and removed too.
pub fn remove(version_dir: &Path, ledger: &mut Ownership, force: bool, backup_dir: &Path) -> io::Result<ApplyOutcome> {
    let target = settings_file(version_dir);
    let existing = std::fs::read(&target).ok();
    remove_inner(&target, existing, ledger, force, version_dir, backup_dir)
}

fn remove_inner(
    target: &Path,
    existing: Option<Vec<u8>>,
    ledger: &mut Ownership,
    force: bool,
    version_dir: &Path,
    backup_dir: &Path,
) -> io::Result<ApplyOutcome> {
    let Some(bytes) = existing else {
        ledger.forget(target);
        return Ok(ApplyOutcome {
            file: display_path(target),
            action: ApplyAction::NothingToRemove,
            flag_count: 0,
            backup: None,
        });
    };

    let ours = ledger.owns(target, &bytes);
    if !ours && !force {
        return Ok(ApplyOutcome {
            file: display_path(target),
            action: ApplyAction::LeftForeignFile,
            flag_count: 0,
            backup: None,
        });
    }
    let backup =
        if ours { None } else { Some(display_path(&backup_foreign(target, &bytes, version_dir, backup_dir)?)) };
    std::fs::remove_file(target)?;
    ledger.forget(target);
    log::info!("Removed {}", display_path(target));
    Ok(ApplyOutcome { file: display_path(target), action: ApplyAction::Removed, flag_count: 0, backup })
}

/// Reads the flags currently present in an installation, if any.
pub fn read_current(version_dir: &Path) -> Option<Result<FlagSet, String>> {
    let bytes = std::fs::read(settings_file(version_dir)).ok()?;
    let parsed: Result<BTreeMap<String, serde_json::Value>, _> = serde_json::from_slice(&bytes);
    Some(match parsed {
        Ok(map) => {
            Ok(map.into_iter().filter_map(|(k, v)| super::model::json_to_flag_string(&v).map(|s| (k, s))).collect())
        }
        Err(e) => Err(format!("ClientAppSettings.json isn't valid JSON: {e}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(pairs: &[(&str, &str)]) -> FlagSet {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        version: PathBuf,
        backups: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let version = dir.path().join("Versions").join("version-abc");
        std::fs::create_dir_all(&version).unwrap();
        let backups = dir.path().join("backups");
        Fixture { version, backups, _dir: dir }
    }

    #[test]
    fn writes_sorted_string_values() {
        let f = fixture();
        let mut ledger = Ownership::default();
        let out = apply(&f.version, &flags(&[("FIntB", "2"), ("FFlagA", "True")]), &mut ledger, &f.backups).unwrap();
        assert_eq!(out.action, ApplyAction::Written);
        let text = std::fs::read_to_string(settings_file(&f.version)).unwrap();
        assert_eq!(text, "{\n  \"FFlagA\": \"True\",\n  \"FIntB\": \"2\"\n}\n");
        assert!(ledger.owns(&settings_file(&f.version), text.as_bytes()));
    }

    #[test]
    fn reapplying_same_flags_is_unchanged() {
        let f = fixture();
        let mut ledger = Ownership::default();
        let set = flags(&[("FFlagA", "True")]);
        apply(&f.version, &set, &mut ledger, &f.backups).unwrap();
        let out = apply(&f.version, &set, &mut ledger, &f.backups).unwrap();
        assert_eq!(out.action, ApplyAction::Unchanged);
    }

    #[test]
    fn foreign_file_is_backed_up_before_overwrite() {
        let f = fixture();
        let target = settings_file(&f.version);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "{\"FFlagMine\": \"True\"}").unwrap();

        let mut ledger = Ownership::default();
        let out = apply(&f.version, &flags(&[("FFlagA", "True")]), &mut ledger, &f.backups).unwrap();
        assert_eq!(out.action, ApplyAction::Written);
        assert!(out.backup.is_some());
        let saved: Vec<_> = std::fs::read_dir(&f.backups).unwrap().collect();
        assert_eq!(saved.len(), 1);
        let content = std::fs::read_to_string(saved[0].as_ref().unwrap().path()).unwrap();
        assert_eq!(content, "{\"FFlagMine\": \"True\"}");
    }

    #[test]
    fn our_own_file_is_replaced_without_backup() {
        let f = fixture();
        let mut ledger = Ownership::default();
        apply(&f.version, &flags(&[("FFlagA", "True")]), &mut ledger, &f.backups).unwrap();
        let out = apply(&f.version, &flags(&[("FFlagA", "False")]), &mut ledger, &f.backups).unwrap();
        assert_eq!(out.action, ApplyAction::Written);
        assert!(out.backup.is_none());
        assert!(!f.backups.exists());
    }

    #[test]
    fn empty_set_removes_only_our_file() {
        let f = fixture();
        let mut ledger = Ownership::default();
        apply(&f.version, &flags(&[("FFlagA", "True")]), &mut ledger, &f.backups).unwrap();
        let out = apply(&f.version, &FlagSet::new(), &mut ledger, &f.backups).unwrap();
        assert_eq!(out.action, ApplyAction::Removed);
        assert!(!settings_file(&f.version).exists());
        assert!(ledger.files.is_empty());
    }

    #[test]
    fn edited_file_is_treated_as_foreign_and_kept() {
        let f = fixture();
        let mut ledger = Ownership::default();
        apply(&f.version, &flags(&[("FFlagA", "True")]), &mut ledger, &f.backups).unwrap();
        std::fs::write(settings_file(&f.version), "{\"FFlagEdited\": \"True\"}").unwrap();

        let out = remove(&f.version, &mut ledger, false, &f.backups).unwrap();
        assert_eq!(out.action, ApplyAction::LeftForeignFile);
        assert!(settings_file(&f.version).exists());

        let forced = remove(&f.version, &mut ledger, true, &f.backups).unwrap();
        assert_eq!(forced.action, ApplyAction::Removed);
        assert!(forced.backup.is_some(), "forced removal keeps a copy");
        assert!(!settings_file(&f.version).exists());
    }

    #[test]
    fn ledger_round_trips() {
        let f = fixture();
        let mut ledger = Ownership::default();
        apply(&f.version, &flags(&[("FFlagA", "True")]), &mut ledger, &f.backups).unwrap();
        let path = f.backups.parent().unwrap().join("applied.json");
        ledger.save(&path).unwrap();
        let loaded = Ownership::load(&path);
        let content = std::fs::read(settings_file(&f.version)).unwrap();
        assert!(loaded.owns(&settings_file(&f.version), &content));
    }

    #[test]
    fn read_current_parses_mixed_json_types() {
        let f = fixture();
        let target = settings_file(&f.version);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, r#"{"FFlagA": true, "FIntB": 3, "FStringC": "x"}"#).unwrap();
        let current = read_current(&f.version).unwrap().unwrap();
        assert_eq!(current["FFlagA"], "True");
        assert_eq!(current["FIntB"], "3");
        assert_eq!(current["FStringC"], "x");
    }

    #[test]
    fn read_current_reports_invalid_json() {
        let f = fixture();
        let target = settings_file(&f.version);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "{not json").unwrap();
        assert!(read_current(&f.version).unwrap().is_err());
    }
}
