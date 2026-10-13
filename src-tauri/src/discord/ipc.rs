//! Discord's local IPC protocol.
//!
//! The Discord desktop app listens on `\\.\pipe\discord-ipc-0` … `-9` on
//! Windows, and on Unix sockets `discord-ipc-0` … `-9` in the temporary folder
//! on macOS.
//! Each message is an 8-byte header (opcode, payload length; little endian)
//! followed by a JSON payload. A session starts with a handshake carrying the
//! application's client ID; Discord answers with a READY dispatch.

use serde_json::{json, Value};
use std::io::{self, Read, Write};

#[cfg(windows)]
type Pipe = std::fs::File;
#[cfg(unix)]
type Pipe = std::os::unix::net::UnixStream;

#[cfg(windows)]
fn open_pipe(path: &str) -> io::Result<Pipe> {
    std::fs::OpenOptions::new().read(true).write(true).open(path)
}

#[cfg(unix)]
fn open_pipe(path: &str) -> io::Result<Pipe> {
    Pipe::connect(path)
}

pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;

/// Where Discord's sockets are, without the trailing number.
#[cfg(windows)]
pub fn default_pipe_base() -> String {
    r"\\.\pipe\discord-ipc-".to_owned()
}

/// Discord puts its socket in the first of these that is set, like other
/// Rich Presence clients look for it.
#[cfg(unix)]
pub fn default_pipe_base() -> String {
    let dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .find_map(|k| std::env::var_os(k).filter(|v| !v.is_empty()))
        .map(std::path::PathBuf::from)
        .or_else(user_temp_dir)
        .unwrap_or_else(|| "/tmp".into());
    format!("{}/discord-ipc-", dir.to_string_lossy().trim_end_matches('/'))
}

/// The per-user temporary folder macOS gives every app, for when TMPDIR isn't set.
#[cfg(unix)]
fn user_temp_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut buf = [0 as libc::c_char; 1024];
        // SAFETY: confstr writes at most `buf.len()` bytes, including the terminator.
        let n = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buf.as_mut_ptr(), buf.len()) };
        if n > 0 && n <= buf.len() {
            // SAFETY: confstr terminated the string.
            let s = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
            return Some(s.to_string_lossy().into_owned().into());
        }
    }
    None
}
const MAX_PAYLOAD: u32 = 64 * 1024;

#[derive(Debug)]
pub enum IpcError {
    /// No Discord pipe exists: Discord is closed, still starting, or not installed.
    NotRunning,
    /// Discord closed the connection, with its reason (e.g. invalid client ID).
    Closed {
        code: i64,
        message: String,
    },
    Io(io::Error),
    Protocol(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::NotRunning => write!(f, "Discord isn't running"),
            IpcError::Closed { code, message } => write!(f, "Discord closed the connection ({code}: {message})"),
            IpcError::Io(e) => write!(f, "{e}"),
            IpcError::Protocol(s) => write!(f, "unexpected reply from Discord: {s}"),
        }
    }
}

impl From<io::Error> for IpcError {
    fn from(e: io::Error) -> Self {
        IpcError::Io(e)
    }
}

