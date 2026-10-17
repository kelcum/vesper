//! Measuring how far each Roblox city is from this PC, in milliseconds.
//!
//! Roblox's game servers can't be pinged before joining: they ignore ICMP
//! and unknown UDP, and Roblox carries traffic over its own backbone from a
//! nearby entry point, so tracing the route ends close to home. Instead,
//! Vesper times a TCP handshake (one round trip; nothing is sent after it)
//! to a public cloud endpoint in 27 cities around the world. Most Roblox
//! cities are in the same metro area as one of them (Roblox's Cape Town and
//! Zürich servers even run in those exact data centres), so their ping is
//! measured. For the others, the measurements are used to work out where this
//! connection reaches the internet, and the ping is estimated from distance.

use serde::Serialize;
use std::time::{Duration, Instant};

const ATTEMPTS: usize = 3;
const TIMEOUT: Duration = Duration::from_secs(2);
/// A city this close to a measured endpoint takes its measurement.
const SAME_METRO_KM: f64 = 160.0;

/// Cloud regions with a public endpoint, and where they are.
pub const ENDPOINTS: &[(&str, f64, f64)] = &[
    ("us-east-1", 39.04, -77.49),
    ("us-east-2", 39.96, -83.00),
    ("us-west-1", 37.35, -121.96),
    ("us-west-2", 45.84, -119.70),
    ("ca-central-1", 45.50, -73.57),
    ("mx-central-1", 20.59, -100.39),
    ("sa-east-1", -23.55, -46.63),
    ("eu-west-1", 53.35, -6.26),
    ("eu-west-2", 51.51, -0.13),
    ("eu-west-3", 48.86, 2.35),
    ("eu-central-1", 50.11, 8.68),
    ("eu-central-2", 47.37, 8.54),
    ("eu-south-1", 45.46, 9.19),
    ("eu-south-2", 41.65, -0.88),
    ("eu-north-1", 59.33, 18.07),
    ("il-central-1", 32.09, 34.78),
    ("me-central-1", 25.20, 55.27),
    ("af-south-1", -33.92, 18.42),
    ("ap-south-1", 19.08, 72.88),
    ("ap-southeast-1", 1.35, 103.82),
    ("ap-southeast-3", -6.20, 106.85),
    ("ap-southeast-5", 3.139, 101.687),
    ("ap-northeast-1", 35.68, 139.69),
    ("ap-northeast-3", 34.69, 135.50),
    ("ap-northeast-2", 37.57, 126.98),
    ("ap-east-1", 22.32, 114.17),
    ("ap-southeast-2", -33.87, 151.21),
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub lat: f64,
    pub lon: f64,
    /// Best handshake time in ms; `None` when the endpoint didn't answer.
    pub ms: Option<f64>,
}

/// Great-circle distance in km.
pub fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    6371.0 * 2.0 * a.sqrt().asin()
}

async fn handshake(host: String) -> Option<f64> {
    let addr = tokio::time::timeout(TIMEOUT, tokio::net::lookup_host((host.as_str(), 443)))
        .await
        .ok()?
        .ok()?
        .find(|a| a.is_ipv4())?;
    let mut best: Option<f64> = None;
    for _ in 0..ATTEMPTS {
        let t = Instant::now();
        if let Ok(Ok(stream)) = tokio::time::timeout(TIMEOUT, tokio::net::TcpStream::connect(addr)).await {
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            drop(stream);
            best = Some(best.map_or(ms, |b: f64| b.min(ms)));
        }
    }
    best
}

/// Times a handshake with every endpoint (in parallel, about two seconds).
pub async fn measure() -> Vec<Sample> {
    let mut set = tokio::task::JoinSet::new();
    for (i, (region, _, _)) in ENDPOINTS.iter().enumerate() {
        let host = format!("dynamodb.{region}.amazonaws.com");
        set.spawn(async move { (i, handshake(host).await) });
    }
    let mut out: Vec<Sample> =
        ENDPOINTS.iter().map(|(_, lat, lon)| Sample { lat: *lat, lon: *lon, ms: None }).collect();
    while let Some(Ok((i, ms))) = set.join_next().await {
        out[i].ms = ms;
    }
    out
}

