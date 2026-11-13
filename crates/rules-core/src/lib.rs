//! Commander rules engine.
//!
//! Implements the Magic: The Gathering Comprehensive Rules (November 6, 2026 edition) for the
//! Commander variant (CR 903): turn structure, priority, the stack, state-based actions,
//! triggered abilities, combat and the continuous-effect layer system (CR 613).
//!
//! The engine is UI- and network-agnostic. A host owns a [`Game`], reads
//! [`Game::decision`] to learn who must act, and feeds answers to [`Game::submit`].

pub mod card;
pub mod engine;
pub mod game;
pub mod ids;
pub mod layers;
pub mod mana;
pub mod state;

pub use card::*;
pub use engine::{Action, Decision, RulesError};
pub use game::{DeckSpec, Game, GameConfig};
pub use ids::*;
pub use layers::{Characteristics, View};
pub use mana::*;
pub use state::*;
