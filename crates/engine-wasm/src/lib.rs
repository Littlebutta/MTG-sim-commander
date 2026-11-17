//! WebAssembly wrapper so the React UI runs the real rules engine, in the desktop window and in
//! a browser during development.

use client_api::{starter_cards, starter_decklists, SeatSpec, Session};
use rules_core::{Action, PlayerId};
use wasm_bindgen::prelude::*;

fn js_err(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub struct WasmSession {
    inner: Session,
}

#[wasm_bindgen]
impl WasmSession {
    /// Quick practice game with the starter decks.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64, players: usize) -> WasmSession {
        WasmSession {
            inner: Session::new_practice(seed, players),
        }
    }

    /// A game from JSON `SeatSpec`s. Seat 0 is the local player.
    pub fn from_seats(seed: u64, seats_json: &str) -> Result<WasmSession, JsValue> {
        let seats: Vec<SeatSpec> = serde_json::from_str(seats_json).map_err(js_err)?;
        if seats.len() < 2 {
            return Err(js_err("a game needs at least two players"));
        }
        Ok(WasmSession {
            inner: Session::new_game(seed, seats),
        })
    }

    /// JSON `PlayerView` for player 0 (the local player).
    pub fn view(&self) -> String {
        serde_json::to_string(&self.inner.view(PlayerId(0))).unwrap()
    }

    /// Submit a JSON `Action` for player 0. Returns an error message if it was illegal.
    pub fn submit(&mut self, action_json: &str) -> Result<(), JsValue> {
        let action: Action = serde_json::from_str(action_json).map_err(js_err)?;
        self.inner.submit(PlayerId(0), action).map_err(js_err)
    }

    /// If a computer player is deciding, JSON `{player, think_ms}`; otherwise `null`.
    pub fn next_bot(&mut self) -> String {
        serde_json::to_string(&self.inner.next_bot()).unwrap()
    }

    /// Play the move the computer player decided on.
    pub fn apply_bot(&mut self) {
        self.inner.apply_bot();
    }

    /// Turn the "go back and respond" house rule on or off.
    pub fn set_rewind(&mut self, enabled: bool) {
        self.inner.rewind_enabled = enabled;
    }

    /// JSON list of the last stack items you passed on, newest first.
    pub fn rewind_points(&self) -> String {
        serde_json::to_string(&self.inner.rewind_points()).unwrap()
    }

    /// Go back to the moment you could respond to that item.
    pub fn rewind(&mut self, index: usize) -> Result<(), JsValue> {
        self.inner.rewind_to(index).map_err(js_err)
    }
}

/// The built-in decks as JSON `[{name, text}]`.
#[wasm_bindgen]
pub fn starter_decks() -> String {
    let list: Vec<serde_json::Value> = starter_decklists()
        .into_iter()
        .map(|(name, text)| serde_json::json!({ "name": name, "text": text }))
        .collect();
    serde_json::to_string(&list).unwrap()
}

/// Check a decklist against the built-in card pool (browser development only; the desktop app
/// checks against the full downloaded card pool).
#[wasm_bindgen]
pub fn check_deck(text: &str) -> String {
    let cards = starter_cards();
    let index = carddb::CardIndex::new(&cards);
    serde_json::to_string(&client_api::check_deck(text, &index)).unwrap()
}

/// Resolve a decklist against the built-in card pool into JSON `ResolvedDeck`.
#[wasm_bindgen]
pub fn resolve_deck(name: &str, text: &str) -> Result<String, JsValue> {
    let cards = starter_cards();
    let index = carddb::CardIndex::new(&cards);
    let deck = client_api::resolve_deck(name, text, &index).map_err(js_err)?;
    Ok(serde_json::to_string(&deck).unwrap())
}

/// Search the built-in card pool. `query_json` is a `SearchQuery`; returns JSON `CardSummary[]`.
#[wasm_bindgen]
pub fn search_cards(query_json: &str) -> Result<String, JsValue> {
    let q: client_api::builder::SearchQuery = serde_json::from_str(query_json).map_err(js_err)?;
    let cards = starter_cards();
    Ok(serde_json::to_string(&client_api::builder::search(&cards, &q)).unwrap())
}

/// Statistics and suggestions for a decklist, from the built-in card pool.
#[wasm_bindgen]
pub fn recommend_cards(text: &str) -> String {
    let cards = starter_cards();
    let index = carddb::CardIndex::new(&cards);
    serde_json::to_string(&client_api::builder::recommend(text, &cards, &index)).unwrap()
}

/// Summaries for a JSON list of card names, from the built-in card pool.
#[wasm_bindgen]
pub fn lookup_cards(names_json: &str) -> Result<String, JsValue> {
    let names: Vec<String> = serde_json::from_str(names_json).map_err(js_err)?;
    let cards = starter_cards();
    let index = carddb::CardIndex::new(&cards);
    Ok(serde_json::to_string(&client_api::builder::lookup(&index, &names)).unwrap())
}