/// Where this connection reaches the internet, fitted from the samples:
/// ping ≈ `base` + distance / `km_per_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub lat: f64,
    pub lon: f64,
    pub base: f64,
    pub km_per_ms: f64,
}

pub fn fit(samples: &[Sample]) -> Option<Model> {
    let known: Vec<(f64, f64, f64)> = samples.iter().filter_map(|s| Some((s.lat, s.lon, s.ms?))).collect();
    if known.len() < 4 {
        return None;
    }
    let mut best: Option<(f64, Model)> = None;
    let mut lat = -56.0;
    while lat <= 70.0 {
        let mut lon = -180.0;
        while lon < 180.0 {
            let d: Vec<f64> = known.iter().map(|(a, b, _)| distance_km(lat, lon, *a, *b)).collect();
            for km_per_ms in [55.0, 65.0, 75.0, 90.0] {
                let mut rest: Vec<f64> = known.iter().zip(&d).map(|((_, _, ms), d)| ms - d / km_per_ms).collect();
                rest.sort_by(f64::total_cmp);
                let base = rest[rest.len() / 2].max(0.0);
                // Relative error: nearby endpoints say most about where the connection is.
                let err: f64 =
                    known.iter().zip(&d).map(|((_, _, ms), d)| (ms - (base + d / km_per_ms)).abs() / ms.max(5.0)).sum();
                if best.as_ref().is_none_or(|(e, _)| err < *e) {
                    best = Some((err, Model { lat, lon, base, km_per_ms }));
                }
            }
            lon += 1.0;
        }
        lat += 1.0;
    }
    best.map(|(_, m)| m)
}

/// Ping to a place: measured when an endpoint is in the same metro area,
/// otherwise estimated. Returns (ms, measured).
pub fn ping_to(samples: &[Sample], model: Option<&Model>, lat: f64, lon: f64) -> Option<(u32, bool)> {
    let nearest = samples
        .iter()
        .filter_map(|s| Some((distance_km(lat, lon, s.lat, s.lon), s.ms?)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((km, ms)) = nearest.filter(|(km, _)| *km <= SAME_METRO_KM) {
        // Within a metro area, a few extra km of fibre.
        return Some(((ms + km / 100.0).round().max(1.0) as u32, true));
    }
    let m = model?;
    let ms = m.base + distance_km(m.lat, m.lon, lat, lon) / m.km_per_ms;
    Some((ms.round().max(1.0) as u32, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_are_great_circle() {
        let d = distance_km(51.51, -0.13, 48.86, 2.35); // London–Paris
        assert!((330.0..360.0).contains(&d), "{d}");
    }

    /// Real handshake times measured from a connection near Frankfurt.
    fn europe() -> Vec<Sample> {
        let ms = [
            114.0, 130.0, 176.0, 168.0, 116.0, 167.0, 199.0, 43.0, 35.0, 38.0, 29.0, 34.0, 37.0, 54.0, 38.0, 81.0,
            140.0, 179.0, 193.0, 188.0, 201.0, 192.0, 253.0, 260.0, 252.0, 210.0, 305.0,
        ];
        ENDPOINTS.iter().zip(ms).map(|((_, lat, lon), ms)| Sample { lat: *lat, lon: *lon, ms: Some(ms) }).collect()
    }

    #[test]
    fn the_connection_is_located_and_cities_estimated() {
        let s = europe();
        let m = fit(&s).unwrap();
        assert!(distance_km(m.lat, m.lon, 50.11, 8.68) < 900.0, "near Frankfurt: {m:?}");
        // Ashburn shares a metro with us-east-1: measured.
        assert_eq!(ping_to(&s, Some(&m), 39.04, -77.49), Some((114, true)));
        // Warsaw has no endpoint: estimated, and closer than New York.
        let (warsaw, measured) = ping_to(&s, Some(&m), 52.23, 21.01).unwrap();
        assert!(!measured);
        assert!(warsaw < 60, "{warsaw}");
        let (chicago, _) = ping_to(&s, Some(&m), 41.88, -87.63).unwrap();
        assert!((110..170).contains(&chicago), "{chicago}");
    }

    #[test]
    fn too_few_answers_give_no_model() {
        let mut s = europe();
        for x in s.iter_mut().skip(3) {
            x.ms = None;
        }
        assert!(fit(&s).is_none());
    }
}
