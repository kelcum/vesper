//! Static hardware facts and whole-system counters.

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
    pub os: String,
    pub cpu: String,
    pub logical_processors: usize,
    pub memory_total: u64,
    pub gpus: Vec<GpuInfo>,
    pub display_hz: Option<u32>,
}

pub fn info() -> SystemInfo {
    SystemInfo {
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

/// "Windows 11 Home 25H2 (build 26200.1234)".
#[cfg(windows)]
pub fn os_name() -> String {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build: u32 = hklm(KEY, "CurrentBuildNumber").and_then(|b| b.parse().ok()).unwrap_or(0);
    let mut product = hklm(KEY, "ProductName").unwrap_or_else(|| "Windows".into());
    // Windows 11 still reports "Windows 10" in ProductName.
    if build >= 22000 {
        product = product.replace("Windows 10", "Windows 11");
    }
    let display = hklm(KEY, "DisplayVersion").map(|d| format!(" {d}")).unwrap_or_default();
    let ubr = hklm_u32(KEY, "UBR").map(|u| format!(".{u}")).unwrap_or_default();
    format!("{product}{display} (build {build}{ubr})")
}

#[cfg(not(windows))]
pub fn os_name() -> String {
    std::env::consts::OS.into()
}

#[cfg(windows)]
pub fn cpu_name() -> Option<String> {
    hklm(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString").map(|s| s.trim().to_owned())
}

#[cfg(not(windows))]
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

#[cfg(not(windows))]
pub fn memory() -> Option<Memory> {
    None
}

/// Hardware graphics adapters (software rasterisers excluded).
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

#[cfg(not(windows))]
pub fn gpus() -> Vec<GpuInfo> {
    Vec::new()
}

/// Refresh rate of the primary display.
#[cfg(windows)]
pub fn display_refresh_hz() -> Option<u32> {
    use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
    let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
    let ok = unsafe { EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut mode) }.as_bool();
    // 0 and 1 mean "hardware default".
    (ok && mode.dmDisplayFrequency > 1).then_some(mode.dmDisplayFrequency)
}

#[cfg(not(windows))]
pub fn display_refresh_hz() -> Option<u32> {
    None
}

/// Whole-system CPU usage from consecutive `GetSystemTimes` readings.
#[derive(Default)]
pub struct CpuMeter {
    last: Option<(u64, u64)>,
}

impl CpuMeter {
    /// Percentage since the previous call (`None` on the first call).
    #[cfg(windows)]
    pub fn sample(&mut self) -> Option<f64> {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::GetSystemTimes;
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
        let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        // Kernel time includes idle time.
        let busy_total = (ft(kernel) + ft(user), ft(idle));
        let prev = self.last.replace(busy_total)?;
        let total = busy_total.0.saturating_sub(prev.0);
        let idle_delta = busy_total.1.saturating_sub(prev.1);
        (total > 0).then(|| (100.0 * (total.saturating_sub(idle_delta)) as f64 / total as f64).clamp(0.0, 100.0))
    }

    #[cfg(not(windows))]
    pub fn sample(&mut self) -> Option<f64> {
        None
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn reads_real_hardware_facts() {
        let i = info();
        assert!(i.os.starts_with("Windows"), "{}", i.os);
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
