use crate::config::UpdateChannel;
use chrono::{DateTime, Utc};
use serde::Serialize;

pub const STABLE_URL: &str = "https://xan.gripe/vesper/updates/latest.json";
pub const BETA_URL: &str = "https://xan.gripe/vesper/updates/beta.json";

pub fn endpoint(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => STABLE_URL,
        UpdateChannel::Beta => BETA_URL,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum UpdateState {
    Idle,
    Checking,
    #[serde(rename_all = "camelCase")]
    UpToDate {
        checked_at: DateTime<Utc>,
    },
    #[serde(rename_all = "camelCase")]
    Available {
        version: String,
        notes: Option<String>,
        date: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Installing,
    #[serde(rename_all = "camelCase")]
    Failed {
        message: String,
    },
}

impl UpdateState {
    pub fn describe(&self) -> String {
        match self {
            UpdateState::Idle => "Not checked yet".into(),
            UpdateState::Checking => "Checking".into(),
            UpdateState::UpToDate { checked_at } => {
                format!("Up to date (checked {})", checked_at.format("%Y-%m-%d %H:%M UTC"))
            }
            UpdateState::Available { version, .. } => format!("Version {version} available"),
            UpdateState::Downloading { .. } => "Downloading".into(),
            UpdateState::Installing => "Installing".into(),
            UpdateState::Failed { message } => format!("Check failed: {message}"),
        }
    }
}

pub fn friendly_error(raw: &str) -> String {
    let r = raw.to_lowercase();
    if r.contains("signature") || r.contains("minisign") {
        "The downloaded update's signature didn't match Vesper's key, so it was not installed.".into()
    } else if r.contains("dns") || r.contains("connect") || r.contains("timed out") || r.contains("network") {
        "Couldn't reach the update server. Check your internet connection.".into()
    } else if r.contains("404") || r.contains("not found") || r.contains("valid release json") {
        "No update information is published for this channel yet.".into()
    } else {
        "The update server sent something Vesper couldn't use.".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_map_to_site_manifests() {
        assert_eq!(endpoint(UpdateChannel::Stable), "https://xan.gripe/vesper/updates/latest.json");
        assert_eq!(endpoint(UpdateChannel::Beta), "https://xan.gripe/vesper/updates/beta.json");
    }

    #[test]
    fn errors_are_explained() {
        assert!(friendly_error("Could not fetch a valid release JSON from the remote").contains("published"));
        assert!(friendly_error("minisign: signature verification failed").contains("signature"));
        assert!(friendly_error("error sending request: dns error").contains("internet"));
    }
}
