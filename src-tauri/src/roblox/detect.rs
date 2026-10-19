use super::version::file_version;
use crate::redact::display_path;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[cfg(not(target_os = "macos"))]
pub const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";
#[cfg(target_os = "macos")]
pub const PLAYER_EXE: &str = "RobloxPlayer";
pub const MAC_APP: &str = "Roblox.app";
const ENV_KEY: &str = r"Software\ROBLOX Corporation\Environments\roblox-player";
const PROTOCOL_KEY: &str = r"Software\Classes\roblox-player\shell\open\command";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

pub trait Registry: Send + Sync {
    fn string(&self, hive: Hive, path: &str, name: &str) -> Option<String>;
}

pub struct SystemRegistry;

#[cfg(windows)]
impl Registry for SystemRegistry {
    fn string(&self, hive: Hive, path: &str, name: &str) -> Option<String> {
        let root = match hive {
            Hive::CurrentUser => windows_registry::CURRENT_USER,
            Hive::LocalMachine => windows_registry::LOCAL_MACHINE,
        };
        root.open(path).ok()?.get_string(name).ok()
    }
}

#[cfg(not(windows))]
impl Registry for SystemRegistry {
    fn string(&self, _: Hive, _: &str, _: &str) -> Option<String> {
        None
    }
}

pub struct DetectEnv {
    pub local_appdata: PathBuf,
    pub program_files_x86: Option<PathBuf>,
    pub registry: Box<dyn Registry>,
    pub applications: Vec<PathBuf>,
}

impl DetectEnv {
    #[cfg(not(target_os = "macos"))]
    pub fn system() -> Option<Self> {
        Some(Self {
            local_appdata: std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?,
            program_files_x86: std::env::var_os("ProgramFiles(x86)").map(PathBuf::from),
            registry: Box::new(SystemRegistry),
            applications: Vec::new(),
        })
    }

