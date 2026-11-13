//! Game state: players, zones, objects and stored continuous effects.

use crate::card::{Ability, CardDef, CounterKind, Duration, Modification, TargetSpec};
use crate::ids::{CardId, ObjectId, PlayerId, Timestamp};
use crate::mana::ManaPool;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Stack,
    Exile,
    Command,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameObject {
    pub id: ObjectId,
    pub card: CardId,
    #[serde(skip)]
    pub def: Arc<CardDef>,
    pub owner: PlayerId,
    /// Controller before layer 2 effects: the player who cast/put it onto the battlefield.
    pub base_controller: PlayerId,
    pub zone: Zone,
    pub timestamp: Timestamp,
    pub tapped: bool,
    /// True while the controller has not continuously controlled it since their most recent turn began (CR 302.6).
    pub summoning_sick: bool,
    pub damage: i32,
    pub damaged_by_deathtouch: bool,
    pub counters: BTreeMap<CounterKind, u32>,
    pub counter_timestamp: Timestamp,
    pub attached_to: Option<ObjectId>,
    pub is_token: bool,
    /// Moved to a graveyard or exile since state-based actions were last checked (CR 903.9a).
    pub new_in_zone: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub life: i32,
    pub poison: u32,
    pub library: Vec<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
    pub mana_pool: ManaPool,
    pub lands_played_this_turn: u32,
    pub land_plays_allowed: u32,
    pub max_hand_size: Option<usize>,
    pub commanders: Vec<CardId>,
    /// Times each commander has been cast from the command zone (CR 903.8).
    pub commander_casts: HashMap<CardId, u32>,
    /// Combat damage taken from each commander, keyed by the commander card (CR 903.10a).
    pub commander_damage: HashMap<CardId, i32>,
    /// Standing answer to the "may put it into the command zone" choices of CR 903.9a/b.
    pub return_commander_to_command_zone: bool,
    pub drew_from_empty_library: bool,
    pub has_lost: bool,
    pub has_left: bool,
    pub mulligans: u32,
}

/// Something on the stack (CR 405).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StackEntry {
    pub id: ObjectId,
    pub controller: PlayerId,
    pub kind: StackKind,
    pub targets: Vec<Target>,
    pub x: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StackKind {
    /// A spell: the card object itself is in the stack zone with this id.
    Spell,
    /// An activated or triggered ability. `source` may no longer exist (last-known info, CR 113.7a).
    Ability {
        source: ObjectId,
        source_name: String,
        ability: Box<Ability>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    Object(ObjectId),
    Player(PlayerId),
}

/// A continuous effect created by a resolving spell or ability (CR 611.2), stored until it expires.
/// Effects from static abilities are not stored; they're generated from the battlefield each time
/// characteristics are computed (CR 611.3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredEffect {
    pub id: u64,
    pub timestamp: Timestamp,
    pub source: Option<ObjectId>,
    pub controller: PlayerId,
    /// Locked-in affected objects (CR 611.2c).
    pub objects: Vec<ObjectId>,
    pub mods: Vec<Modification>,
    pub duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Step {
    Untap,
    Upkeep,
    Draw,
    Main1,
    BeginCombat,
    DeclareAttackers,
    DeclareBlockers,
    FirstStrikeDamage,
    CombatDamage,
    EndCombat,
    Main2,
    End,
    Cleanup,
}

impl Step {
    pub fn next(self) -> Option<Step> {
        use Step::*;
        Some(match self {
            Untap => Upkeep,
            Upkeep => Draw,
            Draw => Main1,
            Main1 => BeginCombat,
            BeginCombat => DeclareAttackers,
            DeclareAttackers => DeclareBlockers,
            DeclareBlockers => FirstStrikeDamage,
            FirstStrikeDamage => CombatDamage,
            CombatDamage => EndCombat,
            EndCombat => Main2,
            Main2 => End,
            End => Cleanup,
            Cleanup => return None,
        })
    }
    pub fn is_main(self) -> bool {
        matches!(self, Step::Main1 | Step::Main2)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Combat {
    /// attacker -> defending player
    pub attackers: Vec<(ObjectId, PlayerId)>,
    /// blocker -> attacker
    pub blockers: Vec<(ObjectId, ObjectId)>,
    pub blocked: Vec<ObjectId>,
    /// Creatures that dealt first-strike damage (CR 510.4).
    pub dealt_first_strike: Vec<ObjectId>,
}

/// A triggered ability waiting to be put on the stack (CR 603.3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingTrigger {
    pub controller: PlayerId,
    pub source: ObjectId,
    pub source_name: String,
    pub ability: Ability,
    /// Object or player the event was about ("that creature"), if any.
    pub subject: Option<Target>,
}

impl PendingTrigger {
    pub fn targets(&self) -> &[TargetSpec] {
        match &self.ability {
            Ability::Triggered { targets, .. } => targets,
            _ => &[],
        }
    }
}
