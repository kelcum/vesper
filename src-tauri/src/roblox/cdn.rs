//! Roblox's public client settings service.
//!
//! Two read-only requests, both anonymous (no cookies, no account):
//! * the latest player version for this system, to tell whether Roblox is up to date;
//! * the flag values Roblox currently ships to this kind of computer, shown
//!   next to each flag as "Roblox's value". Flags Roblox doesn't publish keep
//!   their engine default, which Roblox doesn't disclose — the UI says so
//!   rather than guessing.

use crate::flags::model::FlagSet;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[cfg(not(target_os = "macos"))]
pub const VERSION_URL: &str = "https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer";
#[cfg(not(target_os = "macos"))]
pub const SETTINGS_URL: &str = "https://clientsettingscdn.roblox.com/v2/settings/application/PCDesktopClient";
#[cfg(target_os = "macos")]
pub const VERSION_URL: &str = "https://clientsettingscdn.roblox.com/v2/client-version/MacPlayer";
#[cfg(target_os = "macos")]
pub const SETTINGS_URL: &str = "https://clientsettingscdn.roblox.com/v2/settings/application/MacDesktopClient";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestVersion {
    pub version: String,
    /// Folder name of the latest build, e.g. `version-02c37bc51a384b8f`.
    pub version_hash: String,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedValues {
    pub values: FlagSet,
    pub fetched_at: DateTime<Utc>,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Vesper/", env!("CARGO_PKG_VERSION"), " (+https://xan.gripe/vesper)"))
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub fn parse_version(json: &str) -> Result<LatestVersion, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        version: String,
        client_version_upload: String,
    }
    let raw: Raw = serde_json::from_str(json).map_err(|e| format!("unexpected response: {e}"))?;
    if !raw.client_version_upload.starts_with("version-") {
        return Err("unexpected version folder name".into());
    }
    Ok(LatestVersion { version: raw.version, version_hash: raw.client_version_upload, fetched_at: Utc::now() })
}

/// Extracts `names` from the settings response.
pub fn parse_published(json: &str, names: &[&str]) -> Result<PublishedValues, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        application_settings: BTreeMap<String, serde_json::Value>,
    }
    let raw: Raw = serde_json::from_str(json).map_err(|e| format!("unexpected response: {e}"))?;
    let values = names
        .iter()
        .filter_map(|n| {
            let v = raw.application_settings.get(*n)?;
            crate::flags::model::json_to_flag_string(v).map(|s| (n.to_string(), s))
        })
        .collect();
    Ok(PublishedValues { values, fetched_at: Utc::now() })
}

pub async fn latest_version(client: &reqwest::Client) -> Result<LatestVersion, String> {
    let text = client
        .get(VERSION_URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.without_url().to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    parse_version(&text)
}

pub async fn published_values(client: &reqwest::Client, names: &[&str]) -> Result<PublishedValues, String> {
    let text = client
        .get(SETTINGS_URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.without_url().to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    parse_published(&text, names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_real_version_shape() {
        let v = parse_version(r#"{"version":"0.741.0.7411058","clientVersionUpload":"version-02c37bc51a384b8f","bootstrapperVersion":""}"#)
            .unwrap();
        assert_eq!(v.version, "0.741.0.7411058");
        assert_eq!(v.version_hash, "version-02c37bc51a384b8f");
        assert!(parse_version(r#"{"version":"1","clientVersionUpload":"../evil"}"#).is_err());
        assert!(parse_version("<html>").is_err());
    }

    #[test]
    fn extracts_only_requested_flags() {
        let json = r#"{"applicationSettings":{"DFIntCSGLevelOfDetailSwitchingDistance":"250","FFlagHandleAltEnterFullscreenManually":"True","FFlagUnrelated":"False","FIntNum":12}}"#;
        let p = parse_published(
            json,
            &[
                "DFIntCSGLevelOfDetailSwitchingDistance",
                "FFlagHandleAltEnterFullscreenManually",
                "FIntNum",
                "FFlagMissing",
            ],
        )
        .unwrap();
        assert_eq!(p.values.len(), 3);
        assert_eq!(p.values["DFIntCSGLevelOfDetailSwitchingDistance"], "250");
        assert_eq!(p.values["FIntNum"], "12");
        assert!(!p.values.contains_key("FFlagUnrelated"));
    }
}
