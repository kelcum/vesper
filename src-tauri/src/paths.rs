use std::io;
use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "Vesper";
pub const LOCAL_DIR_NAME: &str = "gripe.xan.vesper";
pub const PORTABLE_MARKER: &str = "portable";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config: PathBuf,
    pub backups: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
    pub portable: bool,
}

impl AppPaths {
    pub fn resolve() -> io::Result<Self> {
        let exe_dir = std::env::current_exe()?
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| io::Error::other("executable has no parent directory"))?;

        if exe_dir.join(PORTABLE_MARKER).is_file() {
            return Ok(Self::portable(&exe_dir.join("data")));
        }

        #[cfg(target_os = "macos")]
        {
            Ok(Self::macos(&env_dir("HOME")?))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let roaming = env_dir("APPDATA")?;
            let local = env_dir("LOCALAPPDATA")?;
            Ok(Self::installed(&roaming, &local))
        }
    }

    pub fn macos(home: &Path) -> Self {
        let library = home.join("Library");
        let config = library.join("Application Support").join(APP_DIR_NAME);
        Self {
            backups: config.join("backups"),
            config,
            logs: library.join("Logs").join(APP_DIR_NAME),
            cache: library.join("Caches").join(LOCAL_DIR_NAME),
            portable: false,
        }
    }

    pub fn installed(roaming: &Path, local: &Path) -> Self {
        let config = roaming.join(APP_DIR_NAME);
        let local = local.join(LOCAL_DIR_NAME);
        Self {
            backups: config.join("backups"),
            config,
            logs: local.join("logs"),
            cache: local.join("cache"),
            portable: false,
        }
    }

    pub fn portable(root: &Path) -> Self {
        Self {
            config: root.join("config"),
            backups: root.join("backups"),
            logs: root.join("logs"),
            cache: root.join("cache"),
            portable: true,
        }
    }

    pub fn ensure(&self) -> io::Result<()> {
        for dir in [&self.config, &self.backups, &self.logs, &self.cache] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    pub fn profiles_file(&self) -> PathBuf {
        self.config.join("profiles.json")
    }

    pub fn history_file(&self) -> PathBuf {
        self.config.join("history.json")
    }

    pub fn datacenters_file(&self) -> PathBuf {
        self.cache.join("datacenters.json")
    }

    pub fn ownership_file(&self) -> PathBuf {
        self.config.join("applied.json")
    }

    pub fn verification_file(&self) -> PathBuf {
        self.cache.join("verification.json")
    }

    pub fn roblox_backups(&self) -> PathBuf {
        self.backups.join("roblox")
    }
}

fn env_dir(name: &str) -> io::Result<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{name} is not set")))
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().ok_or_else(|| io::Error::other("path has no parent directory"))?;
    std::fs::create_dir_all(dir)?;
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_layout_splits_roaming_and_local() {
        let (roaming, local) = (Path::new(r"C:\R"), Path::new(r"C:\L"));
        let p = AppPaths::installed(roaming, local);
        assert_eq!(p.config, roaming.join("Vesper"));
        assert_eq!(p.backups, roaming.join("Vesper").join("backups"));
        assert_eq!(p.logs, local.join("gripe.xan.vesper").join("logs"));
        assert!(!p.portable);
    }

    #[test]
    fn local_folder_matches_the_app_identifier() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], LOCAL_DIR_NAME);
    }

    #[test]
    fn portable_layout_stays_under_root() {
        let p = AppPaths::portable(Path::new(r"D:\Vesper\data"));
        for dir in [&p.config, &p.backups, &p.logs, &p.cache] {
            assert!(dir.starts_with(r"D:\Vesper\data"));
        }
        assert!(p.portable);
    }

    #[test]
    fn macos_layout_follows_the_library_folders() {
        let p = AppPaths::macos(Path::new("/Users/alex"));
        assert_eq!(p.config, Path::new("/Users/alex/Library/Application Support/Vesper"));
        assert_eq!(p.backups, Path::new("/Users/alex/Library/Application Support/Vesper/backups"));
        assert_eq!(p.logs, Path::new("/Users/alex/Library/Logs/Vesper"));
        assert_eq!(p.cache, Path::new("/Users/alex/Library/Caches/gripe.xan.vesper"));
        assert!(!p.portable);
    }

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("nested").join("settings.json");
        write_atomic(&target, b"first").unwrap();
        write_atomic(&target, b"second").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
        let leftovers: Vec<_> = std::fs::read_dir(target.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
