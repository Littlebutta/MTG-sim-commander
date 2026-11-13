//! Self-update from this project's GitHub releases.
//!
//! The game ships as a portable folder. On launch the app asks GitHub for the latest release;
//! if it's newer, it downloads the release's portable zip, checks its SHA-256 against the
//! digest GitHub publishes for the asset, and swaps the new files into the install folder.
//! Running files can't be overwritten on Windows but can be renamed, so each replaced file is
//! renamed to `*.old` first and cleaned up on the next launch. User data under `data/` is
//! never overwritten.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

pub const REPO: &str = "Littlebutta/MTG-sim-commander";

#[derive(Clone, Debug, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub download_url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

pub fn asset_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-portable.zip"
    } else if cfg!(target_os = "macos") {
        "macos-portable.zip"
    } else {
        "linux-portable.zip"
    }
}

/// Decide from GitHub's "latest release" JSON whether there's a newer build for this platform.
pub fn pick_update(release: &Value, current: &str, suffix: &str) -> Option<UpdateInfo> {
    let tag = release.get("tag_name")?.as_str()?;
    let latest = semver::Version::parse(tag.trim_start_matches('v')).ok()?;
    let current = semver::Version::parse(current).ok()?;
    if latest <= current || release.get("draft").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let asset = release.get("assets")?.as_array()?.iter().find(|a| {
        a.get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n.ends_with(suffix))
    })?;
    Some(UpdateInfo {
        version: latest.to_string(),
        notes: release
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        download_url: asset.get("browser_download_url")?.as_str()?.to_string(),
        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
        sha256: asset
            .get("digest")
            .and_then(Value::as_str)
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(str::to_string),
    })
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("CommanderTable-updater")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn check(current: &str) -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None); // no releases yet
    }
    let json: Value = resp
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(pick_update(&json, current, asset_suffix()))
}

pub async fn download(info: &UpdateInfo) -> Result<Vec<u8>, String> {
    let bytes = client()?
        .get(&info.download_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?
        .to_vec();
    verify(&bytes, info.sha256.as_deref())?;
    Ok(bytes)
}

pub fn verify(bytes: &[u8], sha256: Option<&str>) -> Result<(), String> {
    if let Some(expected) = sha256 {
        let got = format!("{:x}", Sha256::digest(bytes));
        if !got.eq_ignore_ascii_case(expected) {
            return Err("the downloaded update is corrupted (checksum mismatch)".into());
        }
    }
    Ok(())
}

/// Path inside the zip with the top-level folder removed, rejecting anything that escapes.
fn relative_path(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    let mut comps = p.components();
    comps.next()?; // "CommanderTable/"
    let rel: PathBuf = comps.collect();
    if rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(rel)
}

/// Install the files from a portable zip into `dir`.
pub fn apply_zip(bytes: &[u8], dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut written = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = relative_path(entry.name()) else {
            continue;
        };
        let target = dir.join(&rel);
        // Never overwrite the player's own data.
        if rel.starts_with("data") && target.exists() {
            continue;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if target.exists() {
            let old = old_path(&target);
            let _ = fs::remove_file(&old);
            fs::rename(&target, &old)
                .map_err(|e| format!("can't replace {}: {e}", rel.display()))?;
        }
        fs::write(&target, &buf).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(mode));
            }
        }
        written.push(rel);
    }
    Ok(written)
}

fn old_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

/// Remove `*.old` files left by a previous update.
pub fn cleanup_old(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "data") {
                cleanup_old(&p);
            }
        } else if p.extension().is_some_and(|x| x == "old") {
            let _ = fs::remove_file(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn release(tag: &str) -> Value {
        serde_json::json!({
            "tag_name": tag,
            "body": "notes",
            "assets": [
                { "name": format!("CommanderTable-{tag}-linux-portable.zip"), "browser_download_url": "https://x/l.zip", "size": 10, "digest": "sha256:abc" },
                { "name": format!("CommanderTable-{tag}-windows-portable.zip"), "browser_download_url": "https://x/w.zip", "size": 20 }
            ]
        })
    }

    #[test]
    fn picks_newer_release_for_platform() {
        let u = pick_update(&release("v0.1.7"), "0.1.3", "windows-portable.zip").unwrap();
        assert_eq!(u.version, "0.1.7");
        assert_eq!(u.download_url, "https://x/w.zip");
        let l = pick_update(&release("v0.1.7"), "0.1.3", "linux-portable.zip").unwrap();
        assert_eq!(l.sha256.as_deref(), Some("abc"));
        assert!(pick_update(&release("v0.1.3"), "0.1.3", "windows-portable.zip").is_none());
        assert!(pick_update(&release("v0.1.2"), "0.1.3", "windows-portable.zip").is_none());
    }

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut out);
            for (name, data) in files {
                z.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                z.write_all(data).unwrap();
            }
            z.finish().unwrap();
        }
        out.into_inner()
    }

    #[test]
    fn applies_update_without_touching_user_data() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("app.exe"), b"old").unwrap();
        fs::create_dir_all(dir.path().join("data")).unwrap();
        fs::write(dir.path().join("data/decks.json"), b"mine").unwrap();
        let z = make_zip(&[
            ("CommanderTable/app.exe", b"new"),
            ("CommanderTable/data/decks.json", b"theirs"),
            ("CommanderTable/data/README.txt", b"readme"),
            ("CommanderTable/../evil.txt", b"x"),
        ]);
        let written = apply_zip(&z, dir.path()).unwrap();
        assert_eq!(fs::read(dir.path().join("app.exe")).unwrap(), b"new");
        assert_eq!(fs::read(dir.path().join("app.exe.old")).unwrap(), b"old");
        assert_eq!(
            fs::read(dir.path().join("data/decks.json")).unwrap(),
            b"mine"
        );
        assert!(dir.path().join("data/README.txt").exists());
        assert!(!dir.path().parent().unwrap().join("evil.txt").exists());
        assert_eq!(written.len(), 2);
        cleanup_old(dir.path());
        assert!(!dir.path().join("app.exe.old").exists());
    }

    #[test]
    fn checksum_mismatch_is_rejected() {
        assert!(verify(
            b"abc",
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        )
        .is_ok());
        assert!(verify(
            b"abd",
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        )
        .is_err());
    }
}
