//! Card art from Scryfall, downloaded the first time a card is shown and kept in `data/art/`.
//!
//! Scryfall asks clients to identify themselves and to keep to about ten requests a second,
//! so requests go out one at a time with a short gap. Cards Scryfall doesn't know are
//! remembered too, so they aren't asked for again.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const API: &str = "https://api.scryfall.com/cards/named";
const GAP: Duration = Duration::from_millis(120);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Art {
    /// `data:image/jpeg;base64,...`, ready for an `<img>`.
    pub url: String,
    pub artist: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    artist: Option<String>,
    missing: bool,
}

pub struct ArtCache {
    last: Mutex<Instant>,
}

impl Default for ArtCache {
    fn default() -> Self {
        ArtCache {
            last: Mutex::new(Instant::now() - GAP),
        }
    }
}

/// File-name-safe key for a card name.
pub fn key(name: &str) -> String {
    let front = name.split(" // ").next().unwrap_or(name);
    let mut k: String = front
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while k.contains("--") {
        k = k.replace("--", "-");
    }
    k.trim_matches('-').chars().take(80).collect()
}

fn dir(data: &Path) -> PathBuf {
    data.join("art")
}

/// Art already on disk: `Some(Some(art))` if cached, `Some(None)` if known to be missing.
pub fn cached(data: &Path, name: &str) -> Option<Option<Art>> {
    let k = key(name);
    let meta: Meta =
        serde_json::from_str(&fs::read_to_string(dir(data).join(format!("{k}.json"))).ok()?)
            .ok()?;
    if meta.missing {
        return Some(None);
    }
    let bytes = fs::read(dir(data).join(format!("{k}.jpg"))).ok()?;
    Some(Some(Art {
        url: format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
        artist: meta.artist,
    }))
}

fn store(
    data: &Path,
    name: &str,
    img: Option<&[u8]>,
    artist: Option<String>,
) -> Result<(), String> {
    let k = key(name);
    fs::create_dir_all(dir(data)).map_err(|e| e.to_string())?;
    if let Some(img) = img {
        fs::write(dir(data).join(format!("{k}.jpg")), img).map_err(|e| e.to_string())?;
    }
    let meta = Meta {
        artist,
        missing: img.is_none(),
    };
    fs::write(
        dir(data).join(format!("{k}.json")),
        serde_json::to_string(&meta).unwrap(),
    )
    .map_err(|e| e.to_string())
}

/// The art-crop URL and artist from a Scryfall card object (front face for double-faced cards).
pub fn art_from_json(v: &serde_json::Value) -> Option<(String, Option<String>)> {
    let face = v
        .get("image_uris")
        .map(|_| v)
        .or_else(|| v.get("card_faces")?.get(0))?;
    let url = face
        .get("image_uris")?
        .get("art_crop")?
        .as_str()?
        .to_string();
    let artist = face
        .get("artist")
        .or_else(|| v.get("artist"))
        .and_then(|a| a.as_str())
        .map(str::to_string);
    Some((url, artist))
}

impl ArtCache {
    async fn polite(&self) {
        let mut last = self.last.lock().await;
        let wait = GAP.saturating_sub(last.elapsed());
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        *last = Instant::now();
    }

    pub async fn get(&self, data: &Path, name: &str) -> Result<Option<Art>, String> {
        if let Some(c) = cached(data, name) {
            return Ok(c);
        }
        let client = reqwest::Client::builder()
            .user_agent("CommanderTable/1.0")
            .build()
            .map_err(|e| e.to_string())?;
        self.polite().await;
        let resp = client
            .get(API)
            .query(&[("exact", name)])
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            store(data, name, None, None)?;
            return Ok(None);
        }
        let json: serde_json::Value = resp
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let Some((url, artist)) = art_from_json(&json) else {
            store(data, name, None, None)?;
            return Ok(None);
        };
        self.polite().await;
        let img = client
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        store(data, name, Some(&img), artist)?;
        Ok(cached(data, name).flatten())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_safe_file_names() {
        assert_eq!(key("Jace, the Mind Sculptor"), "jace-the-mind-sculptor");
        assert_eq!(key("Fire // Ice"), "fire");
        assert_eq!(key("../../etc/passwd"), "etc-passwd");
    }

    #[test]
    fn reads_art_from_single_and_double_faced_cards() {
        let single =
            serde_json::json!({"artist": "A", "image_uris": {"art_crop": "https://x/a.jpg"}});
        assert_eq!(
            art_from_json(&single),
            Some(("https://x/a.jpg".into(), Some("A".into())))
        );
        let dfc = serde_json::json!({"card_faces": [{"artist": "B", "image_uris": {"art_crop": "https://x/b.jpg"}}, {}]});
        assert_eq!(
            art_from_json(&dfc),
            Some(("https://x/b.jpg".into(), Some("B".into())))
        );
    }

    #[test]
    fn caches_art_and_misses() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(cached(d.path(), "Sol Ring"), None);
        store(
            d.path(),
            "Sol Ring",
            Some(b"\xff\xd8jpeg"),
            Some("Mark Tedin".into()),
        )
        .unwrap();
        let a = cached(d.path(), "Sol Ring").unwrap().unwrap();
        assert!(a.url.starts_with("data:image/jpeg;base64,"));
        assert_eq!(a.artist.as_deref(), Some("Mark Tedin"));
        store(d.path(), "Not A Card", None, None).unwrap();
        assert_eq!(cached(d.path(), "Not A Card"), Some(None));
    }
}