pub fn encode(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

pub fn read_frame(r: &mut impl Read) -> Result<(u32, Value), IpcError> {
    let mut header = [0u8; 8];
    r.read_exact(&mut header)?;
    let op = u32::from_le_bytes(header[0..4].try_into().expect("4 bytes"));
    let len = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes"));
    if len > MAX_PAYLOAD {
        return Err(IpcError::Protocol(format!("frame of {len} bytes")));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    let value = serde_json::from_slice(&body).map_err(|e| IpcError::Protocol(e.to_string()))?;
    Ok((op, value))
}

pub struct Connection {
    pipe: Pipe,
    /// Display name of the signed-in Discord user, from READY. Kept in memory only.
    pub user: Option<String>,
}

impl Connection {
    /// Connects to the first available Discord pipe and performs the handshake.
    pub fn connect(pipe_base: &str, client_id: &str) -> Result<Self, IpcError> {
        let pipe = (0..10).find_map(|i| open_pipe(&format!("{pipe_base}{i}")).ok()).ok_or(IpcError::NotRunning)?;
        let mut c = Connection { pipe, user: None };
        c.send(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))?;
        let (op, v) = c.recv()?;
        if op == OP_CLOSE {
            return Err(close_error(&v));
        }
        if v.get("evt").and_then(Value::as_str) != Some("READY") {
            return Err(IpcError::Protocol(format!("expected READY, got op {op}")));
        }
        c.user = v
            .pointer("/data/user/global_name")
            .or_else(|| v.pointer("/data/user/username"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(c)
    }

    fn send(&mut self, op: u32, payload: &Value) -> Result<(), IpcError> {
        self.pipe.write_all(&encode(op, payload))?;
        self.pipe.flush()?;
        Ok(())
    }

    /// Reads the next frame, answering pings along the way.
    fn recv(&mut self) -> Result<(u32, Value), IpcError> {
        loop {
            let (op, v) = read_frame(&mut self.pipe)?;
            match op {
                OP_PING => self.send(OP_PONG, &v)?,
                OP_CLOSE => return Err(close_error(&v)),
                _ => return Ok((op, v)),
            }
        }
    }

    /// Sets (or with `None`, clears) the activity and waits for Discord's answer.
    pub fn set_activity(&mut self, activity: Option<&Value>) -> Result<(), IpcError> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": nonce,
        });
        self.send(OP_FRAME, &payload)?;
        loop {
            let (_, v) = self.recv()?;
            if v.get("nonce").and_then(Value::as_str) != Some(nonce.as_str()) {
                continue; // an unrelated dispatch
            }
            if v.get("evt").and_then(Value::as_str) == Some("ERROR") {
                let msg = v.pointer("/data/message").and_then(Value::as_str).unwrap_or("unknown error");
                return Err(IpcError::Protocol(msg.to_owned()));
            }
            return Ok(());
        }
    }
}

