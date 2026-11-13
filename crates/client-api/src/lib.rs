//! The bridge between the rules engine and a player's screen.
//!
//! A [`Session`] owns one game. Clients ask for a [`PlayerView`] — everything that player is
//! allowed to see, plus hints about what they can do — and send back [`Action`]s. In a networked
//! game the host runs the session and sends each player only their own view, so hidden
//! information (hands, libraries) never leaves the host.

pub mod ai;
mod bot;
pub mod decks;
mod describe;
mod view;

pub use ai::{Brain, Personality};
pub use bot::bot_action;
pub use decks::{check_deck, resolve_deck, DeckCheck, ResolvedDeck};
pub use view::*;

use carddb::{convert_all, AtomicFile, CardIndex, CardRecord};
use rand::{Rng, SeedableRng};
use rules_core::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const STARTER_CARDS: &str = include_str!("../data/starter-cards.json");

/// The small built-in card pool used for practice games before real card data is downloaded.
pub fn starter_cards() -> Vec<CardRecord> {
    let file: AtomicFile = serde_json::from_str(STARTER_CARDS).expect("starter card file parses");
    convert_all(&file)
}

/// The built-in decks, as decklist text in the same format players paste in.
pub fn starter_decklists() -> Vec<(String, String)> {
    let decks: [(&str, &str, &[(u32, &str)]); 4] = [
        (
            "Jenara's Host",
            "Jenara, Asura of War",
            &[
                (14, "Forest"),
                (14, "Plains"),
                (14, "Island"),
                (2, "Titanic Growth"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (4, "Llanowar Elves"),
                (4, "Elvish Visionary"),
                (4, "Llanowar Visionary"),
                (4, "Grizzly Bears"),
                (3, "Giant Spider"),
                (3, "Garruk's Companion"),
                (2, "Craw Wurm"),
                (3, "Giant Growth"),
                (3, "Savannah Lions"),
                (3, "Youthful Knight"),
                (2, "Serra Angel"),
                (2, "Benalish Marshal"),
                (1, "Glorious Anthem"),
                (2, "Raise the Alarm"),
                (2, "Counterspell"),
                (2, "Unsummon"),
                (3, "Wind Drake"),
                (2, "Air Elemental"),
                (2, "Man-o'-War"),
                (2, "Divination"),
            ],
        ),
        (
            "Kamahl's Fury",
            "Kamahl, Pit Fighter",
            &[
                (46, "Mountain"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (6, "Lightning Bolt"),
                (6, "Shock"),
                (5, "Searing Spear"),
                (4, "Lava Axe"),
                (8, "Raging Goblin"),
                (8, "Hill Giant"),
                (6, "Fire Elemental"),
                (8, "Furnace Whelp"),
            ],
        ),
        (
            "Yargle's Hunger",
            "Yargle, Glutton of Urborg",
            &[
                (45, "Swamp"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (6, "Murder"),
                (8, "Vampire Nighthawk"),
                (8, "Typhoid Rats"),
                (6, "Gravedigger"),
                (6, "Night's Whisper"),
                (8, "Hypnotic Specter"),
                (10, "Gray Merchant Acolyte"),
            ],
        ),
        (
            "Isamaru's Pack",
            "Isamaru, Hound of Konda",
            &[
                (46, "Plains"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (10, "Savannah Lions"),
                (10, "Youthful Knight"),
                (8, "Serra Angel"),
                (6, "Benalish Marshal"),
                (4, "Glorious Anthem"),
                (10, "Raise the Alarm"),
                (3, "Day of Judgment"),
            ],
        ),
    ];
    decks
        .iter()
        .map(|(name, cmd, list)| {
            let mut text = format!("Commander\n1 {cmd}\n\nDeck\n");
            for (n, card) in list.iter() {
                text.push_str(&format!("{n} {card}\n"));
            }
            (name.to_string(), text)
        })
        .collect()
}

/// Names for computer players, so a table reads like a game night.
const AI_NAMES: [&str; 12] = [
    "Mara", "Tobias", "Juniper", "Sol", "Priya", "Elias", "Wren", "Dario", "Hana", "Felix",
    "Odessa", "Kit",
];

/// One seat at the table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatSpec {
    pub name: String,
    pub deck: ResolvedDeck,
    /// `None` for a person; otherwise the computer player's style.
    pub ai: Option<Personality>,
}

/// A bot action that has been decided but not yet played, so the UI can show it "thinking".
#[derive(Clone, Debug, Serialize)]
pub struct PendingBot {
    pub player: PlayerId,
    pub think_ms: u32,
    #[serde(skip)]
    action: Option<Action>,
}

pub struct Session {
    pub game: Game,
    pub brains: Vec<Brain>,
    pending: Option<PendingBot>,
}

impl Session {
    /// Build a game from seats. Seat 0 is the local player.
    pub fn new_game(seed: u64, seats: Vec<SeatSpec>) -> Session {
        let decks: Vec<DeckSpec> = seats
            .iter()
            .map(|s| DeckSpec {
                player_name: s.name.clone(),
                commanders: s.deck.commanders.iter().cloned().map(Arc::new).collect(),
                cards: s.deck.cards.iter().cloned().map(Arc::new).collect(),
            })
            .collect();
        let game = Game::new_commander(
            GameConfig {
                seed,
                starting_player: None,
                skip_mulligans: false,
                ..Default::default()
            },
            decks,
        );
        let brains = seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.ai.map(|p| Brain::new(PlayerId(i as u8), p, seed)))
            .collect();
        Session {
            game,
            brains,
            pending: None,
        }
    }

    /// A quick game with the starter decks: you against `players - 1` computer opponents.
    pub fn new_practice(seed: u64, players: usize) -> Session {
        let cards = starter_cards();
        let index = CardIndex::new(&cards);
        let decks = starter_decklists();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut names: Vec<&str> = AI_NAMES.to_vec();
        let seats = decks
            .iter()
            .take(players.clamp(2, 4))
            .enumerate()
            .map(|(i, (name, text))| {
                let deck = decks::resolve_deck(name, text, &index).expect("starter deck resolves");
                if i == 0 {
                    SeatSpec {
                        name: "You".into(),
                        deck,
                        ai: None,
                    }
                } else {
                    let n = names.remove(rng.gen_range(0..names.len()));
                    SeatSpec {
                        name: n.into(),
                        deck,
                        ai: Some(Personality::ALL[rng.gen_range(0..3)]),
                    }
                }
            })
            .collect();
        Session::new_game(seed, seats)
    }

    pub fn view(&self, viewer: PlayerId) -> PlayerView {
        view::build(&self.game, viewer)
    }

    fn observe_all(&mut self) {
        let g = &self.game;
        for b in &mut self.brains {
            b.observe(g);
        }
    }

    /// Submit a person's action. Computer players don't move until asked (see [`Session::next_bot`]).
    pub fn submit(&mut self, player: PlayerId, action: Action) -> Result<(), String> {
        if self.brains.iter().any(|b| b.me == player) {
            return Err("that seat is played by the computer".into());
        }
        self.game
            .submit(player, action)
            .map_err(|e| e.to_string())?;
        self.pending = None;
        self.observe_all();
        Ok(())
    }

    /// If a computer player must decide now, decide (without playing it yet) and say how long
    /// a person would think about it. `None` when a person has to act or the game is over.
    pub fn next_bot(&mut self) -> Option<PendingBot> {
        let p = self.game.decision().player()?;
        if let Some(pb) = &self.pending {
            if pb.player == p {
                return Some(pb.clone());
            }
        }
        let game = &self.game;
        let brain = self.brains.iter_mut().find(|b| b.me == p)?;
        let d = brain.decide(game);
        let pb = PendingBot {
            player: p,
            think_ms: d.think_ms,
            action: Some(d.action),
        };
        self.pending = Some(pb.clone());
        Some(pb)
    }

    /// Play the computer action chosen by [`Session::next_bot`].
    pub fn apply_bot(&mut self) {
        let Some(pb) = self.pending.take() else {
            return;
        };
        let p = pb.player;
        let action = pb.action.unwrap_or(Action::Pass);
        if self.game.submit(p, action).is_err() {
            // A computer player must never stall the table: fall back to the simplest answer.
            let fallback = match self.game.decision() {
                Decision::DeclareAttackers { .. } => Action::Attack(vec![]),
                Decision::DeclareBlockers { .. } => Action::Block(vec![]),
                _ => bot_action(&self.game, p),
            };
            if self.game.submit(p, fallback).is_err() && self.game.submit(p, Action::Pass).is_err()
            {
                let _ = self.game.submit(p, Action::Concede);
            }
        }
        self.observe_all();
    }

    /// Let computer players act, without delays, until a person must decide or the game ends.
    pub fn run_bots(&mut self) {
        for _ in 0..20_000 {
            if self.next_bot().is_none() {
                return;
            }
            self.apply_bot();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_decks_are_100_cards() {
        let cards = starter_cards();
        let index = CardIndex::new(&cards);
        for (name, text) in starter_decklists() {
            let c = check_deck(&text, &index);
            assert_eq!(c.card_count, 100, "{name}");
            assert!(c.playable, "{name}: {:?}", c.problems);
            assert!(c.unknown.is_empty());
        }
    }

    #[test]
    fn starter_cards_are_all_supported() {
        for c in starter_cards() {
            assert!(
                c.def.fully_supported(),
                "{} has unsupported text: {:?}",
                c.def.name,
                c.def.abilities
            );
        }
    }

    #[test]
    fn practice_game_reaches_a_human_decision_and_plays_out() {
        let mut s = Session::new_practice(3, 4);
        s.run_bots();
        assert!(matches!(
            s.game.decision(),
            Decision::Mulligan {
                player: PlayerId(0),
                ..
            }
        ));
        s.submit(PlayerId(0), Action::Keep { bottom: vec![] })
            .unwrap();
        // The person plays the simple way; the computer players think for themselves.
        for _ in 0..3000 {
            s.run_bots();
            match s.game.decision().clone() {
                Decision::GameOver { .. } => break,
                d => {
                    let p = d.player().unwrap();
                    let a = bot_action(&s.game, p);
                    s.submit(p, a).unwrap();
                }
            }
        }
        let v = s.view(PlayerId(0));
        assert!(v.turn > 3, "turn {}", v.turn);
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"players\""));
    }

    #[test]
    fn views_hide_other_hands() {
        let s = Session::new_practice(1, 2);
        let v = s.view(PlayerId(0));
        assert_eq!(v.hand.len(), 7);
        let opp = v.players.iter().find(|p| p.id == 1).unwrap();
        assert_eq!(opp.hand_count, 7);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(
            json.matches("\"zone\":\"Hand\"").count(),
            7,
            "only your own hand's cards are sent"
        );
        assert!(
            !json.contains("\"zone\":\"Library\""),
            "no library contents are sent"
        );
    }
}
