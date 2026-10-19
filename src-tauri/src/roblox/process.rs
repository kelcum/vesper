use std::io;

#[cfg(not(target_os = "macos"))]
pub const PLAYER_PROCESS: &str = "RobloxPlayerBeta.exe";
#[cfg(not(target_os = "macos"))]
pub const INSTALLER_PROCESS: &str = "RobloxPlayerInstaller.exe";
#[cfg(target_os = "macos")]
pub const PLAYER_PROCESS: &str = "RobloxPlayer";
#[cfg(target_os = "macos")]
pub const INSTALLER_PROCESS: &str = "RobloxPlayerInstaller";
#[cfg(windows)]
pub const VESPER_PROCESS: &str = "Vesper.exe";
#[cfg(not(windows))]
pub const VESPER_PROCESS: &str = "Vesper";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub threads: u32,
    pub working_set: u64,
    pub private_bytes: u64,
    pub cpu_time: u64,
    pub create_time: i64,
}

pub fn snapshot() -> io::Result<Vec<ProcessSample>> {
    imp::snapshot()
}

pub fn named<'a>(all: &'a [ProcessSample], name: &str) -> impl Iterator<Item = &'a ProcessSample> {
    let name = name.to_owned();
    all.iter().filter(move |p| p.name.eq_ignore_ascii_case(&name))
}

const UNIX_EPOCH_AS_FILETIME: i64 = 116_444_736_000_000_000;

pub fn filetime_to_system(ft: i64) -> Option<std::time::SystemTime> {
    let since_unix = ft.checked_sub(UNIX_EPOCH_AS_FILETIME)?;
    let since_unix = u64::try_from(since_unix).ok()?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_nanos(since_unix * 100))
}

pub fn start_time(pid: u32) -> Option<std::time::SystemTime> {
    let all = snapshot().ok()?;
    let p = all.iter().find(|p| p.pid == pid)?;
    filetime_to_system(p.create_time)
}

pub fn roblox_pids() -> Vec<u32> {
    snapshot().map(|all| named(&all, PLAYER_PROCESS).map(|p| p.pid).collect()).unwrap_or_default()
}

