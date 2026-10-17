//! Public, anonymous lookups: game names and icons from Roblox, and where
//! Roblox servers are from RoValra's public server database.
//!
//! No cookies or accounts are involved. What each request sends:
//! * Roblox (games, thumbnails, users): the game, place or user id.
//! * RoValra (apis.rovalra.com): a place id to list its servers by city, or
//!   a Roblox server address to look up its city. Nothing about the player.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const ROVALRA: &str = "https://apis.rovalra.com";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub universe_id: u64,
    pub root_place_id: u64,
    pub name: String,
    pub creator: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

impl UserInfo {
    pub fn profile_url(id: u64) -> String {
        format!("https://www.roblox.com/users/{id}/profile")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub city: String,
    pub region: String,
    pub country: String,
    pub country_code: String,
}

impl Location {
    /// "Frankfurt am Main, DE"
    pub fn short(&self) -> String {
        if self.city.is_empty() {
            self.country.clone()
        } else {
            format!("{}, {}", self.city, self.country_code)
        }
    }
}

async fn get_json<T: for<'de> Deserialize<'de>>(http: &reqwest::Client, url: &str) -> Result<T, String> {
    let r = http.get(url).send().await.map_err(|e| format!("couldn't reach {}: {e}", host(url)))?;
    let status = r.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(format!("{} is limiting requests; try again in a minute", host(url)));
    }
    if !status.is_success() {
        return Err(format!("{} answered {status}", host(url)));
    }
    r.json::<T>().await.map_err(|e| format!("unexpected answer from {}: {e}", host(url)))
}

fn host(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default()
}

pub async fn universe_of(http: &reqwest::Client, place_id: u64) -> Result<u64, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct R {
        universe_id: Option<u64>,
    }
    let r: R = get_json(http, &format!("https://apis.roblox.com/universes/v1/places/{place_id}/universe")).await?;
    r.universe_id.ok_or_else(|| "that place doesn't exist".into())
}

pub async fn game_info(http: &reqwest::Client, universe_id: u64) -> Result<GameInfo, String> {
    #[derive(Deserialize)]
    struct List<T> {
        data: Vec<T>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Game {
        id: u64,
        root_place_id: u64,
        name: String,
        creator: Creator,
    }
    #[derive(Deserialize)]
    struct Creator {
        name: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Thumb {
        image_url: Option<String>,
        state: String,
    }
    let games: List<Game> =
        get_json(http, &format!("https://games.roblox.com/v1/games?universeIds={universe_id}")).await?;
    let g = games.data.into_iter().find(|g| g.id == universe_id).ok_or("that game isn't available")?;
    let icon = get_json::<List<Thumb>>(
        http,
        &format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={universe_id}&size=512x512&format=Png&isCircular=false&returnPolicy=PlaceHolder"
        ),
    )
    .await
    .ok()
    .and_then(|l| l.data.into_iter().next())
    .filter(|t| t.state == "Completed")
    .and_then(|t| t.image_url)
    .filter(|u| is_roblox_cdn(u));
    Ok(GameInfo { universe_id, root_place_id: g.root_place_id, name: g.name, creator: g.creator.name, icon_url: icon })
}

/// Thumbnail links must point at Roblox's image CDN.
pub fn is_roblox_cdn(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .is_some_and(|u| u.scheme() == "https" && u.host_str().is_some_and(|h| h.ends_with(".rbxcdn.com")))
}

pub async fn user_info(http: &reqwest::Client, id: u64) -> Result<UserInfo, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct U {
        id: u64,
        name: String,
        display_name: String,
    }
    let u: U = get_json(http, &format!("https://users.roblox.com/v1/users/{id}")).await?;
    Ok(UserInfo { id: u.id, name: u.name, display_name: u.display_name })
}

// ------------------------------------------------------------------ RoValra

/// Where a Roblox server address is, from RoValra's records of Roblox's network.
pub async fn locate_server(http: &reqwest::Client, ip: &str) -> Result<Location, String> {
    #[derive(Deserialize)]
    struct R {
        location: Option<L>,
    }
    #[derive(Deserialize)]
    struct L {
        #[serde(default)]
        city: String,
        #[serde(default)]
        region: String,
        #[serde(default)]
        country_name: String,
        #[serde(default)]
        country_code: String,
    }
    let ip: std::net::Ipv4Addr = ip.parse().map_err(|_| "not a server address".to_string())?;
    let r: R = get_json(http, &format!("{ROVALRA}/v1/geolocation?ip={ip}")).await?;
    let l = r.location.ok_or("RoValra doesn't know that server")?;
    Ok(Location { city: l.city, region: l.region, country: l.country_name, country_code: l.country_code })
}

/// A city with Roblox servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Datacenter {
    pub city: String,
    pub region: String,
    pub country: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
}

