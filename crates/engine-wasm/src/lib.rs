//! WebAssembly wrapper so the React UI can run a full practice game in the browser.
//! The desktop app talks to the same `client-api` through Tauri commands instead.

use client_api::Session;
use rules_core::{Action, PlayerId};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmSession {
    inner: Session,
}

#[wasm_bindgen]
impl WasmSession {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, players: usize) -> WasmSession {
        WasmSession {
            inner: Session::new_practice(seed, players),
        }
    }

    /// JSON `PlayerView` for player 0 (the local human).
    pub fn view(&self) -> String {
        serde_json::to_string(&self.inner.view(PlayerId(0))).unwrap()
    }

    /// Submit a JSON `Action` for player 0. Returns an error message if it was illegal.
    pub fn submit(&mut self, action_json: &str) -> Result<(), JsValue> {
        let action: Action =
            serde_json::from_str(action_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner
            .submit(PlayerId(0), action)
            .map_err(|e| JsValue::from_str(&e))
    }
}
