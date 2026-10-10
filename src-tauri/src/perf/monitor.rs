//! Live performance sampling.
//!
//! The monitor only runs while something on screen needs it (the
//! Performance page or the dashboard summary) and stops when the window is
//! hidden, so Vesper costs nothing while you play with it in the tray.

use super::fps::{FpsStatus, FpsTracer, FrameStats};
use super::gpu::GpuCounters;
use super::system::{memory, CpuMeter};
use crate::roblox::process::{self, PLAYER_PROCESS};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSample {
    pub cpu_percent: Option<f64>,
    pub memory_used: u64,
    pub memory_total: u64,
    pub gpu_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobloxSample {
    pub pid: u32,
    pub processes: usize,
    pub cpu_percent: Option<f64>,
    pub working_set: u64,
    pub private_bytes: u64,
    pub gpu_percent: Option<f64>,
    pub gpu_memory: Option<u64>,
    pub frames: Option<FrameStats>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfSample {
    pub at: i64,
    pub system: SystemSample,
    pub roblox: Option<RobloxSample>,
    pub gpu_error: Option<String>,
    pub fps_status: FpsStatus,
}

pub struct Monitor {
    cpu: CpuMeter,
    gpu: Option<GpuCounters>,
    gpu_error: Option<String>,
    fps: Option<FpsTracer>,
    fps_status: FpsStatus,
    last_cpu: HashMap<u32, (u64, Instant)>,
    logical: f64,
}

impl Monitor {
    pub fn new(want_fps: bool) -> Self {
        let (gpu, gpu_error) = match GpuCounters::open() {
            Ok(g) => (Some(g), None),
            Err(e) => {
                log::info!("GPU counters unavailable: {e}");
                (None, Some(e))
            }
        };
        let (fps, fps_status) = if want_fps {
            match FpsTracer::start() {
                Ok(t) => (Some(t), FpsStatus::Measuring),
                Err(s) => {
                    log::info!("Frame timing unavailable: {s:?}");
                    (None, s)
                }
            }
        } else {
            (None, FpsStatus::Off)
        };
        let mut cpu = CpuMeter::default();
        cpu.sample();
        Self {
            cpu,
            gpu,
            gpu_error,
            fps,
            fps_status,
            last_cpu: HashMap::new(),
            logical: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64,
        }
    }

    pub fn sample(&mut self) -> PerfSample {
        let now = Instant::now();
        let mem = memory();
        let gpu = self.gpu.as_mut().map(|g| g.sample()).unwrap_or_default();
        let all = process::snapshot().unwrap_or_default();
        let players: Vec<_> = process::named(&all, PLAYER_PROCESS).collect();

        let mut cpu_total: Option<f64> = None;
        for p in &players {
            if let Some((prev, at)) = self.last_cpu.get(&p.pid) {
                let wall = now.duration_since(*at).as_secs_f64() * 10_000_000.0;
                if wall > 0.0 {
                    let pct =
                        (p.cpu_time.saturating_sub(*prev) as f64 / (wall * self.logical) * 100.0).clamp(0.0, 100.0);
                    cpu_total = Some(cpu_total.unwrap_or(0.0) + pct);
                }
            }
        }
        self.last_cpu = players.iter().map(|p| (p.pid, (p.cpu_time, now))).collect();

        // The main client is the one using the most memory (crash handlers are tiny).
        let roblox = players.iter().max_by_key(|p| p.working_set).map(|main| RobloxSample {
            pid: main.pid,
            processes: players.len(),
            cpu_percent: cpu_total,
            working_set: players.iter().map(|p| p.working_set).sum(),
            private_bytes: players.iter().map(|p| p.private_bytes).sum(),
            gpu_percent: self
                .gpu
                .as_ref()
                .map(|_| players.iter().filter_map(|p| gpu.per_pid_percent.get(&p.pid)).cloned().fold(0.0, f64::max)),
            gpu_memory: self
                .gpu
                .as_ref()
                .map(|_| players.iter().filter_map(|p| gpu.per_pid_dedicated.get(&p.pid)).sum()),
            frames: self.fps.as_ref().and_then(|f| f.stats(main.pid)),
        });

        PerfSample {
            at: chrono::Utc::now().timestamp_millis(),
            system: SystemSample {
                cpu_percent: self.cpu.sample(),
                memory_used: mem.map(|m| m.total.saturating_sub(m.available)).unwrap_or(0),
                memory_total: mem.map(|m| m.total).unwrap_or(0),
                gpu_percent: gpu.system_percent,
            },
            roblox,
            gpu_error: self.gpu_error.clone(),
            fps_status: self.fps_status.clone(),
        }
    }
}

/// A running monitor thread. Dropping it stops sampling.
pub struct MonitorHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MonitorHandle {
    pub fn spawn(interval: Duration, want_fps: bool, emit: impl Fn(PerfSample) + Send + 'static) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("vesper-perf".into())
            .spawn(move || {
                let mut m = Monitor::new(want_fps);
                while !flag.load(Ordering::Relaxed) {
                    let started = Instant::now();
                    emit(m.sample());
                    // Sleep in short slices so stopping is prompt.
                    while started.elapsed() < interval && !flag.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            })
            .expect("spawn monitor thread");
        Self { stop, thread: Some(thread) }
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn samples_real_system_metrics() {
        let mut m = Monitor::new(false);
        std::thread::sleep(Duration::from_millis(150));
        let s = m.sample();
        assert!(s.system.memory_total > 0);
        assert!(s.system.memory_used <= s.system.memory_total);
        assert!(s.system.cpu_percent.is_some_and(|c| (0.0..=100.0).contains(&c)));
        assert_eq!(s.fps_status, FpsStatus::Off);
    }

    #[test]
    fn handle_emits_and_stops_promptly() {
        let samples = Arc::new(Mutex::new(0));
        let counter = samples.clone();
        let h = MonitorHandle::spawn(Duration::from_millis(100), false, move |_| *counter.lock().unwrap() += 1);
        // Wait for samples rather than assuming a rate: a loaded machine samples slower.
        let deadline = Instant::now() + Duration::from_secs(10);
        while *samples.lock().unwrap() < 2 {
            assert!(Instant::now() < deadline, "no samples");
            std::thread::sleep(Duration::from_millis(20));
        }
        let started = Instant::now();
        drop(h);
        assert!(started.elapsed() < Duration::from_millis(500), "stops within a slice");
        let n = *samples.lock().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(*samples.lock().unwrap(), n, "no samples after stopping");
    }
}
