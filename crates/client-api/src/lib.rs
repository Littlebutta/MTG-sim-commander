//! The bridge between the rules engine and a player's screen.
//!
//! A [`Session`] owns one game. Clients ask for a [`PlayerView`] — everything that player is
//! allowed to see, plus hints about what they can do — and send back [`Action`]s. In a networked
//! game the host runs the session and sends each player only their own view, so hidden
//! information (hands, libraries) never leaves the host.

mod bot;
mod describe;
mod view;

pub use bot::bot_action;
pub use view::*;

use carddb::{convert_all, AtomicFile, CardIndex, CardRecord};
use rules_core::*;
use std::sync::Arc;

const STARTER_CARDS: &str = include_str!("../data/starter-cards.json");

/// The small built-in card pool used for practice games before real card data is downloaded.
pub fn starter_cards() -> Vec<CardRecord> {
    let file: AtomicFile = serde_json::from_str(STARTER_CARDS).expect("starter card file parses");
    convert_all(&file)
}

/// A practice deck: commander name, then (count, card) pairs.
fn practice_decks() -> Vec<(&'static str, &'static str, Vec<(u32, &'static str)>)> {
    vec![
        (
            "You",
            "Jenara, Asura of War",
            vec![
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
            vec![
                (36, "Mountain"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (6, "Lightning Bolt"),
                (6, "Shock"),
                (5, "Searing Spear"),
                (4, "Lava Axe"),
                (8, "Raging Goblin"),
                (8, "Hill Giant"),
                (6, "Fire Elemental"),
                (6, "Furnace Whelp"),
                (12, "Mountain"),
            ],
        ),
        (
            "Yargle's Hunger",
            "Yargle, Glutton of Urborg",
            vec![
                (36, "Swamp"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (6, "Murder"),
                (8, "Vampire Nighthawk"),
                (8, "Typhoid Rats"),
                (6, "Gravedigger"),
                (6, "Night's Whisper"),
                (8, "Hypnotic Specter"),
                (8, "Gray Merchant Acolyte"),
                (11, "Swamp"),
            ],
        ),
        (
            "Isamaru's Pack",
            "Isamaru, Hound of Konda",
            vec![
                (36, "Plains"),
                (1, "Sol Ring"),
                (1, "Mind Stone"),
                (10, "Savannah Lions"),
                (10, "Youthful Knight"),
                (8, "Serra Angel"),
                (6, "Benalish Marshal"),
                (4, "Glorious Anthem"),
                (8, "Raise the Alarm"),
                (3, "Day of Judgment"),
                (12, "Plains"),
            ],
        ),
    ]
}

pub struct Session {
    pub game: Game,
    pub bots: Vec<PlayerId>,
}

impl Session {
    /// A practice game: player 0 is human, the rest are bots.
    pub fn new_practice(seed: u64, players: usize) -> Session {
        let cards = starter_cards();
        let index = CardIndex::new(&cards);
        let get = |n: &str| -> Arc<CardDef> {
            Arc::new(
                index
                    .get(n)
                    .unwrap_or_else(|| panic!("starter card {n}"))
                    .def
                    .clone(),
            )
        };
        let decks: Vec<DeckSpec> = practice_decks()
            .into_iter()
            .take(players.clamp(2, 4))
            .map(|(who, cmd, list)| DeckSpec {
                player_name: who.to_string(),
                commanders: vec![get(cmd)],
                cards: list
                    .iter()
                    .flat_map(|(n, name)| (0..*n).map(|_| get(name)))
                    .take(99)
                    .collect(),
            })
            .collect();
        let n = decks.len();
        let game = Game::new_commander(
            GameConfig {
                seed,
                starting_player: Some(0),
                skip_mulligans: false,
                ..Default::default()
            },
            decks,
        );
        let mut s = Session {
            game,
            bots: (1..n as u8).map(PlayerId).collect(),
        };
        s.run_bots();
        s
    }

    pub fn view(&self, viewer: PlayerId) -> PlayerView {
        view::build(&self.game, viewer)
    }

    pub fn submit(&mut self, player: PlayerId, action: Action) -> Result<(), String> {
        self.game
            .submit(player, action)
            .map_err(|e| e.to_string())?;
        self.run_bots();
        Ok(())
    }

    /// Let bots answer until a human has to decide (or the game ends).
    pub fn run_bots(&mut self) {
        for _ in 0..5000 {
            let Some(p) = self.game.decision().player() else {
                return;
            };
            if !self.bots.contains(&p) {
                return;
            }
            let a = bot_action(&self.game, p);
            if self.game.submit(p, a).is_err() {
                // A bot should never get stuck: fall back to the simplest legal answer.
                let fallback = match self.game.decision() {
                    Decision::DeclareAttackers { .. } => Action::Attack(vec![]),
                    Decision::DeclareBlockers { .. } => Action::Block(vec![]),
                    _ => Action::Pass,
                };
                if self.game.submit(p, fallback).is_err() {
                    let _ = self.game.submit(p, Action::Concede);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(matches!(
            s.game.decision(),
            Decision::Mulligan {
                player: PlayerId(0),
                ..
            }
        ));
        s.submit(PlayerId(0), Action::Keep { bottom: vec![] })
            .unwrap();
        // The human just passes / does nothing; the game must keep moving.
        for _ in 0..3000 {
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
