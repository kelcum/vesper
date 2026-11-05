use super::cdn;
use super::detect::{InstallSource, PLAYER_EXE};
use super::version::{file_version, RobloxVersion};
use crate::config::Settings;
use crate::error::{Problem, ProblemAction, Result};
use crate::state::{events, lock, AppState};
use chrono::{DateTime, Utc};
use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const HISTORY_REPO: &str = "Roblox-DeployHistory-Updates/version-hash-saver";
const CDN: &str = "https://setup.rbxcdn.com";
const INSTALLER_PACKAGE: &str = "RobloxPlayerInstaller.exe";
const MAX_LISTED: usize = 5;
const MAX_RESOLVED: usize = 8;
const LIST_TTL: Duration = Duration::from_secs(30 * 60);
const APP_SETTINGS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Settings>\n\t<ContentFolder>content</ContentFolder>\n\t<BaseUrl>http://www.roblox.com</BaseUrl>\n</Settings>\n";

static PLAYER_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"WindowsPlayer → (\d+\.\d+\.\d+\.\d+)").expect("valid regex"));

#[derive(Debug, Clone, PartialEq)]
pub struct Known {
    pub version: RobloxVersion,
    pub hash: String,
    pub released: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub version: String,
    pub hash: String,
    pub released: Option<DateTime<Utc>>,
    pub versions_back: u32,
    pub too_old: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildList {
    pub builds: Vec<Build>,
    pub base: Option<String>,
    pub partial: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum BuildStatus {
    #[default]
    Idle,
    #[serde(rename_all = "camelCase")]
    Working {
        version: String,
        hash: String,
        done: u64,
        total: u64,
    },
    Failed {
        problem: Problem,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    Refused,
    BlankApp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildIssue {
    pub version: String,
    pub kind: IssueKind,
}

#[derive(Default)]
pub struct BuildsState {
    status: Mutex<BuildStatus>,
    issue: Mutex<Option<BuildIssue>>,
    cancel: Arc<AtomicBool>,
    known: Mutex<Option<(Instant, Vec<Known>)>>,
}

impl BuildsState {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub name: String,
    pub md5: String,
    pub packed: u64,
}

#[derive(Debug)]
pub enum Stop {
    Cancelled,
    Failed(Problem),
}

impl From<Problem> for Stop {
    fn from(p: Problem) -> Self {
        Stop::Failed(p)
    }
}

enum FetchError {
    Cancelled,
    Network(String),
    Disk(io::Error),
}

pub fn valid_hash(s: &str) -> bool {
    s.strip_prefix("version-")
        .is_some_and(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

pub fn parse_manifest(text: &str) -> std::result::Result<Vec<Package>, String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("v0") {
        return Err("it isn't in the format Vesper knows".into());
    }
    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() || !rest.len().is_multiple_of(4) {
        return Err("its list of files is incomplete".into());
    }
    rest.chunks(4)
        .map(|c| {
            let safe_name = !c[0].is_empty()
                && c[0].bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
                && !c[0].contains("..");
            let valid_md5 = c[1].len() == 32 && c[1].bytes().all(|b| b.is_ascii_hexdigit());
            let packed = c[2].parse::<u64>().ok();
            if !safe_name || !valid_md5 || packed.is_none() || c[3].parse::<u64>().is_err() {
                return Err(format!("it has an entry Vesper can't read: {}", c[0]));
            }
            Ok(Package { name: c[0].to_owned(), md5: c[1].to_ascii_lowercase(), packed: packed.unwrap_or(0) })
        })
        .collect()
}

pub fn package_folder(name: &str) -> Option<&'static str> {
    Some(match name {
        "RobloxApp.zip" | "Libraries.zip" | "redist.zip" | "WebView2.zip" => "",
        "WebView2RuntimeInstaller.zip" => "WebView2RuntimeInstaller",
        "shaders.zip" => "shaders",
        "ssl.zip" => "ssl",
        "content-avatar.zip" => "content/avatar",
        "content-configs.zip" => "content/configs",
        "content-fonts.zip" => "content/fonts",
        "content-models.zip" => "content/models",
        "content-sky.zip" => "content/sky",
        "content-sounds.zip" => "content/sounds",
        "content-textures2.zip" => "content/textures",
        "content-textures3.zip" => "PlatformContent/pc/textures",
        "content-terrain.zip" => "PlatformContent/pc/terrain",
        "content-platform-fonts.zip" => "PlatformContent/pc/fonts",
        "content-platform-dictionaries.zip" => "PlatformContent/pc/shared_compression_dictionaries",
        "extracontent-luapackages.zip" => "ExtraContent/LuaPackages",
        "extracontent-translations.zip" => "ExtraContent/translations",
        "extracontent-models.zip" => "ExtraContent/models",
        "extracontent-textures.zip" => "ExtraContent/textures",
        "extracontent-places.zip" => "ExtraContent/places",
        _ => return None,
    })
}

pub fn safe_relative(name: &str) -> Option<PathBuf> {
    let normal = name.replace('\\', "/");
    if normal.starts_with("//") || normal.contains(':') || normal.contains('\0') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in normal.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            p => out.push(p),
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

pub fn parse_commits(json: &str) -> std::result::Result<Vec<(String, RobloxVersion, DateTime<Utc>)>, String> {
    #[derive(Deserialize)]
    struct Commit {
        sha: String,
        commit: Inner,
    }
    #[derive(Deserialize)]
    struct Inner {
        message: String,
        author: Author,
    }
    #[derive(Deserialize)]
    struct Author {
        date: DateTime<Utc>,
    }
    let commits: Vec<Commit> = serde_json::from_str(json).map_err(|e| format!("unexpected answer: {e}"))?;
    Ok(commits
        .into_iter()
        .filter(|c| !c.sha.is_empty() && c.sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .filter_map(|c| {
            let version = PLAYER_ENTRY.captures(&c.commit.message)?[1].parse().ok()?;
            Some((c.sha, version, c.commit.author.date))
        })
        .collect())
}

pub fn rows(known: &[Known], base: Option<RobloxVersion>, refused: Option<RobloxVersion>) -> Vec<Build> {
    let mut sorted: Vec<&Known> = known.iter().filter(|k| base.is_none_or(|b| k.version.1 < b.1)).collect();
    sorted.sort_by_key(|k| std::cmp::Reverse(k.version));
    sorted.dedup_by_key(|k| k.version.1);
    sorted
        .into_iter()
        .take(MAX_LISTED)
        .enumerate()
        .map(|(i, k)| Build {
            version: k.version.to_string(),
            hash: k.hash.clone(),
            released: k.released,
            versions_back: i as u32 + 1,
            too_old: refused.is_some_and(|r| k.version <= r),
        })
        .collect()
}

pub fn versions_root(state: &AppState) -> PathBuf {
    state.paths.cache.join("roblox-versions")
}

fn refused_file(state: &AppState) -> PathBuf {
    versions_root(state).join("refused.txt")
}

fn newest_refused(state: &AppState) -> Option<RobloxVersion> {
    std::fs::read_to_string(refused_file(state)).ok()?.trim().parse().ok()
}

fn pinned_version(state: &AppState) -> Option<String> {
    lock(&state.settings).roblox.pinned_build.as_ref()?;
    state.preferred_install()?.version
}

fn set_issue(app: &AppHandle, issue: Option<BuildIssue>) {
    let state = app.state::<AppState>();
    *lock(&state.builds.issue) = issue.clone();
    let _ = app.emit(events::BUILD_ISSUE, &issue);
}

pub fn note_refused(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(version) = pinned_version(&state) else { return };
    log::info!("Roblox refused {version} when joining (error 280)");
    if let Ok(parsed) = version.parse::<RobloxVersion>() {
        if newest_refused(&state).is_none_or(|r| parsed > r) {
            if let Err(e) = crate::paths::write_atomic(&refused_file(&state), version.as_bytes()) {
                log::warn!("Couldn't remember that Roblox refused {version}: {e}");
            }
        }
    }
    set_issue(app, Some(BuildIssue { version, kind: IssueKind::Refused }));
}

pub fn note_blank_app(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(version) = pinned_version(&state) else { return };
    log::info!("Roblox {version} can't start its app and will show a blank window");
    set_issue(app, Some(BuildIssue { version, kind: IssueKind::BlankApp }));
}

pub fn forget_missing(settings: &mut Settings) {
    let gone = settings.roblox.pinned_build.is_some()
        && !settings.roblox.install_dir.as_deref().is_some_and(|d| d.join(PLAYER_EXE).is_file());
    if gone {
        settings.roblox.pinned_build = None;
        settings.roblox.install_dir = None;
    }
}

pub fn sweep(state: &AppState) {
    if !super::process::roblox_pids().is_empty() {
        return;
    }
    let keep = lock(&state.settings).roblox.pinned_build.clone();
    let Ok(entries) = std::fs::read_dir(versions_root(state)) else { return };
    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("version-") || keep.as_deref() == Some(name.as_str()) {
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => log::info!("Removed the older Roblox {name}"),
            Err(e) => log::warn!("Couldn't remove the older Roblox {name}: {e}"),
        }
        let _ = std::fs::remove_dir_all(state.mod_paths.originals.join(&name));
    }
}

fn set_status(app: &AppHandle, status: BuildStatus) {
    let state = app.state::<AppState>();
    *lock(&state.builds.status) = status.clone();
    let _ = app.emit(events::BUILDS, &status);
}

fn download_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Vesper/", env!("CARGO_PKG_VERSION"), " (+https://xan.gripe/vesper)"))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn download_problem(detail: impl std::fmt::Display) -> Problem {
    Problem::new("builds.download", "Roblox's download didn't finish")
        .what("Vesper couldn't download this Roblox version.")
        .why("The connection dropped, or Roblox's download server didn't answer.")
        .fix("Check your connection and try again.")
        .details(detail)
        .action(ProblemAction::Retry)
}

async fn fetch_once(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: &AtomicBool,
    on_bytes: &mut impl FnMut(u64),
) -> std::result::Result<String, FetchError> {
    let net = |e: reqwest::Error| FetchError::Network(e.to_string());
    let mut response = http.get(url).send().await.map_err(net)?;
    if !response.status().is_success() {
        return Err(FetchError::Network(format!("Roblox answered {}", response.status())));
    }
    let mut file = std::fs::File::create(dest).map_err(FetchError::Disk)?;
    let mut digest = Md5::new();
    let mut got = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(net)? {
        if cancel.load(Ordering::Relaxed) {
            return Err(FetchError::Cancelled);
        }
        digest.update(&chunk);
        file.write_all(&chunk).map_err(FetchError::Disk)?;
        got += chunk.len() as u64;
        on_bytes(got);
    }
    file.flush().map_err(FetchError::Disk)?;
    Ok(hex::encode(digest.finalize()))
}

fn fetch_verified(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    md5: &str,
    cancel: &AtomicBool,
    on_bytes: &mut impl FnMut(u64),
) -> std::result::Result<(), Stop> {
    let mut last = String::new();
    for _ in 0..3 {
        on_bytes(0);
        match tauri::async_runtime::block_on(fetch_once(http, url, dest, cancel, on_bytes)) {
            Ok(digest) if digest == md5 => return Ok(()),
            Ok(_) => last = "the download didn't match Roblox's checksum".into(),
            Err(FetchError::Cancelled) => return Err(Stop::Cancelled),
            Err(FetchError::Disk(e)) => {
                return Err(Stop::Failed(Problem::io("Vesper couldn't save Roblox's download.", &e)))
            }
            Err(FetchError::Network(e)) => last = e,
        }
        log::info!("Retrying a Roblox download: {last}");
    }
    Err(Stop::Failed(download_problem(last)))
}

pub fn extract(archive: &Path, dest: &Path, cancel: &AtomicBool) -> std::result::Result<(), Stop> {
    let disk = |e: io::Error| Stop::Failed(Problem::io("Vesper couldn't unpack Roblox's download.", &e));
    let corrupt = |e: zip::result::ZipError| Stop::Failed(download_problem(e));
    let file = std::fs::File::open(archive).map_err(disk)?;
    let mut zip = zip::ZipArchive::new(file).map_err(corrupt)?;
    for i in 0..zip.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stop::Cancelled);
        }
        let mut entry = zip.by_index(i).map_err(corrupt)?;
        let name = entry.name().map_err(corrupt)?.into_owned();
        if name.chars().all(|c| matches!(c, '/' | '\\')) {
            continue;
        }
        let relative = safe_relative(&name)
            .ok_or_else(|| Stop::Failed(download_problem(format!("the download has an unsafe file path: {name}"))))?;
        let target = dest.join(relative);
        if name.ends_with(['/', '\\']) {
            std::fs::create_dir_all(&target).map_err(disk)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(disk)?;
        }
        let mut out = std::fs::File::create(&target).map_err(disk)?;
        io::copy(&mut entry, &mut out).map_err(disk)?;
    }
    Ok(())
}

pub fn download_build(
    http: &reqwest::Client,
    hash: &str,
    dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> std::result::Result<(), Stop> {
    let disk = |e: io::Error| Stop::Failed(Problem::io("Vesper couldn't save Roblox's download.", &e));
    std::fs::create_dir_all(dir).map_err(disk)?;
    let manifest_url = format!("{CDN}/{hash}-rbxPkgManifest.txt");
    let manifest = tauri::async_runtime::block_on(async {
        http.get(&manifest_url).send().await?.error_for_status()?.text().await
    })
    .map_err(download_problem)?;
    let packages: Vec<Package> = parse_manifest(&manifest)
        .map_err(|e| download_problem(format!("Roblox's file list couldn't be read: {e}")))?
        .into_iter()
        .filter(|p| p.name != INSTALLER_PACKAGE)
        .collect();
    if let Some(unknown) = packages.iter().find(|p| package_folder(&p.name).is_none()) {
        return Err(Stop::Failed(
            Problem::new("builds.unknown_package", "This Roblox version can't be installed")
                .what("It contains a file Vesper doesn't know where to put.")
                .why("Roblox may have changed how it packages its files.")
                .fix("Pick a different version.")
                .details(&unknown.name),
        ));
    }
    let total: u64 = packages.iter().map(|p| p.packed).sum();
    let mut done = 0u64;
    progress(0, total);
    for p in &packages {
        let archive = dir.join(format!("{}.download", p.name));
        let url = format!("{CDN}/{hash}-{}", p.name);
        fetch_verified(http, &url, &archive, &p.md5, cancel, &mut |got| progress(done + got, total))?;
        extract(&archive, &dir.join(package_folder(&p.name).unwrap_or_default()), cancel)?;
        std::fs::remove_file(&archive).map_err(disk)?;
        done += p.packed;
        progress(done, total);
    }
    std::fs::write(dir.join("AppSettings.xml"), APP_SETTINGS).map_err(disk)
}

fn rename_with_retries(from: &Path, to: &Path) -> io::Result<()> {
    let mut last = None;
    for _ in 0..20 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(last.unwrap_or_else(|| io::Error::other("the folder couldn't be renamed")))
}

fn activate(app: &AppHandle, hash: &str, dir: PathBuf) -> Result<()> {
    let state = app.state::<AppState>();
    {
        let mut s = lock(&state.settings);
        s.roblox.install_dir = Some(dir);
        s.roblox.pinned_build = Some(hash.to_owned());
    }
    state.save_settings()?;
    state.redetect(app);
    let _ = app.emit(events::SETTINGS, &state.settings());
    set_issue(app, None);
    sweep(&state);
    Ok(())
}

fn install(app: &AppHandle, hash: &str, version: RobloxVersion, cancel: &AtomicBool) -> std::result::Result<(), Stop> {
    let state = app.state::<AppState>();
    let root = versions_root(&state);
    let dest = root.join(hash);
    let wanted = |dir: &Path| file_version(&dir.join(PLAYER_EXE)) == Some(version);
    if wanted(&dest) {
        activate(app, hash, dest)?;
        return Ok(());
    }
    let staging = root.join(format!("{hash}.partial"));
    let _ = std::fs::remove_dir_all(&staging);
    let http = download_client();
    let mut last_emit: Option<Instant> = None;
    let result = download_build(&http, hash, &staging, cancel, |done, total| {
        if done == total || last_emit.is_none_or(|t| t.elapsed() > Duration::from_millis(120)) {
            last_emit = Some(Instant::now());
            set_status(app, BuildStatus::Working { version: version.to_string(), hash: hash.to_owned(), done, total });
        }
    })
    .and_then(|()| {
        if wanted(&staging) {
            return Ok(());
        }
        Err(Stop::Failed(
            Problem::new("builds.wrong_version", "This Roblox version didn't check out")
                .what(format!("The downloaded files aren't Roblox {version}."))
                .why("The list of older versions may be wrong about this one.")
                .fix("Pick a different version."),
        ))
    });
    if let Err(stop) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(stop);
    }
    let _ = std::fs::remove_dir_all(&dest);
    rename_with_retries(&staging, &dest)
        .map_err(|e| Stop::Failed(Problem::io("Vesper couldn't finish installing this Roblox version.", &e)))?;
    activate(app, hash, dest)?;
    Ok(())
}

async fn resolve(http: reqwest::Client, sha: String, version: RobloxVersion, date: DateTime<Utc>) -> Option<Known> {
    let url = format!("https://raw.githubusercontent.com/{HISTORY_REPO}/{sha}/WindowsPlayer.json");
    let text = http.get(url).send().await.ok()?.error_for_status().ok()?.text().await.ok()?;
    let parsed = cdn::parse_version(&text).ok()?;
    if parsed.version.parse::<RobloxVersion>().ok()? != version || !valid_hash(&parsed.version_hash) {
        return None;
    }
    let manifest = format!("{CDN}/{}-rbxPkgManifest.txt", parsed.version_hash);
    http.head(manifest).send().await.ok()?.error_for_status().ok()?;
    Some(Known { version, hash: parsed.version_hash, released: Some(date) })
}

async fn history(http: &reqwest::Client) -> std::result::Result<Vec<Known>, String> {
    let url = format!("https://api.github.com/repos/{HISTORY_REPO}/commits?path=WindowsPlayer.json&per_page=40");
    let response = http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status()));
    }
    let mut entries = parse_commits(&response.text().await.map_err(|e| e.to_string())?)?;
    entries.sort_by_key(|(_, version, _)| std::cmp::Reverse(*version));
    entries.dedup_by_key(|(_, version, _)| version.1);
    entries.truncate(MAX_RESOLVED);
    let mut tasks = tokio::task::JoinSet::new();
    for (sha, version, date) in entries {
        tasks.spawn(resolve(http.clone(), sha, version, date));
    }
    let mut known = Vec::new();
    while let Some(done) = tasks.join_next().await {
        known.extend(done.ok().flatten());
    }
    Ok(known)
}

