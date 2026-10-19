use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub dedicated_memory: u64,
    pub vendor_id: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub platform: &'static str,
    pub os: String,
    pub cpu: String,
    pub logical_processors: usize,
    pub memory_total: u64,
    pub gpus: Vec<GpuInfo>,
    pub display_hz: Option<u32>,
}

pub fn info() -> SystemInfo {
    SystemInfo {
        platform: std::env::consts::OS,
        os: os_name(),
        cpu: cpu_name().unwrap_or_else(|| "Unknown processor".into()),
        logical_processors: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        memory_total: memory().map(|m| m.total).unwrap_or(0),
        gpus: gpus(),
        display_hz: display_refresh_hz(),
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub total: u64,
    pub available: u64,
}

#[cfg(windows)]
fn hklm(path: &str, name: &str) -> Option<String> {
    windows_registry::LOCAL_MACHINE.open(path).ok()?.get_string(name).ok()
}

#[cfg(windows)]
fn hklm_u32(path: &str, name: &str) -> Option<u32> {
    windows_registry::LOCAL_MACHINE.open(path).ok()?.get_u32(name).ok()
}

#[cfg(windows)]
pub fn os_name() -> String {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build: u32 = hklm(KEY, "CurrentBuildNumber").and_then(|b| b.parse().ok()).unwrap_or(0);
    let mut product = hklm(KEY, "ProductName").unwrap_or_else(|| "Windows".into());
    if build >= 22000 {
        product = product.replace("Windows 10", "Windows 11");
    }
    let display = hklm(KEY, "DisplayVersion").map(|d| format!(" {d}")).unwrap_or_default();
    let ubr = hklm_u32(KEY, "UBR").map(|u| format!(".{u}")).unwrap_or_default();
    format!("{product}{display} (build {build}{ubr})")
}

#[cfg(target_os = "macos")]
pub fn os_name() -> String {
    let version = mac::sysctl_string("kern.osproductversion").unwrap_or_default();
    let build = mac::sysctl_string("kern.osversion").map(|b| format!(" (build {b})")).unwrap_or_default();
    format!("macOS {version}{build}").replace("  ", " ")
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn os_name() -> String {
    std::env::consts::OS.into()
}

#[cfg(windows)]
pub fn cpu_name() -> Option<String> {
    hklm(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString").map(|s| s.trim().to_owned())
}

#[cfg(target_os = "macos")]
pub fn cpu_name() -> Option<String> {
    mac::sysctl_string("machdep.cpu.brand_string").map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn cpu_name() -> Option<String> {
    None
}

#[cfg(windows)]
pub fn memory() -> Option<Memory> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
    Some(Memory { total: m.ullTotalPhys, available: m.ullAvailPhys })
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub fn memory() -> Option<Memory> {
    let total = mac::sysctl_u64("hw.memsize")?;
    let page = mac::sysctl_u64("hw.pagesize").unwrap_or(16384);
    let mut stats = std::mem::MaybeUninit::<libc::vm_statistics64>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    let r = unsafe {
        libc::host_statistics64(libc::mach_host_self(), libc::HOST_VM_INFO64, stats.as_mut_ptr().cast(), &mut count)
    };
    if r != 0 {
        return Some(Memory { total, available: total });
    }
    let v = unsafe { stats.assume_init() };
    let app = (v.internal_page_count as u64).saturating_sub(v.purgeable_count as u64);
    let used = (app + v.wire_count as u64 + v.compressor_page_count as u64) * page;
    Some(Memory { total, available: total.saturating_sub(used) })
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn memory() -> Option<Memory> {
    None
}

#[cfg(windows)]
pub fn gpus() -> Vec<GpuInfo> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else { return Vec::new() };
    let mut out = Vec::new();
    for i in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else { break };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else { continue };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let len = desc.Description.iter().position(|c| *c == 0).unwrap_or(desc.Description.len());
        let name = String::from_utf16_lossy(&desc.Description[..len]).trim().to_owned();
        if out.iter().any(|g: &GpuInfo| g.name == name && g.dedicated_memory == desc.DedicatedVideoMemory as u64) {
            continue;
        }
        out.push(GpuInfo { name, dedicated_memory: desc.DedicatedVideoMemory as u64, vendor_id: desc.VendorId });
    }
    out
}

#[cfg(target_os = "macos")]
pub fn gpus() -> Vec<GpuInfo> {
    let chip = cpu_name().unwrap_or_default();
    if chip.starts_with("Apple") {
        return vec![GpuInfo { name: format!("{chip} GPU"), dedicated_memory: 0, vendor_id: 0x106B }];
    }
    let out = std::process::Command::new("/usr/sbin/system_profiler")
        .args(["-json", "-detailLevel", "mini", "SPDisplaysDataType"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).map(|j| mac::parse_displays(&j)).unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn gpus() -> Vec<GpuInfo> {
    Vec::new()
}

#[cfg(windows)]
pub fn display_refresh_hz() -> Option<u32> {
    use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
    let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
    let ok = unsafe { EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut mode) }.as_bool();
    (ok && mode.dmDisplayFrequency > 1).then_some(mode.dmDisplayFrequency)
}

#[cfg(target_os = "macos")]
pub fn display_refresh_hz() -> Option<u32> {
    let hz = mac::main_display_hz().filter(|hz| *hz > 1.0).or_else(mac::main_screen_max_fps)?;
    Some(hz.round() as u32).filter(|hz| *hz > 1)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn display_refresh_hz() -> Option<u32> {
    None
}

#[derive(Default)]
pub struct CpuMeter {
    #[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
    last: Option<(u64, u64)>,
}

impl CpuMeter {
    #[cfg(windows)]
    pub fn sample(&mut self) -> Option<f64> {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::GetSystemTimes;
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
        let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        let busy_total = (ft(kernel) + ft(user), ft(idle));
        let prev = self.last.replace(busy_total)?;
        let total = busy_total.0.saturating_sub(prev.0);
        let idle_delta = busy_total.1.saturating_sub(prev.1);
        (total > 0).then(|| (100.0 * (total.saturating_sub(idle_delta)) as f64 / total as f64).clamp(0.0, 100.0))
    }

    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    pub fn sample(&mut self) -> Option<f64> {
        let mut load = std::mem::MaybeUninit::<libc::host_cpu_load_info>::zeroed();
        let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
        let r = unsafe {
            libc::host_statistics(
                libc::mach_host_self(),
                libc::HOST_CPU_LOAD_INFO,
                load.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if r != 0 {
            return None;
        }
        let ticks = unsafe { load.assume_init() }.cpu_ticks;
        let idle = ticks[libc::CPU_STATE_IDLE as usize] as u64;
        let total: u64 = ticks.iter().map(|t| *t as u64).sum();
        let prev = self.last.replace((total, idle))?;
        let total_delta = total.saturating_sub(prev.0);
        let idle_delta = idle.saturating_sub(prev.1);
        (total_delta > 0)
            .then(|| (100.0 * total_delta.saturating_sub(idle_delta) as f64 / total_delta as f64).clamp(0.0, 100.0))
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    pub fn sample(&mut self) -> Option<f64> {
        None
    }
}

#[cfg(any(target_os = "macos", test))]
mod mac {
    #[cfg(target_os = "macos")]
    pub fn sysctl_string(name: &str) -> Option<String> {
        let name = std::ffi::CString::new(name).ok()?;
        let mut len = 0usize;
        if unsafe { libc::sysctlbyname(name.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0
            || len == 0
        {
            return None;
        }
        let mut buf = vec![0u8; len];
        if unsafe { libc::sysctlbyname(name.as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) } != 0
        {
            return None;
        }
        buf.truncate(len);
        let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
        String::from_utf8(buf[..end].to_vec()).ok()
    }

    #[cfg(target_os = "macos")]
    pub fn sysctl_u64(name: &str) -> Option<u64> {
        let name = std::ffi::CString::new(name).ok()?;
        let mut value = 0u64;
        let mut len = std::mem::size_of::<u64>();
        let r = unsafe {
            libc::sysctlbyname(name.as_ptr(), (&mut value as *mut u64).cast(), &mut len, std::ptr::null_mut(), 0)
        };
        (r == 0).then_some(value)
    }

    #[cfg(target_os = "macos")]
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayCopyDisplayMode(display: u32) -> *mut std::ffi::c_void;
        fn CGDisplayModeGetRefreshRate(mode: *mut std::ffi::c_void) -> f64;
        fn CGDisplayModeRelease(mode: *mut std::ffi::c_void);
    }

    #[cfg(target_os = "macos")]
    pub fn main_display_hz() -> Option<f64> {
        unsafe {
            let mode = CGDisplayCopyDisplayMode(CGMainDisplayID());
            if mode.is_null() {
                return None;
            }
            let hz = CGDisplayModeGetRefreshRate(mode);
            CGDisplayModeRelease(mode);
            Some(hz)
        }
    }

    #[cfg(target_os = "macos")]
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    #[cfg(target_os = "macos")]
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn objc_msgSend();
    }

    #[cfg(target_os = "macos")]
    pub fn main_screen_max_fps() -> Option<f64> {
        type SendId = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        type SendBool =
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void) -> bool;
        type SendInt = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> isize;
        unsafe {
            let class = objc_getClass(c"NSScreen".as_ptr());
            if class.is_null() {
                return None;
            }
            let msg_send = objc_msgSend as unsafe extern "C" fn();
            let send_id = std::mem::transmute::<unsafe extern "C" fn(), SendId>(msg_send);
            let send_bool = std::mem::transmute::<unsafe extern "C" fn(), SendBool>(msg_send);
            let send_int = std::mem::transmute::<unsafe extern "C" fn(), SendInt>(msg_send);
            let screen = send_id(class, sel_registerName(c"mainScreen".as_ptr()));
            if screen.is_null() {
                return None;
            }
            let fps_sel = sel_registerName(c"maximumFramesPerSecond".as_ptr());
            if !send_bool(screen, sel_registerName(c"respondsToSelector:".as_ptr()), fps_sel) {
                return None;
            }
            let fps = send_int(screen, fps_sel);
            (fps > 0).then_some(fps as f64)
        }
    }

    pub fn parse_displays(json: &str) -> Vec<super::GpuInfo> {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
        let Some(items) = v.get("SPDisplaysDataType").and_then(|d| d.as_array()) else { return Vec::new() };
        items
            .iter()
            .filter_map(|g| {
                let name = g.get("sppci_model").or_else(|| g.get("_name"))?.as_str()?.trim().to_owned();
                let vram = ["spdisplays_vram", "spdisplays_vram_shared", "_spdisplays_vram"]
                    .iter()
                    .find_map(|k| g.get(*k).and_then(|v| v.as_str()))
                    .and_then(parse_size)
                    .unwrap_or(0);
                let vendor = g
                    .get("spdisplays_vendor-id")
                    .and_then(|v| v.as_str())
                    .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0);
                Some(super::GpuInfo { name, dedicated_memory: vram, vendor_id: vendor })
            })
            .collect()
    }

    fn parse_size(s: &str) -> Option<u64> {
        let mut parts = s.split_whitespace();
        let n: u64 = parts.next()?.parse().ok()?;
        let unit = match parts.next()?.to_ascii_uppercase().as_str() {
            "GB" => 1 << 30,
            "MB" => 1 << 20,
            _ => return None,
        };
        Some(n * unit)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn graphics_are_read_from_system_profiler() {
            let json = r#"{"SPDisplaysDataType":[
                {"_name":"kHW_IntelUHDGraphics630Item","sppci_model":"Intel UHD Graphics 630","spdisplays_vram_shared":"1536 MB","spdisplays_vendor-id":"0x8086"},
                {"_name":"AMD Radeon Pro 5500M","sppci_model":"AMD Radeon Pro 5500M","spdisplays_vram":"8 GB","spdisplays_vendor-id":"0x1002"}
            ]}"#;
            let g = super::parse_displays(json);
            assert_eq!(g.len(), 2);
            assert_eq!(g[0].name, "Intel UHD Graphics 630");
            assert_eq!(g[0].dedicated_memory, 1536 << 20);
            assert_eq!(g[1].dedicated_memory, 8 << 30);
            assert_eq!(g[1].vendor_id, 0x1002);
            assert!(super::parse_displays("not json").is_empty());
        }
    }
}

#[cfg(all(test, any(windows, target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn reads_real_hardware_facts() {
        let i = info();
        assert!(i.os.starts_with(if cfg!(windows) { "Windows" } else { "macOS" }), "{}", i.os);
        assert!(i.os.contains("build"), "{}", i.os);
        assert!(i.logical_processors >= 1);
        assert!(i.memory_total > 512 * 1024 * 1024, "at least 512 MB of RAM");
        assert!(!i.cpu.is_empty());
    }

    #[test]
    fn memory_available_is_below_total() {
        let m = memory().unwrap();
        assert!(m.available > 0 && m.available <= m.total);
    }

    #[test]
    fn cpu_meter_needs_two_samples_and_stays_in_range() {
        let mut m = CpuMeter::default();
        assert!(m.sample().is_none());
        std::thread::sleep(std::time::Duration::from_millis(120));
        let v = m.sample().unwrap();
        assert!((0.0..=100.0).contains(&v), "{v}");
    }
}
