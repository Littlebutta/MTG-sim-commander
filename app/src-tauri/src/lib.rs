//! Desktop shell for Commander Table: window, self-update, and (next) Steam networking.

mod update;

use std::path::PathBuf;
use update::UpdateInfo;

/// The folder the game lives in. Everything (data, decks, cards) stays inside it.
fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    update::cleanup_old(&install_dir());
    let _ = std::fs::create_dir_all(install_dir().join("data"));
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_version,
            check_update,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running Commander Table");
}
