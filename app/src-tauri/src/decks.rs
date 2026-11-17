//! Saved decks, one JSON file each in `data/decks/`, so they live in the game folder and
//! survive updates.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedDeck {
    pub id: String,
    pub name: String,
    pub text: String,
    /// Milliseconds since 1970, for sorting.
    pub updated: u64,
}

fn dir(data: &Path) -> PathBuf {
    data.join("decks")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn list(data: &Path) -> Vec<SavedDeck> {
    let mut out: Vec<SavedDeck> = fs::read_dir(dir(data))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_str(&fs::read_to_string(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    out
}

pub fn save(data: &Path, deck: &SavedDeck) -> Result<(), String> {
    if !valid_id(&deck.id) {
        return Err("bad deck id".into());
    }
    fs::create_dir_all(dir(data)).map_err(|e| e.to_string())?;
    let path = dir(data).join(format!("{}.json", deck.id));
    let tmp = path.with_extension("json.part");
    fs::write(&tmp, serde_json::to_string_pretty(deck).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn delete(data: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("bad deck id".into());
    }
    let p = dir(data).join(format!("{id}.json"));
    if p.exists() {
        fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_delete() {
        let d = tempfile::tempdir().unwrap();
        let a = SavedDeck {
            id: "a1".into(),
            name: "Elves".into(),
            text: "1 Forest".into(),
            updated: 1,
        };
        let b = SavedDeck {
            id: "b2".into(),
            name: "Burn".into(),
            text: "1 Mountain".into(),
            updated: 2,
        };
        save(d.path(), &a).unwrap();
        save(d.path(), &b).unwrap();
        assert_eq!(list(d.path()), vec![b.clone(), a.clone()]);
        delete(d.path(), "a1").unwrap();
        assert_eq!(list(d.path()), vec![b]);
        assert!(save(
            d.path(),
            &SavedDeck {
                id: "../x".into(),
                ..a
            }
        )
        .is_err());
    }
}
