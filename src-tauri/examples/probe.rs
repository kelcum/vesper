//! Prints what Vesper detects on this PC, without changing anything.
//!
//!     cargo run --example probe
//!
//! Useful when filing a bug report or checking detection on a new machine.
//! Paths are shown with the user profile hidden.

use std::time::Duration;
use vesper_lib::perf::{monitor::Monitor, system};
use vesper_lib::roblox::{detect, gbs, logs};

fn main() {
    println!("== System");
    let info = system::info();
    println!("  OS:      {}", info.os);
    println!("  CPU:     {} ({} logical)", info.cpu, info.logical_processors);
    println!("  RAM:     {:.1} GB", info.memory_total as f64 / 1e9);
    for g in &info.gpus {
        println!("  GPU:     {} ({:.1} GB dedicated)", g.name, g.dedicated_memory as f64 / 1e9);
    }
    println!("  Display: {}", info.display_hz.map(|h| format!("{h} Hz")).unwrap_or("unknown".into()));

    println!("\n== Roblox");
    let Some(env) = detect::DetectEnv::system() else {
        println!("  %LOCALAPPDATA% is not set");
        return;
    };
    let d = detect::detect(&env, None);
    if d.installs.is_empty() {
        println!("  Not installed");
    }
    for i in &d.installs {
        let marker = if Some(&i.id) == d.preferred.as_ref() { "*" } else { " " };
        println!(
            "  {marker} {:<10} {:<26} {:<16} registered={} flags-file={} {}",
            format!("{:?}", i.source),
            i.version_hash,
            i.version.as_deref().unwrap_or("?"),
            i.registered,
            i.has_flags_file,
            i.location
        );
    }
    if let Some(p) = &d.protocol {
        println!("  roblox-player: handled by {}", p.owner);
    }
    println!("  Store edition present: {}", d.store_app_present);
    match gbs::current_cap(&env.roblox_data()) {
        Ok(cap) => println!("  FramerateCap: {cap:?}"),
        Err(e) => println!("  FramerateCap: {e}"),
    }
    let log_dir = env.roblox_data().join("logs");
    let epoch = std::time::SystemTime::UNIX_EPOCH;
    if let Some(log) = logs::newest_player_log_since(&log_dir, epoch) {
        let name = log.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let load = logs::parse_flag_load(&text);
        println!(
            "  Last log: Roblox {} - read {} local flags, denied {:?}",
            logs::version_from_log_name(&name).unwrap_or("?".into()),
            load.loaded.as_ref().map_or(0, |l| l.len()),
            load.denied
        );
    }

    println!("\n== Live sample");
    let mut m = Monitor::new(true);
    std::thread::sleep(Duration::from_millis(1200));
    let s = m.sample();
    println!(
        "  CPU {:.1}%  RAM {:.1}/{:.1} GB  GPU {}",
        s.system.cpu_percent.unwrap_or(-1.0),
        s.system.memory_used as f64 / 1e9,
        s.system.memory_total as f64 / 1e9,
        s.system.gpu_percent.map(|g| format!("{g:.1}%")).unwrap_or_else(|| "n/a".into())
    );
    if let Some(e) = &s.gpu_error {
        println!("  GPU counters: {e}");
    }
    println!("  Frame timing: {:?}", s.fps_status);
    match &s.roblox {
        Some(r) => {
            println!("  Roblox running: pid {} cpu {:?} mem {:.0} MB", r.pid, r.cpu_percent, r.working_set as f64 / 1e6)
        }
        None => println!("  Roblox is not running"),
    }
}
