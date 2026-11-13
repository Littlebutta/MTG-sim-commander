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
    Discard(Vec<ObjectId>),
    /// Allowed at any time (CR 104.3a).
    Concede,
    /// Standing choice for "may put it into the command zone" (CR 903.9a/b). Allowed any time.
    SetCommanderReturn(bool),
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
}

/// Bookkeeping for one resolving spell/ability.
struct Resolution {
    controller: PlayerId,
    source: Option<ObjectId>,
    targets: Vec<Target>,
    legal: Vec<bool>,
    x: u32,
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
            (Decision::Priority { .. }, Action::Cast { card, targets, x }) => {
                self.cast_spell(player, card, targets, x)?;
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
                self.activate_mana(player, object, ability, color)?;
                self.after_action(player);
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
                        if let Some(o) = self.object_mut(id) {
                            o.tapped = false;
                            o.summoning_sick = false;
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
                    self.combat_damage(true);
                    self.stage = Stage::PriorityCheck(self.priority_start_player());
                } else {
                    self.stage = Stage::StepEnd;
                }
            }
            Step::CombatDamage => {
                self.combat_damage(false);
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
            Step::End => {
                self.fire(Event::EndStep(ap));
                self.stage = Stage::PriorityCheck(self.priority_start_player());
            }
            Step::Cleanup => {
                // 514.1: discard to maximum hand size.
                if self.in_game(ap) {
                    let max = self.player(ap).max_hand_size;
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
            }
        }
        self.effects.retain(|e| e.duration != Duration::EndOfTurn);
        self.version += 1;
        // 514.3a: if SBAs would be performed or triggers are waiting, players get priority.
        let sba = self.perform_sbas();
        if sba || !self.pending_triggers.is_empty() {
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
        });
        self.version += 1;
    }

    // ---------------------------------------------------------------- special actions & casting

    fn sorcery_timing_ok(&self, p: PlayerId) -> bool {
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
        self.check_targets(p, Some(card), &def.spell_targets, &targets, true)?;
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
        if !self.is_on_battlefield(object) || ch.controller != p {
            return illegal("you don't control that permanent");
        }
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
        if *sorcery_speed && !self.sorcery_timing_ok(p) {
            return illegal("activate only as a sorcery");
        }
        self.check_targets(p, Some(object), specs, &targets, false)?;
        self.pay_cost(p, object, &ch, cost, x)?;
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
        };
        self.pay_cost(p, object, &ch, &cost, 0)?;
        for m in mana {
            self.player_mut(p).mana_pool.add(m, 1);
        }
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
    fn auto_mana_sources(&self, p: PlayerId) -> Vec<(ObjectId, Vec<ManaType>)> {
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
                    {
                        continue;
                    }
                    let opts = match produces {
                        ManaProduction::Fixed(v) if v.len() == 1 => v.clone(),
                        ManaProduction::OneOf(v) => v.clone(),
                        ManaProduction::AnyColor => {
                            Color::ALL.iter().map(|c| ManaType::Color(*c)).collect()
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
        let hexproof_ok = |id: ObjectId| {
            let c = view.get(id);
            !(c.is_some_and(|c| c.has_keyword(Keyword::Hexproof) && c.controller != controller))
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
                    (
                        o.def.spell_targets.clone(),
                        o.def.spell_effects.clone(),
                        Some(entry.id),
                    )
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
                    if let Some(new) =
                        self.move_object(entry.id, Zone::Battlefield, Some(entry.controller))
                    {
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
            PlayerRef::ControllerOfTarget(i) => self
                .target_obj(r, i)
                .and_then(|id| self.view().controller(id))
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
                    self.player_mut(p).life += n;
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
                self.leave_battlefield_many(&objs, Zone::Graveyard);
            }
            Effect::Exile { target } => {
                if let Some(id) = self.target_obj(r, *target) {
                    self.leave_battlefield_to(id, Zone::Exile);
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

    fn counter(&mut self, id: ObjectId) {
        let Some(entry) = self.stack.iter().find(|e| e.id == id).cloned() else {
            return;
        };
        match entry.kind {
            StackKind::Spell => {
                self.move_object(id, Zone::Graveyard, None);
            }
            StackKind::Ability { .. } => {
                self.stack.retain(|e| e.id != id);
                self.version += 1;
            }
        }
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
                self.player_mut(c.controller).life += n;
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
        for o in self.objects.values_mut() {
            o.new_in_zone = false;
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
            Decision::Discard { player, .. } if player == p => self.cleanup_rest(),
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
                        if f.this_object {
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
            }
        }
        self.pending_triggers.extend(found);
    }

    // ---------------------------------------------------------------- combat

    fn could_attack(&self, id: ObjectId) -> bool {
        let view = self.view();
        let Some(c) = view.get(id) else { return false };
        let o = &self.objects[&id];
        c.controller == self.active
            && c.is_creature()
            && !o.tapped
            && (!o.summoning_sick || c.has_keyword(Keyword::Haste))
            && !c.has_keyword(Keyword::Defender)
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
            let ac = view.get(*a).unwrap();
            if ac.has_keyword(Keyword::Flying)
                && !bc.has_keyword(Keyword::Flying)
                && !bc.has_keyword(Keyword::Reach)
            {
                return illegal(
                    "creatures with flying can only be blocked by flying or reach (702.9b)",
                );
            }
        }
        for (a, _) in self.combat.attackers.iter().filter(|(_, d)| *d == p) {
            let n = blocks.iter().filter(|(_, x)| x == a).count();
            if n == 1 && view.get(*a).is_some_and(|c| c.has_keyword(Keyword::Menace)) {
                return illegal("menace: needs two or more blockers (702.111b)");
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

    fn combat_creatures(&self) -> Vec<ObjectId> {
        let mut v: Vec<ObjectId> = self.combat.attackers.iter().map(|(a, _)| *a).collect();
        v.extend(self.combat.blockers.iter().map(|(b, _)| *b));
        v
    }

    /// CR 510. Damage assignment among multiple blockers is automatic for now: lethal damage to
    /// each blocker in declaration order, the rest to the last (or to the player with trample).
    fn combat_damage(&mut self, first_strike_step: bool) {
        let view = self.view();
        let deals = |id: ObjectId, game: &Game| -> bool {
            let Some(c) = view.get(id) else { return false };
            let fs = c.has_keyword(Keyword::FirstStrike);
            let ds = c.has_keyword(Keyword::DoubleStrike);
            if first_strike_step {
                fs || ds
            } else {
                ds || !game.combat.dealt_first_strike.contains(&id)
            }
        };
        let mut assignments: Vec<(ObjectId, Target, i32)> = Vec::new();
        for (a, defender) in self.combat.attackers.clone() {
            if !self.is_on_battlefield(a) || !deals(a, self) {
                continue;
            }
            let c = view.get(a).unwrap();
            let mut power = c.power.unwrap_or(0);
            if power <= 0 {
                continue;
            }
            let blockers: Vec<ObjectId> = self
                .combat
                .blockers
                .iter()
                .filter(|(b, x)| *x == a && self.is_on_battlefield(*b))
                .map(|(b, _)| *b)
                .collect();
            if !self.combat.blocked.contains(&a) {
                assignments.push((a, Target::Player(defender), power));
                continue;
            }
            if blockers.is_empty() {
                if c.has_keyword(Keyword::Trample) {
                    assignments.push((a, Target::Player(defender), power));
                }
                continue;
            }
            let dt = c.has_keyword(Keyword::Deathtouch);
            for (i, b) in blockers.iter().enumerate() {
                let bc = view.get(*b).unwrap();
                let remaining_toughness =
                    (bc.toughness.unwrap_or(0) - self.objects[b].damage).max(0);
                let lethal = if dt {
                    remaining_toughness.min(1)
                } else {
                    remaining_toughness
                };
                let last = i == blockers.len() - 1;
                let amt = if last && !c.has_keyword(Keyword::Trample) {
                    power
                } else {
                    lethal.min(power)
                };
                if amt > 0 {
                    assignments.push((a, Target::Object(*b), amt));
                }
                power -= amt;
            }
            if power > 0 {
                assignments.push((a, Target::Player(defender), power));
            }
        }
        for (b, a) in self.combat.blockers.clone() {
            if !self.is_on_battlefield(b) || !self.is_on_battlefield(a) || !deals(b, self) {
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
fn plan_auto_tap(
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
