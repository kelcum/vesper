use crate::flags::model::{json_to_flag_string, FlagSet};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

const LOCAL_MARKER: &str = "LoadClientSettingsFromLocal: \"";
const LOADED_MARKER: &str = "Successfully loaded flags";

static DENIED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Denied local configuration for: ([A-Za-z0-9_]+)").expect("valid regex"));
static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+\.\d+\.\d+\.\d+)_").expect("valid regex"));

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlagLoad {
    pub loaded: Option<FlagSet>,
    pub denied: Vec<String>,
    pub finished: bool,
}

pub fn parse_flag_load(text: &str) -> FlagLoad {
    let mut out = FlagLoad::default();

    if let Some(start) = text.find(LOCAL_MARKER) {
        let rest = &text[start + LOCAL_MARKER.len()..];
        if rest.starts_with('"') {
            out.loaded = Some(FlagSet::new());
        } else if let Some(end) = closing_brace(rest) {
            let json = &rest[..=end];
            if let Ok(map) = serde_json::from_str::<BTreeMap<String, serde_json::Value>>(json) {
                out.loaded =
                    Some(map.into_iter().filter_map(|(k, v)| json_to_flag_string(&v).map(|s| (k, s))).collect());
            }
        }
    }

    out.denied = DENIED.captures_iter(text).map(|c| c[1].to_owned()).collect();

    if let Some(pos) = text.find(LOADED_MARKER) {
        let after = &text[pos..];
        out.finished = after.lines().skip(1).any(|l| !l.trim().is_empty() && !DENIED.is_match(l));
    }
    out
}

fn closing_brace(rest: &str) -> Option<usize> {
    rest.match_indices('}')
        .map(|(i, _)| i)
        .find(|&i| rest[i + 1..].trim_start_matches([' ', '\t', '\r', '\n']).starts_with('"'))
}

pub fn version_from_log_name(name: &str) -> Option<String> {
    VERSION.captures(name).map(|c| c[1].to_owned())
}

pub fn is_player_log(name: &str) -> bool {
    name.ends_with(".log") && name.contains("_Player_") && !name.contains("CrashHandler")
}

pub fn newest_player_log_since(dir: &Path, since: SystemTime) -> Option<PathBuf> {
    let threshold = since.checked_sub(Duration::from_secs(2)).unwrap_or(since);
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| is_player_log(&e.file_name().to_string_lossy()))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            (modified >= threshold).then_some((modified, e.path()))
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Accepted,
    Denied,
    NotRead,
}

