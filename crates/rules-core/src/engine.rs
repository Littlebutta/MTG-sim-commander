//! Turn structure, priority, casting, the stack, state-based actions, triggers and combat.
//!
//! The engine is a state machine. It runs until some player has to decide something, then
//! exposes that as `Game::decision()` and waits for `Game::submit`. Every rules-driven step in
//! between (turn-based actions, state-based actions, putting triggers on the stack,
//! resolution) happens automatically, exactly in the order the Comprehensive Rules give.

use crate::card::*;
use crate::game::{Game, Stage};
use crate::ids::{ObjectId, PlayerId};
use crate::layers::{eval_amount, matches_filter, Characteristics, View};
use crate::mana::{Color, ManaCost, ManaPool, ManaType};
use crate::state::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

/// What the game is waiting for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Keep or mulligan (CR 103.5).
    Mulligan {
        player: PlayerId,
        bottom_count_if_keep: u32,
    },
    Priority {
        player: PlayerId,
    },
    /// Choose order and targets for triggered abilities you control (CR 603.3b, 603.3d).
    /// The answer lists indices into `triggers` from the one put on the stack first (bottom).
    PutTriggersOnStack {
        player: PlayerId,
        count: usize,
    },
    DeclareAttackers {
        player: PlayerId,
    },
    DeclareBlockers {
        player: PlayerId,
    },
    /// Divide a blocked attacker's combat damage among its blockers (CR 510.1c), and with
    /// trample, how much goes past them to the player (CR 702.19b). `blockers` pairs each
    /// blocker with the damage that counts as lethal for it (1 with deathtouch, CR 702.2c).
    /// `suggested` is a legal split the player can accept as is.
    AssignCombatDamage {
        player: PlayerId,
        attacker: ObjectId,
        defender: PlayerId,
        power: i32,
        trample: bool,
        deathtouch: bool,
        blockers: Vec<(ObjectId, i32)>,
        suggested: Vec<(Target, i32)>,
    },
    /// Your commander went to a graveyard, exile, your hand or your library: leave it there,
    /// or put it into the command zone (CR 903.9a–b)? Asked only of players who turned
    /// asking on; for hand and library the question comes right after the move.
    CommanderZone {
        player: PlayerId,
        commander: ObjectId,
        zone: Zone,
    },
    /// Discard down to maximum hand size in cleanup (CR 514.1).
    Discard {
        player: PlayerId,
        count: usize,
    },
    GameOver {
        winner: Option<PlayerId>,
    },
}

