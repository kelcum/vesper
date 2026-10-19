use crate::flags::model::FlagSet;
use crate::flags::verification::LaunchReport;
use crate::logging::LogEntry;
use crate::redact::redact;
use std::fmt::Write;

#[derive(Debug, Clone, Default)]
pub struct RobloxFacts {
    pub version: Option<String>,
    pub version_hash: String,
    pub location: String,
    pub source: String,
    pub registered: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub app_version: String,
    pub portable: bool,
    pub os: String,
    pub cpu: String,
    pub logical_processors: usize,
    pub memory_total: u64,
    pub gpus: Vec<String>,
    pub display_hz: Option<u32>,
    pub roblox: Option<RobloxFacts>,
    pub other_installs: usize,
    pub latest_roblox: Option<String>,
    pub protocol_owner: Option<String>,
    pub roblox_running: bool,
    pub profile: String,
    pub flags: FlagSet,
    pub fps_cap: String,
    pub current_frame_cap: Option<i32>,
    pub flag_mode: String,
    pub last_report: Option<LaunchReport>,
    pub discord: String,
    pub updates: String,
    pub log_tail: Vec<LogEntry>,
}

pub fn render(s: &Snapshot) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "Vesper diagnostics · {}", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"));
    let _ = writeln!(o);
    let _ = writeln!(o, "Client      Vesper {}{}", s.app_version, if s.portable { " (portable)" } else { "" });
    let _ = writeln!(o, "System      {}", s.os);
    let _ = writeln!(o, "CPU         {} ({} logical)", s.cpu, s.logical_processors);
    let _ = writeln!(o, "RAM         {:.1} GB", s.memory_total as f64 / 1_073_741_824.0);
    for g in &s.gpus {
        let _ = writeln!(o, "GPU         {g}");
    }
    let _ = writeln!(o, "Display     {}", s.display_hz.map(|h| format!("{h} Hz")).unwrap_or_else(|| "unknown".into()));
    let _ = writeln!(o);
    match &s.roblox {
        Some(r) => {
            let _ = writeln!(
                o,
                "Roblox      {} ({}){}",
                r.version.as_deref().unwrap_or("unknown version"),
                r.version_hash,
                if r.registered || cfg!(target_os = "macos") { "" } else { " · not the registered install" }
            );
            let _ = writeln!(o, "Source      {}", r.source);
            let _ = writeln!(o, "Path        {}", r.location);
        }
        None => {
            let _ = writeln!(o, "Roblox      not detected");
        }
    }
    if s.other_installs > 0 {
        let _ = writeln!(o, "Other       {} more install(s)", s.other_installs);
    }
    let _ = writeln!(o, "Latest      {}", s.latest_roblox.as_deref().unwrap_or("not checked"));
    if !cfg!(target_os = "macos") {
        let _ = writeln!(o, "Protocol    {}", s.protocol_owner.as_deref().unwrap_or("not registered"));
    }
    let _ = writeln!(o, "Running     {}", if s.roblox_running { "yes" } else { "no" });
    let _ = writeln!(o);
    let _ = writeln!(o, "Profile     {}", s.profile);
    let _ = writeln!(o, "Flag mode   {}", s.flag_mode);
    let _ = writeln!(
        o,
        "FPS limit   {} (Roblox setting now: {})",
        s.fps_cap,
        s.current_frame_cap.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into())
    );
    if s.flags.is_empty() {
        let _ = writeln!(o, "Flags       none");
    } else {
        let _ = writeln!(o, "Flags       {}", s.flags.len());
        for (k, v) in &s.flags {
            let _ = writeln!(o, "  {k} = {v}");
        }
    }
    match &s.last_report {
        Some(r) => {
            let _ = writeln!(
                o,
                "Last check  Roblox {} · {} accepted, {} denied, {} not read{}",
                r.roblox_version.as_deref().unwrap_or("?"),
                r.accepted.len(),
                r.denied.len(),
                r.not_read.len(),
                if r.complete { "" } else { " (incomplete)" }
            );
            for d in &r.denied {
                let _ = writeln!(o, "  denied: {d}");
            }
        }
        None => {
            let _ = writeln!(o, "Last check  none yet");
        }
    }
    let _ = writeln!(o);
    let _ = writeln!(o, "Discord     {}", s.discord);
    let _ = writeln!(o, "Updates     {}", s.updates);
    if !s.log_tail.is_empty() {
        let _ = writeln!(o);
        let _ = writeln!(o, "Recent log");
        for e in &s.log_tail {
            let _ = writeln!(o, "  {} {:5} {} {}", e.ts, e.level, e.area, e.msg);
        }
    }
    o.lines().map(redact).collect::<Vec<_>>().join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        let mut flags = FlagSet::new();
        flags.insert("FIntDebugForceMSAASamples".into(), "4".into());
        Snapshot {
            app_version: "0.1.0".into(),
            os: "Windows 11 Home".into(),
            cpu: "Test CPU".into(),
            logical_processors: 8,
            memory_total: 16 * 1_073_741_824,
            gpus: vec!["Test GPU".into()],
            roblox: Some(RobloxFacts {
                version: Some("0.741.0.7411058".into()),
                version_hash: "version-abc".into(),
                location: r"C:\Users\private.name\AppData\Local\Roblox\Versions\version-abc".into(),
                source: "Official".into(),
                registered: true,
            }),
            profile: "Balanced".into(),
            flags,
            fps_cap: "Match display".into(),
            current_frame_cap: Some(144),
            flag_mode: "Apply on launch".into(),
            discord: "Disabled".into(),
            updates: "Up to date".into(),
            log_tail: vec![LogEntry {
                ts: "t".into(),
                level: "INFO",
                area: "x".into(),
                msg: "launch arg roblox-player:1+gameinfo:TICKET123+launchtime:1".into(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn includes_the_essentials() {
        let r = render(&snapshot());
        for needle in [
            "Vesper 0.1.0",
            "Windows 11 Home",
            "Test GPU",
            "0.741.0.7411058",
            "Balanced",
            "FIntDebugForceMSAASamples = 4",
            "Roblox setting now: 144",
            "Discord     Disabled",
            "Updates     Up to date",
        ] {
            assert!(r.contains(needle), "missing {needle}\n{r}");
        }
    }

    #[test]
    fn never_contains_personal_data_or_secrets() {
        let r = render(&snapshot());
        assert!(!r.contains("private.name"), "{r}");
        assert!(!r.contains("TICKET123"), "{r}");
        assert!(r.contains("%USERPROFILE%"));
    }

    #[test]
    fn missing_roblox_is_stated() {
        let r = render(&Snapshot { roblox: None, ..snapshot() });
        assert!(r.contains("not detected"));
    }
}
