//! Desktop shell for Commander Table: window, self-update, card data, saved decks, and (next)
//! Steam networking.

mod art;
mod cards;
mod decks;
mod update;

use cards::{CardStatus, Pool};
use client_api::{DeckCheck, ResolvedDeck};
use decks::SavedDeck;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use update::UpdateInfo;

/// The folder the game lives in. Everything (data, decks, cards) stays inside it.
fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn data_dir() -> PathBuf {
    install_dir().join("data")
}

type PoolState<'a> = State<'a, Arc<Pool>>;

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn check_update() -> Result<Option<UpdateInfo>, String> {
    update::check(env!("CARGO_PKG_VERSION")).await
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle, info: UpdateInfo) -> Result<(), String> {
    let bytes = update::download(&info).await?;
    update::apply_zip(&bytes, &install_dir())?;
    app.restart();
}

#[tauri::command]
fn card_status(pool: PoolState<'_>) -> CardStatus {
    pool.status()
}

#[tauri::command]
async fn sync_cards(pool: PoolState<'_>) -> Result<CardStatus, String> {
    let pool = pool.inner().clone();
    // Loading and parsing ~30,000 cards is CPU work: keep it off the UI thread.
    tauri::async_runtime::spawn(async move { pool.sync(&data_dir()).await })
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn check_deck(pool: PoolState<'_>, text: String) -> Result<DeckCheck, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_index(&data_dir(), |i| client_api::check_deck(&text, i))
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_cards(
    pool: PoolState<'_>,
    query: client_api::builder::SearchQuery,
) -> Result<Vec<client_api::builder::CardSummary>, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_pool(&data_dir(), |all, _| {
            client_api::builder::search(all, &query)
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn lookup_cards(
    pool: PoolState<'_>,
    names: Vec<String>,
) -> Result<Vec<client_api::builder::CardSummary>, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_index(&data_dir(), |i| client_api::builder::lookup(i, &names))
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn recommend_cards(
    pool: PoolState<'_>,
    text: String,
) -> Result<client_api::builder::Recommendations, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_pool(&data_dir(), |all, i| {
            client_api::builder::recommend(&text, all, i)
        })
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn resolve_deck(
    pool: PoolState<'_>,
    name: String,
    text: String,
) -> Result<ResolvedDeck, String> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        pool.with_index(&data_dir(), |i| client_api::resolve_deck(&name, &text, i))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_decks() -> Vec<SavedDeck> {
    decks::list(&data_dir())
}

#[tauri::command]
fn save_deck(deck: SavedDeck) -> Result<(), String> {
    decks::save(&data_dir(), &deck)
}

#[tauri::command]
fn delete_deck(id: String) -> Result<(), String> {
    decks::delete(&data_dir(), &id)
}

/// Card art for `name`, from the cache or Scryfall. `None` if there is none or we're offline.
#[tauri::command]
async fn card_art(
    art: State<'_, Arc<art::ArtCache>>,
    name: String,
) -> Result<Option<art::Art>, String> {
    let art = art.inner().clone();
    Ok(art.get(&data_dir(), &name).await.unwrap_or(None))
}

/// Player settings (board theme, card back...), kept in `data/settings.json`.
#[tauri::command]
fn load_settings() -> serde_json::Value {
    std::fs::read_to_string(data_dir().join("settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
fn save_settings(settings: serde_json::Value) -> Result<(), String> {
    std::fs::write(data_dir().join("settings.json"), settings.to_string())
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    update::cleanup_old(&install_dir());
    let _ = std::fs::create_dir_all(data_dir());
    tauri::Builder::default()
        .manage(Arc::new(Pool::default()))
        .manage(Arc::new(art::ArtCache::default()))
        .invoke_handler(tauri::generate_handler![
            app_version,
            check_update,
            install_update,
            card_status,
            sync_cards,
            check_deck,
            resolve_deck,
            search_cards,
            recommend_cards,
            lookup_cards,
            list_decks,
            save_deck,
            delete_deck,
            card_art,
            load_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Commander Table");
}
