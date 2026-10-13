//! Roblox client version numbers, e.g. `0.741.0.7411058`.

use serde::{Serialize, Serializer};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RobloxVersion(pub u32, pub u32, pub u32, pub u32);

impl fmt::Display for RobloxVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

impl Serialize for RobloxVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl FromStr for RobloxVersion {
    type Err = String;

    /// Accepts `0.741.0.7411058` and the PE resource form `0, 741, 0, 7411058`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<u32> = s
            .split(['.', ','])
            .map(|p| p.trim().parse::<u32>())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("not a version: {s}"))?;
        match parts.as_slice() {
            [a, b, c, d] => Ok(Self(*a, *b, *c, *d)),
            _ => Err(format!("not a version: {s}")),
        }
    }
}

/// Reads the file version from an executable's version resource.
///
/// The `FileVersion` string is preferred: Roblox build numbers (e.g. 7400927)
/// don't fit the 16-bit fields of the binary VS_FIXEDFILEINFO, which would
/// report a truncated value (60895).
#[cfg(windows)]
pub fn file_version(path: &Path) -> Option<RobloxVersion> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Returns a pointer and length (in the unit VerQueryValue uses) for `sub`.
    unsafe fn query(buf: &[u8], sub: &str) -> Option<(*const core::ffi::c_void, u32)> {
        let sub = wide(sub);
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let ok = VerQueryValueW(buf.as_ptr().cast(), PCWSTR(sub.as_ptr()), &mut ptr, &mut len).as_bool();
        (ok && !ptr.is_null() && len > 0).then_some((ptr as *const _, len))
    }

    let path_w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(path_w.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(path_w.as_ptr()), None, size, buf.as_mut_ptr().cast()).ok()?;

        // 1. The FileVersion string for each language/code page the file declares.
        if let Some((ptr, len)) = query(&buf, r"\VarFileInfo\Translation") {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            for pair in pairs.as_chunks::<2>().0 {
                let key = format!(r"\StringFileInfo\{:04x}{:04x}\FileVersion", pair[0], pair[1]);
                if let Some((sptr, slen)) = query(&buf, &key) {
                    // Length is in characters and includes the terminator.
                    let chars = std::slice::from_raw_parts(sptr as *const u16, slen as usize);
                    let end = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
                    // System files append text: "10.0.26100.9278 (WinBuild.160101.0800)".
                    let text = String::from_utf16_lossy(&chars[..end]);
                    let text = text.split('(').next().unwrap_or_default().trim();
                    if let Ok(v) = text.parse() {
                        return Some(v);
                    }
                }
            }
        }

        // 2. Fall back to the binary block.
        let (ptr, len) = query(&buf, "\\")?;
        if (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>() {
            return None;
        }
        let info = &*(ptr as *const VS_FIXEDFILEINFO);
        if info.dwSignature != 0xFEEF04BD {
            return None;
        }
        Some(RobloxVersion(
            info.dwFileVersionMS >> 16,
            info.dwFileVersionMS & 0xFFFF,
            info.dwFileVersionLS >> 16,
            info.dwFileVersionLS & 0xFFFF,
        ))
    }
}

/// Reads Roblox's version from the Info.plist of the app bundle `path` (the
/// player inside `Contents/MacOS`) belongs to.
#[cfg(target_os = "macos")]
pub fn file_version(path: &Path) -> Option<RobloxVersion> {
    let plist = path.parent()?.parent()?.join("Info.plist");
    let bytes = std::fs::read(&plist).ok()?;
    let text = if bytes.starts_with(b"bplist") {
        // Binary property list: let macOS's own tool convert it.
        let out = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "xml1", "-o", "-"])
            .arg(&plist)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        String::from_utf8(out.stdout).ok()?
    } else {
        String::from_utf8(bytes).ok()?
    };
    plist_version(&text)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn file_version(_path: &Path) -> Option<RobloxVersion> {
    None
}

