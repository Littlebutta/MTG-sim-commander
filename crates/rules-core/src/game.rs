//! The `Game` struct, Commander game setup (CR 903.6–903.7) and zone changes (CR 400).

use crate::card::{CardDef, CounterKind};
use crate::ids::{CardId, ObjectId, PlayerId, Timestamp};
use crate::layers::View;
use crate::mana::ManaPool;
use crate::state::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// A Commander deck as it enters a game.
#[derive(Clone, Debug)]
pub struct DeckSpec {
    pub player_name: String,
    pub commanders: Vec<Arc<CardDef>>,
    /// The other 99 (or 98 with partners).
    pub cards: Vec<Arc<CardDef>>,
}

#[derive(Clone, Debug)]
pub struct GameConfig {
    pub seed: u64,
    pub starting_life: i32,
    pub starting_hand: usize,
    /// Index of the starting player; `None` picks one at random (CR 103.1).
    pub starting_player: Option<usize>,
    /// Skip the mulligan procedure (used by tests and quick-start games).
    pub skip_mulligans: bool,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            seed: 0,
            starting_life: 40,
            starting_hand: 7,
            starting_player: None,
            skip_mulligans: false,
        }
    }
}

/// Where the turn-structure state machine is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Mulligans(VecDeque<PlayerId>),
    StepStart,
    /// Run SBAs and put triggers on the stack, then give `PlayerId` priority (CR 117.5).
    PriorityCheck(PlayerId),
    /// Waiting for an answer to `Game::decision`.
    Waiting,
    /// Defending players still to declare blockers, in APNAP order.
    Blockers(VecDeque<PlayerId>),
    StepEnd,
    GameOver,
}

pub struct Game {
    pub players: Vec<Player>,
    pub objects: BTreeMap<ObjectId, GameObject>,
    pub battlefield: Vec<ObjectId>,
    pub exile: Vec<ObjectId>,
    pub command: Vec<ObjectId>,
    pub(crate) stack_cards: Vec<ObjectId>,
    /// Bottom first; the last entry is the top of the stack.
    pub stack: Vec<StackEntry>,
    pub effects: Vec<StoredEffect>,
    pub pending_triggers: Vec<PendingTrigger>,
    pub combat: Combat,
    pub turn: u32,
    pub active: PlayerId,
    pub step: Step,
    pub log: Vec<String>,
    pub(crate) stage: Stage,
    pub(crate) decision: crate::engine::Decision,
    pub(crate) passes: usize,
    pub(crate) pending_blockers: VecDeque<PlayerId>,
    pub(crate) resume_after_triggers: Option<Stage>,
    /// Last-known information for permanents that just left together (603.10a).
    pub(crate) lookback: Vec<(ObjectId, crate::layers::Characteristics)>,
    pub(crate) rng: ChaCha8Rng,
    pub(crate) next_object: u32,
    pub(crate) next_card: u32,
    pub(crate) next_timestamp: Timestamp,
    pub(crate) next_effect: u64,
    pub(crate) starting_player: PlayerId,
    pub(crate) config: GameConfig,
    /// Bumped on every state change; keys the characteristics cache.
    pub version: u64,
    pub(crate) view_cache: Mutex<Option<(u64, Arc<View>)>>,
}

/// Games are cloned for "what if" simulations (practice AI), so cloning must be cheap:
/// card definitions are shared, the characteristics cache is carried over, and the game log
/// is not copied.
impl Clone for Game {
    fn clone(&self) -> Self {
        Game {
            players: self.players.clone(),
            objects: self.objects.clone(),
            battlefield: self.battlefield.clone(),
            exile: self.exile.clone(),
            command: self.command.clone(),
            stack_cards: self.stack_cards.clone(),
            stack: self.stack.clone(),
            effects: self.effects.clone(),
            pending_triggers: self.pending_triggers.clone(),
            combat: self.combat.clone(),
            turn: self.turn,
            active: self.active,
            step: self.step,
            log: Vec::new(),
            stage: self.stage.clone(),
            decision: self.decision.clone(),
            passes: self.passes,
            pending_blockers: self.pending_blockers.clone(),
            resume_after_triggers: self.resume_after_triggers.clone(),
            lookback: self.lookback.clone(),
            rng: self.rng.clone(),
            next_object: self.next_object,
            next_card: self.next_card,
            next_timestamp: self.next_timestamp,
            next_effect: self.next_effect,
            starting_player: self.starting_player,
            config: self.config.clone(),
            version: self.version,
            view_cache: Mutex::new(self.view_cache.lock().ok().and_then(|c| c.clone())),
        }
    }
}