pub fn verdicts(expected: &FlagSet, load: &FlagLoad) -> BTreeMap<String, Verdict> {
    expected
        .keys()
        .map(|name| {
            let v = if load.denied.iter().any(|d| d == name) {
                Verdict::Denied
            } else if load.loaded.as_ref().is_some_and(|l| l.contains_key(name)) {
                Verdict::Accepted
            } else {
                Verdict::NotRead
            };
            (name.clone(), v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r#"2026-10-07T16:02:22.400Z,0.400280,7dc4,6,Warning [FLog::FlagFetchingStarterModule] Fetching flags from CDN, attempt: 0
2026-10-07T16:02:22.638Z,0.638899,7dc4,6 [FLog::Output] LoadClientSettingsFromLocal: "{
  "FFlagHandleAltEnterFullscreenManually": "False",
  "FFlagDebugDisablePostFx": "True",
  "FIntRenderShadowIntensity": "0"
}"
2026-10-07T16:02:22.642Z,0.642894,7dc4,6,Warning [FLog::FlagFetchingStarterModule] Successfully loaded flags
2026-10-07T16:02:22.645Z,0.645891,7dc4,6,Warning [FLog::FlagFetchingStarterModule] Denied local configuration for: FFlagDebugDisablePostFx
2026-10-07T16:02:22.645Z,0.645891,7dc4,6,Warning [FLog::FlagFetchingStarterModule] Denied local configuration for: FIntRenderShadowIntensity
2026-10-07T16:02:22.647Z,0.647889,6a68,6,Debug [FLog::UpdateController] Wrote UpdateController cache to AppStorage
"#;

    #[test]
    fn parses_loaded_and_denied_flags() {
        let load = parse_flag_load(LOG);
        let loaded = load.loaded.as_ref().unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded["FFlagHandleAltEnterFullscreenManually"], "False");
        assert_eq!(load.denied, vec!["FFlagDebugDisablePostFx", "FIntRenderShadowIntensity"]);
        assert!(load.finished);
    }

    #[test]
    fn parses_settings_echoed_with_trailing_newline() {
        let log = "2026-10-10T13:41:18.584Z,0.584861,774c,6 [FLog::Output] LoadClientSettingsFromLocal: \"{\n  \"FFlagHandleAltEnterFullscreenManually\": \"False\"\n}\n\"\n2026-10-10T13:41:18.585Z,0.585861,774c,6,Warning [FLog::FlagFetchingStarterModule] Successfully loaded flags\n2026-10-10T13:41:18.590Z,0.590000,774c,6,Info [FLog::Other] next line\n";
        let load = parse_flag_load(log);
        let loaded = load.loaded.expect("settings were read");
        assert_eq!(loaded["FFlagHandleAltEnterFullscreenManually"], "False");
        assert!(load.denied.is_empty());
        assert!(load.finished);
    }

    #[test]
    fn unfinished_log_is_detected() {
        let cut = LOG.split("2026-10-07T16:02:22.647Z").next().unwrap();
        let load = parse_flag_load(cut);
        assert!(!load.finished);
        let before_loaded = LOG.split("2026-10-07T16:02:22.642Z").next().unwrap();
        assert!(!parse_flag_load(before_loaded).finished);
    }

    #[test]
    fn verdicts_distinguish_accepted_denied_and_unread() {
        let load = parse_flag_load(LOG);
        let expected: FlagSet = [
            ("FFlagHandleAltEnterFullscreenManually", "False"),
            ("FFlagDebugDisablePostFx", "True"),
            ("FFlagDebugSkyGray", "True"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let v = verdicts(&expected, &load);
        assert_eq!(v["FFlagHandleAltEnterFullscreenManually"], Verdict::Accepted);
        assert_eq!(v["FFlagDebugDisablePostFx"], Verdict::Denied);
        assert_eq!(v["FFlagDebugSkyGray"], Verdict::NotRead);
    }

    #[test]
    fn empty_local_settings_are_recognised() {
        let load = parse_flag_load("x [FLog::Output] LoadClientSettingsFromLocal: \"\"\n");
        assert_eq!(load.loaded, Some(FlagSet::new()));
    }

    #[test]
    fn missing_section_yields_nothing() {
        let load = parse_flag_load("unrelated log text\n");
        assert!(load.loaded.is_none());
        assert!(load.denied.is_empty());
        assert!(!load.finished);
    }

    #[test]
    fn version_and_kind_come_from_file_name() {
        let name = "0.741.0.7411058_20261007T160222Z_Player_DD16B_last.log";
        assert_eq!(version_from_log_name(name).as_deref(), Some("0.741.0.7411058"));
        assert!(is_player_log(name));
        assert!(!is_player_log("0.741.0.7411058_20261007T150322Z_Player_6CD4D_CrashHandler_last.log"));
        assert!(!is_player_log("0.741.0.7411058_20261007T150322Z_Studio_1_last.log"));
    }

    #[test]
    fn newest_log_after_launch_is_selected() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("0.740.0.1_A_Player_1_last.log");
        std::fs::write(&old, "old").unwrap();
        let launch = SystemTime::now() + Duration::from_secs(5);
        assert!(newest_player_log_since(dir.path(), launch).is_none(), "old logs are ignored");
        assert_eq!(newest_player_log_since(dir.path(), SystemTime::now()).as_deref(), Some(old.as_path()));
    }
}