fn base_version(state: &AppState) -> Option<RobloxVersion> {
    let installed = lock(&state.detection)
        .installs
        .iter()
        .filter(|i| i.source != InstallSource::Custom)
        .filter_map(|i| i.version.as_deref()?.parse().ok())
        .max();
    installed.or_else(|| lock(&state.latest).as_ref().and_then(|l| l.version.parse().ok()))
}

fn windows_only() -> Problem {
    Problem::new("builds.windows_only", "Only on Windows for now")
        .what("Vesper can't install older Roblox versions on a Mac yet.")
        .why("Roblox packages its Mac app differently.")
        .fix("Keep using the version you have.")
}

#[tauri::command]
pub async fn builds_list(state: State<'_, AppState>, refresh: bool) -> Result<BuildList> {
    if !cfg!(windows) {
        return Err(windows_only());
    }
    let cached = lock(&state.builds.known)
        .as_ref()
        .filter(|(at, _)| !refresh && at.elapsed() < LIST_TTL)
        .map(|(_, known)| known.clone());
    let (mut known, mut partial) = match cached {
        Some(known) => (known, false),
        None => match history(&state.http).await {
            Ok(known) => {
                let empty = known.is_empty();
                (known, empty)
            }
            Err(e) => {
                log::info!("Couldn't read the list of older Roblox versions: {e}");
                (Vec::new(), true)
            }
        },
    };
    if !partial {
        *lock(&state.builds.known) = Some((Instant::now(), known.clone()));
    }
    match cdn::latest_version(&state.http).await {
        Ok(live) if valid_hash(&live.version_hash) => {
            if let Ok(version) = live.version.parse::<RobloxVersion>() {
                if !known.iter().any(|k| k.hash == live.version_hash) {
                    known.push(Known { version, hash: live.version_hash, released: None });
                }
            }
        }
        Ok(_) => {}
        Err(e) => {
            log::info!("Couldn't read Roblox's current version: {e}");
            partial = partial || known.is_empty();
        }
    }
    let base = base_version(&state);
    let refused = newest_refused(&state);
    Ok(BuildList { builds: rows(&known, base, refused), base: base.map(|b| b.to_string()), partial })
}

