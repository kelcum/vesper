//! Errors as the user sees them.
//!
//! Commands never surface raw error strings. Every failure becomes a
//! [`Problem`]: what happened, the likely reason, what to do next, and
//! optional technical details (redacted) behind a "Copy details" button.

use crate::redact::redact;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum ProblemAction {
    OpenLogs,
    OpenConfigFolder,
    Retry,
    OpenUrl(String),
    OpenSettings(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// Stable identifier, e.g. `roblox.not_installed`. Useful in bug reports.
    pub code: String,
    pub title: String,
    pub what: String,
    pub why: String,
    pub fix: String,
    pub details: Option<String>,
    pub actions: Vec<ProblemAction>,
}

impl Problem {
    pub fn new(code: &str, title: &str) -> Self {
        Self {
            code: code.to_owned(),
            title: title.to_owned(),
            what: String::new(),
            why: String::new(),
            fix: String::new(),
            details: None,
            actions: vec![ProblemAction::OpenLogs],
        }
    }

    pub fn what(mut self, s: impl Into<String>) -> Self {
        self.what = s.into();
        self
    }

    pub fn why(mut self, s: impl Into<String>) -> Self {
        self.why = s.into();
        self
    }

    pub fn fix(mut self, s: impl Into<String>) -> Self {
        self.fix = s.into();
        self
    }

    pub fn details(mut self, s: impl std::fmt::Display) -> Self {
        self.details = Some(redact(&s.to_string()));
        self
    }

    pub fn action(mut self, a: ProblemAction) -> Self {
        if !self.actions.contains(&a) {
            self.actions.insert(0, a);
        }
        self
    }

    /// Generic fallback for unexpected I/O failures.
    pub fn io(context: &str, err: &std::io::Error) -> Self {
        let why = match err.kind() {
            std::io::ErrorKind::PermissionDenied if cfg!(target_os = "macos") => {
                "macOS refused access. The file may be read-only, or macOS may need your permission for Vesper to change it."
            }
            std::io::ErrorKind::PermissionDenied => {
                "Windows refused access. Another program may be using the file, or it is read-only."
            }
            std::io::ErrorKind::NotFound => "A file or folder Vesper expected is missing.",
            _ if cfg!(target_os = "macos") => "macOS reported an unexpected file error.",
            _ => "Windows reported an unexpected file error.",
        };
        Problem::new("io", "Something went wrong")
            .what(context.to_owned())
            .why(why)
            .fix("Try again. If it keeps happening, copy the details and send them through xan.gripe/k3.")
            .details(format!("{err} ({:?})", err.kind()))
            .action(ProblemAction::Retry)
    }

    pub fn internal(context: &str, detail: impl std::fmt::Display) -> Self {
        Problem::new("internal", "Something went wrong")
            .what(context.to_owned())
            .why("Vesper hit an unexpected state.")
            .fix("Try again. If it keeps happening, copy the details and send them through xan.gripe/k3.")
            .details(detail)
    }

    pub fn invalid(what: impl Into<String>, fix: impl Into<String>) -> Self {
        Problem::new("invalid_input", "That value can't be used")
            .what(what)
            .why("The value didn't pass validation.")
            .fix(fix)
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.title, self.what)?;
        if let Some(d) = &self.details {
            write!(f, " ({d})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Problem {}

pub type Result<T> = std::result::Result<T, Problem>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_are_redacted() {
        let p = Problem::internal("x", r"failed at C:\Users\someone\AppData\file");
        assert_eq!(p.details.as_deref(), Some(r"failed at %USERPROFILE%\AppData\file"));
    }

    #[test]
    fn io_problem_explains_permission_errors() {
        let e = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let p = Problem::io("Couldn't save settings", &e);
        assert!(p.why.contains("refused access"));
        assert_eq!(p.actions.first(), Some(&ProblemAction::Retry));
    }

    #[test]
    fn actions_are_not_duplicated() {
        let p = Problem::new("a", "b").action(ProblemAction::OpenLogs);
        assert_eq!(p.actions.iter().filter(|a| **a == ProblemAction::OpenLogs).count(), 1);
    }
}
