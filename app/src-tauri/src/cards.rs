//! The full card pool from MTGJSON, kept in `data/cards/` inside the game folder.
//!
//! On launch the app asks MTGJSON for its current version. When MTGJSON has published new
//! cards, it downloads `AtomicCards` again (checking the published SHA-256) and swaps it in.
//! Until the first download finishes, the built-in starter cards are used.

use carddb::{convert_all, AtomicFile, CardIndex, CardRecord, Meta};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const META_URL: &str = "https://mtgjson.com/api/v5/Meta.json";
const GZ_URL: &str = "https://mtgjson.com/api/v5/AtomicCards.json.gz";

#[derive(Clone, Debug, Default, Serialize)]
pub struct CardStatus {
    /// MTGJSON release date of the cards on disk, if any.
    pub date: Option<String>,
    pub version: Option<String>,
    /// Number of cards in the loaded pool.
    pub count: usize,
    pub downloading: bool,
    pub error: Option<String>,
}

/// The loaded pool, shared by the commands.
#[derive(Default)]
pub struct Pool {
    pub cards: Mutex<Option<Arc<Vec<CardRecord>>>>,
    pub status: Mutex<CardStatus>,
}

fn dir(data: &Path) -> PathBuf {
    data.join("cards")
}

fn read_meta(data: &Path) -> Option<Meta> {
    let s = fs::read_to_string(dir(data).join("meta.json")).ok()?;
    serde_json::from_str(&s).ok()
}

/// The downloaded cards, plus any starter card the download doesn't have.
fn with_starters(mut cards: Vec<CardRecord>) -> Vec<CardRecord> {
    let names: std::collections::HashSet<String> =
        cards.iter().map(|c| c.def.name.clone()).collect();
    cards.extend(
        client_api::starter_cards()
            .into_iter()
            .filter(|c| !names.contains(&c.def.name)),
    );
    cards
}

/// Parse a gzipped AtomicCards file.
pub fn parse_gz(bytes: &[u8]) -> Result<Vec<CardRecord>, String> {
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut json)
        .map_err(|e| format!("card data is damaged: {e}"))?;
    let file: AtomicFile =
        serde_json::from_slice(&json).map_err(|e| format!("card data is damaged: {e}"))?;
    Ok(convert_all(&file))
}

impl Pool {
    pub fn status(&self) -> CardStatus {
        self.status.lock().unwrap().clone()
    }

    /// Load the pool from disk if it isn't loaded yet.
    pub fn load(&self, data: &Path) -> Option<Arc<Vec<CardRecord>>> {
        if let Some(c) = self.cards.lock().unwrap().clone() {
            return Some(c);
        }
        let bytes = fs::read(dir(data).join("AtomicCards.json.gz")).ok()?;
        match parse_gz(&bytes) {
            Ok(cards) => {
                let cards = Arc::new(with_starters(cards));
                let meta = read_meta(data);
                let mut st = self.status.lock().unwrap();
                st.count = cards.len();
                st.date = meta.as_ref().map(|m| m.date.clone());
                st.version = meta.map(|m| m.version);
                *self.cards.lock().unwrap() = Some(cards.clone());
                Some(cards)
            }
            Err(e) => {
                self.status.lock().unwrap().error = Some(e);
                None
            }
        }
    }

    /// Run `f` with an index over the downloaded pool plus the starter cards.
    pub fn with_index<T>(&self, data: &Path, f: impl FnOnce(&CardIndex) -> T) -> T {
        // Before the first download, only the starter cards are available.
        let all = self
            .load(data)
            .unwrap_or_else(|| Arc::new(client_api::starter_cards()));
        let index = CardIndex::new(&all);
        f(&index)
    }

    /// Download new card data if MTGJSON has published any.
    pub async fn sync(&self, data: &Path) -> Result<CardStatus, String> {
        self.load(data);
        {
            let mut st = self.status.lock().unwrap();
            st.downloading = false;
            st.error = None;
        }
        let client = reqwest::Client::builder()
            .user_agent("CommanderTable-cards")
            .build()
            .map_err(|e| e.to_string())?;
        let remote: carddb::mtgjson::MetaFile = client
            .get(META_URL)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("can't reach MTGJSON: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let local = read_meta(data);
        let have_file = dir(data).join("AtomicCards.json.gz").exists();
        if have_file && !carddb::needs_update(local.as_ref(), &remote.data) {
            return Ok(self.status());
        }
        self.status.lock().unwrap().downloading = true;
        let result = self.download(&client, data, &remote.data).await;
        let mut st = self.status.lock().unwrap();
        st.downloading = false;
        if let Err(e) = &result {
            st.error = Some(e.clone());
        }
        drop(st);
        result.map(|_| self.status())
    }

    async fn download(
        &self,
        client: &reqwest::Client,
        data: &Path,
        meta: &Meta,
    ) -> Result<(), String> {
        let bytes = client
            .get(GZ_URL)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("card download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("card download failed: {e}"))?;
        // MTGJSON publishes a checksum next to each file; use it when it's there.
        if let Ok(r) = client.get(format!("{GZ_URL}.sha256")).send().await {
            if let Ok(text) = r.error_for_status().map_err(|_| ()).map(|r| r.text()) {
                if let Ok(text) = text.await {
                    let expected = text.split_whitespace().next().unwrap_or("").to_lowercase();
                    let got = format!("{:x}", Sha256::digest(&bytes));
                    if expected.len() == 64 && expected != got {
                        return Err(
                            "the card download was corrupted; it will retry next launch".into()
                        );
                    }
                }
            }
        }
        // Parse before replacing, so a bad download never breaks the cards we have.
        let cards = parse_gz(&bytes)?;
        let d = dir(data);
        fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let tmp = d.join("AtomicCards.json.gz.part");
        fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, d.join("AtomicCards.json.gz")).map_err(|e| e.to_string())?;
        fs::write(d.join("meta.json"), serde_json::to_string(meta).unwrap())
            .map_err(|e| e.to_string())?;
        let mut st = self.status.lock().unwrap();
        st.count = cards.len();
        st.date = Some(meta.date.clone());
        st.version = Some(meta.version.clone());
        drop(st);
        *self.cards.lock().unwrap() = Some(Arc::new(with_starters(cards)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SAMPLE: &str =
        include_str!("../../../crates/carddb/tests/fixtures/AtomicCards.sample.json");

    fn gz(s: &str) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(s.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn loads_downloaded_cards_and_falls_back_to_starters() {
        let data = tempfile::tempdir().unwrap();
        let pool = Pool::default();
        // Nothing downloaded yet: starter cards still resolve.
        assert!(pool.with_index(data.path(), |i| i.get("Forest").is_some()));
        fs::create_dir_all(dir(data.path())).unwrap();
        fs::write(dir(data.path()).join("AtomicCards.json.gz"), gz(SAMPLE)).unwrap();
        fs::write(
            dir(data.path()).join("meta.json"),
            r#"{"date":"2026-11-10","version":"5.3.0+20261110"}"#,
        )
        .unwrap();
        let cards = pool.load(data.path()).unwrap();
        assert!(!cards.is_empty());
        assert_eq!(pool.status().date.as_deref(), Some("2026-11-10"));
        let name = cards[0].def.name.clone();
        assert!(pool.with_index(data.path(), |i| i.get(&name).is_some()));
    }

    #[test]
    fn damaged_file_is_reported_not_loaded() {
        assert!(parse_gz(b"not gzip").is_err());
    }
}
