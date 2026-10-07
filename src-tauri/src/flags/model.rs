//! Fast Flag names and values.
//!
//! Roblox infers a flag's type from its name prefix and stores every value
//! as a string in ClientAppSettings.json ("True", "250", "text").

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Flags as written to ClientAppSettings.json: name → string value.
pub type FlagSet = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueKind {
    Bool,
    Int,
    String,
}

pub const MAX_NAME_LEN: usize = 128;
pub const MAX_STRING_LEN: usize = 1024;

/// Determines the value type from the flag name prefix.
///
/// Prefixes: `FFlag`/`DFFlag`/`SFFlag` (bool), `FInt`/`DFInt`/`FLog`/`DFLog` (int),
/// `FString`/`DFString` (string).
pub fn kind_of(name: &str) -> Option<ValueKind> {
    let rest = name
        .strip_prefix("DF")
        .or_else(|| name.strip_prefix("SF"))
        .or_else(|| name.strip_prefix('F'))?;
    let (kind, suffix) = if let Some(s) = rest.strip_prefix("Flag") {
        (ValueKind::Bool, s)
    } else if let Some(s) = rest.strip_prefix("Int") {
        (ValueKind::Int, s)
    } else if let Some(s) = rest.strip_prefix("Log") {
        (ValueKind::Int, s)
    } else if let Some(s) = rest.strip_prefix("String") {
        (ValueKind::String, s)
    } else {
        return None;
    };
    (!suffix.is_empty()).then_some(kind)
}

/// Checks a flag name and returns its value type.
pub fn validate_name(name: &str) -> Result<ValueKind, String> {
    if name.is_empty() {
        return Err("Flag name is empty.".into());
    }
    if name.len() > MAX_NAME_LEN {
        return Err(format!("Flag names are at most {MAX_NAME_LEN} characters."));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Flag names may only contain letters, digits and underscores.".into());
    }
    kind_of(name).ok_or_else(|| {
        "Flag names start with FFlag, DFFlag, FInt, DFInt, FString, DFString, FLog or DFLog.".into()
    })
}

/// Normalises a value for its type, e.g. `true` → `True`, ` 042 ` → `42`.
pub fn normalize_value(kind: ValueKind, raw: &str) -> Result<String, String> {
    let v = raw.trim();
    match kind {
        ValueKind::Bool => match v.to_ascii_lowercase().as_str() {
            "true" => Ok("True".into()),
            "false" => Ok("False".into()),
            _ => Err("Use True or False.".into()),
        },
        ValueKind::Int => v
            .parse::<i64>()
            .ok()
            .filter(|n| i32::try_from(*n).is_ok())
            .map(|n| n.to_string())
            .ok_or_else(|| "Use a whole number between -2147483648 and 2147483647.".into()),
        ValueKind::String => {
            if raw.len() > MAX_STRING_LEN {
                Err(format!("Text values are at most {MAX_STRING_LEN} characters."))
            } else if raw.chars().any(char::is_control) {
                Err("Text values can't contain control characters.".into())
            } else {
                Ok(raw.to_owned())
            }
        }
    }
}

/// Converts any JSON scalar (as found in imported files) to Roblox's string form.
pub fn json_to_flag_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Bool(b) => Some(if *b { "True" } else { "False" }.into()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_follow_prefixes() {
        assert_eq!(kind_of("FFlagDebugSkyGray"), Some(ValueKind::Bool));
        assert_eq!(kind_of("DFFlagDisableDPIScale"), Some(ValueKind::Bool));
        assert_eq!(kind_of("SFFlagSomething"), Some(ValueKind::Bool));
        assert_eq!(kind_of("FIntDebugForceMSAASamples"), Some(ValueKind::Int));
        assert_eq!(kind_of("DFIntTextureQualityOverride"), Some(ValueKind::Int));
        assert_eq!(kind_of("FLogNetwork"), Some(ValueKind::Int));
        assert_eq!(kind_of("DFStringSomething"), Some(ValueKind::String));
        assert_eq!(kind_of("FFlag"), None, "prefix alone is not a flag");
        assert_eq!(kind_of("GraphicsQuality"), None);
        assert_eq!(kind_of("DFDouble"), None);
    }

    #[test]
    fn names_are_validated() {
        assert!(validate_name("FFlagDebugSkyGray").is_ok());
        assert!(validate_name("FFlag Debug").is_err());
        assert!(validate_name("").is_err());
        assert!(validate_name(&format!("FFlag{}", "a".repeat(200))).is_err());
        assert!(validate_name("Debug").is_err());
    }

    #[test]
    fn values_are_normalised() {
        assert_eq!(normalize_value(ValueKind::Bool, " true ").unwrap(), "True");
        assert_eq!(normalize_value(ValueKind::Bool, "FALSE").unwrap(), "False");
        assert!(normalize_value(ValueKind::Bool, "1").is_err());
        assert_eq!(normalize_value(ValueKind::Int, " 042 ").unwrap(), "42");
        assert_eq!(normalize_value(ValueKind::Int, "-1").unwrap(), "-1");
        assert!(normalize_value(ValueKind::Int, "2147483648").is_err());
        assert!(normalize_value(ValueKind::Int, "1.5").is_err());
        assert_eq!(normalize_value(ValueKind::String, "a b").unwrap(), "a b");
        assert!(normalize_value(ValueKind::String, "a\nb").is_err());
    }

    #[test]
    fn json_scalars_convert_to_roblox_strings() {
        assert_eq!(json_to_flag_string(&serde_json::json!(true)).unwrap(), "True");
        assert_eq!(json_to_flag_string(&serde_json::json!(250)).unwrap(), "250");
        assert_eq!(json_to_flag_string(&serde_json::json!("x")).unwrap(), "x");
        assert!(json_to_flag_string(&serde_json::json!([1])).is_none());
    }
}