pub async fn datacenters(http: &reqwest::Client) -> Result<Vec<Datacenter>, String> {
    #[derive(Deserialize)]
    struct Entry {
        #[serde(default)]
        inactive: bool,
        location: L,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct L {
        city: String,
        #[serde(default)]
        region: String,
        country: String,
        #[serde(default, rename = "country_name")]
        country_name: String,
        #[serde(default, rename = "latLong")]
        lat_long: Vec<String>,
    }
    let list: Vec<Entry> = get_json(http, &format!("{ROVALRA}/v1/datacenters/list")).await?;
    let mut out: Vec<Datacenter> = list
        .into_iter()
        .filter(|e| !e.inactive && !e.location.city.is_empty())
        .map(|e| {
            let coord = |i: usize| e.location.lat_long.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            Datacenter {
                latitude: coord(0),
                longitude: coord(1),
                city: e.location.city,
                region: e.location.region,
                country: e.location.country_name,
                country_code: e.location.country,
            }
        })
        .collect();
    out.sort_by(|a, b| (&a.country_code, &a.city).cmp(&(&b.country_code, &b.city)));
    out.dedup_by(|a, b| a.city == b.city && a.country_code == b.country_code);
    Ok(out)
}

/// How many servers RoValra has seen for a place, per city ("Ashburn" → 45).
pub async fn server_counts(http: &reqwest::Client, place_id: u64) -> Result<BTreeMap<String, u32>, String> {
    #[derive(Deserialize)]
    struct R {
        counts: Option<C>,
    }
    #[derive(Deserialize)]
    struct C {
        #[serde(default)]
        detailed_regions: BTreeMap<String, Region>,
    }
    #[derive(Deserialize)]
    struct Region {
        #[serde(default)]
        cities: BTreeMap<String, u32>,
    }
    let r: R = get_json(http, &format!("{ROVALRA}/v1/servers/counts?place_id={place_id}")).await?;
    let mut out = BTreeMap::new();
    for region in r.counts.map(|c| c.detailed_regions).unwrap_or_default().into_values() {
        for (city, n) in region.cities {
            *out.entry(city).or_insert(0) += n;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownServer {
    pub id: String,
    pub ip: String,
    pub city: String,
    pub first_seen: Option<String>,
}

/// Servers RoValra has seen for a place in one city.
pub async fn servers_in_city(
    http: &reqwest::Client,
    place_id: u64,
    country_code: &str,
    city: &str,
) -> Result<Vec<KnownServer>, String> {
    #[derive(Deserialize)]
    struct R {
        #[serde(default)]
        servers: Vec<S>,
    }
    #[derive(Deserialize)]
    struct S {
        server_id: String,
        #[serde(default)]
        ip_address: String,
        #[serde(default)]
        city: String,
        first_seen: Option<String>,
    }
    let mut url = url::Url::parse(&format!("{ROVALRA}/v1/servers/region")).expect("valid url");
    url.query_pairs_mut()
        .append_pair("place_id", &place_id.to_string())
        .append_pair("country", country_code)
        .append_pair("city", city)
        .append_pair("limit", "100");
    let r: R = get_json(http, url.as_str()).await?;
    Ok(r.servers
        .into_iter()
        .filter(|s| s.server_id.len() == 36 && s.ip_address.parse::<std::net::Ipv4Addr>().is_ok())
        .map(|s| KnownServer {
            id: s.server_id.to_ascii_lowercase(),
            ip: s.ip_address,
            city: s.city,
            first_seen: s.first_seen,
        })
        .collect())
}

/// A server Roblox currently lists for a place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveServer {
    pub id: String,
    pub playing: u32,
    pub max_players: u32,
}

/// Roblox's own list of open public servers, up to `pages` pages of 100.
pub async fn live_servers(http: &reqwest::Client, place_id: u64, pages: usize) -> Result<Vec<LiveServer>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        data: Vec<S>,
        next_page_cursor: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct S {
        id: String,
        #[serde(default)]
        playing: u32,
        #[serde(default)]
        max_players: u32,
    }
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for i in 0..pages {
        let mut url = url::Url::parse(&format!("https://games.roblox.com/v1/games/{place_id}/servers/Public"))
            .expect("valid url");
        url.query_pairs_mut().append_pair("limit", "100").append_pair("excludeFullGames", "true");
        if let Some(c) = &cursor {
            url.query_pairs_mut().append_pair("cursor", c);
        }
        let page: Page = match get_json(http, url.as_str()).await {
            Ok(p) => p,
            // Later pages are a bonus; keep what was found.
            Err(e) if i > 0 => {
                log::debug!("Stopped reading the server list: {e}");
                break;
            }
            Err(e) => return Err(e),
        };
        out.extend(page.data.into_iter().map(|s| LiveServer {
            id: s.id.to_ascii_lowercase(),
            playing: s.playing,
            max_players: s.max_players,
        }));
        match page.next_page_cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_roblox_image_links_are_kept() {
        assert!(is_roblox_cdn("https://tr.rbxcdn.com/180DAY-abc/512/512/Image/Png/noFilter"));
        assert!(!is_roblox_cdn("http://tr.rbxcdn.com/x"));
        assert!(!is_roblox_cdn("https://evil.example/tr.rbxcdn.com"));
        assert!(!is_roblox_cdn("https://rbxcdn.com.evil.example/x"));
    }

    #[test]
    fn locations_read_naturally() {
        let l = Location {
            city: "Frankfurt am Main".into(),
            region: "Hesse".into(),
            country: "Germany".into(),
            country_code: "DE".into(),
        };
        assert_eq!(l.short(), "Frankfurt am Main, DE");
    }
}
