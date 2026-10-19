use serde::Serialize;
#[cfg(any(windows, test))]
use std::collections::{HashMap, VecDeque};

pub const DXGI_PROVIDER: u128 = 0xCA11C036_0102_4A2D_A6AD_F03CFED5D3C9;
pub const PRESENT_START_ID: u16 = 42;
const KEEP_SECONDS: f64 = 5.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "detail")]
pub enum FpsStatus {
    Off,
    Measuring,
    NeedsPermission,
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameStats {
    pub fps: f64,
    pub frame_time_avg_ms: f64,
    pub frame_time_p99_ms: f64,
    pub frames: usize,
}

pub fn stats(timestamps: &[f64], now: f64) -> Option<FrameStats> {
    let recent: Vec<f64> = timestamps.iter().copied().filter(|t| *t > now - KEEP_SECONDS && *t <= now + 0.5).collect();
    if recent.len() < 2 || now - recent[recent.len() - 1] > 1.0 {
        return None;
    }
    let fps = recent.iter().filter(|t| **t > now - 1.0).count() as f64;
    let mut intervals: Vec<f64> = recent.windows(2).map(|w| (w[1] - w[0]) * 1000.0).filter(|d| *d > 0.0).collect();
    if intervals.is_empty() {
        return None;
    }
    let avg = intervals.iter().sum::<f64>() / intervals.len() as f64;
    intervals.sort_by(|a, b| a.total_cmp(b));
    let idx = ((intervals.len() as f64 * 0.99).ceil() as usize).clamp(1, intervals.len()) - 1;
    Some(FrameStats { fps, frame_time_avg_ms: avg, frame_time_p99_ms: intervals[idx], frames: recent.len() })
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct Presents {
    by_pid: HashMap<u32, VecDeque<f64>>,
}

#[cfg(any(windows, test))]
impl Presents {
    fn push(&mut self, pid: u32, t: f64) {
        let q = self.by_pid.entry(pid).or_default();
        q.push_back(t);
        while q.front().is_some_and(|f| *f < t - KEEP_SECONDS) {
            q.pop_front();
        }
        if self.by_pid.len() > 64 {
            self.by_pid.retain(|_, q| q.back().is_some_and(|b| *b > t - KEEP_SECONDS));
        }
    }
}

pub fn event_clock_now() -> f64 {
    let unix =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    unix + 11_644_473_600.0
}

#[cfg(windows)]
mod imp {
    use super::{event_clock_now, stats, FpsStatus, FrameStats, Presents, PRESENT_START_ID};
    use std::sync::{Arc, Mutex};
    use windows::core::{GUID, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_SUCCESS};
    use windows::Win32::System::Diagnostics::Etw::{
        CloseTrace, ControlTraceW, EnableTraceEx2, OpenTraceW, ProcessTrace, StartTraceW, CONTROLTRACE_HANDLE,
        EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW,
        EVENT_TRACE_PROPERTIES, EVENT_TRACE_REAL_TIME_MODE, PROCESSTRACE_HANDLE, PROCESS_TRACE_MODE_EVENT_RECORD,
        PROCESS_TRACE_MODE_REAL_TIME, WNODE_FLAG_TRACED_GUID,
    };

    const SESSION: &str = "Vesper Frame Timing";
    const TRACE_LEVEL_INFORMATION: u8 = 4;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Props {
        buf: Vec<u64>,
    }

    impl Props {
        fn new() -> Self {
            let header = std::mem::size_of::<EVENT_TRACE_PROPERTIES>();
            let total = header + (SESSION.len() + 1) * 2 + 64;
            let mut p = Props { buf: vec![0; total.div_ceil(8)] };
            let props = p.get();
            props.Wnode.BufferSize = total as u32;
            props.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            props.Wnode.ClientContext = 1;
            props.LogFileMode = EVENT_TRACE_REAL_TIME_MODE;
            props.LoggerNameOffset = header as u32;
            p
        }

        fn get(&mut self) -> &mut EVENT_TRACE_PROPERTIES {
            unsafe { &mut *(self.buf.as_mut_ptr() as *mut EVENT_TRACE_PROPERTIES) }
        }
    }

    fn stop_session(handle: CONTROLTRACE_HANDLE) {
        let name = wide(SESSION);
        let mut props = Props::new();
        unsafe {
            let _ = ControlTraceW(handle, PCWSTR(name.as_ptr()), props.get(), EVENT_TRACE_CONTROL_STOP);
        }
    }

    unsafe extern "system" fn on_event(record: *mut EVENT_RECORD) {
        let r = &*record;
        if r.EventHeader.EventDescriptor.Id != PRESENT_START_ID || r.UserContext.is_null() {
            return;
        }
        let shared = &*(r.UserContext as *const Mutex<Presents>);
        let t = r.EventHeader.TimeStamp as f64 / 10_000_000.0;
        if let Ok(mut p) = shared.lock() {
            p.push(r.EventHeader.ProcessId, t);
        }
    }

    pub struct FpsTracer {
        control: CONTROLTRACE_HANDLE,
        thread: Option<std::thread::JoinHandle<()>>,
        shared: Arc<Mutex<Presents>>,
    }

    impl FpsTracer {
        pub fn start() -> Result<Self, FpsStatus> {
            let name = wide(SESSION);
            let mut control = CONTROLTRACE_HANDLE::default();
            let mut props = Props::new();
            let mut status = unsafe { StartTraceW(&mut control, PCWSTR(name.as_ptr()), props.get()) };
            if status == ERROR_ALREADY_EXISTS {
                stop_session(CONTROLTRACE_HANDLE::default());
                props = Props::new();
                status = unsafe { StartTraceW(&mut control, PCWSTR(name.as_ptr()), props.get()) };
            }
            if status == ERROR_ACCESS_DENIED {
                return Err(FpsStatus::NeedsPermission);
            }
            if status != ERROR_SUCCESS {
                return Err(FpsStatus::Unavailable(format!("Windows couldn't start frame tracing ({})", status.0)));
            }

            let provider = GUID::from_u128(super::DXGI_PROVIDER);
            let enabled = unsafe {
                EnableTraceEx2(
                    control,
                    &provider,
                    EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
                    TRACE_LEVEL_INFORMATION,
                    u64::MAX,
                    0,
                    0,
                    None,
                )
            };
            if enabled != ERROR_SUCCESS {
                stop_session(control);
                return Err(if enabled == ERROR_ACCESS_DENIED {
                    FpsStatus::NeedsPermission
                } else {
                    FpsStatus::Unavailable(format!("Couldn't enable Direct3D events ({})", enabled.0))
                });
            }

            let shared = Arc::new(Mutex::new(Presents::default()));
            let context = Arc::into_raw(shared.clone()) as *mut core::ffi::c_void;
            let mut logger_name = wide(SESSION);
            let mut logfile = EVENT_TRACE_LOGFILEW {
                LoggerName: PWSTR(logger_name.as_mut_ptr()),
                Context: context,
                ..Default::default()
            };
            logfile.Anonymous1.ProcessTraceMode = PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
            logfile.Anonymous2.EventRecordCallback = Some(on_event);

            let trace: PROCESSTRACE_HANDLE = unsafe { OpenTraceW(&mut logfile) };
            if trace.Value == u64::MAX {
                stop_session(control);
                unsafe { drop(Arc::from_raw(context as *const Mutex<Presents>)) };
                return Err(FpsStatus::Unavailable("Couldn't open the frame trace".into()));
            }

            let ctx_addr = context as usize;
            let thread = std::thread::Builder::new()
                .name("vesper-fps".into())
                .spawn(move || {
                    let _keep = logger_name;
                    unsafe {
                        let _ = ProcessTrace(&[trace], None, None);
                        let _ = CloseTrace(trace);
                        drop(Arc::from_raw(ctx_addr as *const Mutex<Presents>));
                    }
                })
                .map_err(|e| FpsStatus::Unavailable(e.to_string()))?;

            log::info!("Frame timing trace started");
            Ok(Self { control, thread: Some(thread), shared })
        }

        pub fn stats(&self, pid: u32) -> Option<FrameStats> {
            let p = self.shared.lock().ok()?;
            let q = p.by_pid.get(&pid)?;
            let v: Vec<f64> = q.iter().copied().collect();
            stats(&v, event_clock_now())
        }

        pub fn any_activity(&self) -> bool {
            let now = event_clock_now();
            self.shared
                .lock()
                .map(|p| p.by_pid.values().any(|q| q.back().is_some_and(|t| now - t < 2.0)))
                .unwrap_or(false)
        }
    }

    impl Drop for FpsTracer {
        fn drop(&mut self) {
            stop_session(self.control);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            log::info!("Frame timing trace stopped");
        }
    }
}

#[cfg(windows)]
pub use imp::FpsTracer;

#[cfg(not(windows))]
pub struct FpsTracer;

#[cfg(not(windows))]
impl FpsTracer {
    pub fn start() -> Result<Self, FpsStatus> {
        Err(FpsStatus::Unavailable(if cfg!(target_os = "macos") {
            "macOS doesn't let other apps count Roblox's frames".into()
        } else {
            "Only available on Windows".into()
        }))
    }
    pub fn stats(&self, _pid: u32) -> Option<FrameStats> {
        None
    }
    pub fn any_activity(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steady(fps: f64, seconds: f64, end: f64) -> Vec<f64> {
        let n = (fps * seconds) as usize;
        (0..n).map(|i| end - seconds + (i as f64 + 1.0) / fps).collect()
    }

    #[test]
    fn steady_sixty_fps() {
        let ts = steady(60.0, 3.0, 100.0);
        let s = stats(&ts, 100.0).unwrap();
        assert!((s.fps - 60.0).abs() <= 1.0, "{}", s.fps);
        assert!((s.frame_time_avg_ms - 16.667).abs() < 0.1);
        assert!((s.frame_time_p99_ms - 16.667).abs() < 0.1);
    }

    #[test]
    fn stutter_shows_in_p99_not_average() {
        let mut ts = steady(144.0, 3.0, 100.0);
        for k in [60usize, 120, 180, 250, 320, 380] {
            let shift = 0.05 - 1.0 / 144.0;
            for t in ts.iter_mut().skip(k) {
                *t += shift;
            }
        }
        let now = *ts.last().unwrap();
        let s = stats(&ts, now).unwrap();
        assert!(s.frame_time_p99_ms > 40.0, "{}", s.frame_time_p99_ms);
        assert!(s.frame_time_avg_ms < 9.0, "{}", s.frame_time_avg_ms);
    }

    #[test]
    fn idle_or_empty_processes_have_no_stats() {
        assert!(stats(&[], 10.0).is_none());
        assert!(stats(&[9.99], 10.0).is_none());
        let ts = steady(60.0, 2.0, 50.0);
        assert!(stats(&ts, 55.0).is_none(), "stopped presenting 5 s ago");
    }

    #[test]
    fn presents_buffer_is_bounded() {
        let mut p = Presents::default();
        for i in 0..10_000 {
            p.push(7, i as f64 * 0.001);
        }
        assert!(p.by_pid[&7].len() <= 5001);
        for pid in 0..100 {
            p.push(pid, 0.0);
        }
        p.push(1000, 100.0);
        assert!(p.by_pid.len() < 100, "stale processes are forgotten");
    }

    #[cfg(windows)]
    #[test]
    fn tracer_starts_or_reports_permission() {
        match FpsTracer::start() {
            Ok(t) => drop(t),
            Err(FpsStatus::NeedsPermission) => {}
            Err(other) => panic!("unexpected: {other:?}"),
        }
    }
}