impl Game {
    pub fn new_commander(config: GameConfig, decks: Vec<DeckSpec>) -> Game {
        let mut g = Game {
            players: Vec::new(),
            objects: BTreeMap::new(),
            battlefield: Vec::new(),
            exile: Vec::new(),
            command: Vec::new(),
            stack_cards: Vec::new(),
            stack: Vec::new(),
            effects: Vec::new(),
            pending_triggers: Vec::new(),
            combat: Combat::default(),
            turn: 0,
            active: PlayerId(0),
            step: Step::Untap,
            log: Vec::new(),
            stage: Stage::StepStart,
            decision: crate::engine::Decision::GameOver { winner: None },
            passes: 0,
            pending_blockers: VecDeque::new(),
            resume_after_triggers: None,
            lookback: Vec::new(),
            rng: ChaCha8Rng::seed_from_u64(config.seed),
            next_object: 1,
            next_card: 1,
            next_timestamp: 1,
            next_effect: 1,
            starting_player: PlayerId(0),
            config: config.clone(),
            version: 0,
            view_cache: Mutex::new(None),
        };
        for (i, deck) in decks.iter().enumerate() {
            let pid = PlayerId(i as u8);
            g.players.push(Player {
                id: pid,
                name: deck.player_name.clone(),
                life: config.starting_life,
                poison: 0,
                library: Vec::new(),
                hand: Vec::new(),
                graveyard: Vec::new(),
                mana_pool: ManaPool::default(),
                lands_played_this_turn: 0,
                land_plays_allowed: 1,
                max_hand_size: Some(7),
                commanders: Vec::new(),
                commander_casts: HashMap::new(),
                commander_damage: HashMap::new(),
                return_commander_to_command_zone: true,
                drew_from_empty_library: false,
                has_lost: false,
                has_left: false,
                mulligans: 0,
            });
            // 903.6: commanders start face up in the command zone.
            for c in &deck.commanders {
                let id = g.create_object(c.clone(), pid, Zone::Command);
                let card = g.objects[&id].card;
                g.players[i].commanders.push(card);
            }
            for c in &deck.cards {
                g.create_object(c.clone(), pid, Zone::Library);
            }
        }
        for i in 0..g.players.len() {
            g.shuffle_library(PlayerId(i as u8));
        }
        let start = config.starting_player.unwrap_or_else(|| {
            use rand::Rng;
            g.rng.gen_range(0..g.players.len().max(1))
        });
        g.starting_player = PlayerId(start as u8);
        g.active = g.starting_player;
        // 903.7: 40 life (set above) and seven cards.
        for i in 0..g.players.len() {
            g.draw_cards(PlayerId(i as u8), config.starting_hand);
        }
        g.log(format!("{} goes first.", g.player(g.starting_player).name));
        if config.skip_mulligans {
            g.begin_turn(g.starting_player);
        } else {
            g.stage = Stage::Mulligans(g.turn_order_from(g.starting_player).into());
        }
        g.run();
        g
    }

    pub fn player(&self, p: PlayerId) -> &Player {
        &self.players[p.0 as usize]
    }
    pub fn player_mut(&mut self, p: PlayerId) -> &mut Player {
        self.version += 1;
        &mut self.players[p.0 as usize]
    }
    pub fn object(&self, id: ObjectId) -> Option<&GameObject> {
        self.objects.get(&id)
    }
    pub(crate) fn object_mut(&mut self, id: ObjectId) -> Option<&mut GameObject> {
        self.version += 1;
        self.objects.get_mut(&id)
    }
    pub fn in_game(&self, p: PlayerId) -> bool {
        let pl = self.player(p);
        !pl.has_left && !pl.has_lost
    }
    pub fn players_in_game(&self) -> Vec<PlayerId> {
        self.players
            .iter()
            .map(|p| p.id)
            .filter(|p| self.in_game(*p))
            .collect()
    }

    /// Players still in the game, in turn order starting with `from` (APNAP order when `from` is the active player, CR 101.4).
    pub fn turn_order_from(&self, from: PlayerId) -> Vec<PlayerId> {
        let n = self.players.len();
        (0..n)
            .map(|i| PlayerId(((from.0 as usize + i) % n) as u8))
            .filter(|p| self.in_game(*p))
            .collect()
    }

    pub fn next_player_after(&self, p: PlayerId) -> Option<PlayerId> {
        let n = self.players.len();
        (1..=n)
            .map(|i| PlayerId(((p.0 as usize + i) % n) as u8))
            .find(|q| self.in_game(*q))
    }

    pub(crate) fn log(&mut self, s: String) {
        self.log.push(s);
    }

    pub(crate) fn new_timestamp(&mut self) -> Timestamp {
        let t = self.next_timestamp;
        self.next_timestamp += 1;
        t
    }

    pub(crate) fn create_object(
        &mut self,
        def: Arc<CardDef>,
        owner: PlayerId,
        zone: Zone,
    ) -> ObjectId {
        let card = CardId(self.next_card);
        self.next_card += 1;
        self.create_object_for_card(def, card, owner, owner, zone)
    }

