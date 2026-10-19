use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use std::path::Path;

pub const SKY_DIR: &str = r"PlatformContent\pc\textures\sky";
pub const FACES: [&str; 6] = ["bk", "dn", "ft", "lf", "rt", "up"];
pub const FACE_SIZE: u32 = 1024;

pub fn face_rel(face: &str) -> String {
    format!(r"{SKY_DIR}\sky512_{face}.tex")
}

pub const PRESETS: &[(&str, &str)] =
    &[("midnight", "Midnight"), ("sunset", "Sunset"), ("dawn", "Dawn"), ("overcast", "Overcast"), ("void", "Void")];

fn to565(c: [f32; 3]) -> u16 {
    let r = (c[0].clamp(0.0, 255.0) * 31.0 / 255.0).round() as u16;
    let g = (c[1].clamp(0.0, 255.0) * 63.0 / 255.0).round() as u16;
    let b = (c[2].clamp(0.0, 255.0) * 31.0 / 255.0).round() as u16;
    (r << 11) | (g << 5) | b
}

fn from565(c: u16) -> [f32; 3] {
    [((c >> 11) & 31) as f32 * 255.0 / 31.0, ((c >> 5) & 63) as f32 * 255.0 / 63.0, (c & 31) as f32 * 255.0 / 31.0]
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

fn encode_block(px: &[[f32; 3]; 16]) -> [u8; 8] {
    let mean = px.iter().fold([0.0f32; 3], |m, p| [m[0] + p[0] / 16.0, m[1] + p[1] / 16.0, m[2] + p[2] / 16.0]);
    let mut cov = [[0.0f32; 3]; 3];
    for p in px {
        let d = [p[0] - mean[0], p[1] - mean[1], p[2] - mean[2]];
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    let mut axis = [1.0f32, 1.0, 1.0];
    for _ in 0..8 {
        let n = [
            cov[0][0] * axis[0] + cov[0][1] * axis[1] + cov[0][2] * axis[2],
            cov[1][0] * axis[0] + cov[1][1] * axis[1] + cov[1][2] * axis[2],
            cov[2][0] * axis[0] + cov[2][1] * axis[1] + cov[2][2] * axis[2],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len < 1e-6 {
            break;
        }
        axis = [n[0] / len, n[1] / len, n[2] / len];
    }
    let proj = |p: &[f32; 3]| (p[0] - mean[0]) * axis[0] + (p[1] - mean[1]) * axis[1] + (p[2] - mean[2]) * axis[2];
    let (lo, hi) = px.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(proj(p)), hi.max(proj(p))));
    let at = |t: f32| [mean[0] + axis[0] * t, mean[1] + axis[1] * t, mean[2] + axis[2] * t];
    let (mut c0, mut c1) = (to565(at(hi)), to565(at(lo)));
    if c0 < c1 {
        std::mem::swap(&mut c0, &mut c1);
    }
    if c0 == c1 {
        return [c0 as u8, (c0 >> 8) as u8, c1 as u8, (c1 >> 8) as u8, 0, 0, 0, 0];
    }
    let (a, b) = (from565(c0), from565(c1));
    let palette = [
        a,
        b,
        [(2.0 * a[0] + b[0]) / 3.0, (2.0 * a[1] + b[1]) / 3.0, (2.0 * a[2] + b[2]) / 3.0],
        [(a[0] + 2.0 * b[0]) / 3.0, (a[1] + 2.0 * b[1]) / 3.0, (a[2] + 2.0 * b[2]) / 3.0],
    ];
    let mut indices = 0u32;
    for (i, p) in px.iter().enumerate() {
        let best = (0..4).min_by(|&x, &y| dist2(*p, palette[x]).total_cmp(&dist2(*p, palette[y]))).unwrap_or(0);
        indices |= (best as u32) << (2 * i);
    }
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    out[4..8].copy_from_slice(&indices.to_le_bytes());
    out
}

fn encode_level(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut out = Vec::with_capacity((w.div_ceil(4) * h.div_ceil(4) * 8) as usize);
    for by in (0..h.max(1)).step_by(4) {
        for bx in (0..w.max(1)).step_by(4) {
            let mut px = [[0.0f32; 3]; 16];
            for (k, p) in px.iter_mut().enumerate() {
                let x = (bx + (k as u32 % 4)).min(w - 1);
                let y = (by + (k as u32 / 4)).min(h - 1);
                let c = img.get_pixel(x, y).0;
                *p = [c[0] as f32, c[1] as f32, c[2] as f32];
            }
            out.extend_from_slice(&encode_block(&px));
        }
    }
    out
}

pub fn encode_dds(face: &RgbaImage) -> Vec<u8> {
    let size = face.width();
    let levels = 32 - size.leading_zeros();
    let mut out = Vec::new();
    out.extend_from_slice(b"DDS ");
    let mut header = [0u32; 31];
    header[0] = 124;
    header[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000 | 0x80000;
    header[2] = size;
    header[3] = size;
    header[4] = size.div_ceil(4) * size.div_ceil(4) * 8;
    header[6] = levels;
    header[18] = 32;
    header[19] = 0x4;
    header[20] = u32::from_le_bytes(*b"DXT1");
    header[26] = 0x1000 | 0x8 | 0x400000;
    for v in header {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let mut level = face.clone();
    for i in 0..levels {
        out.extend_from_slice(&encode_level(&level));
        if i + 1 < levels {
            let next = (level.width() / 2).max(1);
            level = image::imageops::resize(&level, next, next, FilterType::Triangle);
        }
    }
    out
}

pub fn decode_dds(bytes: &[u8]) -> Option<RgbaImage> {
    if bytes.len() < 128 || &bytes[0..4] != b"DDS " || &bytes[84..88] != b"DXT1" {
        return None;
    }
    let h = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    let w = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let data = &bytes[128..];
    if data.len() < (w.div_ceil(4) * h.div_ceil(4) * 8) as usize {
        return None;
    }
    let mut img = RgbaImage::new(w, h);
    let mut i = 0;
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let block = &data[i..i + 8];
            i += 8;
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let idx = u32::from_le_bytes(block[4..8].try_into().ok()?);
            let (a, b) = (from565(c0), from565(c1));
            let pal = if c0 > c1 {
                [
                    a,
                    b,
                    [(2.0 * a[0] + b[0]) / 3.0, (2.0 * a[1] + b[1]) / 3.0, (2.0 * a[2] + b[2]) / 3.0],
                    [(a[0] + 2.0 * b[0]) / 3.0, (a[1] + 2.0 * b[1]) / 3.0, (a[2] + 2.0 * b[2]) / 3.0],
                ]
            } else {
                [a, b, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0], [0.0; 3]]
            };
            for k in 0..16u32 {
                let (x, y) = (bx + k % 4, by + k / 4);
                if x < w && y < h {
                    let c = pal[((idx >> (2 * k)) & 3) as usize];
                    img.put_pixel(x, y, Rgba([c[0] as u8, c[1] as u8, c[2] as u8, 255]));
                }
            }
        }
    }
    Some(img)
}