impl Decision {
    pub fn player(&self) -> Option<PlayerId> {
        match self {
            Decision::Mulligan { player, .. }
            | Decision::Priority { player }
            | Decision::PutTriggersOnStack { player, .. }
            | Decision::DeclareAttackers { player }
            | Decision::DeclareBlockers { player }
            | Decision::AssignCombatDamage { player, .. }
            | Decision::CommanderZone { player, .. }
            | Decision::Discard { player, .. } => Some(*player),
            Decision::GameOver { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Keep {
        bottom: Vec<ObjectId>,
    },
    Mulligan,
    Pass,
    PlayLand(ObjectId),
    Cast {
        card: ObjectId,
        targets: Vec<Target>,
        x: u32,
        /// Which mode of a modal spell (CR 700.2); None for other spells.
        #[serde(default)]
        mode: Option<usize>,
    },
    /// `ability` indexes the object's current abilities (`Characteristics::abilities`).
    Activate {
        object: ObjectId,
        ability: usize,
        targets: Vec<Target>,
        x: u32,
    },
    ActivateMana {
        object: ObjectId,
        ability: usize,
        color: Option<Color>,
    },
    /// (index into the pending triggers shown to you, targets) from bottom to top.
    OrderTriggers(Vec<(usize, Vec<Target>)>),
    Attack(Vec<(ObjectId, PlayerId)>),
    Block(Vec<(ObjectId, ObjectId)>),
    /// Take back a mana ability whose mana hasn't been spent: untap the permanent and remove
    /// its mana from the pool. Only for plain "{T}: Add ..." abilities, until you take another
    /// action or pass (a client convenience, like rewinding an accidental tap).
    UndoMana(ObjectId),
    /// Answer to `Decision::AssignCombatDamage`: how much damage goes where.
    AssignDamage(Vec<(Target, i32)>),
    Discard(Vec<ObjectId>),
    /// Allowed at any time (CR 104.3a).
    Concede,
    /// Standing choice for "may put it into the command zone" (CR 903.9a/b). Allowed any time.
    SetCommanderReturn(bool),
    /// Ask me each time my commander goes to a graveyard or exile, instead of using the
    /// standing choice. Allowed any time.
    SetCommanderAsk(bool),
    /// Answer to `Decision::CommanderZone`: true moves the commander to the command zone.
    CommanderToCommandZone(bool),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RulesError {
    #[error("it isn't your decision to make")]
    NotYourDecision,
    #[error("that action doesn't answer the current decision")]
    WrongAction,
    #[error("illegal: {0}")]
    Illegal(String),
}

fn illegal<T>(s: impl Into<String>) -> Result<T, RulesError> {
    Err(RulesError::Illegal(s.into()))
}

/// What happened, for trigger checks (CR 603.2).
#[derive(Clone, Debug)]
enum Event {
    Entered(ObjectId),
    Died {
        lki: Box<Characteristics>,
        source: ObjectId,
    },
    Cast {
        player: PlayerId,
        spell: ObjectId,
    },
    Upkeep(PlayerId),
    EndStep(PlayerId),
    Attacked(ObjectId),
    CombatDamageToPlayer {
        source: ObjectId,
        player: PlayerId,
    },
    GainedLife(PlayerId),
    BeginCombat(PlayerId),
    FirstMain(PlayerId),
    AttackersDeclared(PlayerId),
}

/// Bookkeeping for one resolving spell/ability.
struct Resolution {
    controller: PlayerId,
    source: Option<ObjectId>,
    targets: Vec<Target>,
    legal: Vec<bool>,
    x: u32,
    /// Controller and power of each target object as the spell or ability began to resolve.
    lki: Vec<Option<(PlayerId, i32)>>,
}

impl Game {
    pub fn decision(&self) -> &Decision {
        &self.decision
    }

    /// Submit a player's answer to the current decision (or an any-time action).
    pub fn submit(&mut self, player: PlayerId, action: Action) -> Result<(), RulesError> {
        match action {
            Action::Concede => {
                if !self.in_game(player) {
                    return illegal("not in the game");
                }
                self.log(format!("{} concedes.", self.player(player).name));
                self.player_mut(player).has_lost = true;
                self.leave_game(player);
                if self.decision.player() == Some(player) || matches!(self.stage, Stage::Waiting) {
                    self.reroute_after_leave(player);
                }
                self.run();
                return Ok(());
            }
            Action::SetCommanderReturn(b) => {
                self.player_mut(player).return_commander_to_command_zone = b;
                return Ok(());
            }
            Action::SetCommanderAsk(b) => {
                self.player_mut(player).ask_commander_zone = b;
                return Ok(());
            }
            _ => {}
        }
        if self.decision.player() != Some(player) {
            return Err(RulesError::NotYourDecision);
        }
        let decision = self.decision.clone();
        match (decision, action) {
            (Decision::Mulligan { .. }, Action::Mulligan) => self.do_mulligan(player),
            (
                Decision::Mulligan {
                    bottom_count_if_keep,
                    ..
                },
                Action::Keep { bottom },
            ) => self.do_keep(player, bottom, bottom_count_if_keep)?,
            (Decision::Priority { .. }, Action::Pass) => self.do_pass(player),
            (Decision::Priority { .. }, Action::PlayLand(card)) => {
                self.play_land(player, card)?;
                self.after_action(player);
            }
            (
                Decision::Priority { .. },
                Action::Cast {
                    card,
                    targets,
                    x,
                    mode,
                },
            ) => {
                self.cast_spell(player, card, targets, x, mode)?;
                self.after_action(player);
            }
            (
                Decision::Priority { .. },
                Action::Activate {
                    object,
                    ability,
                    targets,
                    x,
                },
            ) => {
                self.activate(player, object, ability, targets, x)?;
                self.after_action(player);
            }
            (
                Decision::Priority { .. },
                Action::ActivateMana {
                    object,
                    ability,
                    color,
                },
            ) => {
                // Like after_action, but tapping for mana keeps earlier mana undoable.
                self.activate_mana(player, object, ability, color)?;
                self.passes = 0;
                self.stage = Stage::PriorityCheck(player);
            }
            (Decision::Priority { .. }, Action::UndoMana(object)) => {
                self.undo_mana(player, object)?;
                self.stage = Stage::PriorityCheck(player);
            }
            (Decision::PutTriggersOnStack { .. }, Action::OrderTriggers(order)) => {
                self.order_triggers(player, order)?
            }
            (Decision::DeclareAttackers { .. }, Action::Attack(a)) => {
                self.declare_attackers(player, a)?
            }
            (Decision::DeclareBlockers { .. }, Action::Block(b)) => {
                self.declare_blockers(player, b)?
            }
            (d @ Decision::AssignCombatDamage { .. }, Action::AssignDamage(split)) => {
                self.assign_damage(d, split)?
            }
            (Decision::CommanderZone { commander, .. }, Action::CommanderToCommandZone(b)) => {
                self.answer_commander_zone(commander, b)
            }
            (Decision::Discard { count, .. }, Action::Discard(cards)) => {
                self.cleanup_discard(player, cards, count)?
            }
            _ => return Err(RulesError::WrongAction),
        }
        self.run();
        Ok(())
    }

    fn wait(&mut self, d: Decision) {
        self.decision = d;
        self.stage = Stage::Waiting;
    }

    fn after_action(&mut self, player: PlayerId) {
        // 117.3c: the player who acted receives priority again.
        self.passes = 0;
        self.mana_undo.clear();
        self.stage = Stage::PriorityCheck(player);
    }

    /// Drive the state machine until a decision is needed.
    pub(crate) fn run(&mut self) {
        for _ in 0..100_000 {
            if self.players_in_game().len() <= 1 && !matches!(self.stage, Stage::GameOver) {
                let winner = self.players_in_game().first().copied();
                if let Some(w) = winner {
                    self.log(format!("{} wins the game.", self.player(w).name));
                }
                self.stage = Stage::GameOver;
                self.decision = Decision::GameOver { winner };
                return;
            }
            match self.stage.clone() {
                Stage::GameOver | Stage::Waiting => return,
                Stage::Mulligans(mut q) => {
                    q.retain(|p| self.in_game(*p));
                    match q.front().copied() {
                        Some(p) => {
                            let m = self.player(p).mulligans;
                            // 103.5c: the first mulligan is free in multiplayer games.
                            let free = if self.players.len() > 2 { 1 } else { 0 };
                            self.stage = Stage::Mulligans(q);
                            self.decision = Decision::Mulligan {
                                player: p,
                                bottom_count_if_keep: m.saturating_sub(free),
                            };
                            return;
                        }
                        None => self.begin_turn(self.starting_player),
                    }
                }
                Stage::StepStart => self.start_step(),
                Stage::PriorityCheck(p) => {
                    if !self.check_sba_and_triggers() {
                        return; // waiting on trigger ordering
                    }
                    let p = if self.in_game(p) {
                        p
                    } else {
                        self.next_player_after(p).unwrap_or(p)
                    };
                    self.wait(Decision::Priority { player: p });
                    return;
                }
                Stage::Blockers(mut q) => match q.pop_front() {
                    Some(p) => {
                        self.pending_blockers = q;
                        self.wait(Decision::DeclareBlockers { player: p });
                        return;
                    }
                    None => {
                        self.passes = 0;
                        self.stage = Stage::PriorityCheck(self.priority_start_player());
                    }
                },
                Stage::Damage => {
                    let fs = self.step == Step::FirstStrikeDamage;
                    match self.combat.damage_queue.pop_front() {
                        Some(a) => {
                            if let Some(d) = self.damage_decision(a, fs) {
                                self.wait(d);
                                return;
                            }
                        }
                        None => {
                            self.combat_damage(fs);
                            self.combat.damage_plan.clear();
                            self.passes = 0;
                            self.stage = Stage::PriorityCheck(self.priority_start_player());
                        }
                    }
                }
                Stage::StepEnd => self.end_step(),
            }
        }
        panic!("rules engine did not reach a decision (infinite loop?)");
    }

    // ---------------------------------------------------------------- mulligans

    fn do_mulligan(&mut self, p: PlayerId) {
        let hand = self.player(p).hand.clone();
        for c in hand {
            self.move_to_library(c, None);
        }
        self.shuffle_library(p);
        self.player_mut(p).mulligans += 1;
        let n = self.config.starting_hand;
        self.draw_cards(p, n);
        self.log(format!("{} mulligans.", self.player(p).name));
        if let Stage::Mulligans(mut q) = self.stage.clone() {
            q.pop_front();
            q.push_back(p);
            self.stage = Stage::Mulligans(q);
        }
    }

    fn do_keep(&mut self, p: PlayerId, bottom: Vec<ObjectId>, need: u32) -> Result<(), RulesError> {
        if bottom.len() as u32 != need {
            return illegal(format!("put exactly {need} cards on the bottom"));
        }
        let hand = self.player(p).hand.clone();
        if !bottom.iter().all(|c| hand.contains(c)) {
            return illegal("those cards aren't in your hand");
        }
        for c in bottom {
            let new = self.move_object(c, Zone::Library, None);
            if let Some(n) = new {
                let lib = &mut self.players[p.0 as usize].library;
                lib.retain(|x| *x != n);
                lib.insert(0, n);
            }
        }
        if let Stage::Mulligans(mut q) = self.stage.clone() {
            q.pop_front();
            self.stage = Stage::Mulligans(q);
        }
        Ok(())
    }

    // ---------------------------------------------------------------- turn structure

    pub(crate) fn begin_turn(&mut self, p: PlayerId) {
        self.turn += 1;
        self.active = p;
        self.step = Step::Untap;
        self.combat = Combat::default();
        for pl in self.players.iter_mut() {
            pl.lands_played_this_turn = 0;
        }
        self.log(format!("Turn {}: {}", self.turn, self.player(p).name));
        self.stage = Stage::StepStart;
    }

    /// The player who gets priority first in a step: the active player, or the next player
    /// if the active player has left the game (800.4j).
    fn priority_start_player(&self) -> PlayerId {
        if self.in_game(self.active) {
            self.active
        } else {
            self.next_player_after(self.active).unwrap_or(self.active)
        }
    }

    fn start_step(&mut self) {
        self.passes = 0;
        let ap = self.active;
        // 500.4 / 514.2 durations expiring at step start are handled in end_step.
        match self.step {
            Step::Untap => {
                // 502.3: untap; 302.6: summoning sickness wears off for the active player.
                let view = self.view();
                let ids: Vec<ObjectId> = self.battlefield.clone();
                for id in ids {
                    if view.controller(id) == Some(ap) {
                        let stays = view
                            .get(id)
                            .is_some_and(|c| c.has_keyword(Keyword::DoesntUntap));
                        if let Some(o) = self.object_mut(id) {
                            if !stays {
                                o.tapped = false;
                            }
                            o.summoning_sick = false;
                            o.loyalty_used = false;
                        }
                    }
                }
                self.stage = Stage::StepEnd;
            }
            Step::Upkeep => {
                self.fire(Event::Upkeep(ap));
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
            Step::Draw => {
                // 103.8a: in a two-player game the starting player skips their first draw.
                let skip = self.players.len() == 2 && self.turn == 1;
                if !skip && self.in_game(ap) {
                    self.draw_cards(ap, 1);
                }
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
            Step::Main1 | Step::Main2 | Step::BeginCombat | Step::EndCombat => {
                if self.step == Step::BeginCombat {
                    self.fire(Event::BeginCombat(ap));
                }
                if self.step == Step::Main1 {
                    self.fire(Event::FirstMain(ap));
                }
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
            Step::DeclareAttackers => {
                let can_attack =
                    self.in_game(ap) && self.battlefield.iter().any(|id| self.could_attack(*id));
                if can_attack {
                    self.wait(Decision::DeclareAttackers { player: ap });
                } else {
                    self.combat.attackers.clear();
                    self.skip_to(Step::EndCombat);
                }
            }
            Step::DeclareBlockers => {
                let mut defenders: Vec<PlayerId> = Vec::new();
                for p in self.turn_order_from(ap) {
                    if self.combat.attackers.iter().any(|(_, d)| *d == p) {
                        defenders.push(p);
                    }
                }
                self.stage = Stage::Blockers(defenders.into());
            }
            Step::FirstStrikeDamage => {
                let view = self.view();
                let any = self.combat_creatures().iter().any(|id| {
                    view.get(*id).is_some_and(|c| {
                        c.has_keyword(Keyword::FirstStrike) || c.has_keyword(Keyword::DoubleStrike)
                    })
                });
                if any {
                    self.begin_damage(true);
                } else {
                    self.stage = Stage::StepEnd;
                }
            }
            Step::CombatDamage => self.begin_damage(false),
            Step::End => {
                self.fire(Event::EndStep(ap));
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
            Step::Cleanup => {
                // 514.1: discard to maximum hand size.
                if self.in_game(ap) {
                    let view = self.view();
                    let unlimited = self.battlefield.iter().any(|id| {
                        view.get(*id).is_some_and(|c| {
                            c.controller == ap
                                && c.abilities
                                    .iter()
                                    .any(|a| a.ability == Ability::NoMaxHandSize)
                        })
                    });
                    let max = self.player(ap).max_hand_size.filter(|_| !unlimited);
                    let n = self.player(ap).hand.len();
                    if let Some(max) = max {
                        if n > max {
                            self.wait(Decision::Discard {
                                player: ap,
                                count: n - max,
                            });
                            return;
                        }
                    }
                }
                self.cleanup_rest();
            }
        }
    }

    fn cleanup_rest(&mut self) {
        // 514.2: remove damage and end "until end of turn" effects simultaneously.
        for id in self.battlefield.clone() {
            if let Some(o) = self.object_mut(id) {
                o.damage = 0;
                o.damaged_by_deathtouch = false;
                o.regen_shields = 0;
                o.activated_this_turn.clear();
            }
        }
        self.effects.retain(|e| e.duration != Duration::EndOfTurn);
        self.version += 1;
        // 514.3a: if SBAs would be performed or triggers are waiting, players get priority.
        let sba = self.perform_sbas();
        if sba || !self.pending_triggers.is_empty() || !self.commander_questions().is_empty() {
            self.stage = Stage::PriorityCheck(self.priority_start_player());
        } else {
            self.stage = Stage::StepEnd;
        }
    }

    fn cleanup_discard(
        &mut self,
        p: PlayerId,
        cards: Vec<ObjectId>,
        count: usize,
    ) -> Result<(), RulesError> {
        if cards.len() != count || !cards.iter().all(|c| self.player(p).hand.contains(c)) {
            return illegal(format!("discard exactly {count} cards from your hand"));
        }
        for c in cards {
            self.move_object(c, Zone::Graveyard, None);
        }
        self.cleanup_rest();
        Ok(())
    }

    fn skip_to(&mut self, step: Step) {
        self.empty_mana_pools();
        self.step = step;
        self.stage = Stage::StepStart;
    }

    fn empty_mana_pools(&mut self) {
        for p in self.players.iter_mut() {
            p.mana_pool.empty();
        }
        self.mana_undo.clear();
        self.version += 1;
    }

    fn end_step(&mut self) {
        // 500.5: mana empties as each step and phase ends.
        self.empty_mana_pools();
        if self.step == Step::EndCombat {
            // 500.5a / 511.3
            self.effects.retain(|e| e.duration != Duration::EndOfCombat);
            self.combat = Combat::default();
        }
        if self.step == Step::Cleanup && self.stack.is_empty() && self.passes > 0 {
            // A second cleanup step after priority in cleanup (514.3a).
            self.stage = Stage::StepStart;
            return;
        }
        match self.step.next() {
            Some(s) => {
                self.step = s;
                self.stage = Stage::StepStart;
            }
            None => {
                // 800.4k: players who left don't begin turns.
                let next = self.next_player_after(self.active).unwrap_or(self.active);
                self.begin_turn(next);
            }
        }
    }

    // ---------------------------------------------------------------- priority

    fn do_pass(&mut self, p: PlayerId) {
        self.mana_undo.clear();
        self.passes += 1;
        let in_game = self.players_in_game().len();
        if self.passes >= in_game {
            // 117.4: all players passed in succession.
            self.passes = 0;
            if self.stack.is_empty() {
                self.stage = Stage::StepEnd;
            } else {
                self.resolve_top();
                // 117.3b
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
        } else {
            let next = self.next_player_after(p).unwrap_or(p);
            self.stage = Stage::PriorityCheck(next);
        }
    }

    /// 117.5: perform SBAs until none apply, then put triggers on the stack; repeat.
    /// Returns false if a player must order/target triggers first.
    fn check_sba_and_triggers(&mut self) -> bool {
        loop {
            while self.perform_sbas() {}
            if let Some(id) = self.commander_questions().first().copied() {
                let o = self.object(id).unwrap();
                let (player, zone) = (o.owner, o.zone);
                let cur = self.stage.clone();
                self.wait(Decision::CommanderZone {
                    player,
                    commander: id,
                    zone,
                });
                self.resume_after_triggers = Some(cur);
                return false;
            }
            if self.pending_triggers.is_empty() {
                return true;
            }
            // APNAP order: the active player's triggers go on the stack first (lowest).
            for p in self.turn_order_from(self.active) {
                let mine: Vec<PendingTrigger> = self
                    .pending_triggers
                    .iter()
                    .filter(|t| t.controller == p)
                    .cloned()
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                let needs_choice = mine.len() > 1 || mine.iter().any(|t| !t.targets().is_empty());
                if needs_choice {
                    let cur = self.stage.clone();
                    self.wait(Decision::PutTriggersOnStack {
                        player: p,
                        count: mine.len(),
                    });
                    self.resume_after_triggers = Some(cur);
                    return false;
                }
                self.pending_triggers.retain(|t| t.controller != p);
                for t in mine {
                    self.push_trigger(t, vec![]);
                }
            }
            // 800.4d: triggers controlled by players who left are dropped.
            let gone: Vec<PlayerId> = self
                .players
                .iter()
                .filter(|p| p.has_left || p.has_lost)
                .map(|p| p.id)
                .collect();
            self.pending_triggers
                .retain(|t| !gone.contains(&t.controller));
        }
    }

    /// Commanders that just changed zones and whose owners want to be asked where they go
    /// (CR 903.9a–b).
    fn commander_questions(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(id, o)| {
                o.new_in_zone
                    && matches!(
                        o.zone,
                        Zone::Graveyard | Zone::Exile | Zone::Hand | Zone::Library
                    )
                    && self.is_commander(**id)
                    && self.in_game(o.owner)
                    && self.player(o.owner).ask_commander_zone
            })
            .map(|(id, _)| *id)
            .collect()
    }

    fn answer_commander_zone(&mut self, commander: ObjectId, to_command: bool) {
        if self.objects.contains_key(&commander) {
            if to_command {
                self.move_object(commander, Zone::Command, None);
                self.version += 1;
            } else if let Some(o) = self.object_mut(commander) {
                o.new_in_zone = false;
            }
        }
        self.stage = self
            .resume_after_triggers
            .take()
            .unwrap_or(Stage::PriorityCheck(self.priority_start_player()));
    }

    /// Pending triggers controlled by `p`, in the order a `PutTriggersOnStack` answer indexes them.
    pub fn pending_triggers_for(&self, p: PlayerId) -> Vec<&PendingTrigger> {
        self.pending_triggers
            .iter()
            .filter(|t| t.controller == p)
            .collect()
    }

    fn order_triggers(
        &mut self,
        p: PlayerId,
        order: Vec<(usize, Vec<Target>)>,
    ) -> Result<(), RulesError> {
        let mine: Vec<PendingTrigger> = self
            .pending_triggers
            .iter()
            .filter(|t| t.controller == p)
            .cloned()
            .collect();
        let mut seen: Vec<usize> = order.iter().map(|(i, _)| *i).collect();
        seen.sort();
        if seen != (0..mine.len()).collect::<Vec<_>>() {
            return illegal("order every trigger exactly once");
        }
        for (i, targets) in &order {
            let t = &mine[*i];
            // 603.3d: leaving targets out is only allowed when there's nothing legal to pick.
            if targets.is_empty()
                && t.targets()
                    .iter()
                    .any(|s| self.legal_targets(p, Some(t.source), s).is_empty())
            {
                continue;
            }
            if !targets.is_empty() || !t.targets().is_empty() {
                self.check_targets(p, Some(t.source), t.targets(), targets, false)?;
            }
        }
        self.pending_triggers.retain(|t| t.controller != p);
        for (i, targets) in order {
            let t = mine[i].clone();
            // 603.3d: a trigger with no legal targets is removed from the stack.
            if !t.targets().is_empty() && targets.is_empty() {
                continue;
            }
            self.push_trigger(t, targets);
        }
        self.stage = self
            .resume_after_triggers
            .take()
            .unwrap_or(Stage::PriorityCheck(self.priority_start_player()));
        Ok(())
    }

    fn push_trigger(&mut self, t: PendingTrigger, targets: Vec<Target>) {
        let id = ObjectId(self.next_object);
        self.next_object += 1;
        self.stack.push(StackEntry {
            id,
            controller: t.controller,
            kind: StackKind::Ability {
                source: t.source,
                source_name: t.source_name.clone(),
                ability: Box::new(t.ability),
            },
            targets,
            x: 0,
            mode: None,
        });
        self.version += 1;
    }

    // ---------------------------------------------------------------- special actions & casting

    pub(crate) fn sorcery_timing_ok(&self, p: PlayerId) -> bool {
        p == self.active && self.step.is_main() && self.stack.is_empty()
    }

    fn play_land(&mut self, p: PlayerId, card: ObjectId) -> Result<(), RulesError> {
        let Some(o) = self.object(card) else {
            return illegal("no such card");
        };
        if o.zone != Zone::Hand || o.owner != p || !o.def.has_type(CardType::Land) {
            return illegal("you can only play a land card from your hand");
        }
        if !self.sorcery_timing_ok(p) {
            return illegal(
                "lands can be played only in your main phase with an empty stack (305.1)",
            );
        }
        let pl = self.player(p);
        if pl.lands_played_this_turn >= pl.land_plays_allowed {
            return illegal("you've already played a land this turn (305.2)");
        }
        self.player_mut(p).lands_played_this_turn += 1;
        if let Some(new) = self.move_object(card, Zone::Battlefield, Some(p)) {
            self.fire(Event::Entered(new));
        }
        Ok(())
    }

    pub fn commander_tax(&self, p: PlayerId, card: ObjectId) -> u32 {
        let c = self.objects[&card].card;
        2 * self.player(p).commander_casts.get(&c).copied().unwrap_or(0)
    }

    fn cast_spell(
        &mut self,
        p: PlayerId,
        card: ObjectId,
        targets: Vec<Target>,
        x: u32,
        mode: Option<usize>,
    ) -> Result<(), RulesError> {
        let Some(o) = self.object(card).cloned() else {
            return illegal("no such card");
        };
        let from_command = o.zone == Zone::Command;
        match o.zone {
            Zone::Hand if o.owner == p => {}
            // 903.8: cast your commander from the command zone.
            Zone::Command if o.owner == p && self.is_commander(card) => {}
            _ => return illegal("you can't cast that card from where it is"),
        }
        let def = o.def.clone();
        if def.has_type(CardType::Land) {
            return illegal("lands are played, not cast");
        }
        let view = self.view();
        let chars = view.get(card).cloned().expect("object has characteristics");
        let instant_speed = chars.has_type(CardType::Instant) || chars.has_keyword(Keyword::Flash);
        if !instant_speed && !self.sorcery_timing_ok(p) {
            return illegal("that can only be cast at sorcery speed (307.1)");
        }
        // 601.2c targets.
        let Some((specs, _)) = def.spell_parts(mode) else {
            return illegal("choose one of the spell's modes");
        };
        self.check_targets(p, Some(card), specs, &targets, true)?;
        // 601.2f total cost, including commander tax.
        let tax = if from_command {
            self.commander_tax(p, card)
        } else {
            0
        };
        let x = if def.mana_cost.has_x() { x } else { 0 };
        let cost = def.mana_cost.with_x_and_extra(x, tax);
        // 601.2g–h: mana abilities, then pay.
        self.pay_mana_cost(p, &cost)?;
        if from_command {
            *self
                .player_mut(p)
                .commander_casts
                .entry(o.card)
                .or_insert(0) += 1;
        }
        let new = self
            .move_object(card, Zone::Stack, Some(p))
            .expect("card moves to the stack");
        self.stack.push(StackEntry {
            id: new,
            controller: p,
            kind: StackKind::Spell,
            targets,
            x,
            mode: if def.spell_modes.is_empty() {
                None
            } else {
                mode
            },
        });
        self.log(format!("{} casts {}.", self.player(p).name, def.name));
        self.fire(Event::Cast {
            player: p,
            spell: new,
        });
        Ok(())
    }

    fn activate(
        &mut self,
        p: PlayerId,
        object: ObjectId,
        idx: usize,
        targets: Vec<Target>,
        x: u32,
    ) -> Result<(), RulesError> {
        let view = self.view();
        let Some(ch) = view.get(object).cloned() else {
            return illegal("no such object");
        };
        let Some(inst) = ch.abilities.get(idx).cloned() else {
            return illegal("no such ability");
        };
        let ability = inst.ability.clone();
        let Ability::Activated {
            cost,
            targets: specs,
            sorcery_speed,
            ..
        } = &ability
        else {
            return illegal("that isn't an activated ability (mana abilities use ActivateMana)");
        };
        if cost.discard_this {
            if !self.player(p).hand.contains(&object) {
                return illegal("that ability works only from your hand");
            }
        } else if !self.is_on_battlefield(object) || ch.controller != p {
            return illegal("you don't control that permanent");
        }
        if (*sorcery_speed || cost.loyalty.is_some()) && !self.sorcery_timing_ok(p) {
            return illegal("activate only as a sorcery");
        }
        self.check_targets(p, Some(object), specs, &targets, false)?;
        if cost.once_per_turn && self.objects[&object].activated_this_turn.contains(&idx) {
            return illegal("that ability can be activated only once each turn");
        }
        self.pay_cost(p, object, &ch, cost, x)?;
        if cost.once_per_turn {
            if let Some(o) = self.object_mut(object) {
                o.activated_this_turn.push(idx);
            }
        }
        let id = ObjectId(self.next_object);
        self.next_object += 1;
        self.stack.push(StackEntry {
            id,
            controller: p,
            kind: StackKind::Ability {
                source: object,
                source_name: ch.name.clone(),
                ability: Box::new(ability.clone()),
            },
            targets,
            x,
            mode: None,
        });
        self.version += 1;
        Ok(())
    }

    fn pay_cost(
        &mut self,
        p: PlayerId,
        object: ObjectId,
        ch: &Characteristics,
        cost: &Cost,
        x: u32,
    ) -> Result<(), RulesError> {
        let o = self.objects[&object].clone();
        if cost.tap {
            if o.tapped {
                return illegal("it's already tapped");
            }
            // 302.6: {T} abilities of creatures need haste or continuous control since turn start.
            if ch.is_creature() && o.summoning_sick && !ch.has_keyword(Keyword::Haste) {
                return illegal("that creature has summoning sickness");
            }
        }
        if cost.pay_life as i32 > self.player(p).life {
            return illegal("not enough life");
        }
        if self.player(p).hand.len() < cost.discard as usize + cost.discard_this as usize {
            return illegal("not enough cards in hand to discard");
        }
        let sac_choice = match &cost.sacrifice {
            Some((f, n)) => {
                let c = self.cost_sacrifice_choice(p, object, f, *n);
                if c.len() < *n as usize {
                    return illegal("nothing to sacrifice");
                }
                c
            }
            None => vec![],
        };
        if let Some((k, n)) = &cost.remove_counters {
            if o.counters.get(k).copied().unwrap_or(0) < *n {
                return illegal("not enough counters");
            }
        }
        if let Some(l) = cost.loyalty {
            if o.loyalty_used {
                return illegal("only one loyalty ability per planeswalker each turn (606.3)");
            }
            let have = o.counters.get(&CounterKind::Loyalty).copied().unwrap_or(0) as i32;
            if have + l < 0 {
                return illegal("not enough loyalty");
            }
        }
        let mana = cost.mana.with_x_and_extra(x, 0);
        if !mana.0.is_empty() {
            self.pay_mana_cost(p, &mana)?;
        }
        if cost.tap {
            self.object_mut(object).unwrap().tapped = true;
        }
        if cost.pay_life > 0 {
            self.player_mut(p).life -= cost.pay_life as i32;
        }
        if let Some(l) = cost.loyalty {
            let ts = self.new_timestamp();
            let o = self.object_mut(object).unwrap();
            o.loyalty_used = true;
            let c = o.counters.entry(CounterKind::Loyalty).or_insert(0);
            *c = (*c as i32 + l).max(0) as u32;
            o.counter_timestamp = ts;
            self.version += 1;
        }
        if let Some((k, n)) = &cost.remove_counters {
            let ts = self.new_timestamp();
            if let Some(o) = self.object_mut(object) {
                if let Some(c) = o.counters.get_mut(k) {
                    *c -= n;
                }
                o.counters.retain(|_, v| *v > 0);
                o.counter_timestamp = ts;
            }
            self.version += 1;
        }
        if cost.discard_this {
            self.move_object(object, Zone::Graveyard, None);
        }
        for _ in 0..cost.discard {
            if let Some(c) = self.auto_discard_choice(p) {
                self.move_object(c, Zone::Graveyard, None);
            }
        }
        for id in sac_choice {
            self.sacrifice(id);
        }
        if cost.sacrifice_this {
            self.sacrifice(object);
        }
        Ok(())
    }

    fn activate_mana(
        &mut self,
        p: PlayerId,
        object: ObjectId,
        idx: usize,
        color: Option<Color>,
    ) -> Result<(), RulesError> {
        let view = self.view();
        let Some(ch) = view.get(object).cloned() else {
            return illegal("no such object");
        };
        if !self.is_on_battlefield(object) || ch.controller != p {
            return illegal("you don't control that permanent");
        }
        let Some(Ability::Mana { cost, produces }) =
            ch.abilities.get(idx).map(|a| a.ability.clone())
        else {
            return illegal("that isn't a mana ability");
        };
        let mana: Vec<ManaType> = match &produces {
            ManaProduction::Fixed(v) => v.clone(),
            ManaProduction::OneOf(opts) => {
                let want = color.map(ManaType::Color).unwrap_or(opts[0]);
                if !opts.contains(&want) {
                    return illegal("that ability can't make that color");
                }
                vec![want]
            }
            ManaProduction::AnyColor => vec![ManaType::Color(color.unwrap_or(Color::White))],
            ManaProduction::CommanderIdentity => {
                let ids = self.commander_identity(p);
                // 903.4f: with no colors in the identity, the ability makes no mana.
                let Some(first) = ids.iter().next() else {
                    return illegal("your commander's color identity has no colors");
                };
                let want = color.unwrap_or(first);
                if !ids.contains(want) {
                    return illegal("that color isn't in your commander's color identity");
                }
                vec![ManaType::Color(want)]
            }
        };
        self.pay_cost(p, object, &ch, &cost, 0)?;
        for m in &mana {
            self.player_mut(p).mana_pool.add(*m, 1);
        }
        if cost.deals_damage_to_you > 0 {
            self.deal_damage(
                Some(object),
                Target::Player(p),
                cost.deals_damage_to_you as i32,
                false,
            );
        }
        if cost == Cost::tap() {
            self.mana_undo.push((p, object, mana));
        }
        Ok(())
    }

    /// Permanents whose mana `p` can still take back (see `Action::UndoMana`).
    pub fn undoable_mana(&self, p: PlayerId) -> Vec<ObjectId> {
        self.mana_undo
            .iter()
            .filter(|(who, obj, mana)| {
                *who == p
                    && self.is_on_battlefield(*obj)
                    && self.objects[obj].tapped
                    && self.pool_has(p, mana)
            })
            .map(|(_, obj, _)| *obj)
            .collect()
    }

    fn pool_has(&self, p: PlayerId, mana: &[ManaType]) -> bool {
        let pool = &self.player(p).mana_pool;
        mana.iter()
            .all(|m| pool.get(*m) as usize >= mana.iter().filter(|x| *x == m).count())
    }

    fn undo_mana(&mut self, p: PlayerId, object: ObjectId) -> Result<(), RulesError> {
        if !self.undoable_mana(p).contains(&object) {
            return illegal("that mana has already been used");
        }
        let i = self
            .mana_undo
            .iter()
            .rposition(|(who, obj, _)| *who == p && *obj == object)
            .unwrap();
        let (_, _, mana) = self.mana_undo.remove(i);
        for m in mana {
            self.player_mut(p).mana_pool.remove(m, 1);
        }
        if let Some(o) = self.object_mut(object) {
            o.tapped = false;
        }
        self.version += 1;
        Ok(())
    }

    /// Pay a mana cost: from the pool if possible, otherwise by automatically activating
    /// the player's simple "{T}: Add ..." mana abilities (a convenience the UI can turn off by
    /// activating mana abilities manually first).
    pub(crate) fn pay_mana_cost(&mut self, p: PlayerId, cost: &ManaCost) -> Result<(), RulesError> {
        let life = self.player(p).life;
        let mut pool = self.player(p).mana_pool.clone();
        if let Some(res) = pool.try_pay(cost, life) {
            let pl = self.player_mut(p);
            pl.mana_pool = pool;
            pl.life -= res.life_paid as i32;
            return Ok(());
        }
        let sources = self.auto_mana_sources(p);
        let Some(plan) = plan_auto_tap(&self.player(p).mana_pool, &sources, cost, life) else {
            return illegal(format!("can't pay {cost}"));
        };
        for (obj, m) in plan {
            self.object_mut(obj).unwrap().tapped = true;
            self.player_mut(p).mana_pool.add(m, 1);
        }
        let mut pool = self.player(p).mana_pool.clone();
        let res = pool.try_pay(cost, life).expect("plan pays the cost");
        let pl = self.player_mut(p);
        pl.mana_pool = pool;
        pl.life -= res.life_paid as i32;
        Ok(())
    }

    /// Untapped permanents with a "{T}: Add one mana" ability, and what each could make.
    pub(crate) fn auto_mana_sources(&self, p: PlayerId) -> Vec<(ObjectId, Vec<ManaType>)> {
        let view = self.view();
        let mut out = Vec::new();
        for id in &self.battlefield {
            let o = &self.objects[id];
            let Some(ch) = view.get(*id) else { continue };
            if ch.controller != p || o.tapped {
                continue;
            }
            if ch.is_creature() && o.summoning_sick && !ch.has_keyword(Keyword::Haste) {
                continue;
            }
            for a in &ch.abilities {
                if let Ability::Mana { cost, produces } = &a.ability {
                    if !cost.tap
                        || !cost.mana.0.is_empty()
                        || cost.sacrifice_this
                        || cost.pay_life > 0
                        || cost.deals_damage_to_you > 0
                    {
                        continue;
                    }
                    let opts = match produces {
                        ManaProduction::Fixed(v) if v.len() == 1 => v.clone(),
                        ManaProduction::OneOf(v) => v.clone(),
                        ManaProduction::AnyColor => {
                            Color::ALL.iter().map(|c| ManaType::Color(*c)).collect()
                        }
                        ManaProduction::CommanderIdentity => {
                            let v: Vec<ManaType> = self
                                .commander_identity(p)
                                .iter()
                                .map(ManaType::Color)
                                .collect();
                            if v.is_empty() {
                                continue;
                            }
                            v
                        }
                        _ => continue,
                    };
                    out.push((*id, opts));
                    break;
                }
            }
        }
        // Prefer sources with fewer options so flexible lands are saved for later.
        out.sort_by_key(|(_, o)| o.len());
        out
    }

    fn sacrifice(&mut self, id: ObjectId) {
        let view = self.view();
        let lki = view.get(id).cloned();
        if let Some(new) = self.move_object(id, Zone::Graveyard, None) {
            if let Some(l) = &lki {
                self.fire_leaves(l, new);
            }
            if let Some(l) = lki {
                if l.is_creature() {
                    self.fire(Event::Died {
                        lki: Box::new(l),
                        source: new,
                    });
                }
            }
        }
    }

    // ---------------------------------------------------------------- targets

    fn target_legal(
        &self,
        view: &View,
        controller: PlayerId,
        source: Option<ObjectId>,
        spec: &TargetSpec,
        t: &Target,
    ) -> bool {
        let src_colors = source
            .and_then(|s| view.get(s))
            .map(|c| c.colors)
            .unwrap_or_default();
        let hexproof_ok = |id: ObjectId| {
            let c = view.get(id);
            if c.is_some_and(|c| {
                src_colors
                    .iter()
                    .any(|col| c.protected_from().contains(col))
            }) {
                return false; // 702.16b
            }
            !(c.is_some_and(|c| {
                (c.has_keyword(Keyword::Hexproof) && c.controller != controller)
                    || c.has_keyword(Keyword::Shroud)
            }))
        };
        match (spec, t) {
            (TargetSpec::Any, Target::Player(p)) | (TargetSpec::Player, Target::Player(p)) => {
                self.in_game(*p)
            }
            (TargetSpec::Opponent, Target::Player(p)) => self.in_game(*p) && *p != controller,
            (TargetSpec::Any, Target::Object(id)) => {
                self.is_on_battlefield(*id)
                    && view.get(*id).is_some_and(|c| {
                        c.is_creature()
                            || c.has_type(CardType::Planeswalker)
                            || c.has_type(CardType::Battle)
                    })
                    && hexproof_ok(*id)
            }
            (TargetSpec::Permanent(f), Target::Object(id)) => {
                self.is_on_battlefield(*id)
                    && matches_filter(self, f, *id, &view.chars, source, controller)
                    && hexproof_ok(*id)
            }
            (TargetSpec::Spell(f), Target::Object(id)) => {
                self.stack
                    .iter()
                    .any(|e| e.id == *id && matches!(e.kind, StackKind::Spell))
                    && Some(*id) != source
                    && matches_filter_any_zone(self, f, *id, view, source, controller)
            }
            (TargetSpec::StackObject, Target::Object(id)) => {
                self.stack.iter().any(|e| e.id == *id) && Some(*id) != source
            }
            (TargetSpec::CardInGraveyard(f), Target::Object(id)) => {
                self.objects
                    .get(id)
                    .is_some_and(|o| o.zone == Zone::Graveyard)
                    && matches_filter_any_zone(self, f, *id, view, source, controller)
            }
            _ => false,
        }
    }

    fn check_targets(
        &self,
        controller: PlayerId,
        source: Option<ObjectId>,
        specs: &[TargetSpec],
        targets: &[Target],
        _is_spell: bool,
    ) -> Result<(), RulesError> {
        if specs.len() != targets.len() {
            return illegal(format!("choose {} target(s)", specs.len()));
        }
        let view = self.view();
        for (s, t) in specs.iter().zip(targets) {
            if !self.target_legal(&view, controller, source, s, t) {
                return illegal(format!("{t:?} isn't a legal target"));
            }
        }
        Ok(())
    }

    /// Every legal choice for a target spec, for the UI's targeting mode.
    pub fn legal_targets(
        &self,
        controller: PlayerId,
        source: Option<ObjectId>,
        spec: &TargetSpec,
    ) -> Vec<Target> {
        let view = self.view();
        let mut all: Vec<Target> = self.players.iter().map(|p| Target::Player(p.id)).collect();
        all.extend(self.objects.keys().map(|id| Target::Object(*id)));
        all.extend(self.stack.iter().map(|e| Target::Object(e.id)));
        all.dedup();
        all.into_iter()
            .filter(|t| self.target_legal(&view, controller, source, spec, t))
            .collect()
    }

    // ---------------------------------------------------------------- resolution

    fn resolve_top(&mut self) {
        let Some(entry) = self.stack.last().cloned() else {
            return;
        };
        let view = self.view();
        let (specs, effects, source): (Vec<TargetSpec>, Vec<Effect>, Option<ObjectId>) =
            match &entry.kind {
                StackKind::Spell => {
                    let o = &self.objects[&entry.id];
                    let (t, e) = o.def.spell_parts(entry.mode).unwrap_or_default();
                    (t.to_vec(), e.to_vec(), Some(entry.id))
                }
                StackKind::Ability {
                    source, ability, ..
                } => match ability.as_ref() {
                    Ability::Activated {
                        targets, effects, ..
                    }
                    | Ability::Triggered {
                        targets, effects, ..
                    } => (targets.clone(), effects.clone(), Some(*source)),
                    _ => (vec![], vec![], Some(*source)),
                },
            };
        // 608.2b: recheck targets.
        let legal: Vec<bool> = specs
            .iter()
            .zip(&entry.targets)
            .map(|(s, t)| self.target_legal(&view, entry.controller, source, s, t))
            .collect();
        let all_illegal = !specs.is_empty() && !legal.iter().any(|l| *l);
        let lki: Vec<Option<(PlayerId, i32)>> = entry
            .targets
            .iter()
            .map(|t| match t {
                Target::Object(id) => view.get(*id).map(|c| (c.controller, c.power.unwrap_or(0))),
                _ => None,
            })
            .collect();
        match &entry.kind {
            StackKind::Spell => {
                let name = self.objects[&entry.id].def.name.clone();
                if all_illegal {
                    self.log(format!(
                        "{name} doesn't resolve: all its targets are illegal."
                    ));
                    self.move_object(entry.id, Zone::Graveyard, None);
                    return;
                }
                let is_permanent = view
                    .get(entry.id)
                    .is_some_and(|c| c.types.iter().any(|t| t.is_permanent_type()));
                if is_permanent {
                    // 608.3: permanent spell enters under its controller's control.
                    let aura_target = match (entry.targets.first(), legal.first()) {
                        (Some(Target::Object(t)), Some(true))
                            if self.objects[&entry.id]
                                .def
                                .subtypes
                                .iter()
                                .any(|s| s == "Aura") =>
                        {
                            Some(*t)
                        }
                        _ => None,
                    };
                    if let Some(new) =
                        self.move_object(entry.id, Zone::Battlefield, Some(entry.controller))
                    {
                        // 303.4f: an Aura spell enters attached to what it targeted.
                        if let Some(t) = aura_target {
                            self.object_mut(new).unwrap().attached_to = Some(t);
                        }
                        let def = self.objects[&new].def.clone();
                        for a in &def.abilities {
                            if let Ability::EntersWithCounters {
                                kind,
                                amount: Amount::X,
                            } = a
                            {
                                if entry.x > 0 {
                                    self.add_counters(new, kind.clone(), entry.x);
                                }
                            }
                        }
                        self.log(format!("{name} resolves."));
                        self.fire(Event::Entered(new));
                    }
                } else {
                    let res = Resolution {
                        controller: entry.controller,
                        source,
                        targets: entry.targets.clone(),
                        legal,
                        x: entry.x,
                        lki: lki.clone(),
                    };
                    self.log(format!("{name} resolves."));
                    for e in &effects {
                        self.execute(&res, e);
                    }
                    // 608.2n: instant/sorcery goes to its owner's graveyard.
                    if self
                        .objects
                        .get(&entry.id)
                        .is_some_and(|o| o.zone == Zone::Stack)
                    {
                        self.move_object(entry.id, Zone::Graveyard, None);
                    }
                }
            }
            StackKind::Ability { source_name, .. } => {
                self.stack.pop();
                self.version += 1;
                if all_illegal {
                    self.log(format!(
                        "{source_name}'s ability doesn't resolve: all its targets are illegal."
                    ));
                    return;
                }
                let res = Resolution {
                    controller: entry.controller,
                    source,
                    targets: entry.targets.clone(),
                    legal,
                    x: entry.x,
                    lki: lki.clone(),
                };
                for e in &effects {
                    self.execute(&res, e);
                }
            }
        }
    }

    fn target_obj(&self, r: &Resolution, i: usize) -> Option<ObjectId> {
        match (r.legal.get(i), r.targets.get(i)) {
            (Some(true), Some(Target::Object(id))) => Some(*id),
            _ => None,
        }
    }

    fn players_for(&self, r: &Resolution, who: PlayerRef) -> Vec<PlayerId> {
        match who {
            PlayerRef::You => vec![r.controller],
            PlayerRef::Opponents => self
                .players_in_game()
                .into_iter()
                .filter(|p| *p != r.controller)
                .collect(),
            PlayerRef::Each => self.turn_order_from(self.active),
            PlayerRef::Target(i) => match (r.legal.get(i), r.targets.get(i)) {
                (Some(true), Some(Target::Player(p))) => vec![*p],
                _ => vec![],
            },
            // Falls back on last known information once the target has left (CR 608.2h).
            PlayerRef::ControllerOfTarget(i) => self
                .target_obj(r, i)
                .and_then(|id| self.view().controller(id))
                .or_else(|| {
                    (r.legal.get(i) == Some(&true))
                        .then(|| r.lki.get(i).cloned().flatten().map(|(p, _)| p))
                        .flatten()
                })
                .into_iter()
                .collect(),
        }
    }

    fn amount(&self, r: &Resolution, a: &Amount) -> i32 {
        if *a == Amount::X {
            return r.x as i32;
        }
        let view = self.view();
        let this = r.source.unwrap_or(ObjectId(0));
        eval_amount(self, a, r.source, r.controller, this, &view.chars)
    }

    fn execute(&mut self, r: &Resolution, e: &Effect) {
        match e {
            Effect::DrawCards { who, amount } => {
                let n = self.amount(r, amount).max(0) as usize;
                for p in self.players_for(r, *who) {
                    self.draw_cards(p, n);
                }
            }
            Effect::GainLife { who, amount } => {
                let n = self.amount(r, amount);
                for p in self.players_for(r, *who) {
                    self.gain_life(p, n);
                }
            }
            Effect::LoseLife { who, amount } => {
                let n = self.amount(r, amount);
                for p in self.players_for(r, *who) {
                    self.player_mut(p).life -= n;
                }
            }
            Effect::DealDamage { target, amount } => {
                let n = self.amount(r, amount);
                if let (Some(true), Some(t)) =
                    (r.legal.get(*target), r.targets.get(*target).copied())
                {
                    self.deal_damage(r.source, t, n, false);
                }
            }
            Effect::DealDamageEach {
                filter,
                players,
                amount,
            } => {
                let n = self.amount(r, amount);
                let view = self.view();
                let objs: Vec<ObjectId> = self
                    .battlefield
                    .iter()
                    .copied()
                    .filter(|id| {
                        matches_filter(self, filter, *id, &view.chars, r.source, r.controller)
                    })
                    .collect();
                for o in objs {
                    self.deal_damage(r.source, Target::Object(o), n, false);
                }
                if let Some(pr) = players {
                    for p in self.players_for(r, *pr) {
                        self.deal_damage(r.source, Target::Player(p), n, false);
                    }
                }
            }
            Effect::Destroy { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.destroy(id);
                }
            }
            Effect::DestroyAll { filter } => {
                let view = self.view();
                let objs: Vec<ObjectId> = self
                    .battlefield
                    .iter()
                    .copied()
                    .filter(|id| {
                        matches_filter(self, filter, *id, &view.chars, r.source, r.controller)
                    })
                    .collect();
                // 701.8: destroyed simultaneously.
                let objs: Vec<ObjectId> = objs
                    .into_iter()
                    .filter(|id| {
                        !view
                            .get(*id)
                            .is_some_and(|c| c.has_keyword(Keyword::Indestructible))
                    })
                    .collect();
                let objs: Vec<ObjectId> = objs
                    .into_iter()
                    .filter(|id| !self.regenerate(*id))
                    .collect();
                self.leave_battlefield_many(&objs, Zone::Graveyard);
            }
            Effect::Exile { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    // A card in a graveyard is simply moved; a permanent leaves the battlefield.
                    if self.object(id).is_some_and(|o| o.zone == Zone::Graveyard) {
                        self.move_object(id, Zone::Exile, None);
                    } else {
                        self.leave_battlefield_to(id, Zone::Exile);
                    }
                }
            }
            Effect::ReturnToHand { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.leave_battlefield_to(id, Zone::Hand);
                }
            }
            Effect::Counter { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.counter(id);
                }
            }
            Effect::Tap { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.object_mut(id).unwrap().tapped = true;
                }
            }
            Effect::Untap { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.object_mut(id).unwrap().tapped = false;
                }
            }
            Effect::AddCounters {
                target,
                kind,
                amount,
            } => {
                let n = self.amount(r, amount).max(0) as u32;
                let id = match target {
                    Some(i) => self.target_obj(r, *i),
                    None => r.source.filter(|s| self.is_on_battlefield(*s)),
                };
                if let Some(id) = id {
                    self.add_counters(id, kind.clone(), n);
                }
            }
            Effect::AddMana { mana } => {
                for m in mana {
                    self.player_mut(r.controller).mana_pool.add(*m, 1);
                }
            }
            Effect::AddManaAnyColor { amount } => {
                // TODO: ask for the color; resolution-time choices aren't wired up yet.
                self.player_mut(r.controller)
                    .mana_pool
                    .add(ManaType::Color(Color::White), *amount);
            }
            Effect::CreateToken {
                token,
                amount,
                controller,
            } => {
                let n = self.amount(r, amount).max(0);
                for p in self.players_for(r, *controller) {
                    for _ in 0..n {
                        let id = self.create_object(token.clone(), p, Zone::Battlefield);
                        self.fire(Event::Entered(id));
                    }
                }
            }
            Effect::ApplyContinuous {
                affected,
                mods,
                duration,
            } => {
                let view = self.view();
                let objects: Vec<ObjectId> = match affected {
                    Affected::Target(i) => self.target_obj(r, *i).into_iter().collect(),
                    Affected::ThisObject => r.source.into_iter().collect(),
                    // 611.2c: affected set locked in as the effect begins.
                    Affected::Filter(f) => self
                        .battlefield
                        .iter()
                        .copied()
                        .filter(|id| {
                            matches_filter(self, f, *id, &view.chars, r.source, r.controller)
                        })
                        .collect(),
                    Affected::ControllerOfThis => vec![],
                };
                let ts = self.new_timestamp();
                let id = self.next_effect;
                self.next_effect += 1;
                self.effects.push(StoredEffect {
                    id,
                    timestamp: ts,
                    source: r.source,
                    controller: r.controller,
                    objects,
                    mods: mods.clone(),
                    duration: *duration,
                });
                self.version += 1;
            }
            Effect::Mill { who, amount } => {
                let n = self.amount(r, amount).max(0) as usize;
                for p in self.players_for(r, *who) {
                    for _ in 0..n {
                        if let Some(top) = self.player(p).library.last().copied() {
                            self.move_object(top, Zone::Graveyard, None);
                        }
                    }
                }
            }
            Effect::ReturnFromGraveyardToBattlefield { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    if let Some(new) = self.move_object(id, Zone::Battlefield, Some(r.controller)) {
                        self.fire(Event::Entered(new));
                    }
                }
            }
            Effect::ReturnFromGraveyardToHand { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.move_object(id, Zone::Hand, None);
                }
            }
            Effect::Attach { target } => {
                if let (Some(src), Some(t)) = (r.source, self.target_obj(r, *target)) {
                    if self.is_on_battlefield(src) && src != t {
                        self.object_mut(src).unwrap().attached_to = Some(t);
                        self.version += 1;
                    }
                }
            }
            Effect::Scry { amount } | Effect::Surveil { amount } => {
                let n = self.amount(r, amount).max(0) as usize;
                let surveil = matches!(e, Effect::Surveil { .. });
                self.scry(r.controller, n, surveil);
            }
            Effect::SearchLibrary {
                filter,
                count,
                dest,
            } => self.search_library(r, filter, *count, *dest),
            Effect::AddCountersEach {
                filter,
                kind,
                amount,
            } => {
                let n = self.amount(r, amount).max(0) as u32;
                let view = self.view();
                let objs: Vec<ObjectId> = self
                    .battlefield
                    .iter()
                    .copied()
                    .filter(|id| {
                        matches_filter(self, filter, *id, &view.chars, r.source, r.controller)
                    })
                    .collect();
                for o in objs {
                    self.add_counters(o, kind.clone(), n);
                }
                self.version += 1;
            }
            Effect::Discard { who, amount } => {
                let n = self.amount(r, amount).max(0) as usize;
                for p in self.players_for(r, *who) {
                    for _ in 0..n {
                        if let Some(c) = self.auto_discard_choice(p) {
                            self.move_object(c, Zone::Graveyard, None);
                        }
                    }
                }
            }
            Effect::Fight { target } => {
                let (Some(src), Some(t)) = (r.source, self.target_obj(r, *target)) else {
                    return;
                };
                if !self.is_on_battlefield(src) || !self.is_on_battlefield(t) {
                    return;
                }
                let view = self.view();
                let (Some(a), Some(b)) = (view.get(src).cloned(), view.get(t).cloned()) else {
                    return;
                };
                if !a.is_creature() || !b.is_creature() {
                    return;
                }
                // 701.14b: each deals damage equal to its power to the other.
                self.deal_damage(Some(src), Target::Object(t), a.power.unwrap_or(0), false);
                self.deal_damage(Some(t), Target::Object(src), b.power.unwrap_or(0), false);
            }
            Effect::ControllerOfTargetGainsLifeEqualToPower { target } => {
                // Uses last known information: the target has usually just left (Swords to Plowshares).
                if r.legal.get(*target) == Some(&true) {
                    if let Some((ctl, pow)) = r.lki.get(*target).cloned().flatten() {
                        self.gain_life(ctl, pow);
                    }
                }
            }
            Effect::Regenerate { target } => {
                let id = match target {
                    Some(i) => self.target_obj(r, *i),
                    None => r.source.filter(|s| self.is_on_battlefield(*s)),
                };
                if let Some(o) = id.and_then(|id| self.object_mut(id)) {
                    o.regen_shields += 1;
                }
            }
            Effect::ReturnThisToHand => {
                if let Some(s) = r.source.filter(|s| self.is_on_battlefield(*s)) {
                    self.leave_battlefield_to(s, Zone::Hand);
                }
            }
            Effect::UntapThis => {
                if let Some(o) = r
                    .source
                    .filter(|s| self.is_on_battlefield(*s))
                    .and_then(|s| self.object_mut(s))
                {
                    o.tapped = false;
                }
            }
            Effect::Sacrifice { who, filter } => {
                for p in self.players_for(r, *who) {
                    let view = self.view();
                    // TODO: the sacrificing player should choose; for now the lowest mana value is chosen.
                    let choice = self
                        .battlefield
                        .iter()
                        .copied()
                        .filter(|id| view.controller(*id) == Some(p))
                        .filter(|id| matches_filter(self, filter, *id, &view.chars, None, p))
                        .min_by_key(|id| view.get(*id).map(|c| c.mana_value()).unwrap_or(0));
                    if let Some(c) = choice {
                        self.sacrifice(c);
                    }
                }
            }
        }
    }

