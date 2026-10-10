//! Discord Rich Presence for Vesper itself.
//!
//! Runs on its own thread so Discord being slow, closed or restarting never
//! affects the app. When Discord isn't available it retries quietly with
//! backoff and logs only when the state changes. Turning the setting off
//! clears the presence immediately; closing Vesper closes the pipe, which
//! makes Discord remove it.
//!
//! The presence contains only: Vesper's name and artwork, a short status,
//! the session start time, and a "Get Vesper" button. No paths, games,
//! account names or anything from Roblox.

use super::ipc::{Connection, IpcError, DEFAULT_PIPE_BASE};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PAGE_URL: &str = "https://xan.gripe/vesper";
/// Art asset names uploaded to the Discord application (scripts/brand/discord.mjs).
pub const LARGE_IMAGE: &str = "vesper";
pub const SMALL_PLAYING: &str = "playing";
pub const SMALL_IDLE: &str = "idle";
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const REFRESH: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum PresenceStatus {
    Disabled,
    /// This build has no Discord application ID.
    NotConfigured,
    Connecting,
    Connected,
    DiscordNotRunning,
    Rejected(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PresenceState {
    pub roblox_running: bool,
    pub profile: Option<String>,
    pub show_profile: bool,
    pub show_roblox_status: bool,
}

/// The activity Discord shows. Pure, so it is easy to test for privacy.
pub fn activity(state: &PresenceState, started_unix: i64) -> Value {
    let status = if state.roblox_running { "Playing Roblox" } else { "In the launcher" };
    let line = match (state.show_roblox_status, state.show_profile.then_some(state.profile.as_deref()).flatten()) {
        (true, Some(p)) => Some(format!("{status} · {p} profile")),
        (true, None) => Some(status.to_owned()),
        (false, Some(p)) => Some(format!("{p} profile")),
        (false, None) => None,
    };
    let mut a = json!({
        "details": "Using Vesper",
        "timestamps": { "start": started_unix },
        "assets": {
            "large_image": LARGE_IMAGE,
            "large_text": "Vesper · Roblox, tuned and verified",
        },
        "buttons": [ { "label": "Get Vesper", "url": PAGE_URL } ],
        "instance": false,
    });
    // The badge shows whether Roblox is running, so it follows the same setting.
    if state.show_roblox_status {
        a["assets"]["small_image"] = json!(if state.roblox_running { SMALL_PLAYING } else { SMALL_IDLE });
        a["assets"]["small_text"] = json!(status);
    }
    if let Some(l) = line {
        a["state"] = Value::String(l.chars().take(128).collect());
    }
    a
}

enum Command {
    Configure { enabled: bool, client_id: Option<String> },
    Update(PresenceState),
    Shutdown,
}

pub struct Presence {
    tx: Sender<Command>,
    status: Arc<Mutex<PresenceStatus>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Presence {
    /// Starts the presence thread. `on_status` is called whenever the status changes.
    pub fn start(on_status: impl Fn(PresenceStatus) + Send + 'static) -> Self {
        Self::start_with_pipe(DEFAULT_PIPE_BASE.to_owned(), on_status)
    }

    pub fn start_with_pipe(pipe_base: String, on_status: impl Fn(PresenceStatus) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(PresenceStatus::Disabled));
        let shared = status.clone();
        let thread = std::thread::Builder::new()
            .name("vesper-discord".into())
            .spawn(move || Worker::new(pipe_base, shared, Box::new(on_status)).run(rx))
            .expect("spawn discord thread");
        Self { tx, status, thread: Some(thread) }
    }

    pub fn configure(&self, enabled: bool, client_id: Option<String>) {
        let _ = self.tx.send(Command::Configure { enabled, client_id });
    }

    pub fn update(&self, state: PresenceState) {
        let _ = self.tx.send(Command::Update(state));
    }

    pub fn status(&self) -> PresenceStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or(PresenceStatus::Disabled)
    }

    /// Clears the presence and stops the thread (waits briefly).
    pub fn shutdown(mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Worker {
    pipe_base: String,
    status: Arc<Mutex<PresenceStatus>>,
    on_status: Box<dyn Fn(PresenceStatus) + Send>,
    enabled: bool,
    client_id: Option<String>,
    state: PresenceState,
    started: i64,
    conn: Option<Connection>,
    backoff: Duration,
    next_attempt: Instant,
    last_sent: Instant,
}

impl Worker {
    fn new(
        pipe_base: String,
        status: Arc<Mutex<PresenceStatus>>,
        on_status: Box<dyn Fn(PresenceStatus) + Send>,
    ) -> Self {
        Self {
            pipe_base,
            status,
            on_status,
            enabled: false,
            client_id: None,
            state: PresenceState::default(),
            started: chrono::Utc::now().timestamp(),
            conn: None,
            backoff: Duration::from_secs(5),
            next_attempt: Instant::now(),
            last_sent: Instant::now(),
        }
    }

    fn set_status(&self, s: PresenceStatus) {
        let mut cur = self.status.lock().unwrap_or_else(|e| e.into_inner());
        if *cur != s {
            log::info!("Discord presence: {s:?}");
            *cur = s.clone();
            drop(cur);
            (self.on_status)(s);
        }
    }

    fn wanted(&self) -> bool {
        self.enabled && self.client_id.is_some()
    }

    fn disconnect(&mut self, clear: bool) {
        if let Some(mut c) = self.conn.take() {
            if clear {
                let _ = c.set_activity(None);
            }
        }
    }

    fn push(&mut self) {
        let a = activity(&self.state, self.started);
        let Some(c) = self.conn.as_mut() else { return };
        match c.set_activity(Some(&a)) {
            Ok(()) => self.last_sent = Instant::now(),
            Err(e) => {
                log::debug!("Discord update failed: {e}");
                self.conn = None;
                self.schedule_retry(PresenceStatus::DiscordNotRunning);
            }
        }
    }

    fn schedule_retry(&mut self, s: PresenceStatus) {
        self.next_attempt = Instant::now() + self.backoff;
        self.backoff = (self.backoff * 2).min(MAX_BACKOFF);
        self.set_status(s);
    }

    fn try_connect(&mut self) {
        let Some(id) = self.client_id.clone() else { return };
        if self.status() != PresenceStatus::DiscordNotRunning {
            self.set_status(PresenceStatus::Connecting);
        }
        match Connection::connect(&self.pipe_base, &id) {
            Ok(c) => {
                self.conn = Some(c);
                self.backoff = Duration::from_secs(5);
                self.set_status(PresenceStatus::Connected);
                self.push();
            }
            Err(IpcError::NotRunning) => self.schedule_retry(PresenceStatus::DiscordNotRunning),
            Err(IpcError::Closed { code, message }) => {
                // A rejected client ID won't fix itself quickly; retry slowly.
                self.backoff = MAX_BACKOFF;
                self.schedule_retry(PresenceStatus::Rejected(format!("{message} ({code})")));
            }
            Err(e) => self.schedule_retry(PresenceStatus::Error(e.to_string())),
        }
    }

    fn status(&self) -> PresenceStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or(PresenceStatus::Disabled)
    }

    fn run(mut self, rx: Receiver<Command>) {
        loop {
            let wait = if self.wanted() {
                if self.conn.is_some() {
                    REFRESH.saturating_sub(self.last_sent.elapsed())
                } else {
                    self.next_attempt.saturating_duration_since(Instant::now())
                }
            } else {
                Duration::from_secs(3600)
            };
            match rx.recv_timeout(wait) {
                Ok(Command::Configure { enabled, client_id }) => {
                    self.enabled = enabled;
                    self.client_id = client_id.filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit()));
                    if !self.enabled {
                        self.disconnect(true);
                        self.set_status(PresenceStatus::Disabled);
                    } else if self.client_id.is_none() {
                        self.disconnect(true);
                        self.set_status(PresenceStatus::NotConfigured);
                    } else if self.conn.is_none() {
                        self.backoff = Duration::from_secs(5);
                        self.try_connect();
                    }
                }
                Ok(Command::Update(state)) => {
                    if state != self.state {
                        self.state = state;
                        if self.conn.is_some() {
                            self.push();
                        }
                    }
                }
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    self.disconnect(true);
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if !self.wanted() {
                        continue;
                    }
                    if self.conn.is_some() {
                        // Periodic refresh doubles as a liveness check.
                        self.push();
                    } else if Instant::now() >= self.next_attempt {
                        self.try_connect();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(running: bool, profile: Option<&str>) -> PresenceState {
        PresenceState {
            roblox_running: running,
            profile: profile.map(str::to_owned),
            show_profile: true,
            show_roblox_status: true,
        }
    }

    #[test]
    fn activity_has_brand_button_and_no_private_data() {
        let a = activity(&state(true, Some("Competitive")), 1_700_000_000);
        assert_eq!(a["details"], "Using Vesper");
        assert_eq!(a["state"], "Playing Roblox · Competitive profile");
        assert_eq!(a["buttons"][0]["label"], "Get Vesper");
        assert_eq!(a["buttons"][0]["url"], "https://xan.gripe/vesper");
        assert_eq!(a["assets"]["large_image"], "vesper");
        assert_eq!(a["assets"]["small_image"], "playing");
        assert_eq!(activity(&state(false, None), 0)["assets"]["small_image"], "idle");
        let hidden = PresenceState { show_roblox_status: false, ..state(true, None) };
        assert!(activity(&hidden, 0)["assets"].get("small_image").is_none(), "badge hidden with the status");
        let text = a.to_string().to_lowercase();
        for leak in ["users\\", "appdata", "c:\\", "roblosecurity", "token"] {
            assert!(!text.contains(leak), "activity contains {leak}");
        }
    }

    #[test]
    fn status_line_respects_privacy_toggles() {
        let mut s = state(false, Some("Quality"));
        assert_eq!(activity(&s, 0)["state"], "In the launcher · Quality profile");
        s.show_profile = false;
        assert_eq!(activity(&s, 0)["state"], "In the launcher");
        s.show_roblox_status = false;
        assert!(activity(&s, 0).get("state").is_none());
    }

    fn wait_for(p: &Presence, want: &PresenceStatus) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if &p.status() == want {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn missing_client_id_is_not_configured() {
        let p = Presence::start_with_pipe(format!(r"\\.\pipe\vesper-none-{}-", uuid::Uuid::new_v4().simple()), |_| {});
        p.configure(true, None);
        assert!(wait_for(&p, &PresenceStatus::NotConfigured));
        p.shutdown();
    }

    #[test]
    fn closed_discord_is_tolerated_without_errors() {
        let p = Presence::start_with_pipe(format!(r"\\.\pipe\vesper-none-{}-", uuid::Uuid::new_v4().simple()), |_| {});
        p.configure(true, Some("123".into()));
        assert!(wait_for(&p, &PresenceStatus::DiscordNotRunning));
        p.update(state(true, None));
        p.configure(false, Some("123".into()));
        assert!(wait_for(&p, &PresenceStatus::Disabled));
        p.shutdown();
    }

    #[cfg(windows)]
    #[test]
    fn publishes_updates_and_clears_when_disabled() {
        use crate::discord::ipc::tests::fake;
        let base = format!(r"\\.\pipe\vesper-test-{}-", uuid::Uuid::new_v4().simple());
        let (tx, rx) = std::sync::mpsc::channel();
        let server = fake::serve(&base, fake::Behaviour::Accept, 1, tx);

        let statuses = Arc::new(Mutex::new(Vec::new()));
        let seen = statuses.clone();
        let p = Presence::start_with_pipe(base, move |s| seen.lock().unwrap().push(s));
        p.update(state(false, Some("Balanced")));
        p.configure(true, Some("42".into()));
        assert!(wait_for(&p, &PresenceStatus::Connected));

        let hello = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(hello["client_id"], "42");
        let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(first["args"]["activity"]["state"], "In the launcher · Balanced profile");

        p.update(state(true, Some("Balanced")));
        let second = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(second["args"]["activity"]["state"], "Playing Roblox · Balanced profile");

        p.configure(false, Some("42".into()));
        let cleared = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(cleared["args"]["activity"].is_null(), "disabling clears immediately");
        assert!(wait_for(&p, &PresenceStatus::Disabled));
        p.shutdown();
        server.join().unwrap();
        assert!(statuses.lock().unwrap().contains(&PresenceStatus::Connected));
    }
}
