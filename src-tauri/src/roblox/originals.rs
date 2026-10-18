//! Roblox's own files on macOS, for cleaning up what other tools left behind.
//!
//! Roblox.app is code-signed: `Contents/_CodeSignature/CodeResources` holds
//! the SHA-256 of every file in `Contents/Resources`. That tells Roblox's own
//! cursor apart from one another tool or an older customisation left there.
//!
//! Roblox's own copy of such a file comes from Roblox's public download of the
//! Mac app on setup.rbxcdn.com. Only that file's bytes are read out of the
//! archive, with HTTP range requests, and they're used only if they match the
//! signature. No account or cookie is involved.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::mods::Originals;
use base64::Engine;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::{LazyLock, OnceLock};

const MAC_VERSION_URL: &str = "https://clientsettingscdn.roblox.com/v2/client-version/MacPlayer";
const CDN: &str = "setup.rbxcdn.com";
/// The largest single file taken out of the download (Roblox's biggest fonts are a few MB).
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_DIRECTORY: u64 = 16 * 1024 * 1024;
/// The end-of-archive record plus the longest comment a zip can carry.
const TAIL: u64 = 22 + 65_535;

static FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"<key>([^<]+)</key>\s*<dict>\s*(?:<key>hash</key>\s*<data>[^<]*</data>\s*)?<key>hash2</key>\s*<data>([^<]*)</data>",
    )
    .expect("valid regex")
});

/// SHA-256 of every file in `Contents/Resources`, by its path there (with
/// `/`), from a code signature's CodeResources (an XML property list).
pub fn resource_hashes(xml: &str) -> HashMap<String, [u8; 32]> {
    let Some(start) = xml.find("<key>files2</key>") else { return HashMap::new() };
    let rest = &xml[start..];
    let end = rest.find("<key>rules</key>").or_else(|| rest.find("<key>rules2</key>")).unwrap_or(rest.len());
    FILE.captures_iter(&rest[..end])
        .filter_map(|c| {
            let rel = unescape(&c[1]).strip_prefix("Resources/")?.to_owned();
            let data: String = c[2].chars().filter(|c| !c.is_whitespace()).collect();
            let hash = base64::engine::general_purpose::STANDARD.decode(data).ok()?.try_into().ok()?;
            Some((rel, hash))
        })
        .collect()
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

// ------------------------------------------------------------------ reading a zip over HTTP

fn le16(b: &[u8], i: usize) -> Option<u64> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?) as u64)
}
fn le32(b: &[u8], i: usize) -> Option<u64> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?) as u64)
}
fn le64(b: &[u8], i: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(i..i + 8)?.try_into().ok()?))
}

/// Where the central directory is, from the archive's last bytes.
#[derive(Debug, PartialEq)]
enum Directory {
    At {
        offset: u64,
        size: u64,
    },
    /// A ZIP64 archive: its own end record, at this offset, says.
    Zip64 {
        record: u64,
    },
}

fn find_directory(tail: &[u8]) -> Result<Directory, String> {
    const SIG: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    let i = (0..=tail.len().saturating_sub(22))
        .rev()
        .find(|&i| tail[i..].starts_with(&SIG) && le16(tail, i + 20).is_some_and(|c| i + 22 + c as usize == tail.len()))
        .ok_or("the download isn't a zip archive")?;
    let bad = || "the download's zip directory is damaged".to_string();
    let entries = le16(tail, i + 10).ok_or_else(bad)?;
    let size = le32(tail, i + 12).ok_or_else(bad)?;
    let offset = le32(tail, i + 16).ok_or_else(bad)?;
    if entries == 0xFFFF || size == 0xFFFF_FFFF || offset == 0xFFFF_FFFF {
        let locator =
            i.checked_sub(20).filter(|&l| tail[l..].starts_with(&[0x50, 0x4b, 0x06, 0x07])).ok_or_else(bad)?;
        return Ok(Directory::Zip64 { record: le64(tail, locator + 8).ok_or_else(bad)? });
    }
    Ok(Directory::At { offset, size })
}

