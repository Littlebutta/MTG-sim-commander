//! Adventure mode: the saved journey and adventure deck checks (no singleton, up to four
//! copies, against the full card pool).

use crate::{data_dir, PoolState};
use client_api::adventure::AdventureDeckCheck;
use client_api::ResolvedDeck;
use std::fs;

fn save_path() -> std::path::PathBuf {
    data_dir().join("adventure.json")
}

/// The saved journey, or `null` if there is none.
#[tauri::command]
pub fn load_adventure() -> serde_json::Value {
    fs::read_to_string(save_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Save the journey. Writes a temporary file first so a crash never leaves half a save, and
/// keeps the previous save as `adventure.json.bak`.
#[tauri::command]
pub fn save_adventure(save: serde_json::Value) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = save_path();
    let tmp = dir.join("adventure.json.tmp");
    fs::write(&tmp, save.to_string()).map_err(|e| e.to_string())?;
    if path.exists() {
        let _ = fs::copy(&path, dir.join("adventure.json.bak"));
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_adventure_deck(
    pool: PoolState<'_>,
    text: String,
) -> Result<AdventureDeckCheck, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_index(&data_dir(), |i| {
            client_api::adventure::check_adventure_deck(&text, i)
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resolve_adventure_deck(
    pool: PoolState<'_>,
    name: String,
    text: String,
) -> Result<ResolvedDeck, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_index(&data_dir(), |i| {
            client_api::adventure::resolve_adventure_deck(&name, &text, i)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
