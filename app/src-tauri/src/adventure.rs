//! Adventure mode: saved journeys (in slots) and adventure deck checks (no singleton, up to four
//! copies, against the full card pool).

use crate::{data_dir, PoolState};
use client_api::adventure::AdventureDeckCheck;
use client_api::ResolvedDeck;
use std::fs;

/// Journeys are kept in save slots. Slot 1 is `adventure.json` (where the single save used to
/// live), the others `adventure-2.json` and so on.
fn file_name(slot: Option<u8>) -> String {
    match slot.unwrap_or(1).clamp(1, 9) {
        1 => "adventure.json".into(),
        n => format!("adventure-{n}.json"),
    }
}

/// The journey in a slot, or `null` if there is none.
#[tauri::command]
pub fn load_adventure(slot: Option<u8>) -> serde_json::Value {
    fs::read_to_string(data_dir().join(file_name(slot)))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Save the journey. Writes a temporary file first so a crash never leaves half a save, and
/// keeps the previous save as `adventure.json.bak`.
#[tauri::command]
pub fn save_adventure(slot: Option<u8>, save: serde_json::Value) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = file_name(slot);
    let path = dir.join(&name);
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, save.to_string()).map_err(|e| e.to_string())?;
    if path.exists() {
        let _ = fs::copy(&path, dir.join(format!("{name}.bak")));
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Forget the journey in a slot (a copy stays as `.bak`).
#[tauri::command]
pub fn delete_adventure(slot: Option<u8>) -> Result<(), String> {
    let dir = data_dir();
    let name = file_name(slot);
    let path = dir.join(&name);
    if path.exists() {
        let _ = fs::copy(&path, dir.join(format!("{name}.bak")));
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
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
