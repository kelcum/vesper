//! Removes personal and secret data from text before it is logged,
//! shown in diagnostics, or copied to the clipboard.
//!
//! Vesper never reads Roblox cookies or tokens. This exists as a safety net
//! for text that passes through from elsewhere (paths, launch arguments,
//! error messages from Windows or the network).

use regex::Regex;
use std::sync::LazyLock;

struct Rule {
    re: Regex,
    replacement: &'static str,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let rule = |pattern: &str, replacement: &'static str| Rule {
        re: Regex::new(pattern).expect("valid redaction pattern"),
        replacement,
    };
    vec![
        // Roblox security cookie (value includes the well-known warning prefix).
        rule(
            r"_\|WARNING:-DO-NOT-SHARE-THIS[^|]*\|_[A-Za-z0-9]+",
            "[redacted roblox cookie]",
        ),
        rule(r"(?i)\.ROBLOSECURITY\s*[=:]\s*[^;\s]+", ".ROBLOSECURITY=[redacted]"),
        // roblox-player: launch URIs carry an authentication ticket in `gameinfo:`.
        rule(r"(?i)gameinfo:[^+\s&]+", "gameinfo:[redacted]"),
        // Discord user tokens.
        rule(
            r"\b[MNO][A-Za-z\d_-]{23,27}\.[A-Za-z\d_-]{6}\.[A-Za-z\d_-]{27,}\b",
            "[redacted discord token]",
        ),
        rule(r"(?i)\bbearer\s+[A-Za-z0-9\-._~+/]+=*", "Bearer [redacted]"),
        // key=value / key: value pairs whose key looks secret.
        rule(
            r#"(?i)\b(password|passwd|secret|token|api[_-]?key|cookie|authorization|auth[_-]?ticket)\b(\s*[=:]\s*)("[^"]*"|[^\s,;&]+)"#,
            "$1$2[redacted]",
        ),
        // Windows user profile paths reveal the account name.
        // Usernames may contain spaces, so consume everything up to the next separator.
        rule(r#"(?i)\b[A-Z]:[\\/]+Users[\\/]+[^\\/\r\n"'<>|]+"#, "%USERPROFILE%"),
    ]
});

/// Returns `text` with secrets and personal paths replaced.
pub fn redact(text: &str) -> String {
    let mut out = text.to_owned();
    for rule in RULES.iter() {
        if rule.re.is_match(&out) {
            out = rule.re.replace_all(&out, rule.replacement).into_owned();
        }
    }
    out
}

/// Replaces the user profile prefix of a path for display.
pub fn display_path(path: &std::path::Path) -> String {
    redact(&path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_profile_paths_are_hidden() {
        let s = redact(r"C:\Users\alex\AppData\Local\Roblox\Versions\version-1\RobloxPlayerBeta.exe");
        assert_eq!(s, r"%USERPROFILE%\AppData\Local\Roblox\Versions\version-1\RobloxPlayerBeta.exe");
        assert_eq!(redact("c:/users/Some One/x"), "%USERPROFILE%/x");
        assert_eq!(
            redact(r#"open "C:\Users\Some One\a.txt""#),
            r#"open "%USERPROFILE%\a.txt""#
        );
    }

    #[test]
    fn forward_slash_paths_are_hidden() {
        assert_eq!(redact("C:/Users/alice/AppData"), "%USERPROFILE%/AppData");
    }

    #[test]
    fn roblox_cookie_is_removed() {
        let cookie = "_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you-and-to-steal-your-ROBUX-and-items.|_ABCDEF0123456789";
        let s = redact(&format!("cookie value {cookie} end"));
        assert!(!s.contains("ABCDEF0123456789"));
        assert!(s.contains("[redacted roblox cookie]"));
    }

    #[test]
    fn launch_ticket_is_removed() {
        let uri = "roblox-player:1+launchmode:play+gameinfo:SECRETTICKET123+launchtime:1700000000";
        let s = redact(uri);
        assert!(!s.contains("SECRETTICKET123"));
        assert!(s.contains("gameinfo:[redacted]+launchtime:1700000000"));
    }

    #[test]
    fn secret_pairs_and_bearer_tokens_are_removed() {
        let s = redact(r#"token=abc123 password: "hunter2" Authorization: Bearer eyJhbGciOi.payload.sig"#);
        assert!(!s.contains("abc123"));
        assert!(!s.contains("hunter2"));
        assert!(!s.contains("eyJhbGciOi"));
    }

    #[test]
    fn discord_token_is_removed() {
        let token = "MTA4NjQ5MDQ2MjQ0NTUyNzA0MA.GxYz12.abcdefghijklmnopqrstuvwxyz0123456789";
        assert!(!redact(token).contains("abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn ordinary_text_is_unchanged() {
        let s = "Applied 3 flags to Roblox 0.741.0.7411058 (profile Balanced)";
        assert_eq!(redact(s), s);
    }
}