#[tauri::command]
pub fn builds_status(state: State<'_, AppState>) -> BuildStatus {
    lock(&state.builds.status).clone()
}

#[tauri::command]
pub fn builds_issue(state: State<'_, AppState>) -> Option<BuildIssue> {
    lock(&state.builds.issue).clone()
}

#[tauri::command]
pub fn builds_install(app: AppHandle, state: State<'_, AppState>, hash: String, version: String) -> Result<()> {
    if !cfg!(windows) {
        return Err(windows_only());
    }
    let parsed: RobloxVersion = version.parse().map_err(|_| {
        Problem::invalid("That Roblox version number isn't valid.", "Close this window and open it again.")
    })?;
    if !valid_hash(&hash) {
        return Err(Problem::invalid("That Roblox version ID isn't valid.", "Close this window and open it again."));
    }
    {
        let mut status = lock(&state.builds.status);
        if matches!(*status, BuildStatus::Working { .. }) {
            return Err(Problem::new("builds.busy", "Already downloading a Roblox version")
                .what("Vesper downloads one version at a time.")
                .why("A download is still running.")
                .fix("Wait for it to finish, or cancel it."));
        }
        *status = BuildStatus::Working { version: version.clone(), hash: hash.clone(), done: 0, total: 0 };
    }
    let _ = app.emit(events::BUILDS, &*lock(&state.builds.status));
    state.builds.cancel.store(false, Ordering::Relaxed);
    let cancel = state.builds.cancel.clone();
    let worker = app.clone();
    std::thread::Builder::new()
        .name("vesper-builds".into())
        .spawn(move || {
            let outcome = install(&worker, &hash, parsed, &cancel);
            let status = match outcome {
                Ok(()) => {
                    log::info!("Now using Roblox {parsed} from Vesper's own folder");
                    BuildStatus::Idle
                }
                Err(Stop::Cancelled) => BuildStatus::Idle,
                Err(Stop::Failed(problem)) => {
                    log::warn!("Installing Roblox {parsed} failed: {problem}");
                    BuildStatus::Failed { problem }
                }
            };
            set_status(&worker, status);
        })
        .map_err(|e| Problem::internal("Couldn't start the download.", e))?;
    Ok(())
}

