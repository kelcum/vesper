//! Noticing new Roblox version folders.
//!
//! When Roblox updates itself it installs into a new `version-…` folder,
//! which has no ClientAppSettings.json. With "Keep flags applied" on, Vesper
//! watches the Versions folder (event-driven, no polling) and writes the
//! active profile's flags into a new folder as soon as it appears, then again
//! once the player executable lands, before Roblox starts it.

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct VersionsWatcher {
    _watcher: RecommendedWatcher,
    _thread: std::thread::JoinHandle<()>,
}

fn is_version_dir(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("version-"))
}

impl VersionsWatcher {
    /// Calls `on_new_version(dir)` once when the folder appears and once more
    /// when RobloxPlayerBeta.exe exists inside it.
    pub fn start(versions_dir: &Path, on_new_version: impl Fn(&Path) + Send + 'static) -> notify::Result<Self> {
        std::fs::create_dir_all(versions_dir).ok();
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if matches!(ev.kind, EventKind::Create(_)) {
                    for p in ev.paths.into_iter().filter(|p| is_version_dir(p)) {
                        let _ = tx.send(p);
                    }
                }
            }
        })?;
        watcher.watch(versions_dir, RecursiveMode::NonRecursive)?;

        let thread = std::thread::Builder::new()
            .name("vesper-versions".into())
            .spawn(move || {
                while let Ok(dir) = rx.recv() {
                    log::info!("New Roblox version folder: {}", crate::redact::display_path(&dir));
                    on_new_version(&dir);
                    // Wait (bounded) for the player to be extracted, then apply again.
                    let exe = dir.join(super::detect::PLAYER_EXE);
                    let deadline = Instant::now() + Duration::from_secs(180);
                    while !exe.is_file() && Instant::now() < deadline && dir.is_dir() {
                        std::thread::sleep(Duration::from_millis(250));
                    }
                    if exe.is_file() {
                        on_new_version(&dir);
                    }
                }
            })
            .map_err(|e| notify::Error::generic(&e.to_string()))?;
        Ok(Self { _watcher: watcher, _thread: thread })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn reports_new_version_folders_twice_and_ignores_others() {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let seen = calls.clone();
        let _w = VersionsWatcher::start(dir.path(), move |p| seen.lock().unwrap().push(p.to_path_buf())).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        std::fs::create_dir(dir.path().join("unrelated")).unwrap();
        let v = dir.path().join("version-new");
        std::fs::create_dir(&v).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while calls.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(calls.lock().unwrap().as_slice(), std::slice::from_ref(&v));

        std::fs::write(v.join("RobloxPlayerBeta.exe"), b"x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while calls.lock().unwrap().len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(calls.lock().unwrap().len(), 2, "applies again once the exe exists");
    }
}
