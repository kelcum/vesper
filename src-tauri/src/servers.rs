use crate::net::{self, Model, Sample};
use crate::roblox::web::{self, Datacenter, KnownServer};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const PING_TTL: Duration = Duration::from_secs(10 * 60);
const NEARBY_MS: u32 = 30;
const LIVE_PAGES: usize = 3;
const MAX_CITIES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ServerPick {
    #[default]
    Off,
    LowestPing,
    Region,
}

pub struct RegionDef {
    pub id: &'static str,
    pub label: &'static str,
}

pub const REGIONS: &[RegionDef] = &[
    RegionDef { id: "us-east", label: "NA East" },
    RegionDef { id: "us-central", label: "NA Central" },
    RegionDef { id: "us-west", label: "NA West" },
    RegionDef { id: "latam", label: "Latin America" },
    RegionDef { id: "eu-west", label: "Europe West" },
    RegionDef { id: "eu-east", label: "Europe East & Middle East" },
    RegionDef { id: "asia", label: "Asia" },
    RegionDef { id: "oceania", label: "Oceania" },
    RegionDef { id: "africa", label: "Africa" },
];

pub fn region_label(id: &str) -> String {
    if let Some((_, city)) = city_key(id) {
        return city.to_owned();
    }
    REGIONS.iter().find(|r| r.id == id).map_or_else(|| id.to_owned(), |r| r.label.to_owned())
}

fn city_key(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix("city:")?.split_once(':')
}

pub fn region_of(dc: &Datacenter) -> &'static str {
    match dc.country_code.as_str() {
        "US" | "CA" => match dc.region.as_str() {
            "Illinois" | "Texas" | "Missouri" | "Kansas" | "Minnesota" | "Colorado" => "us-central",
            "California" | "Oregon" | "Washington" | "Nevada" | "Arizona" | "Utah" | "British Columbia" => "us-west",
            _ => "us-east",
        },
        "MX" | "BR" | "CL" | "AR" | "PE" | "CO" => "latam",
        "GB" | "IE" | "FR" | "NL" | "DE" | "CH" | "IT" | "ES" | "BE" | "PT" | "AT" | "DK" | "SE" | "NO" | "FI"
        | "LU" => "eu-west",
        "PL" | "GR" | "TR" | "IL" | "CZ" | "RO" | "HU" | "BG" | "UA" | "AE" | "SA" | "BH" => "eu-east",
        "AU" | "NZ" => "oceania",
        "ZA" | "EG" | "NG" | "KE" => "africa",
        _ => "asia",
    }
}

fn in_choice(dc: &Datacenter, choice: &str) -> bool {
    match city_key(choice) {
        Some((cc, city)) => dc.country_code == cc && dc.city == city,
        None => region_of(dc) == choice,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CityPing {
    pub key: String,
    pub city: String,
    pub country_code: String,
    pub region: &'static str,
    pub region_label: String,
    pub ms: Option<u32>,
    pub measured: bool,
    pub servers: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub server_id: String,
    pub city: String,
    pub country_code: String,
    pub ms: Option<u32>,
    pub playing: u32,
    pub max_players: u32,
    pub fallback: bool,
}

type Pings = (Vec<Sample>, Option<Model>, Instant);

#[derive(Default)]
pub struct ServerCache {
    pings: Mutex<Option<Pings>>,
    datacenters: Mutex<Option<(Vec<Datacenter>, Instant)>>,
}

impl ServerCache {
    pub async fn datacenters(&self, http: &reqwest::Client, cache_file: &Path) -> Result<Vec<Datacenter>, String> {
        if let Some((list, at)) = self.datacenters.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if at.elapsed() < Duration::from_secs(24 * 3600) {
                return Ok(list.clone());
            }
        }
        let list = match web::datacenters(http).await {
            Ok(l) if !l.is_empty() => {
                if let Ok(json) = serde_json::to_vec(&l) {
                    let _ = crate::paths::write_atomic(cache_file, &json);
                }
                l
            }
            Ok(_) | Err(_) => {
                let saved: Option<Vec<Datacenter>> =
                    std::fs::read(cache_file).ok().and_then(|b| serde_json::from_slice(&b).ok());
                saved.ok_or("RoValra's list of Roblox server cities can't be reached right now")?
            }
        };
        *self.datacenters.lock().unwrap_or_else(|e| e.into_inner()) = Some((list.clone(), Instant::now()));
        Ok(list)
    }

    pub async fn pings(&self, refresh: bool) -> (Vec<Sample>, Option<Model>) {
        if !refresh {
            if let Some((s, m, at)) = self.pings.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                if at.elapsed() < PING_TTL {
                    return (s.clone(), *m);
                }
            }
        }
        let samples = net::measure().await;
        let fitted = samples.clone();
        let model = tauri::async_runtime::spawn_blocking(move || net::fit(&fitted)).await.ok().flatten();
        let answered = samples.iter().filter(|s| s.ms.is_some()).count();
        log::info!("Measured ping to {answered} of {} cloud regions", samples.len());
        *self.pings.lock().unwrap_or_else(|e| e.into_inner()) = Some((samples.clone(), model, Instant::now()));
        (samples, model)
    }
}