pub fn force_quit_players() -> io::Result<()> {
    use std::time::{Duration, Instant};
    let pids = roblox_pids();
    let mut failed = None;
    for pid in &pids {
        if let Err(e) = imp::terminate(*pid) {
            failed.get_or_insert(e);
        }
    }
    let t = Instant::now();
    loop {
        let now = roblox_pids();
        if !pids.iter().any(|p| now.contains(p)) {
            return Ok(());
        }
        if t.elapsed() > Duration::from_secs(8) {
            return Err(failed.unwrap_or_else(|| io::Error::other("Roblox is still running after being ended")));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(windows)]
mod imp {
    use super::ProcessSample;
    use std::io;

    pub fn terminate(pid: u32) -> io::Result<()> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
        unsafe {
            let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) else {
                return Ok(());
            };
            let r = TerminateProcess(h, 1);
            let _ = CloseHandle(h);
            r.map_err(|e| io::Error::other(e.message()))
        }
    }

    use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};

    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *const u16,
    }

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
        let mut buf: Vec<u64> = vec![0; 64 * 1024];
        loop {
            let bytes = (buf.len() * 8) as u32;
            let mut needed = 0u32;
            let status = unsafe {
                NtQuerySystemInformation(SystemProcessInformation, buf.as_mut_ptr().cast(), bytes, &mut needed)
            };
            if status.0 == STATUS_INFO_LENGTH_MISMATCH {
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
            let info = unsafe { &*(base.add(offset) as *const SystemProcessInfo) };
            let name = if info.image_name.buffer.is_null() || info.image_name.length == 0 {
                if info.unique_process_id == 0 {
                    "System Idle Process".to_owned()
                } else {
                    "System".to_owned()
                }
            } else {
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

#[cfg(target_os = "macos")]
#[allow(deprecated)]
mod imp {
    use super::{ProcessSample, UNIX_EPOCH_AS_FILETIME};
    use std::ffi::CStr;
    use std::io;
    use std::mem::{size_of, MaybeUninit};
    use std::sync::LazyLock;

    static TIMEBASE: LazyLock<(u64, u64)> = LazyLock::new(|| {
        let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
        if unsafe { libc::mach_timebase_info(&mut info) } == 0 && info.denom != 0 {
            (info.numer as u64, info.denom as u64)
        } else {
            (1, 1)
        }
    });

    pub fn terminate(pid: u32) -> io::Result<()> {
        if unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(e)
        }
    }

    pub fn snapshot() -> io::Result<Vec<ProcessSample>> {
        let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        if n <= 0 {
            return Err(io::Error::last_os_error());
        }
        let mut pids = vec![0 as libc::pid_t; n as usize + 64];
        let bytes = (pids.len() * size_of::<libc::pid_t>()) as libc::c_int;
        let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        pids.truncate(n as usize);
        Ok(pids.into_iter().filter(|p| *p > 0).filter_map(sample).collect())
    }

    fn sample(pid: libc::pid_t) -> Option<ProcessSample> {
        let bsd: libc::proc_bsdinfo = pidinfo(pid, libc::PROC_PIDTBSDINFO)?;
        let task: libc::proc_taskinfo = pidinfo(pid, libc::PROC_PIDTASKINFO)?;
        let name = c_name(&bsd.pbi_name).filter(|n| !n.is_empty()).or_else(|| c_name(&bsd.pbi_comm))?;
        let (numer, denom) = *TIMEBASE;
        let cpu_ns = (task.pti_total_user + task.pti_total_system) as u128 * numer as u128 / denom as u128;
        let started = bsd.pbi_start_tvsec as i64 * 10_000_000 + bsd.pbi_start_tvusec as i64 * 10;
        Some(ProcessSample {
            pid: pid as u32,
            parent_pid: bsd.pbi_ppid,
            name,
            threads: task.pti_threadnum.max(0) as u32,
            working_set: task.pti_resident_size,
            private_bytes: footprint(pid).unwrap_or(task.pti_resident_size),
            cpu_time: (cpu_ns / 100) as u64,
            create_time: started + UNIX_EPOCH_AS_FILETIME,
        })
    }

    fn pidinfo<T>(pid: libc::pid_t, flavor: libc::c_int) -> Option<T> {
        let mut out = MaybeUninit::<T>::zeroed();
        let size = size_of::<T>() as libc::c_int;
        let n = unsafe { libc::proc_pidinfo(pid, flavor, 0, out.as_mut_ptr().cast(), size) };
        (n == size).then(|| unsafe { out.assume_init() })
    }

    fn footprint(pid: libc::pid_t) -> Option<u64> {
        let mut info = MaybeUninit::<libc::rusage_info_v2>::zeroed();
        let r = unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, info.as_mut_ptr().cast()) };
        (r == 0).then(|| unsafe { info.assume_init() }.ri_phys_footprint)
    }

    fn c_name(chars: &[libc::c_char]) -> Option<String> {
        let bytes: Vec<u8> = chars.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
        let mut with_nul = bytes;
        with_nul.push(0);
        CStr::from_bytes_with_nul(&with_nul).ok().map(|c| c.to_string_lossy().into_owned())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn terminate(_pid: u32) -> std::io::Result<()> {
        Ok(())
    }
    pub fn snapshot() -> std::io::Result<Vec<super::ProcessSample>> {
        Ok(Vec::new())
    }
}

#[cfg(all(test, any(windows, target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_this_process_with_real_numbers() {
        let all = snapshot().unwrap();
        assert!(all.len() > 10, "a desktop system runs many processes");
        let me = all.iter().find(|p| p.pid == std::process::id()).expect("current process listed");
        let exe = std::env::current_exe().unwrap();
        let file = exe.file_name().unwrap().to_string_lossy().into_owned();
        assert!(file.to_lowercase().starts_with(&me.name.to_lowercase()), "{file} vs {}", me.name);
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