    #[cfg(target_os = "macos")]
    pub fn system() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute())?;
        Some(Self {
            local_appdata: home.join("Library"),
            program_files_x86: None,
            registry: Box::new(SystemRegistry),
            applications: vec![PathBuf::from("/Applications"), home.join("Applications")],
        })
    }

    pub fn roblox_data(&self) -> PathBuf {
        self.local_appdata.join("Roblox")
    }

    pub fn roblox_logs(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.local_appdata.join("Logs").join("Roblox")
        } else {
            self.roblox_data().join("logs")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallSource {
    Official,
    AllUsers,
    Fishstrap,
    Bloxstrap,
    Custom,
}

impl InstallSource {
    pub fn managed_by(self) -> Option<&'static str> {
        match self {
            InstallSource::Fishstrap => Some("Fishstrap"),
            InstallSource::Bloxstrap => Some("Bloxstrap"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobloxInstall {
    pub id: String,
    pub source: InstallSource,
    pub version_hash: String,
    pub version: Option<String>,
    pub location: String,
    pub registered: bool,
    pub managed_by: Option<String>,
    pub modified: Option<DateTime<Utc>>,
    pub has_flags_file: bool,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl RobloxInstall {
    pub fn exe(&self) -> PathBuf {
        self.dir.join(PLAYER_EXE)
    }

    pub fn content_root(&self) -> PathBuf {
        content_root(&self.dir)
    }

    pub fn build_id(&self) -> String {
        build_id(&self.dir)
    }
}

pub fn content_root(install_dir: &Path) -> PathBuf {
    match install_dir.parent() {
        Some(contents) if cfg!(target_os = "macos") => contents.join("Resources"),
        _ => install_dir.to_path_buf(),
    }
}

pub fn build_id(install_dir: &Path) -> String {
    if cfg!(target_os = "macos") {
        let exe = install_dir.join(PLAYER_EXE);
        let id = file_version(&exe).map(|v| v.to_string()).or_else(|| {
            let secs = modified(&exe)?.timestamp();
            Some(format!("build-{secs}"))
        });
        return format!("mac-{}", id.unwrap_or_else(|| "unknown".into()));
    }
    install_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "unknown".into())
}

pub fn app_bundle(install_dir: &Path) -> Option<PathBuf> {
    install_dir
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")))
        .map(Path::to_path_buf)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolHandler {
    pub owner: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub installs: Vec<RobloxInstall>,
    pub preferred: Option<String>,
    pub protocol: Option<ProtocolHandler>,
    pub store_app_present: bool,
    pub override_problem: Option<String>,
}

impl Detection {
    pub fn preferred_install(&self) -> Option<&RobloxInstall> {
        let id = self.preferred.as_ref()?;
        self.installs.iter().find(|i| &i.id == id)
    }
}

fn modified(path: &Path) -> Option<DateTime<Utc>> {
    std::fs::metadata(path).ok()?.modified().ok().map(DateTime::<Utc>::from)
}

fn install_at(dir: &Path, source: InstallSource, registered: bool) -> Option<RobloxInstall> {
    let exe = dir.join(PLAYER_EXE);
    if !exe.is_file() {
        return None;
    }
    let (version_hash, id_part) = match app_bundle(dir) {
        Some(app) if cfg!(target_os = "macos") => (app.file_name()?.to_string_lossy().into_owned(), display_path(&app)),
        _ => {
            let name = dir.file_name()?.to_string_lossy().into_owned();
            (name.clone(), name)
        }
    };
    Some(RobloxInstall {
        id: format!("{source:?}:{id_part}").to_lowercase(),
        source,
        version: file_version(&exe).map(|v| v.to_string()),
        location: display_path(dir),
        registered,
        managed_by: source.managed_by().map(str::to_owned),
        modified: modified(&exe),
        has_flags_file: crate::flags::apply::settings_file(dir).is_file(),
        version_hash,
        dir: dir.to_path_buf(),
    })
}

fn scan_versions(root: &Path, source: InstallSource, registered_dir: Option<&Path>) -> Vec<RobloxInstall> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("version-"))
        .filter_map(|e| {
            let dir = e.path();
            let registered = registered_dir.is_some_and(|r| same_path(r, &dir));
            install_at(&dir, source, registered)
        })
        .collect()
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

fn classify_handler(command: &str) -> &'static str {
    let c = command.to_lowercase();
    if c.contains("vesper.exe") {
        "Vesper"
    } else if c.contains("fishstrap.exe") {
        "Fishstrap"
    } else if c.contains("bloxstrap.exe") {
        "Bloxstrap"
    } else if c.contains("robloxplayerbeta.exe")
        || c.contains("robloxplayerinstaller.exe")
        || c.contains("robloxplayerlauncher.exe")
    {
        "Roblox"
    } else {
        "Other"
    }
}

fn app_install(app: &Path, source: InstallSource) -> Option<RobloxInstall> {
    install_at(&app.join("Contents").join("MacOS"), source, false)
}

fn scan_apps(dir: &Path, source: InstallSource) -> Vec<RobloxInstall> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")))
        .filter_map(|app| app_install(&app, source))
        .collect()
}

pub fn resolve_custom(dir: &Path) -> Result<RobloxInstall, String> {
    if let Some(i) = install_at(dir, InstallSource::Custom, false) {
        return Ok(i);
    }
    let mut found = if cfg!(target_os = "macos") {
        app_install(dir, InstallSource::Custom)
            .map(|i| vec![i])
            .unwrap_or_else(|| scan_apps(dir, InstallSource::Custom))
    } else {
        scan_versions(dir, InstallSource::Custom, None)
    };
    found.sort_by_key(|i| std::cmp::Reverse(i.modified));
    found.into_iter().next().ok_or_else(|| {
        let wanted = if cfg!(target_os = "macos") { MAC_APP } else { PLAYER_EXE };
        format!("{} doesn't contain {wanted}.", display_path(dir))
    })
}

pub fn detect(env: &DetectEnv, override_dir: Option<&Path>) -> Detection {
    let reg = env.registry.as_ref();
    let registered_exe = reg.string(Hive::CurrentUser, ENV_KEY, "clientExe").map(PathBuf::from);
    let registered_dir = registered_exe.as_deref().and_then(Path::parent).map(Path::to_path_buf);

    let mut installs = Vec::new();

    let mut override_problem = None;
    if let Some(dir) = override_dir {
        match resolve_custom(dir) {
            Ok(i) => installs.push(i),
            Err(e) => override_problem = Some(e),
        }
    }

    let official_root = env.roblox_data().join("Versions");
    installs.extend(scan_versions(&official_root, InstallSource::Official, registered_dir.as_deref()));
    for apps in &env.applications {
        installs.extend(app_install(&apps.join(MAC_APP), InstallSource::Official));
    }

    if let Some(dir) = &registered_dir {
        if !installs.iter().any(|i| same_path(&i.dir, dir)) {
            if let Some(i) = install_at(dir, InstallSource::Official, true) {
                installs.push(i);
            }
        }
    }

    if let Some(pf) = &env.program_files_x86 {
        installs.extend(scan_versions(&pf.join("Roblox").join("Versions"), InstallSource::AllUsers, None));
    }
    installs.extend(scan_versions(
        &env.local_appdata.join("Fishstrap").join("Versions"),
        InstallSource::Fishstrap,
        None,
    ));
    installs.extend(scan_versions(
        &env.local_appdata.join("Bloxstrap").join("Versions"),
        InstallSource::Bloxstrap,
        None,
    ));

    let mut seen = Vec::<PathBuf>::new();
    installs.retain(|i| {
        if seen.iter().any(|s| same_path(s, &i.dir)) {
            false
        } else {
            seen.push(i.dir.clone());
            true
        }
    });

    let rank = |i: &RobloxInstall| match i.source {
        InstallSource::Custom => 0,
        InstallSource::Official if i.registered => 1,
        InstallSource::Official => 2,
        InstallSource::AllUsers => 3,
        InstallSource::Fishstrap | InstallSource::Bloxstrap => 4,
    };
    installs.sort_by(|a, b| rank(a).cmp(&rank(b)).then(b.modified.cmp(&a.modified)));
    let preferred = installs.first().map(|i| i.id.clone());

    let protocol = reg
        .string(Hive::CurrentUser, PROTOCOL_KEY, "")
        .or_else(|| reg.string(Hive::LocalMachine, PROTOCOL_KEY, ""))
        .map(|command| ProtocolHandler {
            owner: classify_handler(&command).to_owned(),
            command: crate::redact::redact(&command),
        });

    let store_app_present = std::fs::read_dir(env.local_appdata.join("Packages"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .any(|e| e.file_name().to_string_lossy().to_uppercase().starts_with("ROBLOXCORPORATION.ROBLOX_"))
        })
        .unwrap_or(false);

    Detection { installs, preferred, protocol, store_app_present, override_problem }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeRegistry(HashMap<(String, String), String>);

    impl FakeRegistry {
        fn with(mut self, path: &str, name: &str, value: &str) -> Self {
            self.0.insert((path.to_owned(), name.to_owned()), value.to_owned());
            self
        }
    }

    impl Registry for FakeRegistry {
        fn string(&self, hive: Hive, path: &str, name: &str) -> Option<String> {
            (hive == Hive::CurrentUser).then(|| self.0.get(&(path.to_owned(), name.to_owned())).cloned())?
        }
    }

    fn make_install(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PLAYER_EXE), b"not really an exe").unwrap();
        dir
    }

    fn env(local: &Path, reg: FakeRegistry) -> DetectEnv {
        DetectEnv {
            local_appdata: local.to_path_buf(),
            program_files_x86: None,
            registry: Box::new(reg),
            applications: Vec::new(),
        }
    }

    #[test]
    fn nothing_installed() {
        let dir = tempfile::tempdir().unwrap();
        let d = detect(&env(dir.path(), FakeRegistry::default()), None);
        assert!(d.installs.is_empty());
        assert!(d.preferred.is_none());
        assert!(d.protocol.is_none());
    }

    #[test]
    fn registered_install_is_preferred_over_newer_stale_folder() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("Roblox").join("Versions");
        let registered = make_install(&versions, "version-aaa");
        std::thread::sleep(std::time::Duration::from_millis(30));
        make_install(&versions, "version-bbb");
        std::fs::create_dir_all(versions.join("version-empty")).unwrap();

        let reg = FakeRegistry::default().with(ENV_KEY, "clientExe", &registered.join(PLAYER_EXE).to_string_lossy());
        let d = detect(&env(dir.path(), reg), None);
        assert_eq!(d.installs.len(), 2);
        let p = d.preferred_install().unwrap();
        assert_eq!(p.version_hash, "version-aaa");
        assert!(p.registered);
    }

    #[test]
    fn newest_official_wins_without_registry() {
        let dir = tempfile::tempdir().unwrap();
        let versions = dir.path().join("Roblox").join("Versions");
        make_install(&versions, "version-old");
        std::thread::sleep(std::time::Duration::from_millis(30));
        make_install(&versions, "version-new");
        let d = detect(&env(dir.path(), FakeRegistry::default()), None);
        assert_eq!(d.preferred_install().unwrap().version_hash, "version-new");
    }

    #[test]
    fn third_party_copies_are_found_but_ranked_last() {
        let dir = tempfile::tempdir().unwrap();
        make_install(&dir.path().join("Fishstrap").join("Versions"), "version-fish");
        std::thread::sleep(std::time::Duration::from_millis(30));
        let d = detect(&env(dir.path(), FakeRegistry::default()), None);
        assert_eq!(d.installs.len(), 1);
        assert_eq!(d.installs[0].managed_by.as_deref(), Some("Fishstrap"));

        make_install(&dir.path().join("Roblox").join("Versions"), "version-official");
        let d = detect(&env(dir.path(), FakeRegistry::default()), None);
        assert_eq!(d.preferred_install().unwrap().source, InstallSource::Official);
    }

    #[test]
    fn custom_folder_overrides_everything_and_bad_folder_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        make_install(&dir.path().join("Roblox").join("Versions"), "version-official");
        let custom_root = dir.path().join("Games").join("RobloxVersions");
        if cfg!(target_os = "macos") {
            make_install(&custom_root.join(MAC_APP).join("Contents"), "MacOS");
        } else {
            make_install(&custom_root, "version-custom");
        }

        let d = detect(&env(dir.path(), FakeRegistry::default()), Some(&custom_root));
        assert_eq!(d.preferred_install().unwrap().source, InstallSource::Custom);

        let d = detect(&env(dir.path(), FakeRegistry::default()), Some(&dir.path().join("nope")));
        assert!(d.override_problem.is_some());
        assert_eq!(d.preferred_install().unwrap().source, InstallSource::Official);
    }

    #[test]
    fn protocol_owner_is_classified() {
        let dir = tempfile::tempdir().unwrap();
        let reg = FakeRegistry::default().with(
            PROTOCOL_KEY,
            "",
            r#""C:\Users\someone\AppData\Local\Fishstrap\Fishstrap.exe" -player "%1""#,
        );
        let d = detect(&env(dir.path(), reg), None);
        let p = d.protocol.unwrap();
        assert_eq!(p.owner, "Fishstrap");
        assert!(!p.command.contains("someone"), "command is redacted");
        assert_eq!(classify_handler(r"C:\x\RobloxPlayerInstaller.exe %1"), "Roblox");
        assert_eq!(classify_handler(r"C:\x\Vesper.exe --protocol %1"), "Vesper");
    }

    #[test]
    fn roblox_app_in_an_applications_folder_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("Applications");
        let macos = apps.join(MAC_APP).join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::write(macos.join(PLAYER_EXE), b"not really a binary").unwrap();
        std::fs::create_dir_all(apps.join("Other.app").join("Contents").join("MacOS")).unwrap();

        let mut e = env(dir.path(), FakeRegistry::default());
        e.applications = vec![apps.clone(), dir.path().join("missing")];
        let d = detect(&e, None);
        assert_eq!(d.installs.len(), 1);
        let i = d.preferred_install().unwrap();
        assert_eq!(i.source, InstallSource::Official);
        assert_eq!(i.dir, macos);
        assert_eq!(app_bundle(&i.dir).as_deref(), Some(apps.join(MAC_APP).as_path()));
        if cfg!(target_os = "macos") {
            assert_eq!(i.content_root(), apps.join(MAC_APP).join("Contents").join("Resources"));
            assert!(i.build_id().starts_with("mac-"), "{}", i.build_id());
            assert_eq!(i.version_hash, MAC_APP);
        }

        if cfg!(target_os = "macos") {
            assert_eq!(resolve_custom(&apps.join(MAC_APP)).unwrap().dir, macos);
            assert_eq!(resolve_custom(&apps).unwrap().dir, macos);
            assert!(resolve_custom(&apps.join("Other.app")).is_err());
        }
    }

    #[test]
    fn store_edition_is_noticed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Packages").join("ROBLOXCORPORATION.ROBLOX_55nm5eh3cm0pr")).unwrap();
        assert!(detect(&env(dir.path(), FakeRegistry::default()), None).store_app_present);
    }

    #[test]
    fn existing_flags_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let v = make_install(&dir.path().join("Roblox").join("Versions"), "version-x");
        let f = crate::flags::apply::settings_file(&v);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "{}").unwrap();
        assert!(detect(&env(dir.path(), FakeRegistry::default()), None).installs[0].has_flags_file);
    }
}
