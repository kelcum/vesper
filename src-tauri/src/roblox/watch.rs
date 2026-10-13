//! Noticing Roblox updates.
//!
//! When Roblox updates itself on Windows it installs into a new `version-…`
//! folder, which has no ClientAppSettings.json. With "Keep flags applied" on,
//! Vesper watches the Versions folder (event-driven, no polling) and writes the
//! active profile's flags into a new folder as soon as it appears, again once
//! the player executable lands, before Roblox starts it, and a last time when
//! Roblox's installer finishes unpacking the rest (textures, fonts).
//!
//! On macOS Roblox replaces Roblox.app in place, which removes the flags and
//! customisations. Vesper watches the app and applies them again once the
//! update has settled.

use super::detect::{DetectEnv, RobloxInstall};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Starts watching for Roblox updates. `on_new_version` receives the install
/// folder to bring up to date.
pub fn watch_updates(
    env: &DetectEnv,
    install: Option<&RobloxInstall>,
    on_new_version: impl Fn(&Path) + Send + 'static,
) -> notify::Result<VersionsWatcher> {
    if cfg!(target_os = "macos") {
        let install = install.ok_or_else(|| notify::Error::generic("Roblox isn't installed"))?;
        VersionsWatcher::start_app(&install.dir, on_new_version)
    } else {
        VersionsWatcher::start(&env.roblox_data().join("Versions"), on_new_version)
    }
}

pub struct VersionsWatcher {
    _watcher: RecommendedWatcher,
    _thread: std::thread::JoinHandle<()>,
}

fn is_version_dir(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("version-"))
}

fn installer_running() -> bool {
    super::process::snapshot()
        .is_ok_and(|all| super::process::named(&all, super::process::INSTALLER_PROCESS).next().is_some())
}

impl VersionsWatcher {
    /// Calls `on_new_version(dir)` once when the folder appears, once more
    /// when RobloxPlayerBeta.exe exists inside it, and again after Roblox's
    /// installer exits if it was still running then.
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
                        if installer_running() {
                            let deadline = Instant::now() + Duration::from_secs(300);
                            while installer_running() && Instant::now() < deadline {
                                std::thread::sleep(Duration::from_millis(500));
                            }
                            on_new_version(&dir);
                        }
                    }
                }
            })
            .map_err(|e| notify::Error::generic(&e.to_string()))?;
        Ok(Self { _watcher: watcher, _thread: thread })
    }

    /// Watches the app bundle that `install_dir` (its `Contents/MacOS`) belongs
    /// to and calls `on_new_version(install_dir)` once changes to it have
    /// settled and Roblox's installer has finished. Vesper's own writes cause
    /// one more call, which finds nothing left to do.
    pub fn start_app(install_dir: &Path, on_new_version: impl Fn(&Path) + Send + 'static) -> notify::Result<Self> {
        let app = super::detect::app_bundle(install_dir)
            .ok_or_else(|| notify::Error::generic("Roblox isn't inside an app bundle"))?;
        // Events arrive with symbolic links resolved (/var is /private/var).
        let app = std::fs::canonicalize(&app).unwrap_or(app);
        let parent = app.parent().ok_or_else(|| notify::Error::generic("Roblox.app has no parent folder"))?.to_owned();
        let dir = install_dir.to_path_buf();
        let (tx, rx) = mpsc::channel::<()>();
        let watched = app.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if !matches!(ev.kind, EventKind::Access(_)) && ev.paths.iter().any(|p| p.starts_with(&watched)) {
                    let _ = tx.send(());
                }
            }
        })?;
        // The parent, so replacing the whole app is noticed too.
        watcher.watch(&parent, RecursiveMode::Recursive)?;

        let thread = std::thread::Builder::new()
            .name("vesper-app-watch".into())
            .spawn(move || {
                let exe = dir.join(super::detect::PLAYER_EXE);
                while rx.recv().is_ok() {
                    // Let the update settle: no changes for a few seconds and the installer gone.
                    let deadline = Instant::now() + Duration::from_secs(300);
                    loop {
                        match rx.recv_timeout(Duration::from_secs(3)) {
                            Ok(()) if Instant::now() < deadline => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => return,
                            _ => {}
                        }
                        if installer_running() && Instant::now() < deadline {
                            continue;
                        }
                        break;
                    }
                    if exe.is_file() {
                        log::debug!("Roblox.app changed; checking flags and customisations");
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
        let tmp = tempfile::tempdir().unwrap();
        // macOS reports events under the real path (/private/var, not /var).
        let dir = tmp.path().canonicalize().unwrap();
        let calls = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let seen = calls.clone();
        let _w = VersionsWatcher::start(&dir, move |p| seen.lock().unwrap().push(p.to_path_buf())).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        std::fs::create_dir(dir.join("unrelated")).unwrap();
        let v = dir.join("version-new");
        std::fs::create_dir(&v).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while calls.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(calls.lock().unwrap().as_slice(), std::slice::from_ref(&v));

        std::fs::write(v.join(crate::roblox::detect::PLAYER_EXE), b"x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while calls.lock().unwrap().len() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(calls.lock().unwrap().len(), 2, "applies again once the exe exists");
    }

    #[test]
    fn changes_to_the_app_bundle_are_reported_once_settled() {
        let dir = tempfile::tempdir().unwrap();
        let macos = dir.path().join("Roblox.app").join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(dir.path().join("Other.app")).unwrap();
        let calls = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let seen = calls.clone();
        let _w = VersionsWatcher::start_app(&macos, move |p| seen.lock().unwrap().push(p.to_path_buf())).unwrap();
        std::thread::sleep(Duration::from_millis(300));

        std::fs::write(dir.path().join("Other.app").join("unrelated"), b"x").unwrap();
        std::fs::write(macos.join(crate::roblox::detect::PLAYER_EXE), b"x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while calls.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(calls.lock().unwrap().as_slice(), std::slice::from_ref(&macos));
    }
}