    /// Scry/surveil N, chosen automatically: keep what the player needs (lands while they
    /// have few, spells they could soon cast), send the rest to the bottom or graveyard.
    fn scry(&mut self, p: PlayerId, n: usize, surveil: bool) {
        let view = self.view();
        let lands = self
            .battlefield
            .iter()
            .filter(|id| {
                view.get(**id)
                    .is_some_and(|c| c.controller == p && c.has_type(CardType::Land))
            })
            .count() as u32;
        let hand_lands = self
            .player(p)
            .hand
            .iter()
            .filter(|id| self.objects[id].def.has_type(CardType::Land))
            .count() as u32;
        let lib = &self.player(p).library;
        let top: Vec<ObjectId> = lib.iter().rev().take(n).copied().collect();
        let mut away = Vec::new();
        for id in &top {
            let d = &self.objects[id].def;
            let keep = if d.has_type(CardType::Land) {
                lands + hand_lands < 6
            } else {
                d.mana_cost.mana_value() <= lands + hand_lands + 2
            };
            if !keep {
                away.push(*id);
            }
        }
        for id in away {
            if surveil {
                self.move_object(id, Zone::Graveyard, None);
            } else {
                self.move_to_library(id, Some(usize::MAX));
            }
        }
        self.log(format!(
            "{} {} {n}.",
            self.player(p).name,
            if surveil { "surveils" } else { "scries" }
        ));
    }