fn direction(face: &str, u: f32, v: f32) -> [f32; 3] {
    let (a, b) = (2.0 * u - 1.0, 1.0 - 2.0 * v);
    let d = match face {
        "ft" => [a, b, -1.0],
        "lf" => [1.0, b, a],
        "bk" => [-a, b, 1.0],
        "rt" => [-1.0, b, -a],
        "up" => {
            let (ru, rv) = (1.0 - v, u);
            [2.0 * ru - 1.0, 1.0, 1.0 - 2.0 * rv]
        }
        _ => {
            let (ru, rv) = (v, 1.0 - u);
            [2.0 * ru - 1.0, -1.0, 2.0 * rv - 1.0]
        }
    };
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    [d[0] / len, d[1] / len, d[2] / len]
}

pub fn render(size: u32, color: &(dyn Fn([f32; 3]) -> [f32; 3] + Sync)) -> Vec<(&'static str, RgbaImage)> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = FACES
            .iter()
            .map(|&face| {
                s.spawn(move || {
                    let mut img = RgbaImage::new(size, size);
                    for (x, y, p) in img.enumerate_pixels_mut() {
                        let d = direction(face, (x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
                        let c = color(d);
                        *p = Rgba([
                            c[0].clamp(0.0, 255.0) as u8,
                            c[1].clamp(0.0, 255.0) as u8,
                            c[2].clamp(0.0, 255.0) as u8,
                            255,
                        ]);
                    }
                    (face, img)
                })
            })
            .collect();
        jobs.into_iter().map(|j| j.join().expect("face render")).collect()
    })
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn hex(c: u32) -> [f32; 3] {
    [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32]
}

fn gradient(d: [f32; 3], ground: u32, horizon: u32, mid: u32, zenith: u32) -> [f32; 3] {
    let y = d[1];
    if y < 0.0 {
        return mix(hex(horizon), hex(ground), (-y * 4.0).powf(0.6));
    }
    if y < 0.25 {
        mix(hex(horizon), hex(mid), y / 0.25)
    } else {
        mix(hex(mid), hex(zenith), ((y - 0.25) / 0.75).powf(0.8))
    }
}

fn stars(d: [f32; 3]) -> f32 {
    if d[1] < 0.02 {
        return 0.0;
    }
    let scale = 260.0;
    let cell = [(d[0] * scale).floor(), (d[1] * scale).floor(), (d[2] * scale).floor()];
    let mut h = (cell[0] as i32 as u32).wrapping_mul(73_856_093)
        ^ (cell[1] as i32 as u32).wrapping_mul(19_349_663)
        ^ (cell[2] as i32 as u32).wrapping_mul(83_492_791);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    if h % 1000 >= 7 {
        return 0.0;
    }
    let centre = [cell[0] + 0.5, cell[1] + 0.5, cell[2] + 0.5];
    let dist =
        ((d[0] * scale - centre[0]).powi(2) + (d[1] * scale - centre[1]).powi(2) + (d[2] * scale - centre[2]).powi(2))
            .sqrt();
    let brightness = 0.35 + (h % 650) as f32 / 1000.0;
    (1.0 - dist * 1.6).max(0.0) * brightness * (d[1] * 3.0).min(1.0)
}

fn glow(d: [f32; 3], toward: [f32; 3], size: f32) -> f32 {
    let dot = d[0] * toward[0] + d[1] * toward[1] + d[2] * toward[2];
    ((dot - 1.0) / size).exp()
}

pub fn preset_color(id: &str) -> Option<fn([f32; 3]) -> [f32; 3]> {
    Some(match id {
        "midnight" => |d| {
            let base = gradient(d, 0x05060c, 0x1d2550, 0x111739, 0x04050d);
            let s = stars(d) * 255.0;
            [base[0] + s, base[1] + s, base[2] + s * 0.95]
        },
        "sunset" => |d| {
            let base = gradient(d, 0x2a1a2a, 0xffa05a, 0xd8667a, 0x2c2f6b);
            let sun = [0.0, 0.08, -0.997];
            let g = glow(d, sun, 0.02) * 0.9 + glow(d, sun, 0.25) * 0.35;
            mix(base, [255.0, 226.0, 170.0], g)
        },
        "dawn" => |d| {
            let base = gradient(d, 0x6f7f9f, 0xffd8c2, 0xc7d6f0, 0x7ea6e0);
            let sun = [0.6, 0.1, -0.79];
            mix(base, [255.0, 244.0, 225.0], glow(d, sun, 0.05) * 0.8)
        },
        "overcast" => |d| gradient(d, 0x5d636b, 0xc6cbd2, 0xa6adb7, 0x858d98),
        "void" => |_| [0.0, 0.0, 0.0],
        _ => return None,
    })
}

fn sample_panorama(img: &RgbaImage, d: [f32; 3]) -> [f32; 3] {
    let lon = d[0].atan2(-d[2]);
    let lat = d[1].clamp(-1.0, 1.0).asin();
    let (w, h) = (img.width() as f32, img.height() as f32);
    let x = (0.5 + lon / std::f32::consts::TAU) * w - 0.5;
    let y = (0.5 - lat / std::f32::consts::PI) * h - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xx: f32, yy: f32| {
        let xi = (xx as i64).rem_euclid(img.width() as i64) as u32;
        let yi = (yy.max(0.0) as u32).min(img.height() - 1);
        let p = img.get_pixel(xi, yi).0;
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    let top = mix(px(x0, y0), px(x0 + 1.0, y0), fx);
    let bottom = mix(px(x0, y0 + 1.0), px(x0 + 1.0, y0 + 1.0), fx);
    mix(top, bottom, fy)
}

fn load_image(path: &Path) -> Result<DynamicImage, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Couldn't read that file: {e}"))?;
    if meta.len() > 64 * 1024 * 1024 {
        return Err("That picture is larger than 64 MB.".into());
    }
    image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("Couldn't read that file: {e}"))?
        .decode()
        .map_err(|_| "That file isn't a picture Vesper can read (PNG, JPEG or WebP).".to_string())
}