fn zip64_directory(record: &[u8]) -> Result<Directory, String> {
    let bad = || "the download's zip directory is damaged".to_string();
    if !record.starts_with(&[0x50, 0x4b, 0x06, 0x06]) {
        return Err(bad());
    }
    Ok(Directory::At { size: le64(record, 40).ok_or_else(bad)?, offset: le64(record, 48).ok_or_else(bad)? })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    offset: u64,
    compressed: u64,
    size: u64,
    method: u64,
}

/// Every file in a zip, by name, from its central directory.
fn read_directory(cd: &[u8]) -> Result<HashMap<String, Entry>, String> {
    let bad = || "the download's zip directory is damaged".to_string();
    let mut out = HashMap::new();
    let mut p = 0;
    while cd[p..].starts_with(&[0x50, 0x4b, 0x01, 0x02]) {
        let method = le16(cd, p + 10).ok_or_else(bad)?;
        let mut compressed = le32(cd, p + 20).ok_or_else(bad)?;
        let mut size = le32(cd, p + 24).ok_or_else(bad)?;
        let name_len = le16(cd, p + 28).ok_or_else(bad)? as usize;
        let extra_len = le16(cd, p + 30).ok_or_else(bad)? as usize;
        let comment_len = le16(cd, p + 32).ok_or_else(bad)? as usize;
        let mut offset = le32(cd, p + 42).ok_or_else(bad)?;
        let name_end = p + 46 + name_len;
        let extra_end = name_end + extra_len;
        let name = String::from_utf8_lossy(cd.get(p + 46..name_end).ok_or_else(bad)?).into_owned();
        // ZIP64: whatever didn't fit in 32 bits is in extra field 1, in this order.
        if [size, compressed, offset].contains(&0xFFFF_FFFF) {
            let mut e = name_end;
            while e + 4 <= extra_end {
                let (id, len) = (le16(cd, e).ok_or_else(bad)?, le16(cd, e + 2).ok_or_else(bad)? as usize);
                if id == 1 {
                    let mut q = e + 4;
                    for field in [&mut size, &mut compressed, &mut offset] {
                        if *field == 0xFFFF_FFFF {
                            *field = le64(cd, q).ok_or_else(bad)?;
                            q += 8;
                        }
                    }
                }
                e += 4 + len;
            }
        }
        if !name.ends_with('/') {
            out.insert(name, Entry { offset, compressed, size, method });
        }
        p = extra_end + comment_len;
        if p > cd.len() {
            return Err(bad());
        }
    }
    Ok(out)
}

fn inflate(method: u64, data: &[u8], size: u64) -> Result<Vec<u8>, String> {
    let out = match method {
        0 => data.to_vec(),
        8 => {
            let mut out = Vec::with_capacity(size as usize);
            flate2::read::DeflateDecoder::new(data)
                .take(size + 1)
                .read_to_end(&mut out)
                .map_err(|e| format!("the file in the download is damaged: {e}"))?;
            out
        }
        m => return Err(format!("the download uses a compression Vesper can't read ({m})")),
    };
    if out.len() as u64 != size {
        return Err("the file in the download is damaged".into());
    }
    Ok(out)
}

async fn get_range(http: &reqwest::Client, url: &str, from: u64, len: u64) -> Result<Vec<u8>, String> {
    if len == 0 {
        return Ok(Vec::new());
    }
    let r = http
        .get(url)
        .header(reqwest::header::RANGE, format!("bytes={from}-{}", from + len - 1))
        .send()
        .await
        .map_err(|e| format!("couldn't reach {CDN}: {e}"))?;
    if r.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!("{CDN} answered {}", r.status()));
    }
    let b = r.bytes().await.map_err(|e| format!("the download from {CDN} stopped: {e}"))?;
    if b.len() as u64 != len {
        return Err(format!("the download from {CDN} stopped early"));
    }
    Ok(b.to_vec())
}