    /// Search the controller's library (chosen automatically: lands that add colors the player
    /// lacks first, otherwise the costliest matching cards).
    fn search_library(&mut self, r: &Resolution, filter: &Filter, count: u32, dest: SearchDest) {
        let p = r.controller;
        let view = self.view();
        let mut found: Vec<ObjectId> = self
            .player(p)
            .library
            .iter()
            .copied()
            .filter(|id| matches_filter_any_zone(self, filter, *id, &view, r.source, p))
            .collect();
        let identity = self.commander_identity(p);
        let have: Vec<String> = self
            .battlefield
            .iter()
            .filter_map(|id| view.get(*id))
            .filter(|c| c.controller == p)
            .flat_map(|c| c.subtypes.clone())
            .collect();
        let score = |g: &Game, id: &ObjectId| -> i64 {
            let d = &g.objects[id].def;
            if d.has_type(CardType::Land) {
                let basic_color = ["Plains", "Island", "Swamp", "Mountain", "Forest"]
                    .iter()
                    .zip(Color::ALL)
                    .find(|(t, _)| d.subtypes.iter().any(|s| s == *t));
                let fits =
                    basic_color.is_none_or(|(_, c)| identity.is_empty() || identity.contains(c));
                let owned = d.subtypes.iter().filter(|s| have.contains(s)).count() as i64;
                (fits as i64) * 100 - owned * 3
            } else {
                d.mana_cost.mana_value() as i64
            }
        };
        found.sort_by_key(|id| -score(self, id));
        let mut picked: Vec<ObjectId> = Vec::new();
        for id in found {
            if picked.len() as u32 >= count {
                break;
            }
            // Spread land searches across types ("two basic lands": a Forest and an Island).
            let d = &self.objects[&id].def;
            if d.has_type(CardType::Land)
                && picked.iter().any(|x| self.objects[x].def.name == d.name)
                && picked.len() < count as usize
            {
                continue;
            }
            picked.push(id);
        }
        if (picked.len() as u32) < count {
            // Fill up with duplicates if that's all there is.
            let more: Vec<ObjectId> = self
                .player(p)
                .library
                .iter()
                .copied()
                .filter(|id| !picked.contains(id))
                .filter(|id| matches_filter_any_zone(self, filter, *id, &view, r.source, p))
                .take((count as usize).saturating_sub(picked.len()))
                .collect();
            picked.extend(more);
        }
        let mut on_top = Vec::new();
        for (i, id) in picked.iter().enumerate() {
            let name = self.objects[id].def.name.clone();
            match dest {
                SearchDest::Hand => {
                    self.move_object(*id, Zone::Hand, None);
                }
                SearchDest::Battlefield { tapped } => {
                    self.put_onto_battlefield(*id, p, tapped);
                }
                SearchDest::BattlefieldTappedThenHand => {
                    if i == 0 {
                        self.put_onto_battlefield(*id, p, true);
                    } else {
                        self.move_object(*id, Zone::Hand, None);
                    }
                }
                SearchDest::TopOfLibrary => on_top.push(*id),
            }
            self.log(format!("{} finds {name}.", self.player(p).name));
        }
        self.shuffle_library(p);
        for id in on_top {
            self.move_to_library(id, Some(0));
        }
    }

