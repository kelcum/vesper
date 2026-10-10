//! The flag catalog: what Vesper knows about each Fast Flag, and how sure it is.
//!
//! The catalog lives in `resources/flags/catalog.json` and is compiled into
//! the binary. It only describes flags on Roblox's published allowlist.
//! Each entry records its sources, a confidence level for the described
//! behaviour, and any Roblox-published default value.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub const CATALOG_JSON: &str = include_str!("../../resources/flags/catalog.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub schema: u32,
    pub reviewed: String,
    pub allowlist: AllowlistInfo,
    pub sources: BTreeMap<String, Source>,
    pub flags: Vec<FlagInfo>,
    pub known_ignored: Vec<KnownIgnored>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowlistInfo {
    pub title: String,
    pub url: String,
    pub publisher: String,
    pub effective: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

/// How confident the catalog is about what a flag does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Behavior {
    /// Consistent across multiple independent sources (launchers, Roblox staff posts).
    Established,
    /// Name and allowlist status are verified, but the exact effect is not documented.
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlagStatus {
    Active,
    RemovalAnnounced,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum InputSpec {
    Toggle,
    Choice { options: Vec<ChoiceOption> },
    Number { min: i64, max: i64, step: i64 },
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedDefault {
    pub value: String,
    /// Key into `sources`.
    pub source: String,
    pub observed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub flag: String,
    #[serde(alias = "value")]
    pub when: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub roblox_version: String,
    pub result: String,
    pub date: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagInfo {
    pub name: String,
    pub title: String,
    pub summary: String,
    pub details: String,
    pub categories: Vec<String>,
    pub risk: Risk,
    pub behavior: Behavior,
    pub input: InputSpec,
    pub roblox_default: Option<PublishedDefault>,
    pub status: FlagStatus,
    #[serde(default)]
    pub conflicts: Vec<Relation>,
    #[serde(default)]
    pub requires: Vec<Relation>,
    pub references: Vec<String>,
    #[serde(default)]
    pub observations: Vec<Observation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownIgnored {
    pub name: String,
    pub reason: String,
    #[serde(default)]
    pub replacement: Option<String>,
}

pub const CATEGORIES: &[&str] =
    &["rendering", "graphics", "performance", "networking", "physics", "ui", "debug", "experimental", "engine", "misc"];

static CATALOG: LazyLock<Catalog> =
    LazyLock::new(|| serde_json::from_str(CATALOG_JSON).expect("embedded flag catalog is valid"));

impl Catalog {
    pub fn embedded() -> &'static Catalog {
        &CATALOG
    }

    pub fn get(&self, name: &str) -> Option<&FlagInfo> {
        self.flags.iter().find(|f| f.name == name)
    }

    pub fn is_allowlisted(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn known_ignored(&self, name: &str) -> Option<&KnownIgnored> {
        self.known_ignored.iter().find(|k| k.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::model::{kind_of, ValueKind};

    /// The allowlist as published by Roblox on the DevForum (checked 2026-10-07).
    const PUBLISHED_ALLOWLIST: &[&str] = &[
        "DFIntCSGLevelOfDetailSwitchingDistance",
        "DFIntCSGLevelOfDetailSwitchingDistanceL12",
        "DFIntCSGLevelOfDetailSwitchingDistanceL23",
        "DFIntCSGLevelOfDetailSwitchingDistanceL34",
        "FFlagHandleAltEnterFullscreenManually",
        "DFFlagTextureQualityOverrideEnabled",
        "DFIntTextureQualityOverride",
        "FIntDebugForceMSAASamples",
        "DFFlagDisableDPIScale",
        "FFlagDebugGraphicsPreferD3D11",
        "FFlagDebugSkyGray",
        "DFFlagDebugPauseVoxelizer",
        "DFIntDebugFRMQualityLevelOverride",
        "FIntFRMMaxGrassDistance",
        "FIntFRMMinGrassDistance",
        "FFlagDebugGraphicsPreferVulkan",
        "FFlagDebugGraphicsPreferOpenGL",
        "FIntGrassMovementReducedMotionFactor",
    ];

    #[test]
    fn catalog_matches_published_allowlist_exactly() {
        let c = Catalog::embedded();
        let mut ours: Vec<&str> = c.flags.iter().map(|f| f.name.as_str()).collect();
        let mut published = PUBLISHED_ALLOWLIST.to_vec();
        ours.sort();
        published.sort();
        assert_eq!(ours, published);
    }

    #[test]
    fn entries_are_internally_consistent() {
        let c = Catalog::embedded();
        for f in &c.flags {
            let kind = kind_of(&f.name).unwrap_or_else(|| panic!("{} has no valid prefix", f.name));
            match (&f.input, kind) {
                (InputSpec::Toggle, ValueKind::Bool) => {}
                (InputSpec::Choice { .. } | InputSpec::Number { .. }, ValueKind::Int) => {}
                (InputSpec::Text, ValueKind::String) => {}
                (input, kind) => panic!("{}: input {input:?} doesn't suit {kind:?}", f.name),
            }
            assert!(!f.references.is_empty(), "{} needs at least one source", f.name);
            assert!(f.references.contains(&"allowlist".to_string()), "{} must cite the allowlist", f.name);
            for r in &f.references {
                assert!(c.sources.contains_key(r), "{}: unknown source {r}", f.name);
            }
            if let Some(d) = &f.roblox_default {
                assert!(c.sources.contains_key(&d.source), "{}: unknown default source", f.name);
            }
            for cat in &f.categories {
                assert!(CATEGORIES.contains(&cat.as_str()), "{}: unknown category {cat}", f.name);
            }
            for rel in f.conflicts.iter().chain(f.requires.iter()) {
                assert!(c.get(&rel.flag).is_some(), "{}: relation to unknown flag {}", f.name, rel.flag);
            }
            if let InputSpec::Number { min, max, .. } = f.input {
                assert!(min <= max, "{}: min > max", f.name);
            }
            if f.behavior == Behavior::Uncertain {
                let text = format!("{} {}", f.summary, f.details).to_lowercase();
                assert!(
                    ["reported", "documented", "isn't", "aren't", "hasn't", "commonly"]
                        .iter()
                        .any(|w| text.contains(w)),
                    "{}: uncertain behaviour must be described as such",
                    f.name
                );
            }
        }
    }

    #[test]
    fn known_ignored_flags_are_not_in_the_catalog() {
        let c = Catalog::embedded();
        for k in &c.known_ignored {
            assert!(!c.is_allowlisted(&k.name), "{} is both allowlisted and ignored", k.name);
        }
        assert!(c.known_ignored("DFIntTaskSchedulerTargetFps").is_some());
    }
}