fn find_dc<'a>(dcs: &'a [Datacenter], c: &CityPing) -> Option<&'a Datacenter> {
    dcs.iter().find(|d| d.country_code == c.country_code && d.city.eq_ignore_ascii_case(&c.city))
}

fn city_ping(dc: &Datacenter, ping: Option<(u32, bool)>, servers: u32) -> CityPing {
    let region = region_of(dc);
    CityPing {
        key: format!("city:{}:{}", dc.country_code, dc.city),
        city: dc.city.clone(),
        country_code: dc.country_code.clone(),
        region,
        region_label: region_label(region),
        ms: ping.map(|p| p.0),
        measured: ping.is_some_and(|p| p.1),
        servers,
    }
}

fn by_ping(list: &mut [CityPing]) {
    list.sort_by_key(|c| (c.ms.is_none(), c.ms.unwrap_or(u32::MAX), std::cmp::Reverse(c.servers)));
}

pub async fn scan(
    http: &reqwest::Client,
    cache: &ServerCache,
    cache_file: &Path,
    place_id: Option<u64>,
    refresh: bool,
) -> Result<Vec<CityPing>, String> {
    let dcs = cache.datacenters(http, cache_file).await?;
    let counts: Option<BTreeMap<String, u32>> = match place_id {
        Some(p) => Some(web::server_counts(http, p).await?),
        None => None,
    };
    let (samples, model) = cache.pings(refresh).await;
    if samples.iter().all(|s| s.ms.is_none()) {
        return Err("Vesper couldn't reach any of the test endpoints. Check your internet connection.".into());
    }
    let mut list: Vec<CityPing> = dcs
        .iter()
        .filter_map(|dc| {
            let servers = match &counts {
                Some(c) => *c.iter().find(|(city, _)| city.eq_ignore_ascii_case(&dc.city))?.1,
                None => 0,
            };
            if counts.is_some() && servers == 0 {
                return None;
            }
            Some(city_ping(dc, net::ping_to(&samples, model.as_ref(), dc.latitude, dc.longitude), servers))
        })
        .collect();
    by_ping(&mut list);
    Ok(list)
}