/// Roblox's Mac app download, read a file at a time.
pub struct RemoteZip {
    url: String,
    entries: HashMap<String, Entry>,
    /// The app folder at the top of the archive ("RobloxPlayer.app").
    app: String,
}

/// The app at the top of the archive, from a name like `X.app/Contents/…`.
fn top_app(entries: &HashMap<String, Entry>) -> Option<String> {
    entries.keys().find_map(|n| {
        let (app, _) = n.split_once(".app/Contents/")?;
        (!app.contains('/')).then(|| format!("{app}.app"))
    })
}

impl RemoteZip {
    pub async fn open(http: &reqwest::Client, url: &str) -> Result<Self, String> {
        let head = http.head(url).send().await.map_err(|e| format!("couldn't reach {CDN}: {e}"))?;
        if !head.status().is_success() {
            return Err(format!("{CDN} answered {}", head.status()));
        }
        let total = head
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
            .ok_or_else(|| format!("{CDN} didn't say how big the download is"))?;
        let tail_len = TAIL.min(total);
        let tail = get_range(http, url, total - tail_len, tail_len).await?;
        let dir = match find_directory(&tail)? {
            Directory::Zip64 { record } => zip64_directory(&get_range(http, url, record, 56).await?)?,
            at => at,
        };
        let Directory::At { offset, size } = dir else { unreachable!("ZIP64 records point at the directory") };
        if size > MAX_DIRECTORY || offset + size > total {
            return Err("the download's zip directory is damaged".into());
        }
        let entries = read_directory(&get_range(http, url, offset, size).await?)?;
        let app = top_app(&entries).ok_or("the download doesn't hold an app")?;
        Ok(Self { url: url.to_owned(), entries, app })
    }

    /// The bytes of `rel`, a path in the app's `Contents` folder (with `/`).
    pub async fn read(&self, http: &reqwest::Client, rel: &str) -> Result<Vec<u8>, String> {
        let e = self.entries.get(&format!("{}/Contents/{rel}", self.app)).ok_or("it isn't in Roblox's download")?;
        if e.size > MAX_FILE || e.compressed > MAX_FILE {
            return Err("it's too big to download".into());
        }
        let head = get_range(http, &self.url, e.offset, 30).await?;
        if !head.starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
            return Err("the download's zip directory is damaged".into());
        }
        let skip = 30 + le16(&head, 26).unwrap_or(0) + le16(&head, 28).unwrap_or(0);
        let data = get_range(http, &self.url, e.offset + skip, e.compressed).await?;
        inflate(e.method, &data, e.size)
    }
}

async fn latest_mac_build(http: &reqwest::Client) -> Result<String, String> {
    let r = http.get(MAC_VERSION_URL).send().await.map_err(|e| format!("couldn't reach Roblox: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Roblox answered {}", r.status()));
    }
    let text = r.text().await.map_err(|e| format!("couldn't read Roblox's answer: {e}"))?;
    Ok(crate::roblox::cdn::parse_version(&text)?.version_hash)
}

// ------------------------------------------------------------------ Originals for Roblox.app

fn slashes(rel: &str) -> String {
    rel.replace('\\', "/")
}

/// Roblox.app's own files: which they are from its signature, and their bytes from Roblox's download.
pub struct MacOriginals {
    hashes: HashMap<String, [u8; 32]>,
    http: reqwest::Client,
    download: OnceLock<Result<RemoteZip, String>>,
    /// Set after a download fails, so a bad connection costs one timeout, not one per file.
    offline: OnceLock<String>,
}

impl MacOriginals {
    /// Reads the signature of the Roblox.app whose `Contents/Resources` is `content_root`.
    pub fn read(content_root: &Path, http: reqwest::Client) -> Option<Self> {
        let file = content_root.parent()?.join("_CodeSignature").join("CodeResources");
        let hashes = resource_hashes(&std::fs::read_to_string(file).ok()?);
        if hashes.is_empty() {
            log::info!("Roblox.app's signature lists no files, so leftovers can't be told apart");
            return None;
        }
        Some(Self { hashes, http, download: OnceLock::new(), offline: OnceLock::new() })
    }

