//! Discord Rich Presence, built in.

pub mod ipc;
pub mod presence;

/// Vesper's Discord application ID, set at build time with `VESPER_DISCORD_CLIENT_ID`.
/// Application IDs are public (they appear in every presence); this is not a secret.
pub fn client_id() -> Option<String> {
    option_env!("VESPER_DISCORD_CLIENT_ID")
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_owned)
}