pub fn faces_from_panorama(path: &Path) -> Result<Vec<(&'static str, RgbaImage)>, String> {
    let img = load_image(path)?;
    let ratio = img.width() as f32 / img.height().max(1) as f32;
    if !(1.6..=2.4).contains(&ratio) {
        return Err(format!(
            "A panorama is twice as wide as it is tall (360° × 180°). This picture is {}×{}.",
            img.width(),
            img.height()
        ));
    }
    let img = if img.width() > 4096 { img.resize(4096, 2048, FilterType::Triangle) } else { img };
    let rgba = img.to_rgba8();
    Ok(render(FACE_SIZE, &|d| sample_panorama(&rgba, d)))
}

pub fn face_of_name(name: &str) -> Option<&'static str> {
    let stem = Path::new(name).file_stem()?.to_string_lossy().to_lowercase();
    let words: &[(&str, &[&str])] = &[
        ("bk", &["back", "bk"]),
        ("dn", &["down", "bottom", "dn"]),
        ("ft", &["front", "ft"]),
        ("lf", &["left", "lf"]),
        ("rt", &["right", "rt"]),
        ("up", &["top", "up"]),
    ];
    let mut found = None;
    for (face, keys) in words {
        let hit = keys.iter().any(|k| {
            stem.ends_with(k)
                || stem.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == *k)
                || (k.len() > 2 && stem.contains(k))
        });
        if hit {
            if found.is_some() {
                return None;
            }
            found = Some(*face);
        }
    }
    found
}

