//! Validation of a set of flags before it is saved or applied.
//!
//! Errors block applying (malformed names or values, contradictory graphics
//! API choices). Warnings explain things that will work but probably not as
//! intended (a flag Roblox ignores, a value outside documented options).

use super::catalog::{Catalog, FlagStatus, InputSpec};
use super::model::{normalize_value, validate_name, FlagSet};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A one-click resolution the UI can offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Fix {
    Set { flag: String, value: String },
    Remove { flag: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub flag: String,
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub fix: Option<Fix>,
}

impl Issue {
    fn new(flag: &str, severity: Severity, code: &'static str, message: impl Into<String>) -> Self {
        Self { flag: flag.to_owned(), severity, code, message: message.into(), fix: None }
    }

    fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

/// Returns the set with every value normalised, or the first error per flag.
pub fn normalize(flags: &FlagSet) -> Result<FlagSet, Vec<Issue>> {
    let mut out = FlagSet::new();
    let mut errors = Vec::new();
    for (name, value) in flags {
        match validate_name(name) {
            Err(e) => errors.push(Issue::new(name, Severity::Error, "invalid_name", e)),
            Ok(kind) => match normalize_value(kind, value) {
                Ok(v) => {
                    out.insert(name.clone(), v);
                }
                Err(e) => errors.push(Issue::new(name, Severity::Error, "invalid_value", e)),
            },
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// Checks a flag set against syntax rules and the catalog.
pub fn validate(flags: &FlagSet, catalog: &Catalog) -> Vec<Issue> {
    let normalized = match normalize(flags) {
        Ok(n) => n,
        Err(errors) => return errors,
    };
    let mut issues = Vec::new();

    for (name, value) in &normalized {
        let Some(info) = catalog.get(name) else {
            let message = match catalog.known_ignored(name) {
                Some(k) => k.reason.clone(),
                None => "Not on Roblox's allowlist. Roblox has ignored local flags like this since \
                         September 2025. Vesper will confirm after your next launch."
                    .to_owned(),
            };
            issues.push(
                Issue::new(name, Severity::Warning, "not_allowlisted", message)
                    .with_fix(Fix::Remove { flag: name.clone() }),
            );
            continue;
        };

        match &info.input {
            InputSpec::Choice { options } if !options.iter().any(|o| &o.value == value) => {
                let allowed: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
                issues.push(Issue::new(
                    name,
                    Severity::Warning,
                    "outside_options",
                    format!("{value} isn't one of the documented values ({}).", allowed.join(", ")),
                ));
            }
            InputSpec::Number { min, max, .. } => {
                if let Ok(n) = value.parse::<i64>() {
                    if n < *min || n > *max {
                        issues.push(Issue::new(
                            name,
                            Severity::Warning,
                            "out_of_range",
                            format!("{n} is outside the usual range {min}–{max}."),
                        ));
                    }
                }
            }
            _ => {}
        }

        if info.status == FlagStatus::RemovalAnnounced {
            issues.push(Issue::new(
                name,
                Severity::Info,
                "removal_announced",
                "Roblox has said it will remove this flag. It may stop having an effect.",
            ));
        }

        for rel in &info.conflicts {
            // Report each pair once, from the alphabetically first flag.
            if value == &rel.when && normalized.get(&rel.flag) == Some(&rel.when) && name.as_str() < rel.flag.as_str() {
                issues.push(
                    Issue::new(
                        name,
                        Severity::Error,
                        "conflict",
                        format!("Conflicts with {}: {}", rel.flag, rel.reason),
                    )
                    .with_fix(Fix::Remove { flag: rel.flag.clone() }),
                );
            }
        }

        for rel in &info.requires {
            if normalized.get(&rel.flag) != Some(&rel.when) {
                issues.push(
                    Issue::new(
                        name,
                        Severity::Warning,
                        "requires",
                        format!("Has no effect unless {} is {}. {}", rel.flag, rel.when, rel.reason),
                    )
                    .with_fix(Fix::Set { flag: rel.flag.clone(), value: rel.when.clone() }),
                );
            }
        }
    }

    let num = |k: &str| normalized.get(k).and_then(|v| v.parse::<i64>().ok());
    if let (Some(min), Some(max)) = (num("FIntFRMMinGrassDistance"), num("FIntFRMMaxGrassDistance")) {
        if min > max {
            issues.push(
                Issue::new(
                    "FIntFRMMinGrassDistance",
                    Severity::Warning,
                    "grass_order",
                    format!("Minimum grass distance ({min}) is greater than the maximum ({max})."),
                )
                .with_fix(Fix::Set { flag: "FIntFRMMinGrassDistance".into(), value: max.to_string() }),
            );
        }
    }

    issues.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.flag.cmp(&b.flag)));
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(pairs: &[(&str, &str)]) -> FlagSet {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn codes(issues: &[Issue]) -> Vec<&'static str> {
        issues.iter().map(|i| i.code).collect()
    }

    #[test]
    fn clean_allowlisted_set_has_no_issues() {
        let flags = set(&[
            ("FIntDebugForceMSAASamples", "4"),
            ("DFFlagTextureQualityOverrideEnabled", "true"),
            ("DFIntTextureQualityOverride", "2"),
        ]);
        assert!(validate(&flags, Catalog::embedded()).is_empty());
    }

    #[test]
    fn malformed_names_and_values_are_errors() {
        let issues = validate(&set(&[("Not A Flag", "1"), ("FIntDebugForceMSAASamples", "four")]), Catalog::embedded());
        assert!(has_errors(&issues));
        assert_eq!(codes(&issues).len(), 2);
    }

    #[test]
    fn non_allowlisted_flags_warn_with_known_reason() {
        let issues = validate(&set(&[("DFIntTaskSchedulerTargetFps", "240")]), Catalog::embedded());
        assert_eq!(codes(&issues), vec!["not_allowlisted"]);
        assert!(issues[0].message.contains("frame-rate"));
        assert_eq!(issues[0].fix, Some(Fix::Remove { flag: "DFIntTaskSchedulerTargetFps".into() }));
        assert!(!has_errors(&issues));
    }

    #[test]
    fn graphics_api_conflict_is_reported_once() {
        let issues = validate(
            &set(&[("FFlagDebugGraphicsPreferVulkan", "True"), ("FFlagDebugGraphicsPreferOpenGL", "True")]),
            Catalog::embedded(),
        );
        assert_eq!(codes(&issues), vec!["conflict"]);
        assert!(has_errors(&issues));
    }

    #[test]
    fn disabled_api_preferences_do_not_conflict() {
        let issues = validate(
            &set(&[("FFlagDebugGraphicsPreferVulkan", "True"), ("FFlagDebugGraphicsPreferOpenGL", "False")]),
            Catalog::embedded(),
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn texture_level_requires_override_flag() {
        let issues = validate(&set(&[("DFIntTextureQualityOverride", "1")]), Catalog::embedded());
        assert_eq!(codes(&issues), vec!["requires"]);
        assert_eq!(
            issues[0].fix,
            Some(Fix::Set { flag: "DFFlagTextureQualityOverrideEnabled".into(), value: "True".into() })
        );
    }

    #[test]
    fn documented_options_and_ranges_are_checked() {
        let issues = validate(
            &set(&[("FIntDebugForceMSAASamples", "16"), ("DFIntDebugFRMQualityLevelOverride", "50")]),
            Catalog::embedded(),
        );
        let mut c = codes(&issues);
        c.sort();
        assert_eq!(c, vec!["out_of_range", "outside_options"]);
    }

    #[test]
    fn grass_minimum_above_maximum_warns() {
        let issues = validate(
            &set(&[("FIntFRMMinGrassDistance", "200"), ("FIntFRMMaxGrassDistance", "100")]),
            Catalog::embedded(),
        );
        assert_eq!(codes(&issues), vec!["grass_order"]);
    }

    #[test]
    fn removal_announced_flag_is_flagged_as_info() {
        let issues = validate(&set(&[("FFlagDebugGraphicsPreferD3D11", "True")]), Catalog::embedded());
        assert_eq!(codes(&issues), vec!["removal_announced"]);
        assert_eq!(issues[0].severity, Severity::Info);
    }

    #[test]
    fn errors_sort_before_warnings() {
        let issues = validate(
            &set(&[
                ("FFlagDebugGraphicsPreferVulkan", "True"),
                ("FFlagDebugGraphicsPreferOpenGL", "True"),
                ("FIntSomethingUnknown", "1"),
            ]),
            Catalog::embedded(),
        );
        assert_eq!(issues[0].severity, Severity::Error);
    }
}
