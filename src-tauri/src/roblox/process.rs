//! Process enumeration without opening process handles.
//!
//! Roblox's anti-cheat reacts to tools that hold handles to the game process,
//! so Vesper never opens one for monitoring. `NtQuerySystemInformation`
//! returns CPU time and memory for every process in a single call, the same
//! data Task Manager's process list is built from.

use std::io;

pub const PLAYER_PROCESS: &str = "RobloxPlayerBeta.exe";
pub const INSTALLER_PROCESS: &str = "RobloxPlayerInstaller.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub threads: u32,
    /// Physical memory in use, bytes.
    pub working_set: u64,
    /// Private committed memory, bytes.
    pub private_bytes: u64,
    /// User + kernel CPU time, 100 ns units.
    pub cpu_time: u64,
    /// Process start time as a Windows FILETIME value.
    pub create_time: i64,
}

/// All processes running now.
pub fn snapshot() -> io::Result<Vec<ProcessSample>> {
    imp::snapshot()
}

pub fn named<'a>(all: &'a [ProcessSample], name: &str) -> impl Iterator<Item = &'a ProcessSample> {
    let name = name.to_owned();
    all.iter().filter(move |p| p.name.eq_ignore_ascii_case(&name))
}

/// Converts a Windows FILETIME (100 ns since 1601) to `SystemTime`.
pub fn filetime_to_system(ft: i64) -> Option<std::time::SystemTime> {
    const UNIX_EPOCH_AS_FILETIME: i64 = 116_444_736_000_000_000;
    let since_unix = ft.checked_sub(UNIX_EPOCH_AS_FILETIME)?;
    let since_unix = u64::try_from(since_unix).ok()?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_nanos(since_unix * 100))
}

/// When a process started, from a snapshot (no handle needed).
pub fn start_time(pid: u32) -> Option<std::time::SystemTime> {
    let all = snapshot().ok()?;
    let p = all.iter().find(|p| p.pid == pid)?;
    filetime_to_system(p.create_time)
}

/// PIDs of running Roblox player processes.
pub fn roblox_pids() -> Vec<u32> {
    snapshot().map(|all| named(&all, PLAYER_PROCESS).map(|p| p.pid).collect()).unwrap_or_default()
}