pub fn faces_from_files(paths: &[std::path::PathBuf]) -> Result<Vec<(&'static str, RgbaImage)>, String> {
    let mut out: Vec<(&'static str, RgbaImage)> = Vec::new();
    for p in paths {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let face = face_of_name(&name).ok_or_else(|| {
            format!(
                "Vesper can't tell which side “{name}” is. Name the six pictures Back, Down, Front, Left, Right and Up (Bk, Dn, Ft, Lf, Rt, Up also work)."
            )
        })?;
        if out.iter().any(|(f, _)| *f == face) {
            return Err(format!("Two pictures are for the same side ({face})."));
        }
        let img = load_image(p)?;
        out.push((face, img.resize_exact(FACE_SIZE, FACE_SIZE, FilterType::Lanczos3).to_rgba8()));
    }
    let missing: Vec<&str> = FACES.iter().filter(|f| !out.iter().any(|(g, _)| g == *f)).copied().collect();
    if !missing.is_empty() {
        return Err(format!("A skybox needs six pictures; missing: {}.", missing.join(", ")));
    }
    Ok(out)
}

pub fn preview_strip(faces: &[(&str, RgbaImage)], height: u32) -> RgbaImage {
    let mut strip = RgbaImage::new(height * 4, height);
    for (i, name) in ["rt", "ft", "lf", "bk"].iter().enumerate() {
        if let Some((_, img)) = faces.iter().find(|(f, _)| f == name) {
            let small = image::imageops::resize(img, height, height, FilterType::Triangle);
            image::imageops::overlay(&mut strip, &small, (i as u32 * height) as i64, 0);
        }
    }
    strip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dxt1_round_trip_keeps_colours_close() {
        let mut img = RgbaImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]);
        }
        let dds = encode_dds(&img);
        let expected: usize = (0..7).map(|l| ((64u32 >> l).div_ceil(4).pow(2) * 8) as usize).sum();
        assert_eq!(dds.len(), 128 + expected);
        assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 7, "mip count");
        let back = decode_dds(&dds).unwrap();
        let err: f64 = img
            .pixels()
            .zip(back.pixels())
            .map(|(a, b)| (0..3).map(|k| (a.0[k] as f64 - b.0[k] as f64).abs()).sum::<f64>())
            .sum::<f64>()
            / (64.0 * 64.0 * 3.0);
        assert!(err < 4.0, "average error {err}");
    }

    #[test]
    fn roblox_sized_faces_match_its_file_size() {
        let face = RgbaImage::from_pixel(FACE_SIZE, FACE_SIZE, Rgba([10, 20, 30, 255]));
        assert_eq!(encode_dds(&face).len(), 699_192);
    }

    fn edge(img: &RgbaImage, side: &str) -> Vec<[u8; 4]> {
        let n = img.width() - 1;
        match side {
            "left" => (0..=n).map(|y| img.get_pixel(0, y).0).collect(),
            "right" => (0..=n).map(|y| img.get_pixel(n, y).0).collect(),
            "top" => (0..=n).map(|x| img.get_pixel(x, 0).0).collect(),
            _ => (0..=n).map(|x| img.get_pixel(x, n).0).collect(),
        }
    }

    fn close(a: &[[u8; 4]], b: &[[u8; 4]]) -> bool {
        a.iter().zip(b).all(|(p, q)| (0..3).all(|k| (p[k] as i32 - q[k] as i32).abs() <= 12))
    }

    #[test]
    fn rendered_faces_join_like_robloxs() {
        let colour = |d: [f32; 3]| [(d[0] + 1.0) * 127.0, (d[1] + 1.0) * 127.0, (d[2] + 1.0) * 127.0];
        let faces: std::collections::HashMap<&str, RgbaImage> = render(64, &colour).into_iter().collect();
        let ring = ["rt", "ft", "lf", "bk"];
        for i in 0..4 {
            let (a, b) = (&faces[ring[i]], &faces[ring[(i + 1) % 4]]);
            assert!(close(&edge(a, "right"), &edge(b, "left")), "{} → {}", ring[i], ring[(i + 1) % 4]);
        }
        let up = image::imageops::rotate90(&faces["up"]);
        let dn = image::imageops::rotate270(&faces["dn"]);
        assert!(close(&edge(&up, "bottom"), &edge(&faces["ft"], "top")), "up meets ft");
        assert!(close(&edge(&dn, "top"), &edge(&faces["ft"], "bottom")), "dn meets ft");
        let up_left: Vec<[u8; 4]> = edge(&up, "left");
        let rt_top: Vec<[u8; 4]> = edge(&faces["rt"], "top");
        assert!(close(&up_left, &rt_top), "up meets rt");
        assert!(faces["ft"].get_pixel(32, 32).0[2] < 10, "ft looks toward -z");
        assert!(faces["up"].get_pixel(32, 32).0[1] > 245);
    }

    #[test]
    fn panoramas_put_the_middle_straight_ahead() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pano.png");
        let mut pano = RgbaImage::new(512, 256);
        for (x, _, p) in pano.enumerate_pixels_mut() {
            *p = if x < 256 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) };
        }
        pano.save(&path).unwrap();
        let faces: std::collections::HashMap<&str, RgbaImage> =
            faces_from_panorama(&path).unwrap().into_iter().collect();
        assert!(faces["rt"].get_pixel(512, 512).0[0] > 200, "left of ahead comes from the left half");
        assert!(faces["lf"].get_pixel(512, 512).0[2] > 200, "right of ahead from the right half");
        assert!(faces_from_panorama(&dir.path().join("missing.png")).is_err());
        let square = dir.path().join("square.png");
        RgbaImage::new(64, 64).save(&square).unwrap();
        assert!(faces_from_panorama(&square).unwrap_err().contains("twice as wide"));
    }

    #[test]
    fn face_names_are_understood() {
        assert_eq!(face_of_name("SkyboxBk.png"), Some("bk"));
        assert_eq!(face_of_name("sky_down.jpg"), Some("dn"));
        assert_eq!(face_of_name("Bottom.png"), Some("dn"));
        assert_eq!(face_of_name("front.png"), Some("ft"));
        assert_eq!(face_of_name("SkyboxLf.png"), Some("lf"));
        assert_eq!(face_of_name("right.png"), Some("rt"));
        assert_eq!(face_of_name("top.png"), Some("up"));
        assert_eq!(face_of_name("SkyboxUp.png"), Some("up"));
        assert_eq!(face_of_name("holiday.png"), None);
    }

    #[test]
    fn every_preset_renders() {
        for (id, _) in PRESETS {
            let f = preset_color(id).unwrap();
            assert_eq!(render(16, &f).len(), 6);
        }
        assert!(preset_color("nope").is_none());
    }
}
