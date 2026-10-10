//! Undoing Vesper's changes to Roblox.
//!
//! Used by "Reset Fast Flags" in the app and by the uninstaller
//! (`Vesper.exe --cleanup`). Only files whose content still matches what
//! Vesper wrote are removed; anything else is left alone unless forced,
//! and forced removals keep a backup.

use crate::flags::apply::{self, ApplyAction, Ownership};
use crate::paths::AppPaths;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSummary {
    pub removed: usize,
    pub kept_foreign: usize,
    pub already_gone: usize,
}

/// Removes every flags file in the ledger.
pub fn remove_all_flags(ledger: &mut Ownership, force: bool, backups: &Path) -> CleanupSummary {
    let mut s = CleanupSummary::default();
    for file in ledger.paths() {
        // <version>\ClientSettings\ClientAppSettings.json
        let Some(version_dir) = file.parent().and_then(Path::parent) else { continue };
        match apply::remove(version_dir, ledger, force, backups) {
            Ok(o) => match o.action {
                ApplyAction::Removed => s.removed += 1,
                ApplyAction::LeftForeignFile => s.kept_foreign += 1,
                _ => s.already_gone += 1,
            },
            Err(e) => {
                log::warn!("Couldn't remove {}: {e}", crate::redact::display_path(&file));
                s.kept_foreign += 1;
            }
        }
    }
    s
}

/// Entry point for `Vesper.exe --cleanup`. Never shows UI; always exits 0 so
/// the uninstaller continues.
pub fn run_cli() -> i32 {
    let Ok(paths) = AppPaths::resolve() else { return 0 };
    crate::logging::init(&paths.logs, log::LevelFilter::Info);
    log::info!("Uninstall cleanup started");

    let mut ledger = Ownership::load(&paths.ownership_file());
    let summary = remove_all_flags(&mut ledger, false, &paths.roblox_backups());
    let _ = ledger.save(&paths.ownership_file());
    log::info!("Removed {} flags file(s), left {} not written by Vesper", summary.removed, summary.kept_foreign);

    let settings = crate::config::Settings::load(&paths.settings_file()).settings;
    if let (Some(original), Some(local)) = (settings.original_fps_cap, std::env::var_os("LOCALAPPDATA")) {
        if crate::roblox::process::roblox_pids().is_empty() {
            let roblox = Path::new(&local).join("Roblox");
            match crate::roblox::gbs::set_cap(&roblox, original) {
                Ok(_) => log::info!("Restored Roblox's frame-rate cap to {original}"),
                Err(e) => log::warn!("Couldn't restore the frame-rate cap: {e}"),
            }
        }
    }
    log::logger().flush();
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::model::FlagSet;

    #[test]
    fn removes_owned_files_and_keeps_edited_ones() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("b");
        let a = dir.path().join("version-a");
        let b = dir.path().join("version-b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut flags = FlagSet::new();
        flags.insert("FFlagDebugSkyGray".into(), "True".into());
        let mut ledger = Ownership::default();
        apply::apply(&a, &flags, &mut ledger, &backups).unwrap();
        apply::apply(&b, &flags, &mut ledger, &backups).unwrap();
        // Someone edits b's file afterwards.
        std::fs::write(apply::settings_file(&b), "{\"FFlagTheirs\":\"True\"}").unwrap();

        let s = remove_all_flags(&mut ledger, false, &backups);
        assert_eq!(s, CleanupSummary { removed: 1, kept_foreign: 1, already_gone: 0 });
        assert!(!apply::settings_file(&a).exists());
        assert!(apply::settings_file(&b).exists(), "edited file kept");

        let s = remove_all_flags(&mut ledger, false, &backups);
        assert_eq!(s.kept_foreign, 1, "still tracked, still kept");
        let s = remove_all_flags(&mut ledger, true, &backups);
        assert_eq!(s.removed, 1);
        assert!(ledger.files.is_empty());
    }
}