    fn download(&self) -> Result<&RemoteZip, String> {
        self.download
            .get_or_init(|| {
                tauri::async_runtime::block_on(async {
                    let build = latest_mac_build(&self.http).await?;
                    let zip = RemoteZip::open(&self.http, &format!("https://{CDN}/mac/{build}-RobloxPlayer.zip")).await;
                    if zip.is_ok() {
                        log::info!("Reading Roblox's own files from the {build} download");
                    }
                    zip
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

impl Originals for MacOriginals {
    fn files_in(&self, area: &str) -> Vec<String> {
        let area = slashes(area);
        let dir = format!("{area}/");
        let mut files: Vec<String> =
            self.hashes.keys().filter(|k| **k == area || k.starts_with(&dir)).map(|k| k.replace('/', "\\")).collect();
        files.sort();
        files
    }

    fn is_roblox(&self, rel: &str, bytes: &[u8]) -> Option<bool> {
        self.hashes.get(&slashes(rel)).map(|h| Sha256::digest(bytes).as_slice() == h)
    }

    fn fetch(&self, rel: &str) -> Result<Vec<u8>, String> {
        let rel = slashes(rel);
        let want = self.hashes.get(&rel).ok_or("it isn't part of Roblox")?;
        if let Some(e) = self.offline.get() {
            return Err(e.clone());
        }
        let bytes = tauri::async_runtime::block_on(self.download()?.read(&self.http, &format!("Resources/{rel}")))
            .inspect_err(|e| {
                let _ = self.offline.set(e.clone());
            })?;
        if Sha256::digest(&bytes).as_slice() != want {
            return Err("Roblox's download has another version of it; let Roblox update, then try again".into());
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SIGNATURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>files</key>
	<dict>
		<key>Resources/content/textures/MouseLockedCursor.png</key>
		<data>
		AAAAAAAAAAAAAAAAAAAAAAAAAAA=
		</data>
	</dict>
	<key>files2</key>
	<dict>
		<key>MacOS/RobloxCrashHandler</key>
		<dict>
			<key>cdhash</key>
			<data>
			reHzoAvxrQR8sjQ7A2vWfYTKMO4=
			</data>
		</dict>
		<key>Resources/content/textures/MouseLockedCursor.png</key>
		<dict>
			<key>hash2</key>
			<data>
			SlfxARvFcIeJk5UuxulLMICptvbmrB2guNvnhDqWmgI=
			</data>
		</dict>
		<key>Resources/content/fonts/R&amp;D.ttf</key>
		<dict>
			<key>hash</key>
			<data>
			AAAAAAAAAAAAAAAAAAAAAAAAAAA=
			</data>
			<key>hash2</key>
			<data>
			AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
			</data>
			<key>optional</key>
			<true/>
		</dict>
	</dict>
	<key>rules</key>
	<dict>
		<key>Resources/content/fonts/Fake.ttf</key>
		<dict>
			<key>hash2</key>
			<data>
			AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
			</data>
		</dict>
	</dict>
</dict>
</plist>"#;

    #[test]
    fn signature_lists_resource_hashes() {
        let h = resource_hashes(SIGNATURE);
        assert_eq!(h.len(), 2, "{:?}", h.keys());
        assert_eq!(hex::encode(h["content/textures/MouseLockedCursor.png"]).len(), 64);
        assert_eq!(h["content/fonts/R&D.ttf"], [0u8; 32]);
        assert!(!h.contains_key("MacOS/RobloxCrashHandler"));
    }

    /// A zip with one stored and one compressed file, built the way Roblox's is.
    fn zip(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut cd = Vec::new();
        for (name, bytes, deflate) in files {
            let data = if *deflate {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(bytes).unwrap();
                e.finish().unwrap()
            } else {
                bytes.to_vec()
            };
            let method: u16 = if *deflate { 8 } else { 0 };
            let offset = out.len() as u32;
            // Local header with sizes left to a data descriptor, plus an extra field.
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 8, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 4 + 4 + 4 + 4]);
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&4u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&[0xAA, 0xBB, 0, 0]);
            out.extend_from_slice(&data);
            cd.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 3, 20, 0, 8, 0]);
            cd.extend_from_slice(&method.to_le_bytes());
            cd.extend_from_slice(&[0; 4 + 4]);
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0; 2 + 2 + 2 + 2 + 4]);
            cd.extend_from_slice(&offset.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn files_are_read_out_of_a_zip() {
        let cursor = b"cursor bytes".repeat(20);
        let z = zip(&[
            ("RobloxPlayer.app/Contents/Resources/content/textures/MouseLockedCursor.png", &cursor, true),
            ("RobloxPlayer.app/Contents/Resources/content/sounds/oof.ogg", b"OggS", false),
            ("RobloxPlayer.app/Contents/MacOS/Helper.app/Contents/Resources/content/x.png", b"nested", false),
            ("RobloxPlayer.app/Contents/Resources/content/fonts/", b"", false),
        ]);
        let Directory::At { offset, size } = find_directory(&z[z.len() - 200.min(z.len())..]).unwrap() else {
            panic!("not ZIP64")
        };
        let entries = read_directory(&z[offset as usize..(offset + size) as usize]).unwrap();
        assert_eq!(entries.len(), 3, "folders aren't files: {:?}", entries.keys());
        assert_eq!(top_app(&entries).as_deref(), Some("RobloxPlayer.app"));
        for (rel, want) in
            [("content/textures/MouseLockedCursor.png", &cursor[..]), ("content/sounds/oof.ogg", b"OggS")]
        {
            let e = entries[&format!("RobloxPlayer.app/Contents/Resources/{rel}")];
            let head = &z[e.offset as usize..];
            let skip = (30 + le16(head, 26).unwrap() + le16(head, 28).unwrap()) as usize;
            let data = &head[skip..skip + e.compressed as usize];
            assert_eq!(inflate(e.method, data, e.size).unwrap(), want);
        }
    }

    #[test]
    fn damaged_archives_are_rejected() {
        assert!(find_directory(b"not a zip at all, just some text").is_err());
        assert!(inflate(8, b"\x00\x01garbage", 10).is_err());
        assert!(inflate(0, b"short", 10).is_err());
        assert!(inflate(14, b"lzma", 4).is_err());
        let mut z = zip(&[("RobloxPlayer.app/Contents/Resources/a.png", b"a", false)]);
        let at = z.len() - 22 + 12;
        z[at..at + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(find_directory(&z).is_err(), "ZIP64 marker without its locator");
    }

    /// Reads a real file out of Roblox's current Mac download and checks it
    /// against the signature in that same download. Needs the internet:
    /// `cargo test -- --ignored roblox_mac_download`.
    #[test]
    #[ignore]
    fn roblox_mac_download() {
        tauri::async_runtime::block_on(async {
            let http = crate::roblox::cdn::http_client();
            let build = latest_mac_build(&http).await.unwrap();
            let zip = RemoteZip::open(&http, &format!("https://{CDN}/mac/{build}-RobloxPlayer.zip")).await.unwrap();
            assert!(zip.entries.len() > 1000, "{} files", zip.entries.len());
            let signature = zip.read(&http, "_CodeSignature/CodeResources").await.unwrap();
            let hashes = resource_hashes(std::str::from_utf8(&signature).unwrap());
            assert!(hashes.len() > 1000, "{} signed files", hashes.len());
            for rel in
                ["content/textures/MouseLockedCursor.png", "content/textures/ui/Emotes/Large/SegmentedCircle.png"]
            {
                let bytes = zip.read(&http, &format!("Resources/{rel}")).await.unwrap();
                assert!(bytes.starts_with(b"\x89PNG"), "{rel}");
                assert_eq!(Sha256::digest(&bytes).as_slice(), hashes[rel], "{rel} matches the signature");
            }
        });
    }
}