#[cfg(windows)]
mod imp {
    use super::ProcessSample;
    use std::io;
    use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};

    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *const u16,
    }

    /// x64 layout of SYSTEM_PROCESS_INFORMATION including the fields the
    /// public header calls `Reserved1` (CPU times live there).
    #[repr(C)]
    struct SystemProcessInfo {
        next_entry_offset: u32,
        number_of_threads: u32,
        working_set_private_size: i64,
        hard_fault_count: u32,
        number_of_threads_high_watermark: u32,
        cycle_time: u64,
        create_time: i64,
        user_time: i64,
        kernel_time: i64,
        image_name: UnicodeString,
        base_priority: i32,
        unique_process_id: usize,
        inherited_from_unique_process_id: usize,
        handle_count: u32,
        session_id: u32,
        unique_process_key: usize,
        peak_virtual_size: usize,
        virtual_size: usize,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_page_count: usize,
    }

    const _: () = assert!(std::mem::size_of::<usize>() == 8, "layout is defined for 64-bit Windows");
    const _: () = assert!(std::mem::offset_of!(SystemProcessInfo, image_name) == 56);
    const _: () = assert!(std::mem::offset_of!(SystemProcessInfo, unique_process_id) == 80);
    const _: () = assert!(std::mem::offset_of!(SystemProcessInfo, working_set_size) == 144);
    const _: () = assert!(std::mem::offset_of!(SystemProcessInfo, private_page_count) == 200);

    pub fn snapshot() -> io::Result<Vec<ProcessSample>> {
        // u64 storage keeps the buffer 8-byte aligned.
        let mut buf: Vec<u64> = vec![0; 64 * 1024];
        loop {
            let bytes = (buf.len() * 8) as u32;
            let mut needed = 0u32;
            let status = unsafe {
                NtQuerySystemInformation(SystemProcessInformation, buf.as_mut_ptr().cast(), bytes, &mut needed)
            };
            if status.0 == STATUS_INFO_LENGTH_MISMATCH {
                // Processes can start between calls; leave headroom.
                buf = vec![0; (needed as usize / 8) + 16 * 1024];
                continue;
            }
            if status.0 < 0 {
                return Err(io::Error::other(format!("NtQuerySystemInformation failed: 0x{:08X}", status.0)));
            }
            return Ok(parse(&buf, bytes as usize));
        }
    }

    fn parse(buf: &[u64], len: usize) -> Vec<ProcessSample> {
        let base = buf.as_ptr().cast::<u8>();
        let mut out = Vec::with_capacity(400);
        let mut offset = 0usize;
        loop {
            if offset + std::mem::size_of::<SystemProcessInfo>() > len {
                break;
            }
            // SAFETY: offset is within the buffer the kernel filled, and entries are 8-byte aligned.
            let info = unsafe { &*(base.add(offset) as *const SystemProcessInfo) };
            let name = if info.image_name.buffer.is_null() || info.image_name.length == 0 {
                if info.unique_process_id == 0 {
                    "System Idle Process".to_owned()
                } else {
                    "System".to_owned()
                }
            } else {
                // SAFETY: the kernel points ImageName into the same buffer.
                let chars = unsafe {
                    std::slice::from_raw_parts(info.image_name.buffer, (info.image_name.length / 2) as usize)
                };
                String::from_utf16_lossy(chars)
            };
            out.push(ProcessSample {
                pid: info.unique_process_id as u32,
                parent_pid: info.inherited_from_unique_process_id as u32,
                name,
                threads: info.number_of_threads,
                working_set: info.working_set_size as u64,
                private_bytes: info.private_page_count as u64,
                cpu_time: (info.user_time.max(0) + info.kernel_time.max(0)) as u64,
                create_time: info.create_time,
            });
            if info.next_entry_offset == 0 {
                break;
            }
            offset += info.next_entry_offset as usize;
        }
        out
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn snapshot() -> std::io::Result<Vec<super::ProcessSample>> {
        Ok(Vec::new())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_this_process_with_real_numbers() {
        let all = snapshot().unwrap();
        assert!(all.len() > 10, "a Windows system runs many processes");
        let me = all.iter().find(|p| p.pid == std::process::id()).expect("current process listed");
        let exe = std::env::current_exe().unwrap();
        assert!(exe.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case(&me.name));
        assert!(me.working_set > 1024 * 1024, "uses at least 1 MB");
        assert!(me.threads >= 1);
        assert!(me.create_time > 0);
    }

    #[test]
    fn cpu_time_increases_with_work() {
        let before = snapshot().unwrap().into_iter().find(|p| p.pid == std::process::id()).unwrap().cpu_time;
        let mut x = 0u64;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(150) {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        std::hint::black_box(x);
        let after = snapshot().unwrap().into_iter().find(|p| p.pid == std::process::id()).unwrap().cpu_time;
        assert!(after > before, "{after} > {before}");
    }

    #[test]
    fn start_time_of_this_process_is_recent() {
        let t = start_time(std::process::id()).expect("own start time");
        let age = std::time::SystemTime::now().duration_since(t).expect("in the past");
        assert!(age < std::time::Duration::from_secs(600), "{age:?}");
        assert!(filetime_to_system(0).is_none(), "before 1970 is rejected");
    }

    #[test]
    fn name_lookup_is_case_insensitive() {
        let all = vec![ProcessSample {
            pid: 1,
            parent_pid: 0,
            name: "RobloxPlayerBeta.exe".into(),
            threads: 1,
            working_set: 0,
            private_bytes: 0,
            cpu_time: 0,
            create_time: 0,
        }];
        assert_eq!(named(&all, "robloxplayerbeta.EXE").count(), 1);
    }
}
