//! Following Roblox's log to know which game and server the player is in.
//!
//! While Roblox runs, Vesper reads new lines of the client's own log once a
//! second. It only extracts a handful of facts:
//!
//! ```text
//! [FLog::Output] ! Joining game '<server id>' place <place id> at <internal ip>
//! [FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:<id>, ... universeid:<id>, ... userid:<id>,
//! [FLog::Network] UDMUX Address = <server ip>, Port = ...
//! [FLog::GameJoinUtil] GameJoinUtil::joinGamePostPrivateServer
//! [FLog::GameJoinUtil] GameJoinUtil::initiateTeleportToReservedServer
//! [FLog::SingleSurfaceApp] leaveUGCGameInternal
//! ```
//!
//! Nothing else is kept. Join tickets and account names that the log also
//! contains are never read into a value, stored or shown.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, SystemTime};

static JOINING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"! Joining game '([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})' place (\d+)",
    )
    .expect("valid regex")
});
static LOAD_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Report game_join_loadtime: placeid:(\d+),.*?universeid:(\d+)").expect("valid regex"));
static USER_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Report game_join_loadtime: .*?\buserid:(\d+)").expect("valid regex"));
static UDMUX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"UDMUX Address = (\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})").expect("valid regex"));

const PRIVATE_MARKER: &str = "GameJoinUtil::joinGamePostPrivateServer";
const RESERVED_MARKER: &str = "GameJoinUtil::initiateTeleportToReservedServer";
const LEAVE_MARKER: &str = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ServerKind {
    #[default]
    Public,
    /// A private (VIP) server: joining needs the owner's invite link.
    Private,
    /// A server a game reserved for a match or party; nobody can join by link.
    Reserved,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub place_id: u64,
    pub universe_id: Option<u64>,
    /// The server's id (Roblox calls it the game instance or job id).
    pub job_id: String,
    pub kind: ServerKind,
    /// The server's public address, used to look up where it is.
    pub server_ip: Option<String>,
    pub joined_at: DateTime<Utc>,
}

impl Activity {
    /// A link that opens Roblox in this exact server, when others can join it.
    pub fn join_url(&self) -> Option<String> {
        (self.kind == ServerKind::Public).then(|| {
            format!("https://www.roblox.com/games/start?placeId={}&gameInstanceId={}", self.place_id, self.job_id)
        })
    }

    pub fn page_url(&self) -> String {
        format!("https://www.roblox.com/games/{}", self.place_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Entered a game or another server.
    Joined,
    /// Learned more about the current one (universe, address).
    Updated,
    Left,
}

/// Turns log lines into the current game.
#[derive(Debug, Default)]
pub struct Tracker {
    pub current: Option<Activity>,
    /// The signed-in player's user id, from the join report.
    pub user_id: Option<u64>,
    next_kind: ServerKind,
}

fn line_time(line: &str) -> Option<DateTime<Utc>> {
    let stamp = line.split(',').next()?;
    DateTime::parse_from_rfc3339(stamp).ok().map(|t| t.with_timezone(&Utc))
}

impl Tracker {
    pub fn feed(&mut self, line: &str) -> Option<Change> {
        if line.contains(PRIVATE_MARKER) {
            self.next_kind = ServerKind::Private;
            return None;
        }
        if line.contains(RESERVED_MARKER) {
            self.next_kind = ServerKind::Reserved;
            return None;
        }
        if let Some(c) = JOINING.captures(line) {
            let job_id = c[1].to_ascii_lowercase();
            let place_id: u64 = c[2].parse().ok()?;
            let kind = std::mem::take(&mut self.next_kind);
            if let Some(cur) = &mut self.current {
                // Reconnecting to the same server keeps the session going.
                if cur.job_id == job_id && cur.place_id == place_id {
                    return None;
                }
            }
            let universe_id = self.current.as_ref().filter(|c| c.place_id == place_id).and_then(|c| c.universe_id);
            self.current = Some(Activity {
                place_id,
                universe_id,
                job_id,
                kind,
                server_ip: None,
                joined_at: line_time(line).unwrap_or_else(Utc::now),
            });
            return Some(Change::Joined);
        }
        if line.contains("Report game_join_loadtime:") {
            if let Some(u) = USER_ID.captures(line).and_then(|c| c[1].parse().ok()) {
                self.user_id = Some(u);
            }
            let c = LOAD_TIME.captures(line)?;
            let (place, universe): (u64, u64) = (c[1].parse().ok()?, c[2].parse().ok()?);
            let cur = self.current.as_mut().filter(|a| a.place_id == place)?;
            if cur.universe_id != Some(universe) {
                cur.universe_id = Some(universe);
                return Some(Change::Updated);
            }
            return None;
        }
        if let Some(c) = UDMUX.captures(line) {
            let cur = self.current.as_mut()?;
            if cur.server_ip.as_deref() != Some(&c[1]) {
                cur.server_ip = Some(c[1].to_owned());
                return Some(Change::Updated);
            }
            return None;
        }
        if line.contains(LEAVE_MARKER) && self.current.take().is_some() {
            return Some(Change::Left);
        }
        None
    }
}

/// Reads a growing log file line by line.
struct Tail {
    path: PathBuf,
    offset: u64,
    partial: String,
}

impl Tail {
    fn new(path: PathBuf) -> Self {
        Self { path, offset: 0, partial: String::new() }
    }

    fn read_lines(&mut self) -> Vec<String> {
        let Ok(mut f) = std::fs::File::open(&self.path) else { return vec![] };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return vec![];
        }
        let mut buf = Vec::new();
        if f.take(8 * 1024 * 1024).read_to_end(&mut buf).is_err() {
            return vec![];
        }
        self.offset += buf.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&buf));
        let mut lines: Vec<String> = self.partial.split('\n').map(str::to_owned).collect();
        self.partial = lines.pop().unwrap_or_default();
        lines
    }
}

