use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuSample {
    pub system_percent: Option<f64>,
    pub per_pid_percent: HashMap<u32, f64>,
    pub per_pid_dedicated: HashMap<u32, u64>,
}

pub fn parse_engine_instance(name: &str) -> Option<(u32, String)> {
    let pid = name.strip_prefix("pid_")?.split('_').next()?.parse().ok()?;
    let engtype = name.rsplit_once("engtype_").map(|(_, t)| t.to_owned()).unwrap_or_default();
    Some((pid, engtype))
}

pub fn aggregate(engines: &[(String, f64)], memory: &[(String, f64)]) -> GpuSample {
    let mut per_pid_type: HashMap<(u32, String), f64> = HashMap::new();
    for (name, value) in engines {
        if let Some((pid, t)) = parse_engine_instance(name) {
            *per_pid_type.entry((pid, t)).or_default() += value.max(0.0);
        }
    }
    let mut per_pid_percent: HashMap<u32, f64> = HashMap::new();
    let mut per_type_total: HashMap<String, f64> = HashMap::new();
    for ((pid, t), v) in &per_pid_type {
        let e = per_pid_percent.entry(*pid).or_default();
        *e = e.max(*v);
        *per_type_total.entry(t.clone()).or_default() += v;
    }
    for v in per_pid_percent.values_mut() {
        *v = v.min(100.0);
    }
    let system_percent = (!engines.is_empty()).then(|| per_type_total.values().cloned().fold(0.0, f64::max).min(100.0));

    let mut per_pid_dedicated: HashMap<u32, u64> = HashMap::new();
    for (name, value) in memory {
        if let Some((pid, _)) = parse_engine_instance(name) {
            *per_pid_dedicated.entry(pid).or_default() += value.max(0.0) as u64;
        }
    }
    GpuSample { system_percent, per_pid_percent, per_pid_dedicated }
}

#[cfg(windows)]
mod imp {
    use super::{aggregate, GpuSample};
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    };

    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    pub struct GpuCounters {
        query: PDH_HQUERY,
        engine: PDH_HCOUNTER,
        memory: Option<PDH_HCOUNTER>,
    }

    unsafe impl Send for GpuCounters {}

    impl GpuCounters {
        pub fn open() -> Result<Self, String> {
            unsafe {
                let mut query = PDH_HQUERY::default();
                let s = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
                if s != 0 {
                    return Err(format!("PdhOpenQuery failed (0x{s:08X})"));
                }
                let mut engine = PDH_HCOUNTER::default();
                let path = HSTRING::from(r"\GPU Engine(*)\Utilization Percentage");
                let s = PdhAddEnglishCounterW(query, &path, 0, &mut engine);
                if s != 0 {
                    PdhCloseQuery(query);
                    return Err(format!("GPU engine counters are unavailable on this system (0x{s:08X})"));
                }
                let mut mem = PDH_HCOUNTER::default();
                let mem_path = HSTRING::from(r"\GPU Process Memory(*)\Dedicated Usage");
                let memory = (PdhAddEnglishCounterW(query, &mem_path, 0, &mut mem) == 0).then_some(mem);
                PdhCollectQueryData(query);
                Ok(Self { query, engine, memory })
            }
        }

        pub fn sample(&mut self) -> GpuSample {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return GpuSample::default();
                }
                let engines = read_array(self.engine);
                let memory = self.memory.map(|c| read_array(c)).unwrap_or_default();
                aggregate(&engines, &memory)
            }
        }
    }

    impl Drop for GpuCounters {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }

    unsafe fn read_array(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
        let mut size = 0u32;
        let mut count = 0u32;
        let s = PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
        if s != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        let mut buf: Vec<u64> = vec![0; (size as usize).div_ceil(8)];
        let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
            return Vec::new();
        }
        let slice = std::slice::from_raw_parts(items, count as usize);
        slice
            .iter()
            .filter(|i| i.FmtValue.CStatus == 0)
            .filter_map(|i| {
                let name = i.szName.to_string().ok()?;
                Some((name, i.FmtValue.Anonymous.doubleValue))
            })
            .collect()
    }
}

#[cfg(windows)]
pub use imp::GpuCounters;

#[cfg(not(windows))]
pub struct GpuCounters;

#[cfg(not(windows))]
impl GpuCounters {
    pub fn open() -> Result<Self, String> {
        Err(if cfg!(target_os = "macos") {
            "Vesper can't read GPU counters on macOS".into()
        } else {
            "GPU counters are only available on Windows".into()
        })
    }
    pub fn sample(&mut self) -> GpuSample {
        GpuSample::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_parse() {
        assert_eq!(
            parse_engine_instance("pid_1234_luid_0x00000000_0x0000D1C3_phys_0_eng_0_engtype_3D"),
            Some((1234, "3D".to_string()))
        );
        assert_eq!(
            parse_engine_instance("pid_99_luid_0x00000000_0x0000D1C3_phys_0_eng_8_engtype_VideoDecode"),
            Some((99, "VideoDecode".to_string()))
        );
        assert_eq!(parse_engine_instance("_Total"), None);
    }

    #[test]
    fn aggregation_matches_task_manager_rules() {
        let engines = vec![
            ("pid_1_luid_a_phys_0_eng_0_engtype_3D".to_string(), 30.0),
            ("pid_1_luid_a_phys_0_eng_1_engtype_3D".to_string(), 10.0),
            ("pid_1_luid_a_phys_0_eng_2_engtype_Copy".to_string(), 5.0),
            ("pid_2_luid_a_phys_0_eng_0_engtype_3D".to_string(), 20.0),
            ("pid_2_luid_a_phys_0_eng_3_engtype_VideoDecode".to_string(), 50.0),
        ];
        let memory = vec![("pid_1_luid_a_phys_0".to_string(), 1000.0), ("pid_1_luid_b_phys_0".to_string(), 24.0)];
        let s = aggregate(&engines, &memory);
        assert_eq!(s.per_pid_percent[&1], 40.0, "3D engines of one process add up");
        assert_eq!(s.per_pid_percent[&2], 50.0, "busiest engine type wins");
        assert_eq!(s.system_percent, Some(60.0), "3D total 60 beats decode 50");
        assert_eq!(s.per_pid_dedicated[&1], 1024);
    }

    #[test]
    fn empty_counters_mean_unknown_not_zero() {
        assert_eq!(aggregate(&[], &[]).system_percent, None);
    }

    #[cfg(windows)]
    #[test]
    fn real_counters_open_or_explain() {
        match GpuCounters::open() {
            Ok(mut c) => {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let s = c.sample();
                if let Some(p) = s.system_percent {
                    assert!((0.0..=100.0).contains(&p));
                }
            }
            Err(e) => assert!(e.contains("unavailable") || e.contains("Pdh"), "{e}"),
        }
    }
}
