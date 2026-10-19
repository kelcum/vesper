use serde::Serialize;
use url::Url;

pub const MAX_LAUNCH_DATA_BYTES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LaunchTarget {
    App,
    #[serde(rename_all = "camelCase")]
    Experience {
        place_id: u64,
        game_instance_id: Option<String>,
        link_code: Option<String>,
        launch_data: Option<String>,
    },
    ShareLink { url: String },
}

const HELP: &str = "Paste a Roblox game link, a place ID, or a roblox:// link.";

pub fn parse(input: &str) -> Result<LaunchTarget, String> {
    let s = input.trim();
    if s.is_empty() {
        return Ok(LaunchTarget::App);
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return experience(s, None, None, None);
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("roblox://") {
        return parse_app_link(&s["roblox://".len()..]);
    }
    if lower.starts_with("roblox-player:") {
        return Err("Website launch links contain a one-time login ticket and can't be reused. \
                    Paste the game's page link instead."
            .into());
    }
    let with_scheme = if lower.starts_with("http://") || lower.starts_with("https://") {
        s.to_owned()
    } else {
        format!("https://{s}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| HELP.to_string())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host == "ro.blox.com" {
        return Ok(LaunchTarget::ShareLink { url: url.to_string() });
    }
    if host != "roblox.com" && host != "www.roblox.com" && host != "web.roblox.com" {
        return Err(HELP.into());
    }
    let segments: Vec<&str> = url.path_segments().map(|s| s.filter(|p| !p.is_empty()).collect()).unwrap_or_default();
    let query = |key: &str| url.query_pairs().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.into_owned());
    match segments.as_slice() {
        ["share", ..] | ["share-links", ..] => Ok(LaunchTarget::ShareLink { url: url.to_string() }),
        ["games", "start"] => {
            let place = query("placeId").ok_or_else(|| HELP.to_string())?;
            experience(&place, query("gameInstanceId"), query("linkCode"), query("launchData"))
        }
        ["games", id, ..] => experience(id, None, query("privateServerLinkCode"), None),
        _ => Err(HELP.into()),
    }
}

fn parse_app_link(rest: &str) -> Result<LaunchTarget, String> {
    let rest = rest.trim_end_matches('/');
    let query =
        rest.strip_prefix("experiences/start?").or_else(|| rest.strip_prefix("experiences/start/?")).unwrap_or(rest);
    if query.is_empty() {
        return Ok(LaunchTarget::App);
    }
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
    let get = |key: &str| pairs.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone());
    let place = get("placeId").ok_or_else(|| HELP.to_string())?;
    experience(&place, get("gameInstanceId"), get("linkCode"), get("launchData"))
}

fn experience(
    place: &str,
    instance: Option<String>,
    link_code: Option<String>,
    launch_data: Option<String>,
) -> Result<LaunchTarget, String> {
    let place_id: u64 =
        place.parse().ok().filter(|n| *n > 0).ok_or_else(|| "That place ID isn't a valid number.".to_string())?;
    let game_instance_id = match instance.filter(|s| !s.is_empty()) {
        Some(id) if is_guid(&id) => Some(id.to_ascii_lowercase()),
        Some(_) => return Err("The server ID must look like 1a2b3c4d-....".into()),
        None => None,
    };
    let link_code = match link_code.filter(|s| !s.is_empty()) {
        Some(c) if c.len() <= 64 && c.chars().all(|ch| ch.is_ascii_digit()) => Some(c),
        Some(_) => return Err("That private server link code isn't valid.".into()),
        None => None,
    };
    let launch_data = match launch_data.filter(|s| !s.is_empty()) {
        Some(d) if d.len() <= MAX_LAUNCH_DATA_BYTES => Some(d),
        Some(_) => return Err(format!("Launch data can be at most {MAX_LAUNCH_DATA_BYTES} bytes.")),
        None => None,
    };
    Ok(LaunchTarget::Experience { place_id, game_instance_id, link_code, launch_data })
}

fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn launch_argument(target: &LaunchTarget) -> Option<String> {
    match target {
        LaunchTarget::App | LaunchTarget::ShareLink { .. } => None,
        LaunchTarget::Experience { place_id, game_instance_id, link_code, launch_data } => {
            let mut s = format!("roblox://placeId={place_id}");
            if let Some(id) = game_instance_id {
                s.push_str(&format!("&gameInstanceId={id}"));
            }
            if let Some(code) = link_code {
                s.push_str(&format!("&linkCode={code}"));
            }
            if let Some(data) = launch_data {
                let encoded: String = url::form_urlencoded::byte_serialize(data.as_bytes()).collect();
                s.push_str(&format!("&launchData={encoded}"));
            }
            Some(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(id: u64) -> LaunchTarget {
        LaunchTarget::Experience { place_id: id, game_instance_id: None, link_code: None, launch_data: None }
    }

    #[test]
    fn empty_input_opens_the_app() {
        assert_eq!(parse("  ").unwrap(), LaunchTarget::App);
        assert_eq!(launch_argument(&LaunchTarget::App), None);
    }

    #[test]
    fn place_ids_and_game_urls() {
        assert_eq!(parse("920587237").unwrap(), place(920587237));
        assert_eq!(parse("https://www.roblox.com/games/920587237/Adopt-Me").unwrap(), place(920587237));
        assert_eq!(parse("roblox.com/games/920587237").unwrap(), place(920587237));
        assert!(parse("0").is_err());
    }

    #[test]
    fn private_server_links_keep_the_link_code() {
        let t = parse("https://www.roblox.com/games/123/Name?privateServerLinkCode=0123456789").unwrap();
        assert_eq!(launch_argument(&t).unwrap(), "roblox://placeId=123&linkCode=0123456789");
        assert!(parse("https://www.roblox.com/games/123?privateServerLinkCode=abc").is_err());
    }

    #[test]
    fn start_links_and_app_links() {
        let guid = "1A2B3C4D-1111-2222-3333-444455556666";
        let t = parse(&format!("https://www.roblox.com/games/start?placeId=5&gameInstanceId={guid}")).unwrap();
        assert_eq!(launch_argument(&t).unwrap(), format!("roblox://placeId=5&gameInstanceId={}", guid.to_lowercase()));
        assert_eq!(parse("roblox://placeId=77").unwrap(), place(77));
        assert_eq!(parse("roblox://experiences/start?placeId=77").unwrap(), place(77));
        assert_eq!(parse("roblox://").unwrap(), LaunchTarget::App);
    }

    #[test]
    fn launch_data_is_limited_and_encoded() {
        let t = parse("roblox://placeId=1&launchData=hello%20world%26x").unwrap();
        assert_eq!(launch_argument(&t).unwrap(), "roblox://placeId=1&launchData=hello+world%26x");
        let long = "a".repeat(201);
        assert!(parse(&format!("roblox://placeId=1&launchData={long}")).is_err());
    }

    #[test]
    fn hostile_input_is_rejected_not_passed_through() {
        assert!(parse("roblox://placeId=1 --some-flag").is_err());
        assert!(parse("roblox://placeId=1&gameInstanceId=\"; calc.exe").is_err());
        assert!(parse("https://evil.example.com/games/1").is_err());
        assert!(parse("file:///C:/Windows/System32/calc.exe").is_err());
    }

    #[test]
    fn website_tickets_are_refused() {
        let e = parse("roblox-player:1+launchmode:play+gameinfo:SECRET").unwrap_err();
        assert!(e.contains("one-time"));
    }

    #[test]
    fn share_links_go_to_the_browser() {
        assert!(matches!(
            parse("https://www.roblox.com/share?code=abcdef&type=Server").unwrap(),
            LaunchTarget::ShareLink { .. }
        ));
        assert!(matches!(parse("https://ro.blox.com/Ebh5?af_dp=x").unwrap(), LaunchTarget::ShareLink { .. }));
    }
}