    /// What a "sacrifice a creature" cost sacrifices when the game chooses: tokens first, then
    /// the cheapest.
    pub(crate) fn cost_sacrifice_choice(
        &self,
        p: PlayerId,
        source: ObjectId,
        f: &Filter,
        n: u32,
    ) -> Vec<ObjectId> {
        let view = self.view();
        let mut c: Vec<ObjectId> = self
            .battlefield
            .iter()
            .copied()
            .filter(|id| view.controller(*id) == Some(p))
            .filter(|id| matches_filter(self, f, *id, &view.chars, Some(source), p))
            .collect();
        c.sort_by_key(|id| {
            let o = &self.objects[id];
            (!o.is_token, o.def.mana_cost.mana_value(), *id == source)
        });
        c.truncate(n as usize);
        c
    }

    fn put_onto_battlefield(&mut self, id: ObjectId, controller: PlayerId, tapped: bool) {
        if let Some(new) = self.move_object(id, Zone::Battlefield, Some(controller)) {
            if tapped {
                self.object_mut(new).unwrap().tapped = true;
            }
            self.fire(Event::Entered(new));
        }
    }

    /// The card a player discards when the game chooses for them: an extra land if they hold
    /// plenty, otherwise their costliest card.
    fn auto_discard_choice(&self, p: PlayerId) -> Option<ObjectId> {
        let hand = &self.player(p).hand;
        let lands: Vec<ObjectId> = hand
            .iter()
            .copied()
            .filter(|id| self.objects[id].def.has_type(CardType::Land))
            .collect();
        if lands.len() >= 3 {
            return lands.last().copied();
        }
        hand.iter().copied().max_by_key(|id| {
            let d = &self.objects[id].def;
            (!d.has_type(CardType::Land), d.mana_cost.mana_value())
        })
    }

