//! Remembers what Roblox said about each flag the last time it was launched.

use crate::paths::write_atomic;
use crate::roblox::logs::Verdict;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FlagCheck {
    pub verdict: String,
    pub roblox_version: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReport {
    pub roblox_version: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub accepted: Vec<String>,
    pub denied: Vec<String>,
    pub not_read: Vec<String>,
    /// False when Roblox was still starting and the log was incomplete.
    pub complete: bool,
}

impl LaunchReport {
    pub fn from_verdicts(version: Option<String>, verdicts: &BTreeMap<String, Verdict>, complete: bool) -> Self {
        let pick = |want: Verdict| -> Vec<String> {
            verdicts.iter().filter(|(_, v)| **v == want).map(|(k, _)| k.clone()).collect()
        };
        Self {
            roblox_version: version,
            checked_at: Utc::now(),
            accepted: pick(Verdict::Accepted),
            denied: pick(Verdict::Denied),
            not_read: pick(Verdict::NotRead),
            complete,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationStore {
    pub flags: BTreeMap<String, FlagCheck>,
    pub last_report: Option<LaunchReport>,
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Accepted => "accepted",
        Verdict::Denied => "denied",
        Verdict::NotRead => "notRead",
    }
}

impl VerificationStore {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self).map_err(io::Error::other)?)
    }

    /// Records a launch. "Not read" results never overwrite a real verdict,
    /// because they say nothing about whether Roblox accepts the flag.
    pub fn record(&mut self, report: LaunchReport, verdicts: &BTreeMap<String, Verdict>) {
        for (name, v) in verdicts {
            if *v == Verdict::NotRead && self.flags.contains_key(name) {
                continue;
            }
            self.flags.insert(
                name.clone(),
                FlagCheck {
                    verdict: verdict_name(*v).to_owned(),
                    roblox_version: report.roblox_version.clone(),
                    checked_at: report.checked_at,
                },
            );
        }
        self.last_report = Some(report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_read_does_not_overwrite_real_results() {
        let mut store = VerificationStore::default();
        let mut v = BTreeMap::new();
        v.insert("FFlagA".to_string(), Verdict::Accepted);
        store.record(LaunchReport::from_verdicts(Some("0.741".into()), &v, true), &v);

        let mut later = BTreeMap::new();
        later.insert("FFlagA".to_string(), Verdict::NotRead);
        later.insert("FFlagB".to_string(), Verdict::NotRead);
        store.record(LaunchReport::from_verdicts(Some("0.742".into()), &later, true), &later);

        assert_eq!(store.flags["FFlagA"].verdict, "accepted");
        assert_eq!(store.flags["FFlagA"].roblox_version.as_deref(), Some("0.741"));
        assert_eq!(store.flags["FFlagB"].verdict, "notRead");
        assert_eq!(store.last_report.as_ref().unwrap().not_read.len(), 2);
    }

    #[test]
    fn store_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.json");
        let mut store = VerificationStore::default();
        let mut v = BTreeMap::new();
        v.insert("FFlagA".to_string(), Verdict::Denied);
        store.record(LaunchReport::from_verdicts(None, &v, false), &v);
        store.save(&path).unwrap();
        let loaded = VerificationStore::load(&path);
        assert_eq!(loaded.flags["FFlagA"].verdict, "denied");
        assert!(!loaded.last_report.unwrap().complete);
    }
}