/// Follows the newest player log while Roblox runs. Dropping it stops the thread.
pub struct ActivityWatcher {
    stop: Arc<AtomicBool>,
}

impl Drop for ActivityWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl ActivityWatcher {
    /// `since` is when Roblox started; older logs are ignored. `on_change`
    /// receives the tracker after every change.
    pub fn start(
        logs_dir: PathBuf,
        since: SystemTime,
        on_change: impl Fn(Change, &Tracker) + Send + 'static,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::Builder::new().name("vesper-activity".into()).spawn(move || {
            let mut tracker = Tracker::default();
            let mut tail: Option<Tail> = None;
            let mut checked = SystemTime::UNIX_EPOCH;
            while !flag.load(Ordering::Relaxed) {
                // Roblox starts a new log when it restarts itself; look for one now and then.
                if tail.is_none() || checked.elapsed().map_or(true, |e| e > Duration::from_secs(5)) {
                    checked = SystemTime::now();
                    if let Some(newest) = super::logs::newest_player_log_since(&logs_dir, since) {
                        if tail.as_ref().is_none_or(|t| t.path != newest) {
                            if tracker.current.take().is_some() {
                                on_change(Change::Left, &tracker);
                            }
                            tail = Some(Tail::new(newest));
                        }
                    }
                }
                if let Some(t) = tail.as_mut() {
                    for line in t.read_lines() {
                        if let Some(change) = tracker.feed(&line) {
                            on_change(change, &tracker);
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            if tracker.current.take().is_some() {
                on_change(Change::Left, &tracker);
            }
        })?;
        Ok(Self { stop })
    }
}

/// Replays a whole log, for tests and for picking up a game already in progress.
pub fn replay(path: &Path) -> Tracker {
    let mut t = Tracker::default();
    if let Ok(text) = std::fs::read(path) {
        for line in String::from_utf8_lossy(&text).lines() {
            t.feed(line);
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lines in the format of a real Roblox 0.743 player log; ids are made up.
    const LOG: &str = "\
2026-10-17T04:51:21.968Z,5.968251,753c,6 [FLog::Output] ! Joining game '0d6a1c3e-4b7f-4e21-9a55-3f0c2d8e1b11' place 6884319169 at 10.0.0.11
2026-10-17T04:51:21.968Z,5.968251,753c,6 [FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:6884319169, join_time:0.64, universeid:2626227051, referral_page:, sid:11111111, clienttime:1792212682.32,
2026-10-17T04:51:21.969Z,5.969249,6fb8,7 [FLog::Network] UDMUX Address = 128.116.0.33, Port = 50001 | RCC Server Address = 10.0.0.11, Port = 50001
2026-10-17T04:51:22.082Z,6.082891,761c,6,Info [DFLog::NetworkClient] Connection accepted from 128.116.0.33|50001
2026-10-17T04:51:22.870Z,6.870530,1b14,6,Info [FLog::UgcExperienceController] UgcExperienceController: doTeleport: joinScriptUrl https://assetgame.roblox.com/Game/Join.ashx?ticket=SECRET
2026-10-17T04:51:22.938Z,6.938205,6fb8,6 [FLog::Output] ! Joining game '7c2e9f40-1d3b-4a8e-b6c5-92e0f4a1d722' place 13204204170 at 10.0.0.12
2026-10-17T04:51:22.938Z,6.938205,6fb8,6 [FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:13204204170, join_time:0, universeid:2626227051, referral_page:, sid:22222222, clienttime:1792212683.29, userid:42,
2026-10-17T04:51:23.021Z,7.021132,2eb0,7 [FLog::Network] UDMUX Address = 128.116.1.33, Port = 50002 | RCC Server Address = 10.0.0.12, Port = 50002
";

    #[test]
    fn follows_joins_teleports_and_server_addresses() {
        let mut t = Tracker::default();
        let changes: Vec<Change> = LOG.lines().filter_map(|l| t.feed(l)).collect();
        assert_eq!(
            changes,
            [Change::Joined, Change::Updated, Change::Updated, Change::Joined, Change::Updated, Change::Updated]
        );
        let a = t.current.as_ref().unwrap();
        assert_eq!(a.place_id, 13204204170);
        assert_eq!(a.universe_id, Some(2626227051));
        assert_eq!(a.job_id, "7c2e9f40-1d3b-4a8e-b6c5-92e0f4a1d722");
        assert_eq!(a.server_ip.as_deref(), Some("128.116.1.33"));
        assert_eq!(a.kind, ServerKind::Public);
        assert_eq!(a.joined_at.to_rfc3339(), "2026-10-17T04:51:22.938+00:00");
        assert_eq!(t.user_id, Some(42));
        assert_eq!(
            a.join_url().unwrap(),
            "https://www.roblox.com/games/start?placeId=13204204170&gameInstanceId=7c2e9f40-1d3b-4a8e-b6c5-92e0f4a1d722"
        );
    }

    #[test]
    fn private_and_reserved_servers_have_no_join_link() {
        let mut t = Tracker::default();
        t.feed("2026-10-17T01:00:00.000Z,1,1,6 [FLog::GameJoinUtil] GameJoinUtil::joinGamePostPrivateServer");
        t.feed("2026-10-17T01:00:01.000Z,1,1,6 [FLog::Output] ! Joining game '00000000-0000-0000-0000-000000000001' place 5 at 10.0.0.1");
        assert_eq!(t.current.as_ref().unwrap().kind, ServerKind::Private);
        assert!(t.current.as_ref().unwrap().join_url().is_none());
        t.feed("2026-10-17T01:00:02.000Z,1,1,6 [FLog::GameJoinUtil] GameJoinUtil::initiateTeleportToReservedServer");
        t.feed("2026-10-17T01:00:03.000Z,1,1,6 [FLog::Output] ! Joining game '00000000-0000-0000-0000-000000000002' place 6 at 10.0.0.1");
        assert_eq!(t.current.as_ref().unwrap().kind, ServerKind::Reserved);
        t.feed("2026-10-17T01:00:04.000Z,1,1,6 [FLog::Output] ! Joining game '00000000-0000-0000-0000-000000000003' place 6 at 10.0.0.1");
        assert_eq!(t.current.as_ref().unwrap().kind, ServerKind::Public, "the marker applies to one join");
    }

    #[test]
    fn leaving_and_reconnecting() {
        let mut t = Tracker::default();
        let join = "2026-10-17T01:00:01.000Z,1,1,6 [FLog::Output] ! Joining game '00000000-0000-0000-0000-000000000001' place 5 at 10.0.0.1";
        assert_eq!(t.feed(join), Some(Change::Joined));
        assert_eq!(t.feed(join), None, "rejoining the same server isn't a new session");
        assert_eq!(
            t.feed("2026-10-17T01:00:09.000Z,9,1,6 [FLog::SingleSurfaceApp] leaveUGCGameInternal"),
            Some(Change::Left)
        );
        assert!(t.current.is_none());
        assert_eq!(t.feed("2026-10-17T01:00:10.000Z,9,1,6 [FLog::SingleSurfaceApp] leaveUGCGameInternal"), None);
    }

    #[test]
    fn nothing_but_the_facts_is_kept() {
        let mut t = Tracker::default();
        for l in LOG.lines() {
            t.feed(l);
        }
        let text = serde_json::to_string(&t.current).unwrap();
        assert!(!text.contains("SECRET") && !text.contains("ticket"));
    }

    #[test]
    fn tail_reads_only_new_complete_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x_Player_1_last.log");
        std::fs::write(&path, "one\ntw").unwrap();
        let mut tail = Tail::new(path.clone());
        assert_eq!(tail.read_lines(), ["one"]);
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(tail.read_lines(), ["two", "three"]);
        assert!(tail.read_lines().is_empty());
    }
}