    fn counter(&mut self, id: ObjectId) {
        let Some(entry) = self.stack.iter().find(|e| e.id == id).cloned() else {
            return;
        };
        match entry.kind {
            StackKind::Spell => {
                if self
                    .view()
                    .get(id)
                    .is_some_and(|c| c.has_keyword(Keyword::CantBeCountered))
                {
                    return;
                }
                self.move_object(id, Zone::Graveyard, None);
            }
            StackKind::Ability { .. } => {
                self.stack.retain(|e| e.id != id);
                self.version += 1;
            }
        }
    }

    /// "When ~ leaves the battlefield" triggers of an object that just left (CR 603.10a).
    fn fire_leaves(&mut self, lki: &Characteristics, new: ObjectId) {
        for a in &lki.abilities {
            if let Ability::Triggered {
                event: TriggerEvent::LeavesBattlefield(f),
                ..
            } = &a.ability
            {
                if f.this_object {
                    self.pending_triggers.push(PendingTrigger {
                        controller: lki.controller,
                        source: new,
                        source_name: lki.name.clone(),
                        ability: a.ability.clone(),
                        subject: Some(Target::Object(new)),
                    });
                }
            }
        }
    }

    fn gain_life(&mut self, p: PlayerId, n: i32) {
        if n <= 0 || !self.in_game(p) {
            return;
        }
        self.player_mut(p).life += n;
        self.fire(Event::GainedLife(p));
    }

    /// Use a regeneration shield instead of being destroyed (701.19b): tap it, remove its
    /// damage and take it out of combat. Returns whether it regenerated.
    fn regenerate(&mut self, id: ObjectId) -> bool {
        let Some(o) = self.object_mut(id) else {
            return false;
        };
        if o.regen_shields == 0 {
            return false;
        }
        o.regen_shields -= 1;
        o.tapped = true;
        o.damage = 0;
        o.damaged_by_deathtouch = false;
        self.combat.attackers.retain(|(a, _)| *a != id);
        self.combat.blockers.retain(|(b, _)| *b != id);
        self.version += 1;
        let name = self.objects[&id].def.name.clone();
        self.log(format!("{name} regenerates."));
        true
    }

    /// 701.8: destroy, respecting indestructible.
    fn destroy(&mut self, id: ObjectId) {
        if !self.is_on_battlefield(id) {
            return;
        }
        let view = self.view();
        if view
            .get(id)
            .is_some_and(|c| c.has_keyword(Keyword::Indestructible))
        {
            return;
        }
        if self.regenerate(id) {
            return;
        }
        self.leave_battlefield_to(id, Zone::Graveyard);
    }

    fn leave_battlefield_to(&mut self, id: ObjectId, zone: Zone) {
        self.leave_battlefield_many(&[id], zone);
    }

    /// Several permanents leaving at the same time (a wrath, simultaneous SBAs). Leaves-the-
    /// battlefield triggers look back in time (603.10a), so each departing permanent can see
    /// the others leave too.
    fn leave_battlefield_many(&mut self, ids: &[ObjectId], zone: Zone) {
        let view = self.view();
        let mut left: Vec<(ObjectId, Characteristics)> = Vec::new();
        for id in ids {
            if !self.is_on_battlefield(*id) {
                continue;
            }
            let lki = view.get(*id).cloned();
            if let (Some(new), Some(l)) = (self.move_object(*id, zone, None), lki) {
                left.push((new, l));
            }
        }
        for (new, l) in &left {
            self.fire_leaves(l, *new);
        }
        if zone != Zone::Graveyard {
            return;
        }
        self.lookback = left.clone();
        for (new, l) in left {
            if l.is_creature() {
                self.fire(Event::Died {
                    lki: Box::new(l),
                    source: new,
                });
            }
        }
        self.lookback.clear();
    }

    pub(crate) fn deal_damage(
        &mut self,
        source: Option<ObjectId>,
        to: Target,
        n: i32,
        combat: bool,
    ) {
        if n <= 0 {
            return;
        }
        let view = self.view();
        let src = source.and_then(|s| view.get(s).cloned());
        let deathtouch = src
            .as_ref()
            .is_some_and(|c| c.has_keyword(Keyword::Deathtouch));
        let lifelink = src
            .as_ref()
            .is_some_and(|c| c.has_keyword(Keyword::Lifelink));
        match to {
            Target::Player(p) => {
                if !self.in_game(p) {
                    return; // 800.4e
                }
                self.player_mut(p).life -= n;
                if combat {
                    if let Some(s) = source {
                        let card = self.objects.get(&s).map(|o| o.card);
                        // 903.10a: track combat damage dealt by each commander.
                        if let Some(card) = card.filter(|_| self.is_commander(s)) {
                            *self.player_mut(p).commander_damage.entry(card).or_insert(0) += n;
                        }
                        self.fire(Event::CombatDamageToPlayer {
                            source: s,
                            player: p,
                        });
                    }
                }
            }
            Target::Object(id) => {
                let Some(c) = view.get(id).cloned() else {
                    return;
                };
                // 702.16e: damage from a source of a protected color is prevented.
                if src
                    .as_ref()
                    .is_some_and(|s| s.colors.iter().any(|col| c.protected_from().contains(col)))
                {
                    return;
                }
                if c.has_type(CardType::Planeswalker) && !c.is_creature() {
                    let o = self.object_mut(id).unwrap();
                    let cur = o.counters.get(&CounterKind::Loyalty).copied().unwrap_or(0);
                    o.counters
                        .insert(CounterKind::Loyalty, cur.saturating_sub(n as u32));
                } else {
                    let o = self.object_mut(id).unwrap();
                    o.damage += n;
                    if deathtouch {
                        o.damaged_by_deathtouch = true;
                    }
                }
            }
        }
        if lifelink {
            if let Some(c) = src {
                self.gain_life(c.controller, n);
            }
        }
    }

    // ---------------------------------------------------------------- state-based actions

    /// Perform all applicable SBAs simultaneously (CR 704.3). Returns whether any happened.
    pub(crate) fn perform_sbas(&mut self) -> bool {
        let view = self.view();
        let mut any = false;
        let mut losers: Vec<PlayerId> = Vec::new();
        for p in self.players_in_game() {
            let pl = self.player(p);
            // 704.5a, b, c; 903.10a / 704.6c
            if pl.life <= 0
                || pl.drew_from_empty_library
                || pl.poison >= 10
                || pl.commander_damage.values().any(|d| *d >= 21)
            {
                losers.push(p);
            }
        }
        let mut to_graveyard: Vec<ObjectId> = Vec::new();
        let mut to_destroy: Vec<ObjectId> = Vec::new();
        let mut cease: Vec<ObjectId> = Vec::new();
        let mut to_command: Vec<ObjectId> = Vec::new();
        let mut counter_fix: Vec<(ObjectId, u32)> = Vec::new();
        let mut unattach: Vec<ObjectId> = Vec::new();
        for (id, o) in &self.objects {
            // 704.5d
            if o.is_token && o.zone != Zone::Battlefield {
                cease.push(*id);
                continue;
            }
            // 903.9a
            if o.new_in_zone
                && matches!(o.zone, Zone::Graveyard | Zone::Exile)
                && self.is_commander(*id)
                && !self.player(o.owner).ask_commander_zone
                && self.player(o.owner).return_commander_to_command_zone
            {
                to_command.push(*id);
            }
            if o.zone != Zone::Battlefield {
                continue;
            }
            let Some(c) = view.get(*id) else { continue };
            if c.is_creature() {
                let t = c.toughness.unwrap_or(0);
                if t <= 0 {
                    to_graveyard.push(*id); // 704.5f
                } else if (o.damage >= t || o.damaged_by_deathtouch)
                    && !c.has_keyword(Keyword::Indestructible)
                {
                    to_destroy.push(*id); // 704.5g, h
                }
            }
            if c.has_type(CardType::Planeswalker)
                && o.counters.get(&CounterKind::Loyalty).copied().unwrap_or(0) == 0
            {
                to_graveyard.push(*id); // 704.5i
            }
            if c.has_subtype("Aura") && o.attached_to.is_none_or(|a| !self.is_on_battlefield(a)) {
                to_graveyard.push(*id); // 704.5m (approximation: must be attached to a permanent)
            }
            // 704.5n: Equipment attached to something that isn't a creature on the battlefield
            // becomes unattached.
            if let Some(a) = o.attached_to {
                if c.has_subtype("Equipment")
                    && !(self.is_on_battlefield(a) && view.get(a).is_some_and(|x| x.is_creature()))
                {
                    unattach.push(*id);
                }
            }
            // 704.5q
            let plus = o
                .counters
                .get(&CounterKind::PlusOnePlusOne)
                .copied()
                .unwrap_or(0);
            let minus = o
                .counters
                .get(&CounterKind::MinusOneMinusOne)
                .copied()
                .unwrap_or(0);
            if plus > 0 && minus > 0 {
                counter_fix.push((*id, plus.min(minus)));
            }
        }
        // 704.5j legend rule. TODO: the controller should choose which to keep; we keep the newest.
        let mut legends: std::collections::HashMap<(PlayerId, String), Vec<ObjectId>> =
            Default::default();
        for id in &self.battlefield {
            if let Some(c) = view.get(*id) {
                if c.supertypes.contains(&Supertype::Legendary) {
                    legends
                        .entry((c.controller, c.name.clone()))
                        .or_default()
                        .push(*id);
                }
            }
        }
        for (_, mut ids) in legends {
            if ids.len() > 1 {
                ids.sort_by_key(|id| self.objects[id].timestamp);
                ids.pop();
                to_graveyard.extend(ids);
            }
        }

        // Clear "since last SBA check" flags.
        for p in self.players.iter_mut() {
            p.drew_from_empty_library = false;
        }
        // A commander whose owner wants to be asked keeps its flag until they answer.
        let asking = self.commander_questions();
        for (id, o) in self.objects.iter_mut() {
            if !asking.contains(id) {
                o.new_in_zone = false;
            }
        }

        for p in losers {
            any = true;
            self.log(format!("{} loses the game.", self.player(p).name));
            self.player_mut(p).has_lost = true;
            self.leave_game(p);
        }
        for id in cease {
            any = true;
            if let Some(o) = self.objects.remove(&id) {
                let owner = o.owner;
                self.zone_list_remove(o.zone, owner, id);
            }
        }
        // 704.3: all applicable SBAs happen simultaneously.
        let mut dying: Vec<ObjectId> = to_graveyard;
        let to_destroy: Vec<ObjectId> = to_destroy
            .into_iter()
            .filter(|id| !self.regenerate(*id))
            .collect();
        dying.extend(to_destroy);
        dying.sort();
        dying.dedup();
        if !dying.is_empty() {
            any = true;
            self.leave_battlefield_many(&dying, Zone::Graveyard);
        }
        for id in to_command {
            if self.objects.contains_key(&id) {
                any = true;
                self.move_object(id, Zone::Command, None);
            }
        }
        for id in unattach {
            any = true;
            if let Some(o) = self.object_mut(id) {
                o.attached_to = None;
            }
        }
        for (id, n) in counter_fix {
            any = true;
            let o = self.object_mut(id).unwrap();
            *o.counters.get_mut(&CounterKind::PlusOnePlusOne).unwrap() -= n;
            *o.counters.get_mut(&CounterKind::MinusOneMinusOne).unwrap() -= n;
            o.counters.retain(|_, v| *v > 0);
        }
        if any {
            self.version += 1;
        }
        any
    }

    fn zone_list_remove(&mut self, zone: Zone, owner: PlayerId, id: ObjectId) {
        let p = owner.0 as usize;
        match zone {
            Zone::Library => self.players[p].library.retain(|x| *x != id),
            Zone::Hand => self.players[p].hand.retain(|x| *x != id),
            Zone::Graveyard => self.players[p].graveyard.retain(|x| *x != id),
            Zone::Battlefield => self.battlefield.retain(|x| *x != id),
            Zone::Exile => self.exile.retain(|x| *x != id),
            Zone::Command => self.command.retain(|x| *x != id),
            Zone::Stack => {
                self.stack_cards.retain(|x| *x != id);
                self.stack.retain(|e| e.id != id);
            }
        }
        self.version += 1;
    }

    // ---------------------------------------------------------------- leaving the game

    /// CR 800.4a.
    fn leave_game(&mut self, p: PlayerId) {
        self.player_mut(p).has_left = true;
        // Objects owned by the player leave the game.
        let owned: Vec<(ObjectId, Zone)> = self
            .objects
            .values()
            .filter(|o| o.owner == p)
            .map(|o| (o.id, o.zone))
            .collect();
        for (id, zone) in owned {
            self.objects.remove(&id);
            self.zone_list_remove(zone, p, id);
        }
        // Effects giving them control end.
        self.effects
            .retain(|e| !(e.controller == p && e.mods.iter().any(|m| m.layer() == Layer::L2)));
        self.effects
            .retain(|e| !e.mods.contains(&Modification::GainControl(p)));
        // Abilities they control on the stack cease to exist.
        self.stack.retain(|e| e.controller != p);
        self.pending_triggers.retain(|t| t.controller != p);
        // Anything they still control is exiled.
        let view = self.view();
        let still: Vec<ObjectId> = self
            .battlefield
            .iter()
            .copied()
            .filter(|id| view.controller(*id) == Some(p))
            .collect();
        for id in still {
            self.move_object(id, Zone::Exile, None);
        }
        self.combat.attackers.retain(|(_, d)| *d != p);
        self.version += 1;
    }