fn close_error(v: &Value) -> IpcError {
    IpcError::Closed {
        code: v.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: v.get("message").and_then(Value::as_str).unwrap_or("").to_owned(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let bytes = encode(OP_FRAME, &json!({"cmd": "SET_ACTIVITY"}));
        assert_eq!(&bytes[0..4], &1u32.to_le_bytes());
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize, bytes.len() - 8);
        let (op, v) = read_frame(&mut bytes.as_slice()).unwrap();
        assert_eq!(op, OP_FRAME);
        assert_eq!(v["cmd"], "SET_ACTIVITY");
    }

    #[test]
    fn oversized_and_garbled_frames_are_rejected() {
        let mut huge = Vec::new();
        huge.extend_from_slice(&1u32.to_le_bytes());
        huge.extend_from_slice(&(10 * 1024 * 1024u32).to_le_bytes());
        assert!(matches!(read_frame(&mut huge.as_slice()), Err(IpcError::Protocol(_))));
        let mut bad = encode(OP_FRAME, &json!({}));
        bad.truncate(9);
        bad[4] = 1;
        bad[8] = b'{';
        assert!(read_frame(&mut bad.as_slice()).is_err());
    }

    /// A socket base no Discord listens on, unique to one test.
    pub(crate) fn test_base(tag: &str) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        if cfg!(windows) {
            format!(r"\\.\pipe\vesper-{tag}-{id}-")
        } else {
            // Unix socket paths are short (104 bytes on macOS).
            std::env::temp_dir().join(format!("vp-{tag}-{}-", &id[..12])).to_string_lossy().into_owned()
        }
    }

    #[test]
    fn missing_discord_is_reported_as_not_running() {
        assert!(matches!(Connection::connect(&test_base("none"), "1"), Err(IpcError::NotRunning)));
    }

    #[cfg(unix)]
    #[test]
    fn socket_base_ends_with_the_discord_name() {
        let base = default_pipe_base();
        assert!(base.ends_with("/discord-ipc-"), "{base}");
        assert!(!base.contains("//discord"), "{base}");
    }

    /// A minimal fake Discord on a real pipe (Windows) or socket (Unix), for integration tests.
    pub(crate) mod fake {
        use super::super::*;

        pub enum Behaviour {
            Accept,
            RejectClientId,
        }

        #[cfg(windows)]
        mod server {
            use super::super::super::Pipe;
            use std::os::windows::io::FromRawHandle;
            use windows::core::HSTRING;
            use windows::Win32::Foundation::HANDLE;
            use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
            use windows::Win32::System::Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
                PIPE_WAIT,
            };

            pub type Pending = HANDLE;

            pub struct Server(String);

            impl Server {
                pub fn new(name: String) -> Self {
                    Self(name)
                }

                /// Creates the next pipe instance; clients can connect from now on.
                pub fn prepare(&mut self) -> Pending {
                    unsafe {
                        CreateNamedPipeW(
                            &HSTRING::from(self.0.as_str()),
                            PIPE_ACCESS_DUPLEX,
                            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                            PIPE_UNLIMITED_INSTANCES,
                            65536,
                            65536,
                            0,
                            None,
                        )
                    }
                }

                pub fn accept(&mut self, handle: Pending) -> Option<Pipe> {
                    let _ = unsafe { ConnectNamedPipe(handle, None) };
                    Some(unsafe { Pipe::from_raw_handle(handle.0) })
                }
            }
        }

        #[cfg(unix)]
        mod server {
            use super::super::super::Pipe;
            use std::os::unix::net::UnixListener;

            /// Unix listeners accept without preparing anything per client.
            pub struct Pending;

            pub struct Server(UnixListener, String);

            impl Server {
                pub fn new(name: String) -> Self {
                    let _ = std::fs::remove_file(&name);
                    Self(UnixListener::bind(&name).expect("bind the fake Discord socket"), name)
                }

                pub fn prepare(&mut self) -> Pending {
                    Pending
                }

                pub fn accept(&mut self, _: Pending) -> Option<Pipe> {
                    self.0.accept().ok().map(|(s, _)| s)
                }
            }

            impl Drop for Server {
                fn drop(&mut self) {
                    let _ = std::fs::remove_file(&self.1);
                }
            }
        }

        /// Serves `connections` clients on `{base}0`. Every received frame is sent on `log`.
        pub fn serve(
            base: &str,
            behaviour: Behaviour,
            connections: usize,
            log: std::sync::mpsc::Sender<Value>,
        ) -> std::thread::JoinHandle<()> {
            let name = format!("{base}0");
            let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
            let r2 = ready.clone();
            let h = std::thread::spawn(move || {
                let mut server = server::Server::new(name);
                let mut first = true;
                for _ in 0..connections {
                    let pending = server.prepare();
                    if first {
                        r2.wait();
                        first = false;
                    }
                    let Some(mut pipe) = server.accept(pending) else { continue };
                    let Ok((_, hello)) = read_frame(&mut pipe) else { continue };
                    let _ = log.send(hello);
                    match behaviour {
                        Behaviour::RejectClientId => {
                            let _ = pipe
                                .write_all(&encode(OP_CLOSE, &json!({"code": 4000, "message": "Invalid Client ID"})));
                            continue;
                        }
                        Behaviour::Accept => {
                            let ready = json!({"cmd": "DISPATCH", "evt": "READY", "data": {"v": 1, "user": {"username": "tester"}}});
                            let _ = pipe.write_all(&encode(OP_FRAME, &ready));
                        }
                    }
                    while let Ok((_, frame)) = read_frame(&mut pipe) {
                        let nonce = frame.get("nonce").cloned().unwrap_or(Value::Null);
                        let _ = log.send(frame);
                        let reply = json!({"cmd": "SET_ACTIVITY", "data": {}, "evt": null, "nonce": nonce});
                        if pipe.write_all(&encode(OP_FRAME, &reply)).is_err() {
                            break;
                        }
                    }
                }
            });
            ready.wait();
            h
        }
    }

    #[test]
    fn handshake_and_activity_against_fake_discord() {
        let base = test_base("test");
        let (tx, rx) = std::sync::mpsc::channel();
        let server = fake::serve(&base, fake::Behaviour::Accept, 1, tx);

        let mut c = Connection::connect(&base, "123456789").unwrap();
        assert_eq!(c.user.as_deref(), Some("tester"));
        let hello = rx.recv().unwrap();
        assert_eq!(hello["client_id"], "123456789");
        assert_eq!(hello["v"], 1);

        c.set_activity(Some(&json!({"details": "Using Vesper"}))).unwrap();
        let frame = rx.recv().unwrap();
        assert_eq!(frame["cmd"], "SET_ACTIVITY");
        assert_eq!(frame["args"]["activity"]["details"], "Using Vesper");
        assert_eq!(frame["args"]["pid"], std::process::id());

        c.set_activity(None).unwrap();
        assert!(rx.recv().unwrap()["args"]["activity"].is_null(), "clearing sends a null activity");
        drop(c);
        server.join().unwrap();
    }

    #[test]
    fn invalid_client_id_is_reported() {
        let base = test_base("test");
        let (tx, _rx) = std::sync::mpsc::channel();
        let server = fake::serve(&base, fake::Behaviour::RejectClientId, 1, tx);
        match Connection::connect(&base, "bad") {
            Err(IpcError::Closed { code, .. }) => assert_eq!(code, 4000),
            Err(e) => panic!("unexpected {e}"),
            Ok(_) => panic!("should have been rejected"),
        }
        server.join().unwrap();
    }
}
