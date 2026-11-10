use crate::paths::write_atomic;
use crate::roblox::activity::{Activity, ServerKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MAX_VISITS: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub place_id: u64,
    pub universe_id: Option<u64>,
    pub job_id: String,
    pub kind: ServerKind,
    pub name: Option<String>,
    pub creator: Option<String>,
    pub icon_url: Option<String>,
    pub location: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub pinned: bool,
}

impl Visit {
    pub fn from_activity(a: &Activity) -> Self {
        Self {
            place_id: a.place_id,
            universe_id: a.universe_id,
            job_id: a.job_id.clone(),
            kind: a.kind,
            name: None,
            creator: None,
            icon_url: None,
            location: None,
            joined_at: a.joined_at,
            left_at: None,
            pinned: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct History {
    pub visits: Vec<Visit>,
}

impl History {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)
    }

    pub fn joined(&mut self, a: &Activity) {
        if let Some(i) = self.visits.iter().position(|v| v.job_id == a.job_id && v.place_id == a.place_id) {
            let mut v = self.visits.remove(i);
            v.joined_at = a.joined_at;
            v.left_at = None;
            v.universe_id = v.universe_id.or(a.universe_id);
            self.visits.insert(0, v);
        } else {
            self.visits.insert(0, Visit::from_activity(a));
        }
        self.trim();
    }

    fn trim(&mut self) {
        let mut unpinned = 0;
        self.visits.retain(|v| {
            if v.pinned {
                return true;
            }
            unpinned += 1;
            unpinned <= MAX_VISITS
        });
    }

    pub fn set_pinned(&mut self, job_id: &str, pinned: bool) -> bool {
        self.update(job_id, |v| v.pinned = pinned)
    }

    pub fn pinned(&self) -> impl Iterator<Item = &Visit> {
        self.visits.iter().filter(|v| v.pinned)
    }

    pub fn update(&mut self, job_id: &str, f: impl FnOnce(&mut Visit)) -> bool {
        match self.visits.iter_mut().find(|v| v.job_id == job_id) {
            Some(v) => {
                let before = v.clone();
                f(v);
                *v != before
            }
            None => false,
        }
    }

    pub fn left(&mut self, job_id: &str, at: DateTime<Utc>) {
        self.update(job_id, |v| {
            if v.left_at.is_none() {
                v.left_at = Some(at);
            }
        });
    }

    pub fn close_all(&mut self, at: DateTime<Utc>) {
        for v in self.visits.iter_mut().filter(|v| v.left_at.is_none()) {
            v.left_at = Some(at);
        }
    }

    pub fn remove(&mut self, job_id: &str) {
        self.visits.retain(|v| v.job_id != job_id);
    }

    pub fn recent_for_menu(&self, limit: usize) -> Vec<&Visit> {
        let mut seen = Vec::new();
        self.visits
            .iter()
            .filter(|v| v.name.is_some())
            .filter(|v| {
                let key = v.universe_id.map_or(v.place_id, |u| u);
                if seen.contains(&key) {
                    false
                } else {
                    seen.push(key);
                    true
                }
            })
            .take(limit)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(place: u64, job: &str) -> Activity {
        Activity {
            place_id: place,
            universe_id: Some(1),
            job_id: job.into(),
            kind: ServerKind::Public,
            server_ip: Some("128.116.1.1".into()),
            joined_at: Utc::now(),
        }
    }

    #[test]
    fn rejoining_moves_the_entry_to_the_top() {
        let mut h = History::default();
        h.joined(&act(1, "a"));
        h.joined(&act(2, "b"));
        h.left("b", Utc::now());
        let again = Activity { joined_at: Utc::now() + chrono::Duration::hours(1), ..act(1, "a") };
        h.joined(&again);
        assert_eq!(h.visits.len(), 2);
        assert_eq!(h.visits[0].job_id, "a");
        assert_eq!(h.visits[0].joined_at, again.joined_at, "time in the server starts again");
        assert!(h.visits[1].left_at.is_some());
    }

    #[test]
    fn list_is_capped_and_saved_without_server_addresses() {
        let mut h = History::default();
        for i in 0..(MAX_VISITS + 5) {
            h.joined(&act(i as u64, &format!("job-{i}")));
        }
        assert_eq!(h.visits.len(), MAX_VISITS);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        h.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("128.116"), "addresses aren't stored");
        assert_eq!(History::load(&path), h);
    }

    #[test]
    fn pinned_visits_survive_the_cap() {
        let mut h = History::default();
        h.joined(&act(1, "keep"));
        assert!(h.set_pinned("keep", true));
        assert!(!h.set_pinned("nope", true));
        for i in 0..(MAX_VISITS + 20) {
            h.joined(&act(100 + i as u64, &format!("job-{i}")));
        }
        assert_eq!(h.visits.len(), MAX_VISITS + 1);
        assert!(h.visits.iter().any(|v| v.job_id == "keep" && v.pinned));
        assert_eq!(h.pinned().count(), 1);
        h.set_pinned("keep", false);
        h.joined(&act(999, "one-more"));
        assert_eq!(h.visits.len(), MAX_VISITS);
        assert!(!h.visits.iter().any(|v| v.job_id == "keep"));
    }

    #[test]
    fn the_menu_lists_named_games_once() {
        let mut h = History::default();
        for (place, job) in [(1, "a"), (1, "b"), (2, "c"), (3, "d")] {
            h.joined(&act(place, job));
        }
        h.update("a", |v| v.name = Some("First".into()));
        h.update("b", |v| v.name = Some("First".into()));
        h.update("c", |v| v.name = Some("Second".into()));
        let menu: Vec<&str> = h.recent_for_menu(5).iter().map(|v| v.job_id.as_str()).collect();
        assert_eq!(menu, ["c", "b"], "unnamed visits are skipped and each game appears once");
    }

    #[test]
    fn closing_roblox_ends_open_visits() {
        let mut h = History::default();
        h.joined(&act(1, "a"));
        h.close_all(Utc::now());
        assert!(h.visits[0].left_at.is_some());
        h.remove("a");
        assert!(h.visits.is_empty());
    }
}