    fn create_object_for_card(
        &mut self,
        def: Arc<CardDef>,
        card: CardId,
        owner: PlayerId,
        controller: PlayerId,
        zone: Zone,
    ) -> ObjectId {
        let id = ObjectId(self.next_object);
        self.next_object += 1;
        let ts = self.new_timestamp();
        let is_token = def.is_token;
        self.objects.insert(
            id,
            GameObject {
                id,
                card,
                def,
                owner,
                base_controller: controller,
                zone,
                timestamp: ts,
                tapped: false,
                summoning_sick: true,
                damage: 0,
                damaged_by_deathtouch: false,
                counters: BTreeMap::new(),
                counter_timestamp: ts,
                attached_to: None,
                is_token,
                new_in_zone: matches!(zone, Zone::Graveyard | Zone::Exile),
            },
        );
        self.zone_list_mut(zone, owner).push(id);
        self.version += 1;
        id
    }

    fn zone_list_mut(&mut self, zone: Zone, owner: PlayerId) -> &mut Vec<ObjectId> {
        let p = owner.0 as usize;
        match zone {
            Zone::Library => &mut self.players[p].library,
            Zone::Hand => &mut self.players[p].hand,
            Zone::Graveyard => &mut self.players[p].graveyard,
            Zone::Battlefield => &mut self.battlefield,
            Zone::Exile => &mut self.exile,
            Zone::Command => &mut self.command,
            // The stack zone's order lives in `Game::stack`; this list is unused bookkeeping.
            Zone::Stack => &mut self.stack_cards,
        }
    }

    pub fn is_commander(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| self.player(o.owner).commanders.contains(&o.card))
    }

    /// Move an object to another zone (CR 400.7: it becomes a new object, returned here).
    /// Applies the commander replacement effect of CR 903.9b. Returns `None` if the object
    /// no longer exists (e.g. a token that ceased to exist).
    pub fn move_object(
        &mut self,
        id: ObjectId,
        mut to: Zone,
        controller: Option<PlayerId>,
    ) -> Option<ObjectId> {
        let obj = self.objects.get(&id)?.clone();
        if matches!(to, Zone::Hand | Zone::Library)
            && self.is_commander(id)
            && self.player(obj.owner).return_commander_to_command_zone
        {
            to = Zone::Command;
        }
        // Remove from old zone.
        let from = obj.zone;
        self.zone_list_mut(from, obj.owner).retain(|x| *x != id);
        if from == Zone::Stack {
            self.stack.retain(|e| e.id != id);
        }
        self.objects.remove(&id);
        self.combat.attackers.retain(|(a, _)| *a != id);
        self.combat.blockers.retain(|(b, a)| *b != id && *a != id);
        self.version += 1;

        let ctl = match to {
            Zone::Battlefield | Zone::Stack => controller.unwrap_or(obj.owner),
            _ => obj.owner,
        };
        let new_id = self.create_object_for_card(obj.def.clone(), obj.card, obj.owner, ctl, to);
        if let Some(o) = self.objects.get_mut(&new_id) {
            o.is_token = obj.is_token;
        }
        let name = obj.def.name.clone();
        self.log(format!("{name}: {:?} -> {:?}", from, to));
        Some(new_id)
    }

    /// Put into library at a position from the top (0 = top).
    pub fn move_to_library(&mut self, id: ObjectId, from_top: Option<usize>) -> Option<ObjectId> {
        let new_id = self.move_object(id, Zone::Library, None)?;
        let owner = self.objects[&new_id].owner;
        if self.objects[&new_id].zone == Zone::Library {
            let lib = &mut self.players[owner.0 as usize].library;
            lib.retain(|x| *x != new_id);
            // Library is stored bottom..top; index from the top.
            let pos = from_top.map_or(0, |n| lib.len().saturating_sub(n));
            lib.insert(pos, new_id);
        }
        Some(new_id)
    }

    pub fn shuffle_library(&mut self, p: PlayerId) {
        let mut lib = std::mem::take(&mut self.players[p.0 as usize].library);
        lib.shuffle(&mut self.rng);
        self.players[p.0 as usize].library = lib;
        self.version += 1;
    }

    /// Draw `n` cards (CR 121). Drawing from an empty library flags the player for 704.5b.
    pub fn draw_cards(&mut self, p: PlayerId, n: usize) {
        for _ in 0..n {
            match self.players[p.0 as usize].library.last().copied() {
                Some(top) => {
                    self.move_object(top, Zone::Hand, None);
                }
                None => {
                    self.player_mut(p).drew_from_empty_library = true;
                }
            }
        }
    }

    pub fn add_counters(&mut self, id: ObjectId, kind: CounterKind, n: u32) {
        let ts = self.new_timestamp();
        if let Some(o) = self.object_mut(id) {
            *o.counters.entry(kind).or_insert(0) += n;
            o.counter_timestamp = ts;
        }
    }

    pub fn hand(&self, p: PlayerId) -> &[ObjectId] {
        &self.player(p).hand
    }

    /// Test/debug helper: create a card directly in a zone.
    pub fn debug_create(&mut self, def: Arc<CardDef>, owner: PlayerId, zone: Zone) -> ObjectId {
        let id = self.create_object(def, owner, zone);
        if zone == Zone::Battlefield {
            self.objects.get_mut(&id).unwrap().summoning_sick = false;
        }
        id
    }

    pub fn debug_set_life(&mut self, p: PlayerId, life: i32) {
        self.player_mut(p).life = life;
    }
}
