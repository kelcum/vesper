//! Starting the Roblox player process.
//!
//! The process handle is released immediately after creation. Roblox's
//! anti-cheat reacts to tools that keep handles to the game open, so all
//! later monitoring uses handle-free APIs (see `process.rs`).
//!
//! Priority is set through the process creation flags, so it needs no
//! extra access. CPU affinity, when chosen, is applied in the same instant
//! using the creation handle, before it is closed.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    #[default]
    Normal,
    AboveNormal,
    High,
}

impl Priority {
    #[cfg(windows)]
    fn creation_flag(self) -> u32 {
        use windows::Win32::System::Threading::{
            ABOVE_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        };
        match self {
            Priority::Normal => NORMAL_PRIORITY_CLASS.0,
            Priority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS.0,
            Priority::High => HIGH_PRIORITY_CLASS.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub exe: PathBuf,
    pub working_dir: PathBuf,
    /// A validated launch URI from `deeplink::launch_argument`, or `None` to open the app.
    pub argument: Option<String>,
    pub priority: Priority,
    /// Bit mask of logical processors, or `None` to leave Windows in charge.
    pub affinity: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spawned {
    pub pid: u32,
    /// `None` when no affinity was requested.
    pub affinity_error: Option<String>,
}

/// The mask covering every logical processor on this machine.
pub fn all_processors_mask() -> u64 {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(64);
    if n == 64 {
        u64::MAX
    } else {
        (1u64 << n) - 1
    }
}

/// Validates an affinity mask against this machine.
pub fn validate_affinity(mask: u64) -> Result<u64, String> {
    let valid = mask & all_processors_mask();
    if valid == 0 {
        Err("Choose at least one processor.".into())
    } else {
        Ok(valid)
    }
}

#[cfg(windows)]
pub fn spawn(req: &SpawnRequest) -> std::io::Result<Spawned> {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Threading::SetProcessAffinityMask;

    let mut cmd = Command::new(&req.exe);
    cmd.current_dir(&req.working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(req.priority.creation_flag());
    if let Some(arg) = &req.argument {
        cmd.arg(arg);
    }

    let child = cmd.spawn()?;
    let pid = child.id();

    let affinity_error = req.affinity.map(|mask| {
        let handle = HANDLE(child.as_raw_handle());
        match unsafe { SetProcessAffinityMask(handle, mask as usize) } {
            Ok(()) => String::new(),
            Err(e) => e.message(),
        }
    });

    // Dropping `Child` closes the process and thread handles without waiting or killing.
    drop(child);

    Ok(Spawned { pid, affinity_error: affinity_error.filter(|e| !e.is_empty()) })
}

#[cfg(not(windows))]
pub fn spawn(_req: &SpawnRequest) -> std::io::Result<Spawned> {
    Err(std::io::Error::other("Roblox can only be launched on Windows"))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::roblox::process;

    fn system32() -> PathBuf {
        PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into())).join("System32")
    }

    /// Spawns a real, harmless process through the same code path Roblox uses.
    #[test]
    fn spawns_with_priority_and_releases_the_process() {
        let req = SpawnRequest {
            exe: system32().join("cmd.exe"),
            working_dir: system32(),
            argument: Some("/c exit 0".into()),
            priority: Priority::AboveNormal,
            affinity: Some(1),
        };
        // cmd.exe treats the single argument as its command line.
        let s = spawn(&req).unwrap();
        assert!(s.pid > 0);
        assert_eq!(s.affinity_error, None);

        // The child exits by itself; we hold no handle, so wait via snapshots.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while process::snapshot().unwrap().iter().any(|p| p.pid == s.pid) {
            assert!(std::time::Instant::now() < deadline, "child should exit on its own");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn missing_executable_is_an_error() {
        let req = SpawnRequest {
            exe: PathBuf::from(r"C:\definitely\not\here\RobloxPlayerBeta.exe"),
            working_dir: system32(),
            argument: None,
            priority: Priority::Normal,
            affinity: None,
        };
        assert_eq!(spawn(&req).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn affinity_masks_are_validated() {
        assert_eq!(validate_affinity(0b1).unwrap(), 1);
        assert!(validate_affinity(0).is_err());
        assert_eq!(validate_affinity(u64::MAX).unwrap(), all_processors_mask());
    }
}
