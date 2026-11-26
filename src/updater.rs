//! Self-update from this repo's GitHub releases (Windows).
//! Reference module for desktop apps, Tauri apps and CLIs built by the /projet chain
//! (taken from PokAgent's src/updater.rs, made generic).
//! Copy to src/updater.rs (Tauri: src-tauri/src/updater.rs), set REPO, keep the tests.
//! If the latest release has a higher version: download <APP>.exe, check it against
//! <APP>.exe.sha256 from the same release, swap the running exe, then the caller restarts.
//! HTTP via WinHTTP, hashing via BCrypt: no extra crate besides `windows` and `serde_json`.
//!
//! Cargo.toml:
//!   serde_json = "1"
//!   [dependencies.windows] features = ["Win32_Foundation", "Win32_Networking_WinHttp", "Win32_Security", "Win32_Security_Cryptography"]
//!
//! Scheduling (desktop / Tauri): background thread, first check 60 s after launch, then every 6 h,
//! skipped if the user disabled auto-update in the config; on Outcome::Installed -> restart
//! (spawn current_exe with the same args, then exit). Call cleanup() at startup.
//! CLI: `<app> update` runs check_and_install(true); plus at most one silent check per 24 h
//! (timestamp file in data_dir()) that only prints "new version vX.Y.Z available, run `<app> update`".

use serde_json::Value;
use std::path::PathBuf;
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
use windows::core::{PCWSTR, w};

pub const REPO: &str = "williamloree/OpenKiller";
pub const APP: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// HTTPS GET; redirects (release asset downloads) are followed.
fn get(url: &str) -> Result<Vec<u8>, String> {
    let rest = url.strip_prefix("https://").ok_or("not an HTTPS url")?;
    let (host, path) = match rest.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (whost, wpath) = (wide(host), wide(&path));
    let agent = wide(&format!("{APP}-updater"));
    unsafe {
        let s = Handle(WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if s.0.is_null() {
            return Err("WinHttpOpen".into());
        }
        let c = Handle(WinHttpConnect(
            s.0,
            PCWSTR(whost.as_ptr()),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if c.0.is_null() {
            return Err("connect failed".into());
        }
        let r = Handle(WinHttpOpenRequest(
            c.0,
            w!("GET"),
            PCWSTR(wpath.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ));
        if r.0.is_null() {
            return Err("request failed".into());
        }
        WinHttpSendRequest(r.0, None, None, 0, 0, 0).map_err(|e| e.to_string())?;
        WinHttpReceiveResponse(r.0, std::ptr::null_mut()).map_err(|e| e.to_string())?;
        let mut code: u32 = 0;
        let mut len = 4u32;
        WinHttpQueryHeaders(
            r.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut code as *mut u32 as *mut _),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(|e| e.to_string())?;
        if code != 200 {
            return Err(format!("HTTP {code}"));
        }
        let mut out = Vec::new();
        loop {
            let mut avail = 0u32;
            WinHttpQueryDataAvailable(r.0, &mut avail).map_err(|e| e.to_string())?;
            if avail == 0 {
                break;
            }
            let start = out.len();
            out.resize(start + avail as usize, 0);
            let mut read = 0u32;
            WinHttpReadData(r.0, out[start..].as_mut_ptr() as *mut _, avail, &mut read)
                .map_err(|e| e.to_string())?;
            out.truncate(start + read as usize);
        }
        Ok(out)
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut out = [0u8; 32];
    unsafe {
        let _ = BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out);
    }
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_ver(v: &str) -> Vec<u64> {
    v.trim_start_matches('v')
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

pub fn newer(remote: &str, local: &str) -> bool {
    parse_ver(remote) > parse_ver(local)
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(APP)
}

/// Last version already tried: avoids a loop if a release is mis-versioned.
fn tried_file() -> PathBuf {
    data_dir().join("update_tried.txt")
}

pub enum Outcome {
    UpToDate,
    Installed(String),
}

/// Looks for a newer release and installs it in place of the running exe. Caller restarts.
pub fn check_and_install(manual: bool) -> Result<Outcome, String> {
    let body = get(&format!(
        "https://api.github.com/repos/{REPO}/releases/latest"
    ))?;
    let rel: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let tag = rel
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("release without tag")?
        .to_string();
    if !newer(&tag, VERSION) {
        return Ok(Outcome::UpToDate);
    }
    if !manual && std::fs::read_to_string(tried_file()).ok().as_deref() == Some(tag.as_str()) {
        return Ok(Outcome::UpToDate);
    }
    let asset = |name: &str| -> Option<String> {
        rel.get("assets")?
            .as_array()?
            .iter()
            .find(|a| a.get("name").and_then(Value::as_str) == Some(name))?
            .get("browser_download_url")?
            .as_str()
            .map(str::to_string)
    };
    let exe_name = format!("{APP}.exe");
    let exe_url = asset(&exe_name).ok_or(format!("{exe_name} missing from release"))?;
    let sha_url = asset(&format!("{exe_name}.sha256"))
        .ok_or(format!("{exe_name}.sha256 missing from release"))?;
    let expected: String = String::from_utf8_lossy(&get(&sha_url)?)
        .chars()
        .take(64)
        .collect::<String>()
        .to_lowercase();
    let exe = get(&exe_url)?;
    if sha256_hex(&exe) != expected {
        return Err("SHA-256 mismatch: download rejected".into());
    }
    let cur = std::env::current_exe().map_err(|e| e.to_string())?;
    let new = cur.with_extension("exe.new");
    let old = cur.with_extension("exe.old");
    std::fs::write(&new, &exe).map_err(|e| format!("write failed: {e}"))?;
    let _ = std::fs::remove_file(&old);
    // Windows allows renaming a running executable.
    std::fs::rename(&cur, &old).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new, &cur) {
        let _ = std::fs::rename(&old, &cur);
        return Err(e.to_string());
    }
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(tried_file(), &tag);
    Ok(Outcome::Installed(tag))
}

/// Removes the previous exe left by the last update. Call at startup.
pub fn cleanup() {
    if let Ok(cur) = std::env::current_exe() {
        let _ = std::fs::remove_file(cur.with_extension("exe.old"));
        let _ = std::fs::remove_file(cur.with_extension("exe.new"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(newer("v0.2.0", "0.1.9"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(newer("v0.10.0", "0.9.0"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("v0.0.9", "0.1.0"));
    }
    #[test]
    fn sha() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    /// Network: cargo test http -- --ignored
    #[test]
    #[ignore]
    fn http() {
        let body = get(&format!("https://api.github.com/repos/{REPO}")).unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["full_name"], REPO);
    }
}