    fn reroute_after_leave(&mut self, p: PlayerId) {
        match self.decision.clone() {
            Decision::Priority { player } if player == p => {
                // 800.4a: priority passes to the next player; it counts as a pass in the sequence.
                self.passes = self
                    .passes
                    .min(self.players_in_game().len().saturating_sub(1));
                let next = self.next_player_after(p).unwrap_or(p);
                self.stage = Stage::PriorityCheck(next);
            }
            Decision::DeclareAttackers { player } if player == p => self.skip_to(Step::EndCombat),
            Decision::DeclareBlockers { player } if player == p => {
                self.stage = Stage::Blockers(std::mem::take(&mut self.pending_blockers));
            }
            Decision::AssignCombatDamage { player, .. } if player == p => {
                self.stage = Stage::Damage;
            }
            Decision::Discard { player, .. } if player == p => self.cleanup_rest(),
            Decision::CommanderZone { player, .. } if player == p => {
                self.stage = self
                    .resume_after_triggers
                    .take()
                    .unwrap_or(Stage::PriorityCheck(self.priority_start_player()));
            }
            Decision::PutTriggersOnStack { .. } => {
                self.stage = self
                    .resume_after_triggers
                    .take()
                    .unwrap_or(Stage::PriorityCheck(self.priority_start_player()));
            }
            Decision::Mulligan { player, .. } if player == p => {}
            _ => {
                // Someone else's decision is still pending; keep waiting for it.
            }
        }
    }

    // ---------------------------------------------------------------- triggers