#[tauri::command]
pub fn builds_cancel(state: State<'_, AppState>) {
    state.builds.cancel();
}

#[tauri::command]
pub fn builds_dismiss(app: AppHandle, state: State<'_, AppState>) {
    let failed = matches!(*lock(&state.builds.status), BuildStatus::Failed { .. });
    if failed {
        set_status(&app, BuildStatus::Idle);
    }
}

#[tauri::command]
pub fn builds_use_latest(app: AppHandle, state: State<'_, AppState>) -> Result<Settings> {
    if matches!(*lock(&state.builds.status), BuildStatus::Working { .. }) {
        return Err(Problem::new("builds.busy", "A Roblox version is still downloading")
            .what("Vesper can't switch while it's downloading.")
            .why("The download would replace your choice when it finishes.")
            .fix("Wait for it to finish, or cancel it."));
    }
    {
        let mut s = lock(&state.settings);
        s.roblox.install_dir = None;
        s.roblox.pinned_build = None;
    }
    state.save_settings()?;
    state.redetect(&app);
    let settings = state.settings();
    let _ = app.emit(events::SETTINGS, &settings);
    set_issue(&app, None);
    let worker = app.clone();
    std::thread::spawn(move || sweep(&worker.state::<AppState>()));
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(version: &str, hash: &str) -> Known {
        Known { version: version.parse().unwrap(), hash: hash.into(), released: None }
    }

    #[test]
    fn hashes_must_look_like_roblox_builds() {
        assert!(valid_hash("version-5a21b87bbc2641cf"));
        assert!(!valid_hash("version-5a21b87bbc2641"));
        assert!(!valid_hash("version-5A21B87BBC2641CF"));
        assert!(!valid_hash("version-../../../../etc"));
        assert!(!valid_hash("5a21b87bbc2641cf"));
    }

    #[test]
    fn manifests_are_read_and_checked() {
        let good = "v0\r\nRobloxApp.zip\r\n81738fb2aaaaaaaaaaaaaaaaaaaaaaaa\r\n137795773\r\n174128171\r\nssl.zip\r\nA0D355CDAAAAAAAAAAAAAAAAAAAAAAAA\r\n130569\r\n228725\r\n";
        let packages = parse_manifest(good).unwrap();
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "RobloxApp.zip");
        assert_eq!(packages[0].packed, 137795773);
        assert_eq!(packages[1].md5, "a0d355cdaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(parse_manifest("v1\nRobloxApp.zip\nx\n1\n1").is_err());
        assert!(parse_manifest("v0\nRobloxApp.zip\nabc").is_err());
        assert!(parse_manifest("v0\n..\\evil.zip\n81738fb2aaaaaaaaaaaaaaaaaaaaaaaa\n1\n1").is_err());
        assert!(parse_manifest("v0\nssl.zip\nnothex\n1\n1").is_err());
    }

    #[test]
    fn every_package_of_a_current_build_has_a_place() {
        for name in [
            "RobloxApp.zip",
            "content-avatar.zip",
            "content-configs.zip",
            "content-fonts.zip",
            "content-models.zip",
            "content-sky.zip",
            "content-sounds.zip",
            "shaders.zip",
            "ssl.zip",
            "content-textures2.zip",
            "content-textures3.zip",
            "content-terrain.zip",
            "content-platform-fonts.zip",
            "content-platform-dictionaries.zip",
            "extracontent-places.zip",
            "extracontent-luapackages.zip",
            "extracontent-translations.zip",
            "extracontent-models.zip",
            "extracontent-textures.zip",
            "WebView2.zip",
            "WebView2RuntimeInstaller.zip",
        ] {
            assert!(package_folder(name).is_some(), "{name}");
        }
        assert_eq!(package_folder(INSTALLER_PACKAGE), None);
    }

    #[test]
    fn archive_paths_stay_inside_the_folder() {
        assert_eq!(safe_relative(r"content\fonts\a.ttf").unwrap(), Path::new("content").join("fonts").join("a.ttf"));
        assert_eq!(safe_relative("a/b/").unwrap(), Path::new("a").join("b"));
        assert_eq!(safe_relative(r"..\..\evil.exe"), None);
        assert_eq!(safe_relative("a/../../evil.exe"), None);
        assert_eq!(safe_relative(r"C:\Windows\evil.exe"), None);
        assert_eq!(safe_relative(r"\abilities\"), Some(PathBuf::from("abilities")));
        assert_eq!(safe_relative("/etc/passwd").unwrap(), Path::new("etc").join("passwd"));
        assert_eq!(safe_relative(r"\..\evil.exe"), None);
        assert_eq!(safe_relative(r"\\server\share\x"), None);
        assert_eq!(safe_relative(""), None);
    }

    #[test]
    fn extracting_writes_files_and_refuses_escapes() {
        use zip::write::SimpleFileOptions;
        let dir = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        let build = |name: &str, entries: &[&str]| {
            let path = dir.path().join(name);
            let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            for e in entries {
                w.start_file(*e, stored).unwrap();
                w.write_all(b"data").unwrap();
            }
            w.finish().unwrap();
            path
        };
        let good = build("good.zip", &[r"\", r"sub\file.txt", "top.txt"]);
        let out = dir.path().join("out");
        extract(&good, &out, &cancel).unwrap();
        assert_eq!(std::fs::read(out.join("sub").join("file.txt")).unwrap(), b"data");
        assert_eq!(std::fs::read(out.join("top.txt")).unwrap(), b"data");

        let bad = build("bad.zip", &[r"..\escaped.txt"]);
        let out2 = dir.path().join("out2");
        assert!(matches!(extract(&bad, &out2, &cancel), Err(Stop::Failed(_))));
        assert!(!dir.path().join("escaped.txt").exists());

        cancel.store(true, Ordering::Relaxed);
        assert!(matches!(extract(&good, &dir.path().join("out3"), &cancel), Err(Stop::Cancelled)));
    }

    #[test]
    fn commit_history_gives_player_versions_only() {
        let json = r#"[
          {"sha":"d28372088a","commit":{"message":"WindowsPlayer → 0.745.0.7451137, MacPlayer → 0.745.0.7451137","author":{"date":"2026-10-27T16:22:31Z"}}},
          {"sha":"abc123","commit":{"message":"MacPlayer → 0.744.0.7441080","author":{"date":"2026-10-20T20:24:47Z"}}},
          {"sha":"../evil","commit":{"message":"WindowsPlayer → 0.743.0.7431134","author":{"date":"2026-10-15T22:10:29Z"}}}
        ]"#;
        let commits = parse_commits(json).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].0, "d28372088a");
        assert_eq!(commits[0].1, "0.745.0.7451137".parse().unwrap());
        assert!(parse_commits("{\"message\":\"rate limited\"}").is_err());
    }

    #[test]
    fn rows_count_back_from_the_installed_version() {
        let list = [
            known("0.743.0.7431129", "version-a"),
            known("0.746.0.7461264", "version-e"),
            known("0.745.0.7451137", "version-d"),
            known("0.743.0.7431134", "version-b"),
            known("0.744.0.7441080", "version-c"),
        ];
        let base = "0.746.100.7461235".parse().ok();
        let r = rows(&list, base, None);
        let got: Vec<(&str, u32)> = r.iter().map(|b| (b.version.as_str(), b.versions_back)).collect();
        assert_eq!(got, [("0.745.0.7451137", 1), ("0.744.0.7441080", 2), ("0.743.0.7431134", 3)]);
        assert!(r.iter().all(|b| !b.too_old));
    }

    #[test]
    fn a_refused_version_marks_itself_and_older_ones() {
        let list = [
            known("0.745.0.7451137", "version-d"),
            known("0.743.0.7431134", "version-b"),
            known("0.742.0.7421053", "version-a"),
            known("0.741.0.7411058", "version-z"),
        ];
        let r = rows(&list, "0.746.0.1".parse().ok(), "0.742.0.7421053".parse().ok());
        let flags: Vec<bool> = r.iter().map(|b| b.too_old).collect();
        assert_eq!(flags, [false, false, true, true]);
    }

    #[test]
    fn rows_are_limited() {
        let list: Vec<Known> = (700..730).map(|m| known(&format!("0.{m}.0.1"), "version-x")).collect();
        let r = rows(&list, None, None);
        assert_eq!(r.len(), MAX_LISTED);
        assert_eq!(r[0].version, "0.729.0.1");
    }

    #[test]
    fn a_deleted_build_is_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        s.roblox.install_dir = Some(dir.path().join("version-5a21b87bbc2641cf"));
        s.roblox.pinned_build = Some("version-5a21b87bbc2641cf".into());
        forget_missing(&mut s);
        assert_eq!(s.roblox.pinned_build, None);
        assert_eq!(s.roblox.install_dir, None);

        let mut chosen = Settings::default();
        chosen.roblox.install_dir = Some(dir.path().join("somewhere"));
        forget_missing(&mut chosen);
        assert!(chosen.roblox.install_dir.is_some(), "a folder the user chose is left alone");
    }

    #[test]
    #[ignore = "reads the real version history"]
    fn lists_real_builds() {
        let known = tauri::async_runtime::block_on(history(&cdn::http_client())).unwrap();
        assert!(known.len() >= MAX_LISTED, "only {} builds", known.len());
        assert!(known.iter().all(|k| valid_hash(&k.hash) && k.released.is_some()));
        let top = known.iter().map(|k| k.version).max().unwrap();
        let listed = rows(&known, Some(RobloxVersion(top.0, top.1 + 1, 0, 0)), None);
        assert_eq!(listed.len(), MAX_LISTED);
        assert_eq!(listed[0].versions_back, 1);
        assert_eq!(listed[0].version, top.to_string());
        for b in &listed {
            println!("{} {} back {}", b.version, b.hash, b.versions_back);
        }
    }

    #[test]
    #[ignore = "downloads a real Roblox build"]
    fn downloads_a_real_build() {
        let dir = PathBuf::from(std::env::var("VESPER_BUILD_TEST_DIR").unwrap());
        let hash = std::env::var("VESPER_BUILD_TEST_HASH").unwrap();
        let cancel = AtomicBool::new(false);
        let mut last = (0, 0);
        download_build(&download_client(), &hash, &dir, &cancel, |d, t| last = (d, t)).unwrap();
        assert_eq!(last.0, last.1);
        assert!(last.1 > 50_000_000);
        assert!(file_version(&dir.join(PLAYER_EXE)).is_some());
        assert!(dir.join("AppSettings.xml").is_file());
        assert!(!dir.join(INSTALLER_PACKAGE).exists());
    }
}