pub async fn pick(
    http: &reqwest::Client,
    cache: &ServerCache,
    cache_file: &Path,
    place_id: u64,
    mode: ServerPick,
    region: &str,
) -> Result<Pick, String> {
    let cities = scan(http, cache, cache_file, Some(place_id), false).await?;
    if cities.is_empty() {
        return Err("RoValra hasn't seen servers for this game yet".into());
    }
    let dcs = cache.datacenters(http, cache_file).await?;
    let wanted: Vec<&CityPing> = match mode {
        ServerPick::Region => {
            cities.iter().filter(|c| find_dc(&dcs, c).is_some_and(|dc| in_choice(dc, region))).collect()
        }
        _ => vec![],
    };
    let fallback = mode == ServerPick::Region && wanted.is_empty();
    let order: Vec<&CityPing> = if mode == ServerPick::Region && !fallback {
        wanted
    } else {
        let best = cities.first().and_then(|c| c.ms);
        cities
            .iter()
            .filter(|c| match (best, c.ms) {
                (Some(b), Some(ms)) => ms <= b + NEARBY_MS,
                (None, _) => true,
                (Some(_), None) => false,
            })
            .collect()
    };
    let live = web::live_servers(http, place_id, LIVE_PAGES).await?;
    let live: HashMap<&str, &web::LiveServer> = live.iter().map(|s| (s.id.as_str(), s)).collect();
    for city in order.into_iter().take(MAX_CITIES) {
        let Some(dc) = find_dc(&dcs, city) else { continue };
        let known: Vec<KnownServer> = match web::servers_in_city(http, place_id, &dc.country_code, &dc.city).await {
            Ok(k) => k,
            Err(e) => {
                log::info!("RoValra servers in {}: {e}", dc.city);
                continue;
            }
        };
        let best = known
            .iter()
            .filter_map(|s| live.get(s.id.as_str()))
            .filter(|s| s.max_players == 0 || s.playing < s.max_players)
            .max_by_key(|s| s.playing);
        if let Some(s) = best {
            return Ok(Pick {
                server_id: s.id.clone(),
                city: city.city.clone(),
                country_code: city.country_code.clone(),
                ms: city.ms,
                playing: s.playing,
                max_players: s.max_players,
                fallback,
            });
        }
    }
    Err(if fallback {
        format!("this game has no servers in {} right now", region_label(region))
    } else {
        "no open server could be confirmed near you right now".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dc(city: &str, region: &str, cc: &str) -> Datacenter {
        Datacenter {
            city: city.into(),
            region: region.into(),
            country: String::new(),
            country_code: cc.into(),
            latitude: 0.0,
            longitude: 0.0,
        }
    }

    #[test]
    fn cities_fall_into_familiar_regions() {
        assert_eq!(region_of(&dc("Ashburn", "Virginia", "US")), "us-east");
        assert_eq!(region_of(&dc("New York City", "New York", "US")), "us-east");
        assert_eq!(region_of(&dc("Chicago", "Illinois", "US")), "us-central");
        assert_eq!(region_of(&dc("Dallas", "Texas", "US")), "us-central");
        assert_eq!(region_of(&dc("San Jose", "California", "US")), "us-west");
        assert_eq!(region_of(&dc("Frankfurt am Main", "Hesse", "DE")), "eu-west");
        assert_eq!(region_of(&dc("Warsaw", "Mazovia", "PL")), "eu-east");
        assert_eq!(region_of(&dc("Tokyo", "Kantō", "JP")), "asia");
        assert_eq!(region_of(&dc("Sydney", "New South Wales", "AU")), "oceania");
        assert_eq!(region_of(&dc("Santiago de Querétaro", "Querétaro", "MX")), "latam");
    }

    #[test]
    fn a_city_can_be_chosen_directly() {
        let fra = dc("Frankfurt am Main", "Hesse", "DE");
        assert!(in_choice(&fra, "city:DE:Frankfurt am Main"));
        assert!(in_choice(&fra, "eu-west"));
        assert!(!in_choice(&fra, "us-east"));
        assert_eq!(region_label("city:DE:Frankfurt am Main"), "Frankfurt am Main");
        assert_eq!(region_label("us-east"), "NA East");
    }

    #[test]
    fn unanswered_cities_sort_last() {
        let mut l = vec![
            city_ping(&dc("A", "", "US"), None, 50),
            city_ping(&dc("B", "", "US"), Some((80, true)), 1),
            city_ping(&dc("C", "", "US"), Some((30, false)), 1),
        ];
        by_ping(&mut l);
        let order: Vec<&str> = l.iter().map(|c| c.city.as_str()).collect();
        assert_eq!(order, ["C", "B", "A"]);
    }

    #[test]
    #[ignore]
    fn live_pick() {
        let http = crate::roblox::cdn::http_client();
        let cache = ServerCache::default();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("dc.json");
        tauri::async_runtime::block_on(async {
            let t = Instant::now();
            let cities = scan(&http, &cache, &file, Some(606849621), false).await.unwrap();
            for c in &cities {
                println!(
                    "{:>24} {:<4} {:<26} {:?} ms{}, {} servers",
                    c.city,
                    c.country_code,
                    c.region_label,
                    c.ms,
                    if c.measured { "" } else { " (est.)" },
                    c.servers
                );
            }
            println!("scan took {:?}", t.elapsed());
            let p = pick(&http, &cache, &file, 606849621, ServerPick::LowestPing, "").await;
            println!("lowest ping: {p:?}");
            let p = pick(&http, &cache, &file, 606849621, ServerPick::Region, "us-east").await;
            println!("NA East: {p:?}");
            let p = pick(&http, &cache, &file, 606849621, ServerPick::Region, "africa").await;
            println!("Africa: {p:?}");
        });
    }
}
