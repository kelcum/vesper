pub mod ipc;
pub mod presence;

const APPLICATION_ID: &str = "1558555027126100138";

pub fn client_id() -> Option<String> {
    let id = option_env!("VESPER_DISCORD_CLIENT_ID").map(str::trim).filter(|s| !s.is_empty()).unwrap_or(APPLICATION_ID);
    id.chars().all(|c| c.is_ascii_digit()).then(|| id.to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_build_has_an_application_id() {
        let id = super::client_id().expect("an application ID");
        assert!(id.len() >= 17 && id.chars().all(|c| c.is_ascii_digit()), "{id}");
    }
}