    fn fire(&mut self, ev: Event) {
        let view = self.view();
        let mut found: Vec<PendingTrigger> = Vec::new();
        let mut consider = |game: &Game, src: ObjectId, ch: &Characteristics| {
            for a in &ch.abilities {
                let Ability::Triggered { event, .. } = &a.ability else {
                    continue;
                };
                let you = ch.controller;
                let hit = |f: &Filter, obj: ObjectId, chars: &std::collections::HashMap<ObjectId, Characteristics>| {
                    matches_filter(game, f, obj, chars, Some(src), you)
                };
                let (matched, subject) = match (event, &ev) {
                    (TriggerEvent::EntersBattlefield(f), Event::Entered(o)) => {
                        (hit(f, *o, &view.chars), Some(Target::Object(*o)))
                    }
                    (TriggerEvent::YouCastSpell(f), Event::Cast { player, spell }) => (
                        *player == you
                            && matches_filter_any_zone(game, f, *spell, &view, None, you),
                        Some(Target::Object(*spell)),
                    ),
                    (TriggerEvent::BeginningOfUpkeep(w), Event::Upkeep(p))
                    | (TriggerEvent::BeginningOfEndStep(w), Event::EndStep(p)) => {
                        (player_matches(*w, you, *p), Some(Target::Player(*p)))
                    }
                    (TriggerEvent::Attacks(f), Event::Attacked(o)) => {
                        (hit(f, *o, &view.chars), Some(Target::Object(*o)))
                    }
                    (
                        TriggerEvent::DealsCombatDamageToPlayer(f),
                        Event::CombatDamageToPlayer { source, player },
                    ) => (hit(f, *source, &view.chars), Some(Target::Player(*player))),
                    (TriggerEvent::YouGainLife, Event::GainedLife(p)) => {
                        (*p == you, Some(Target::Player(*p)))
                    }
                    (TriggerEvent::BeginningOfCombat(w), Event::BeginCombat(p))
                    | (TriggerEvent::BeginningOfFirstMain(w), Event::FirstMain(p)) => {
                        (player_matches(*w, you, *p), Some(Target::Player(*p)))
                    }
                    (TriggerEvent::YouAttack, Event::AttackersDeclared(p)) => {
                        (*p == you, Some(Target::Player(*p)))
                    }
                    (TriggerEvent::PlayerCastsSpell(w, f), Event::Cast { player, spell }) => (
                        player_matches(*w, you, *player)
                            && matches_filter_any_zone(game, f, *spell, &view, None, you),
                        Some(Target::Object(*spell)),
                    ),
                    _ => (false, None),
                };
                if matched {
                    found.push(PendingTrigger {
                        controller: you,
                        source: src,
                        source_name: ch.name.clone(),
                        ability: a.ability.clone(),
                        subject,
                    });
                }
            }
            // 702.108a prowess: a triggered ability the keyword stands for.
            if let Event::Cast { player, spell } = &ev {
                if *player == ch.controller
                    && ch.has_keyword(Keyword::Prowess)
                    && view.get(*spell).is_some_and(|c| !c.is_creature())
                {
                    for _ in ch
                        .abilities
                        .iter()
                        .filter(|a| a.ability == Ability::Keyword(Keyword::Prowess))
                    {
                        found.push(PendingTrigger {
                            controller: ch.controller,
                            source: src,
                            source_name: ch.name.clone(),
                            ability: Ability::Triggered {
                                event: TriggerEvent::YouCastSpell(Filter {
                                    not_types: vec![CardType::Creature],
                                    ..Default::default()
                                }),
                                targets: vec![],
                                effects: vec![Effect::ApplyContinuous {
                                    affected: Affected::ThisObject,
                                    mods: vec![Modification::ModifyPT(1, 1)],
                                    duration: Duration::EndOfTurn,
                                }],
                                optional: false,
                            },
                            subject: Some(Target::Object(*spell)),
                        });
                    }
                }
            }
        };
        match &ev {
            Event::Died { lki, source } => {
                // 603.10a: leaves-the-battlefield abilities look back in time. Check the dying
                // object's own last-known abilities plus everything still on the battlefield.
                let mut lookback = view.chars.clone();
                lookback.insert(*source, (**lki).clone());
                for a in &lki.abilities {
                    if let Ability::Triggered {
                        event: TriggerEvent::Dies(f),
                        ..
                    } = &a.ability
                    {
                        // "Whenever a creature you control dies" on a creature sees itself die too.
                        if f.this_object
                            || matches_filter(
                                self,
                                f,
                                *source,
                                &lookback,
                                Some(*source),
                                lki.controller,
                            )
                        {
                            found.push(PendingTrigger {
                                controller: lki.controller,
                                source: *source,
                                source_name: lki.name.clone(),
                                ability: a.ability.clone(),
                                subject: Some(Target::Object(*source)),
                            });
                        }
                    }
                }
                for (id, ch) in self
                    .battlefield
                    .iter()
                    .filter_map(|id| view.get(*id).map(|c| (*id, c.clone())))
                    .chain(self.lookback.iter().filter(|(id, _)| id != source).cloned())
                    .collect::<Vec<_>>()
                {
                    for (id_lb, lki2) in &self.lookback {
                        lookback.insert(*id_lb, lki2.clone());
                    }
                    for a in &ch.abilities {
                        if let Ability::Triggered {
                            event: TriggerEvent::Dies(f),
                            ..
                        } = &a.ability
                        {
                            if !f.this_object
                                && matches_filter(
                                    self,
                                    f,
                                    *source,
                                    &lookback,
                                    Some(id),
                                    ch.controller,
                                )
                            {
                                found.push(PendingTrigger {
                                    controller: ch.controller,
                                    source: id,
                                    source_name: ch.name.clone(),
                                    ability: a.ability.clone(),
                                    subject: Some(Target::Object(*source)),
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                for id in self.battlefield.clone() {
                    if let Some(ch) = view.get(id) {
                        consider(self, id, ch);
                    }
                }
                // "When you cast ~": the spell's own ability, while it's on the stack.
                if let Event::Cast { spell, .. } = &ev {
                    if let Some(ch) = view.get(*spell) {
                        for a in &ch.abilities {
                            if let Ability::Triggered {
                                event: TriggerEvent::CastThis,
                                ..
                            } = &a.ability
                            {
                                found.push(PendingTrigger {
                                    controller: ch.controller,
                                    source: *spell,
                                    source_name: ch.name.clone(),
                                    ability: a.ability.clone(),
                                    subject: Some(Target::Object(*spell)),
                                });
                            }
                        }
                    }
                }
            }
        }
        self.pending_triggers.extend(found);
    }

    // ---------------------------------------------------------------- combat

    pub(crate) fn could_attack(&self, id: ObjectId) -> bool {
        let view = self.view();
        let Some(c) = view.get(id) else { return false };
        let o = &self.objects[&id];
        c.controller == self.active
            && c.is_creature()
            && !o.tapped
            && (!o.summoning_sick || c.has_keyword(Keyword::Haste))
            && !c.has_keyword(Keyword::Defender)
            && !c.has_keyword(Keyword::CantAttack)
    }

    fn declare_attackers(
        &mut self,
        p: PlayerId,
        attacks: Vec<(ObjectId, PlayerId)>,
    ) -> Result<(), RulesError> {
        let mut seen = std::collections::HashSet::new();
        for (a, d) in &attacks {
            if !seen.insert(*a) {
                return illegal("a creature can attack only once");
            }
            if !self.could_attack(*a) {
                return illegal("that creature can't attack");
            }
            if *d == p || !self.in_game(*d) {
                return illegal("attack an opponent who is still in the game");
            }
        }
        let view = self.view();
        self.combat.attackers = attacks.clone();
        for (a, _) in &attacks {
            if !view
                .get(*a)
                .is_some_and(|c| c.has_keyword(Keyword::Vigilance))
            {
                self.object_mut(*a).unwrap().tapped = true;
            }
        }
        if attacks.is_empty() {
            // 508.8: skip blockers and damage.
            self.skip_to(Step::EndCombat);
            return Ok(());
        }
        self.fire(Event::AttackersDeclared(p));
        for (a, _) in &attacks {
            self.fire(Event::Attacked(*a));
        }
        self.passes = 0;
        self.stage = Stage::PriorityCheck(self.priority_start_player());
        Ok(())
    }

    fn declare_blockers(
        &mut self,
        p: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    ) -> Result<(), RulesError> {
        let view = self.view();
        let mut seen = std::collections::HashSet::new();
        for (b, a) in &blocks {
            if !seen.insert(*b) {
                return illegal("each creature can block only one attacker");
            }
            let Some(bc) = view.get(*b) else {
                return illegal("no such blocker");
            };
            if bc.controller != p
                || !bc.is_creature()
                || self.objects[b].tapped
                || !self.is_on_battlefield(*b)
            {
                return illegal("blockers must be untapped creatures you control");
            }
            if !self.combat.attackers.iter().any(|(x, d)| x == a && *d == p) {
                return illegal("you can only block creatures attacking you");
            }
            let Some(ac) = view.get(*a) else {
                return illegal("no such attacker");
            };
            if let Some(why) = self.block_problem(&view, ac, bc) {
                return illegal(why);
            }
        }
        for (a, _) in self.combat.attackers.iter().filter(|(_, d)| *d == p) {
            let n = blocks.iter().filter(|(_, x)| x == a).count();
            if n == 1 && view.get(*a).is_some_and(|c| c.has_keyword(Keyword::Menace)) {
                return illegal("menace: needs two or more blockers (702.111b)");
            }
            if n > 1
                && view
                    .get(*a)
                    .is_some_and(|c| c.has_keyword(Keyword::CantBeBlockedByMoreThanOne))
            {
                return illegal("that creature can't be blocked by more than one creature");
            }
        }
        for (b, a) in &blocks {
            self.combat.blockers.push((*b, *a));
            if !self.combat.blocked.contains(a) {
                self.combat.blocked.push(*a);
            }
        }
        self.version += 1;
        self.stage = Stage::Blockers(std::mem::take(&mut self.pending_blockers));
        Ok(())
    }

    /// The attackers `blocker` could legally be declared as blocking for player `p`, taken
    /// one block at a time (menace is checked on the whole declaration).
    pub fn blockable_attackers(&self, p: PlayerId, blocker: ObjectId) -> Vec<ObjectId> {
        let view = self.view();
        let Some(bc) = view.get(blocker) else {
            return vec![];
        };
        self.combat
            .attackers
            .iter()
            .filter(|(a, d)| {
                *d == p
                    && view
                        .get(*a)
                        .is_some_and(|ac| self.block_problem(&view, ac, bc).is_none())
            })
            .map(|(a, _)| *a)
            .collect()
    }

    /// Why `blocker` can't block `attacker` (evasion, protection, restrictions), if it can't.
    fn block_problem(
        &self,
        view: &View,
        attacker: &Characteristics,
        blocker: &Characteristics,
    ) -> Option<&'static str> {
        let a = |k| attacker.has_keyword(k);
        let b = |k| blocker.has_keyword(k);
        if b(Keyword::CantBlock) {
            return Some("that creature can't block");
        }
        if a(Keyword::CantBeBlocked) {
            return Some("that creature can't be blocked");
        }
        if a(Keyword::Flying) && !b(Keyword::Flying) && !b(Keyword::Reach) {
            return Some("creatures with flying can only be blocked by flying or reach (702.9b)");
        }
        if b(Keyword::CanBlockOnlyFlyers) && !a(Keyword::Flying) {
            return Some("that creature can block only creatures with flying");
        }
        if a(Keyword::Fear)
            && !blocker.has_type(CardType::Artifact)
            && !blocker.colors.contains(Color::Black)
        {
            return Some("fear: only artifact or black creatures can block it (702.36b)");
        }
        if a(Keyword::Intimidate)
            && !blocker.has_type(CardType::Artifact)
            && !attacker.colors.iter().any(|c| blocker.colors.contains(c))
        {
            return Some("intimidate: only artifact creatures or ones sharing a color can block it (702.13b)");
        }
        if a(Keyword::Shadow) != b(Keyword::Shadow) {
            return Some(
                "shadow: creatures with shadow block and are blocked only by shadow (702.28b)",
            );
        }
        if a(Keyword::Horsemanship) && !b(Keyword::Horsemanship) {
            return Some("horsemanship: only creatures with horsemanship can block it (702.31b)");
        }
        if a(Keyword::Skulk) && blocker.power.unwrap_or(0) > attacker.power.unwrap_or(0) {
            return Some("skulk: creatures with greater power can't block it (702.118b)");
        }
        for (k, land) in [
            (Keyword::Plainswalk, "Plains"),
            (Keyword::Islandwalk, "Island"),
            (Keyword::Swampwalk, "Swamp"),
            (Keyword::Mountainwalk, "Mountain"),
            (Keyword::Forestwalk, "Forest"),
        ] {
            if a(k)
                && self.battlefield.iter().any(|id| {
                    view.get(*id).is_some_and(|c| {
                        c.controller == blocker.controller
                            && c.has_type(CardType::Land)
                            && c.has_subtype(land)
                    })
                })
            {
                return Some(
                    "landwalk: it can't be blocked while you control that land type (702.14c)",
                );
            }
        }
        if blocker
            .colors
            .iter()
            .any(|c| attacker.protected_from().contains(c))
        {
            return Some("protection: it can't be blocked by creatures of that color (702.16e)");
        }
        None
    }

    fn combat_creatures(&self) -> Vec<ObjectId> {
        let mut v: Vec<ObjectId> = self.combat.attackers.iter().map(|(a, _)| *a).collect();
        v.extend(self.combat.blockers.iter().map(|(b, _)| *b));
        v
    }

    /// CR 510. Damage assignment among multiple blockers is automatic for now: lethal damage to
    /// each blocker in declaration order, the rest to the last (or to the player with trample).
    /// Whether `id` deals combat damage in this combat damage step (CR 510.4, 702.4c).
    fn deals_combat_damage(&self, id: ObjectId, first_strike_step: bool, view: &View) -> bool {
        let Some(c) = view.get(id) else { return false };
        let fs = c.has_keyword(Keyword::FirstStrike);
        let ds = c.has_keyword(Keyword::DoubleStrike);
        if first_strike_step {
            fs || ds
        } else {
            ds || !self.combat.dealt_first_strike.contains(&id)
        }
    }

    fn blockers_of(&self, a: ObjectId) -> Vec<ObjectId> {
        self.combat
            .blockers
            .iter()
            .filter(|(b, x)| *x == a && self.is_on_battlefield(*b))
            .map(|(b, _)| *b)
            .collect()
    }

    /// 510.1: before damage is dealt, the controller of each attacker blocked by two or more
    /// creatures (or with trample) chooses how to assign its damage.
    fn begin_damage(&mut self, first_strike_step: bool) {
        let view = self.view();
        self.combat.damage_plan.clear();
        self.combat.damage_queue = self
            .combat
            .attackers
            .iter()
            .map(|(a, _)| *a)
            .filter(|a| {
                self.is_on_battlefield(*a)
                    && self.combat.blocked.contains(a)
                    && self.deals_combat_damage(*a, first_strike_step, &view)
            })
            .collect();
        self.stage = Stage::Damage;
    }

    /// The choice the attacker's controller has to make, if there is a real one.
    fn damage_decision(&self, a: ObjectId, first_strike_step: bool) -> Option<Decision> {
        let view = self.view();
        if !self.is_on_battlefield(a) || !self.deals_combat_damage(a, first_strike_step, &view) {
            return None;
        }
        let c = view.get(a)?;
        let power = c.power.unwrap_or(0);
        let player = view.controller(a)?;
        let defender = self.combat.attackers.iter().find(|(x, _)| *x == a)?.1;
        let blockers = self.blockers_of(a);
        let trample = c.has_keyword(Keyword::Trample);
        if power <= 0 || !self.in_game(player) || blockers.is_empty() {
            return None;
        }
        if blockers.len() < 2 && !trample {
            return None;
        }
        let deathtouch = c.has_keyword(Keyword::Deathtouch);
        let blockers: Vec<(ObjectId, i32)> = blockers
            .into_iter()
            .map(|b| (b, self.lethal_damage(b, deathtouch, &view)))
            .collect();
        let suggested = suggest_damage(&blockers, defender, power, trample);
        Some(Decision::AssignCombatDamage {
            player,
            attacker: a,
            defender,
            power,
            trample,
            deathtouch,
            blockers,
            suggested,
        })
    }

    /// Lethal damage for a blocker (CR 702.2c, 702.19b): its toughness minus damage already
    /// marked on it, or 1 from a source with deathtouch. A creature that already has lethal
    /// damage marked needs none.
    fn lethal_damage(&self, b: ObjectId, deathtouch: bool, view: &View) -> i32 {
        let t = view.get(b).and_then(|c| c.toughness).unwrap_or(0);
        let left = (t - self.objects.get(&b).map(|o| o.damage).unwrap_or(0)).max(0);
        if deathtouch {
            left.min(1)
        } else {
            left
        }
    }

    fn assign_damage(&mut self, d: Decision, split: Vec<(Target, i32)>) -> Result<(), RulesError> {
        let Decision::AssignCombatDamage {
            attacker,
            defender,
            power,
            trample,
            blockers,
            ..
        } = d
        else {
            return Err(RulesError::WrongAction);
        };
        let mut total = 0;
        let mut to_player = 0;
        let mut on: Vec<(ObjectId, i32)> = blockers.iter().map(|(b, _)| (*b, 0)).collect();
        for (t, n) in &split {
            if *n < 0 {
                return illegal("damage can't be negative");
            }
            total += n;
            match t {
                Target::Player(p) if *p == defender && trample => to_player += n,
                Target::Player(_) => {
                    return illegal("only a creature with trample can assign damage to the player")
                }
                Target::Object(o) => match on.iter_mut().find(|(b, _)| b == o) {
                    Some(e) => e.1 += n,
                    None => return illegal("that creature isn't blocking this attacker"),
                },
            }
        }
        if total != power {
            return illegal(format!("assign exactly {power} damage"));
        }
        if to_player > 0
            && on
                .iter()
                .zip(&blockers)
                .any(|((_, got), (_, lethal))| got < lethal)
        {
            return illegal(
                "trample: every blocker needs lethal damage before any goes to the player (702.19b)",
            );
        }
        self.combat.damage_plan.retain(|(a, _)| *a != attacker);
        self.combat.damage_plan.push((attacker, split));
        self.stage = Stage::Damage;
        Ok(())
    }

    fn combat_damage(&mut self, first_strike_step: bool) {
        let view = self.view();
        let mut assignments: Vec<(ObjectId, Target, i32)> = Vec::new();
        for (a, defender) in self.combat.attackers.clone() {
            if !self.is_on_battlefield(a) || !self.deals_combat_damage(a, first_strike_step, &view)
            {
                continue;
            }
            let c = view.get(a).unwrap();
            let power = c.power.unwrap_or(0);
            if power <= 0 {
                continue;
            }
            if !self.combat.blocked.contains(&a) {
                assignments.push((a, Target::Player(defender), power));
                continue;
            }
            let blockers = self.blockers_of(a);
            let trample = c.has_keyword(Keyword::Trample);
            if blockers.is_empty() {
                // 702.19e: a blocked trampler with no blockers left deals all its damage to
                // the player; otherwise a blocked creature with no blockers deals none (509.1h).
                if trample {
                    assignments.push((a, Target::Player(defender), power));
                }
                continue;
            }
            let chosen = self
                .combat
                .damage_plan
                .iter()
                .find(|(x, _)| *x == a)
                .map(|(_, s)| s.clone());
            let split = chosen.unwrap_or_else(|| {
                let dt = c.has_keyword(Keyword::Deathtouch);
                let lethal: Vec<(ObjectId, i32)> = blockers
                    .iter()
                    .map(|b| (*b, self.lethal_damage(*b, dt, &view)))
                    .collect();
                suggest_damage(&lethal, defender, power, trample)
            });
            for (t, n) in split {
                if n > 0 {
                    assignments.push((a, t, n));
                }
            }
        }
        for (b, a) in self.combat.blockers.clone() {
            if !self.is_on_battlefield(b)
                || !self.is_on_battlefield(a)
                || !self.deals_combat_damage(b, first_strike_step, &view)
            {
                continue;
            }
            let p = view.get(b).and_then(|c| c.power).unwrap_or(0);
            if p > 0 {
                assignments.push((b, Target::Object(a), p));
            }
        }
        // 510.2: all combat damage is dealt simultaneously.
        for (src, to, n) in assignments {
            self.deal_damage(Some(src), to, n, true);
            if first_strike_step && !self.combat.dealt_first_strike.contains(&src) {
                self.combat.dealt_first_strike.push(src);
            }
        }
    }
}

/// A sensible split of an attacker's damage: kill as many blockers as possible (cheapest
/// first), then send the rest to the player with trample, or pile it on a blocker that
/// didn't die.
pub fn suggest_damage(
    blockers: &[(ObjectId, i32)],
    defender: PlayerId,
    power: i32,
    trample: bool,
) -> Vec<(Target, i32)> {
    let mut order: Vec<usize> = (0..blockers.len()).collect();
    order.sort_by_key(|i| blockers[*i].1);
    let mut amt = vec![0; blockers.len()];
    let mut left = power;
    for i in order.iter().copied() {
        let need = blockers[i].1;
        if need <= left {
            amt[i] = need;
            left -= need;
        }
    }
    let killed_all = blockers.iter().zip(&amt).all(|((_, l), a)| a >= l);
    let mut out: Vec<(Target, i32)> = Vec::new();
    if left > 0 {
        if trample && killed_all {
            out.push((Target::Player(defender), left));
        } else {
            // Most damage on the first blocker that won't die, or the last one if all do.
            let i = order
                .iter()
                .copied()
                .find(|i| amt[*i] < blockers[*i].1)
                .unwrap_or(*order.last().unwrap());
            amt[i] += left;
        }
    }
    let mut res: Vec<(Target, i32)> = blockers
        .iter()
        .zip(amt)
        .filter(|(_, a)| *a > 0)
        .map(|((b, _), a)| (Target::Object(*b), a))
        .collect();
    res.extend(out);
    res
}

fn player_matches(w: PlayerRef, you: PlayerId, p: PlayerId) -> bool {
    match w {
        PlayerRef::You => p == you,
        PlayerRef::Opponents => p != you,
        _ => true,
    }
}

/// Filter check for objects not on the battlefield (spells, cards in graveyards).
fn matches_filter_any_zone(
    game: &Game,
    f: &Filter,
    id: ObjectId,
    view: &View,
    source: Option<ObjectId>,
    you: PlayerId,
) -> bool {
    matches_filter(game, f, id, &view.chars, source, you)
}

/// Pick mana sources to tap so that `pool` plus their mana pays `cost`. Exhaustive over
/// combinations of the smallest size that could work, with a node budget.
pub(crate) fn plan_auto_tap(
    pool: &ManaPool,
    sources: &[(ObjectId, Vec<ManaType>)],
    cost: &ManaCost,
    life: i32,
) -> Option<Vec<(ObjectId, ManaType)>> {
    let need = cost.mana_value().saturating_sub(pool.total()) as usize;
    let mut budget = 200_000usize;
    for k in need.max(1)..=sources.len() {
        let mut chosen: Vec<(ObjectId, ManaType)> = Vec::new();
        if search_tap(pool, sources, cost, life, 0, k, &mut chosen, &mut budget) {
            return Some(chosen);
        }
        if budget == 0 {
            break;
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn search_tap(
    pool: &ManaPool,
    sources: &[(ObjectId, Vec<ManaType>)],
    cost: &ManaCost,
    life: i32,
    start: usize,
    k: usize,
    chosen: &mut Vec<(ObjectId, ManaType)>,
    budget: &mut usize,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    if chosen.len() == k {
        let mut p = pool.clone();
        for (_, m) in chosen.iter() {
            p.add(*m, 1);
        }
        return p.try_pay(cost, life).is_some();
    }
    for i in start..sources.len() {
        if sources.len() - i < k - chosen.len() {
            break;
        }
        let (id, opts) = &sources[i];
        for m in opts {
            chosen.push((*id, *m));
            if search_tap(pool, sources, cost, life, i + 1, k, chosen, budget) {
                return true;
            }
            chosen.pop();
        }
    }
    false
}

/// Convenience used by tests and the client.
pub fn card(def: CardBuilder) -> Arc<CardDef> {
    def.build()
}

#[allow(dead_code)]
fn _assert_send(_: &VecDeque<PlayerId>) {}
