//! The bridge between the rules engine and a player's screen.
//!
//! A [`Session`] owns one game. Clients ask for a [`PlayerView`] — everything that player is
//! allowed to see, plus hints about what they can do — and send back [`Action`]s. In a networked
//! game the host runs the session and sends each player only their own view, so hidden
//! information (hands, libraries) never leaves the host.

pub mod adventure;
pub mod ai;
mod bot;
pub mod builder;
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
const ADVENTURE_CARDS: &str = include_str!("../data/adventure-cards.json");

/// The small built-in card pool used for practice games before real card data is downloaded:
/// the starter decks' cards plus the extra cards adventure mode hands out.
pub fn starter_cards() -> Vec<CardRecord> {
    let mut cards = Vec::new();
    for src in [STARTER_CARDS, ADVENTURE_CARDS] {
        let file: AtomicFile = serde_json::from_str(src).expect("built-in card file parses");
        cards.extend(convert_all(&file));
    }
    cards
}

/// The built-in decks, as decklist text in the same format players paste in. Each is a legal
/// singleton Commander deck: one commander, basic lands, and one copy of everything else.
pub fn starter_decklists() -> Vec<(String, String)> {
    #[allow(clippy::type_complexity)]
    let decks: [(&str, &str, &[(&str, u32)], &[&str]); 4] = [
        (
            "Jenara's Host",
            "Jenara, Asura of War",
            &[("Forest", 13), ("Plains", 13), ("Island", 12)],
            &[
                "Air Elemental",
                "Azure Drake",
                "Cancel",
                "Concentrate",
                "Coral Merfolk",
                "Counterspell",
                "Divination",
                "Essence Scatter",
                "Horned Turtle",
                "Inspiration",
                "Mahamoti Djinn",
                "Man-o'-War",
                "Merfolk of the Pearl Trident",
                "Negate",
                "Repulse",
                "Silver Myr",
                "Snapping Drake",
                "Storm Crow",
                "Unsummon",
                "Vodalian Soldiers",
                "Wall of Air",
                "Watercourser",
                "Wind Drake",
                "Birds of Paradise",
                "Llanowar Elves",
                "Elvish Mystic",
                "Copper Myr",
                "Elvish Visionary",
                "Llanowar Visionary",
                "Garruk's Companion",
                "Giant Spider",
                "Kalonian Tusker",
                "Centaur Courser",
                "Rumbling Baloth",
                "Colossal Dreadmaw",
                "Harmonize",
                "Naturalize",
                "Giant Growth",
                "Titanic Growth",
                "Overrun",
                "Deadly Recluse",
                "Craw Wurm",
                "Serra Angel",
                "Glorious Anthem",
                "Raise the Alarm",
                "Savannah Lions",
                "Youthful Knight",
                "Wall of Omens",
                "Inspiring Overseer",
                "Seraph of Dawn",
                "Attended Knight",
                "Gold Myr",
                "Avacyn's Pilgrim",
                "Bronze Sable",
                "Hedron Archive",
                "Mind Stone",
                "Ornithopter",
                "Palladium Myr",
                "Sol Ring",
                "Thran Dynamo",
                "Yotian Soldier",
            ],
        ),
        (
            "Kamahl's Fury",
            "Kamahl, Pit Fighter",
            &[("Mountain", 38)],
            &[
                "Anaba Bodyguard",
                "Beetleback Chief",
                "Bonebreaker Giant",
                "Borderland Minotaur",
                "Brute Force",
                "Brute Strength",
                "Canyon Minotaur",
                "Charging Monstrosaur",
                "Cobblebrute",
                "Dragon Fodder",
                "Earth Elemental",
                "Ember Shot",
                "Fanatical Firebrand",
                "Fervor",
                "Fire Elemental",
                "Fire Imp",
                "Flame Slash",
                "Flametongue Kavu",
                "Furnace Whelp",
                "Goblin Bully",
                "Goblin Instigator",
                "Goblin Roughrider",
                "Gray Ogre",
                "Hill Giant",
                "Hordeling Outburst",
                "Infuriate",
                "Iron Myr",
                "Krenko's Command",
                "Lava Axe",
                "Lightning Blast",
                "Lightning Bolt",
                "Lightning Elemental",
                "Lightning Strike",
                "Mogg Fanatic",
                "Mons's Goblin Raiders",
                "Mudbutton Torchrunner",
                "Onakke Ogre",
                "Prodigal Pyromancer",
                "Pyroclasm",
                "Raging Cougar",
                "Raging Goblin",
                "Searing Spear",
                "Shatter",
                "Shivan Dragon",
                "Shivan Hellkite",
                "Shock",
                "Sparkmage Apprentice",
                "Sure Strike",
                "Torch Fiend",
                "Uncaged Fury",
                "Volcanic Dragon",
                "Volcanic Hammer",
                "Vulshok Berserker",
                "Bronze Sable",
                "Hedron Archive",
                "Mind Stone",
                "Ornithopter",
                "Palladium Myr",
                "Sol Ring",
                "Thran Dynamo",
                "Yotian Soldier",
            ],
        ),
        (
            "Yargle's Hunger",
            "Yargle, Glutton of Urborg",
            &[("Swamp", 39)],
            &[
                "Ambition's Cost",
                "Bloodhunter Bat",
                "Bog Imp",
                "Cadaver Imp",
                "Child of Night",
                "Dakmor Lancer",
                "Dark Ritual",
                "Disentomb",
                "Disfigure",
                "Doom Blade",
                "Douse in Gloom",
                "Dread Shade",
                "Drown in Sorrow",
                "Dusk Imp",
                "Dusk Legion Zealot",
                "Essence Drain",
                "Feral Abomination",
                "Feral Shadow",
                "Final Reward",
                "Giant Cockroach",
                "Gifted Aetherborn",
                "Grasp of Darkness",
                "Gravedigger",
                "Hand of Death",
                "Hollow Dogs",
                "Last Gasp",
                "Leaden Myr",
                "Muck Rats",
                "Murder",
                "Night's Whisper",
                "Phyrexian Rager",
                "Raise Dead",
                "Ravenous Chupacabra",
                "Scathe Zombies",
                "Serpent Assassin",
                "Shrivel",
                "Sign in Blood",
                "Skeleton Archer",
                "Soul Feast",
                "Sovereign's Bite",
                "Tithebearer Giant",
                "Typhoid Rats",
                "Vampire Champion",
                "Vampire Neonate",
                "Vampire Nighthawk",
                "Vampire Sovereign",
                "Vampire Spawn",
                "Vampire of the Dire Moon",
                "Vampiric Feast",
                "Vicious Hunger",
                "Walking Corpse",
                "Zombie Goliath",
                "Bronze Sable",
                "Hedron Archive",
                "Mind Stone",
                "Ornithopter",
                "Palladium Myr",
                "Sol Ring",
                "Thran Dynamo",
                "Yotian Soldier",
            ],
        ),
        (
            "Isamaru's Pack",
            "Isamaru, Hound of Konda",
            &[("Plains", 39)],
            &[
                "Ajani's Sunstriker",
                "Angel of Mercy",
                "Angelic Wall",
                "Archangel",
                "Ardent Militia",
                "Attended Knight",
                "Aven Cloudchaser",
                "Aven Sentry",
                "Benalish Knight",
                "Benalish Marshal",
                "Benalish Trapper",
                "Blinding Mage",
                "Charge",
                "Day of Judgment",
                "Disenchant",
                "Ekundu Griffin",
                "Elite Vanguard",
                "Gideon's Lawkeeper",
                "Glorious Anthem",
                "Glory Seeker",
                "Gold Myr",
                "Griffin Sentinel",
                "Healer's Hawk",
                "Inspired Charge",
                "Inspiring Overseer",
                "Knight of Meadowgrain",
                "Lone Missionary",
                "Master Decoy",
                "Midnight Haunting",
                "Pillarfield Ox",
                "Raise the Alarm",
                "Revitalize",
                "Sanctuary Cat",
                "Savannah Lions",
                "Seraph of Dawn",
                "Serra Angel",
                "Shepherd of the Lost",
                "Shining Aerosaur",
                "Silvercoat Lion",
                "Standing Troops",
                "Steadfast Guard",
                "Sungrace Pegasus",
                "Sunlance",
                "Suntail Hawk",
                "Tireless Missionaries",
                "Tundra Wolves",
                "Venerable Monk",
                "Wall of Omens",
                "Warrior's Honor",
                "Wild Griffin",
                "Wrath of God",
                "Youthful Knight",
                "Bronze Sable",
                "Hedron Archive",
                "Mind Stone",
                "Ornithopter",
                "Palladium Myr",
                "Sol Ring",
                "Thran Dynamo",
                "Yotian Soldier",
            ],
        ),
    ];
    decks
        .iter()
        .map(|(name, cmd, lands, cards)| {
            let mut text = format!("Commander\n1 {cmd}\n\nDeck\n");
            for (land, n) in lands.iter() {
                text.push_str(&format!("{n} {land}\n"));
            }
            for card in cards.iter() {
                text.push_str(&format!("1 {card}\n"));
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
    /// Starting life for this seat, when it differs from the game's (adventure bosses).
    #[serde(default)]
    pub life: Option<i32>,
}

/// A bot action that has been decided but not yet played, so the UI can show it "thinking".
#[derive(Clone, Debug, Serialize)]
pub struct PendingBot {
    pub player: PlayerId,
    pub think_ms: u32,
    #[serde(skip)]
    action: Option<Action>,
}

/// A moment you could have responded to something on the stack, kept so you can go back to it.
#[derive(Clone)]
struct RewindPoint {
    stack_id: ObjectId,
    label: RewindView,
    game: Game,
}

/// What the rewind panel shows for one point.
#[derive(Clone, Debug, Serialize)]
pub struct RewindView {
    pub index: usize,
    pub name: String,
    pub controller: PlayerId,
    pub turn: u32,
}

/// How many stack items back a player may rewind to respond (house rule, practice games).
pub const REWIND_DEPTH: usize = 2;

pub struct Session {
    pub game: Game,
    pub brains: Vec<Brain>,
    pending: Option<PendingBot>,
    rewind: Vec<RewindPoint>,
    /// Allow going back to respond to the last few stack items.
    pub rewind_enabled: bool,
}

impl Session {
    /// Build a game from seats. Seat 0 is the local player.
    pub fn new_game(seed: u64, seats: Vec<SeatSpec>) -> Session {
        Session::new_game_with(seed, seats, GameConfig::default().starting_life)
    }

    /// Build a game from seats with a different starting life total (adventure duels use 20).
    pub fn new_game_with(seed: u64, seats: Vec<SeatSpec>, starting_life: i32) -> Session {
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
                starting_life,
                ..Default::default()
            },
            decks,
        );
        let mut game = game;
        for (i, s) in seats.iter().enumerate() {
            if let Some(life) = s.life {
                game.players[i].life = life;
            }
        }
        // People are asked where their commander goes each time; computer players always
        // take it back to the command zone.
        for (i, s) in seats.iter().enumerate() {
            if s.ai.is_none() {
                game.player_mut(PlayerId(i as u8)).ask_commander_zone = true;
            }
        }
        let brains = seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.ai.map(|p| Brain::new(PlayerId(i as u8), p, seed)))
            .collect();
        Session {
            game,
            brains,
            pending: None,
            rewind: Vec::new(),
            rewind_enabled: true,
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
                        life: None,
                    }
                } else {
                    let n = names.remove(rng.gen_range(0..names.len()));
                    SeatSpec {
                        name: n.into(),
                        deck,
                        ai: Some(Personality::ALL[rng.gen_range(0..3)]),
                        life: None,
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

    /// Hand a seat to a computer player (someone left an online game). Does nothing if it
    /// already is one.
    pub fn make_ai(&mut self, player: PlayerId, personality: Personality) {
        if self.brains.iter().any(|b| b.me == player) {
            return;
        }
        self.game.player_mut(player).ask_commander_zone = false;
        let mut b = Brain::new(player, personality, 0x5eed ^ player.0 as u64);
        b.observe(&self.game);
        self.brains.push(b);
        self.pending = None;
    }

    /// Submit a person's action. Computer players don't move until asked (see [`Session::next_bot`]).
    pub fn submit(&mut self, player: PlayerId, action: Action) -> Result<(), String> {
        if self.brains.iter().any(|b| b.me == player) {
            return Err("that seat is played by the computer".into());
        }
        let before = self.rewind_candidate(player, &action);
        self.game
            .submit(player, action)
            .map_err(|e| e.to_string())?;
        if let Some(p) = before {
            self.rewind.push(p);
            if self.rewind.len() > REWIND_DEPTH {
                self.rewind.remove(0);
            }
        }
        self.pending = None;
        self.observe_all();
        Ok(())
    }

    /// When a person passes with something on the stack, remember that moment (once per item).
    fn rewind_candidate(&self, player: PlayerId, action: &Action) -> Option<RewindPoint> {
        if !self.rewind_enabled || *action != Action::Pass {
            return None;
        }
        if !matches!(self.game.decision(), Decision::Priority { player: p } if *p == player) {
            return None;
        }
        let top = self.game.stack.last()?;
        if self.rewind.iter().any(|r| r.stack_id == top.id) {
            return None;
        }
        let name = match &top.kind {
            StackKind::Spell => self
                .game
                .object(top.id)
                .map(|o| o.def.name.clone())
                .unwrap_or_default(),
            StackKind::Ability { source_name, .. } => format!("{source_name} (ability)"),
        };
        Some(RewindPoint {
            stack_id: top.id,
            label: RewindView {
                index: 0,
                name,
                controller: top.controller,
                turn: self.game.turn,
            },
            game: self.game.clone(),
        })
    }

    /// The moments you can go back to, newest first.
    pub fn rewind_points(&self) -> Vec<RewindView> {
        self.rewind
            .iter()
            .enumerate()
            .rev()
            // Only this turn: going back further would undo whole turns of play.
            .filter(|(_, r)| r.label.turn == self.game.turn)
            .map(|(i, r)| RewindView {
                index: i,
                ..r.label.clone()
            })
            .collect()
    }

    /// Go back to the moment you could respond to that stack item. Later points are dropped.
    pub fn rewind_to(&mut self, index: usize) -> Result<(), String> {
        if self
            .rewind
            .get(index)
            .is_none_or(|r| r.label.turn != self.game.turn)
        {
            return Err("that moment is too far back".into());
        }
        let point = self.rewind.remove(index);
        self.rewind.truncate(index);
        let mut game = point.game;
        // The clone left the log behind; keep the real history and note the rewind.
        game.log = std::mem::take(&mut self.game.log);
        game.log
            .push(format!("Rewound to respond to {}.", point.label.name));
        self.game = game;
        self.pending = None;
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
    fn online_messages_fit_in_a_steam_message() {
        // Steam's reliable messages top out at 512 KB; decks and views are sent whole.
        let cards = starter_cards();
        let index = carddb::CardIndex::new(&cards);
        let mut biggest = 0;
        for (name, text) in starter_decklists() {
            let deck = resolve_deck(&name, &text, &index).unwrap();
            biggest = biggest.max(serde_json::to_string(&deck).unwrap().len());
        }
        let mut s = Session::new_practice(3, 4);
        s.run_bots();
        let view = serde_json::to_string(&s.view(PlayerId(0))).unwrap().len();
        assert!(biggest < 400_000, "deck is {biggest} bytes");
        assert!(view < 400_000, "view is {view} bytes");
    }

    #[test]
    fn starter_decks_are_100_cards() {
        let cards = starter_cards();
        let index = CardIndex::new(&cards);
        for (name, text) in starter_decklists() {
            let c = check_deck(&text, &index);
            assert_eq!(c.card_count, 100, "{name}");
            assert!(c.playable, "{name}: {:?}", c.problems);
            assert!(c.legal, "{name}: {:?}", c.problems);
            assert!(c.unknown.is_empty());
        }
    }

    #[test]
    fn duplicate_cards_make_a_deck_unplayable() {
        let cards = starter_cards();
        let index = CardIndex::new(&cards);
        let (_, text) = starter_decklists().remove(1);
        let doubled = text.replace("1 Lightning Bolt", "2 Lightning Bolt");
        let c = check_deck(&doubled, &index);
        assert!(!c.playable);
        assert!(
            c.problems.iter().any(|p| p.contains("Lightning Bolt")),
            "{:?}",
            c.problems
        );
        assert!(check_deck(&text, &index).playable);
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
    fn rewind_returns_to_a_stack_item_you_passed_on() {
        let mut s = Session::new_practice(4, 2);
        let mut rewound = false;
        for _ in 0..5000 {
            s.run_bots();
            let d = s.game.decision().clone();
            let Some(p) = d.player() else { break };
            if matches!(d, Decision::Priority { .. })
                && !s.game.stack.is_empty()
                && s.rewind_points().is_empty()
            {
                let top = s.game.stack.last().unwrap().id;
                s.submit(p, Action::Pass).unwrap();
                s.run_bots();
                let points = s.rewind_points();
                assert_eq!(points.len(), 1);
                s.rewind_to(points[0].index).unwrap();
                assert_eq!(s.game.stack.last().map(|e| e.id), Some(top));
                assert!(matches!(s.game.decision(), Decision::Priority { player } if *player == p));
                assert!(s.rewind_points().is_empty());
                rewound = true;
                break;
            }
            let a = bot_action(&s.game, p);
            s.submit(p, a).unwrap();
        }
        assert!(rewound, "never saw a stack item to respond to");
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