/// The string value of `key` in an XML property list.
pub fn plist_string(xml: &str, key: &str) -> Option<String> {
    let at = xml.find(&format!("<key>{key}</key>"))?;
    let rest = &xml[at..];
    let open = rest.find("<string>")?;
    // The value must be the next element, not a later key's.
    if rest[..open].matches("<key>").count() > 1 {
        return None;
    }
    let value = &rest[open + "<string>".len()..];
    Some(value[..value.find("</string>")?].trim().to_owned())
}

/// Roblox's version from its Info.plist. Accepts a full `0.695.0.6950754` in
/// either version key, or a three-part short version plus a build number.
pub fn plist_version(xml: &str) -> Option<RobloxVersion> {
    let short = plist_string(xml, "CFBundleShortVersionString");
    let build = plist_string(xml, "CFBundleVersion");
    for v in [&short, &build].into_iter().flatten() {
        if let Ok(v) = v.parse() {
            return Some(v);
        }
    }
    let (short, build) = (short?, build?);
    let build = build.rsplit('.').next()?;
    format!("{short}.{build}").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_forms() {
        assert_eq!("0.741.0.7411058".parse::<RobloxVersion>().unwrap(), RobloxVersion(0, 741, 0, 7411058));
        assert_eq!("0, 740, 0, 7400927".parse::<RobloxVersion>().unwrap(), RobloxVersion(0, 740, 0, 7400927));
        assert!("0.741".parse::<RobloxVersion>().is_err());
        assert!("version-abc".parse::<RobloxVersion>().is_err());
    }

    #[test]
    fn orders_numerically() {
        let a: RobloxVersion = "0.99.0.1".parse().unwrap();
        let b: RobloxVersion = "0.741.0.1".parse().unwrap();
        assert!(a < b);
        assert_eq!(b.to_string(), "0.741.0.1");
    }

    #[test]
    fn reads_versions_from_info_plist() {
        let plist = |short: &str, build: &str| {
            format!(
                "<?xml version=\"1.0\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleExecutable</key>\n\t<string>RobloxPlayer</string>\n\t<key>CFBundleShortVersionString</key>\n\t<string>{short}</string>\n\t<key>CFBundleVersion</key>\n\t<string>{build}</string>\n</dict>\n</plist>\n"
            )
        };
        let full = RobloxVersion(0, 695, 0, 6950754);
        assert_eq!(plist_version(&plist("0.695.0.6950754", "0.695.0.6950754")), Some(full));
        assert_eq!(plist_version(&plist("0.695.0", "6950754")), Some(full));
        assert_eq!(plist_version(&plist("1.0", "1")), None);
        assert_eq!(plist_string(&plist("a", "b"), "CFBundleExecutable").as_deref(), Some("RobloxPlayer"));
        assert_eq!(plist_string("<key>Empty</key><true/><key>Next</key><string>x</string>", "Empty"), None);
    }

    /// Roblox build numbers overflow the binary version fields; the string must be used.
    #[cfg(windows)]
    #[test]
    fn reads_untruncated_roblox_version_when_installed() {
        let Some(local) = std::env::var_os("LOCALAPPDATA") else { return };
        let versions = std::path::PathBuf::from(local).join("Roblox").join("Versions");
        let Ok(rd) = std::fs::read_dir(versions) else { return };
        for e in rd.filter_map(Result::ok) {
            let exe = e.path().join("RobloxPlayerBeta.exe");
            if exe.is_file() {
                let v = file_version(&exe).expect("Roblox has a version resource");
                assert!(v.3 > 65_535, "build number looks truncated: {v}");
                return;
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn reads_a_real_windows_binary_version() {
        // notepad.exe always ships with a version resource. Unmanifested test
        // binaries get Windows' compatibility "version lie" (6.2.x) for system
        // files, so check the build number, which is reported truthfully.
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let v = file_version(Path::new(&windir).join("System32").join("notepad.exe").as_path());
        assert!(v.is_some_and(|v| v.2 >= 10240), "got {v:?}");
    }
}
