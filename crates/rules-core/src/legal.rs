//! Enumerating what a player can do right now, so a client can highlight playable cards.
//! These are conveniences for the UI: `Game::submit` remains the authority on legality.

use crate::card::{Ability, CardType, CounterKind, Keyword};
use crate::engine::{plan_auto_tap, CastOptions, Decision};
use crate::game::Game;
use crate::ids::{ObjectId, PlayerId};
use crate::layers::Characteristics;
use crate::mana::{ManaCost, ManaType};
use crate::state::Zone;

impl Game {
    /// Could `p` pay `cost` from their pool plus untapped simple mana sources?
    pub fn can_afford(&self, p: PlayerId, cost: &ManaCost) -> bool {
        let life = self.player(p).life;
        let pool = self.player(p).mana_pool.clone();
        if pool.clone().try_pay(cost, life).is_some() {
            return true;
        }
        plan_auto_tap(&pool, &self.auto_mana_sources(p), cost, life).is_some()
    }

    /// Like `can_afford`, with convoke/improvise/delve helpers.
    pub fn can_afford_with(
        &self,
        p: PlayerId,
        cost: &ManaCost,
        helpers: &[(ObjectId, Vec<ManaType>)],
    ) -> bool {
        if helpers.is_empty() {
            return self.can_afford(p, cost);
        }
        let life = self.player(p).life;
        let pool = self.player(p).mana_pool.clone();
        let mut sources = self.auto_mana_sources(p);
        for h in helpers {
            if !sources.iter().any(|(id, _)| *id == h.0) {
                sources.push(h.clone());
            }
        }
        plan_auto_tap(&pool, &sources, cost, life).is_some()
    }

    fn has_priority(&self, p: PlayerId) -> bool {
        matches!(self.decision(), Decision::Priority { player } if *player == p)
    }

    pub fn playable_lands(&self, p: PlayerId) -> Vec<ObjectId> {
        if !self.has_priority(p) || !self.sorcery_timing_ok(p) {
            return vec![];
        }
        let pl = self.player(p);
        if pl.lands_played_this_turn >= pl.land_plays_allowed {
            return vec![];
        }
        pl.hand
            .iter()
            .copied()
            .filter(|id| self.objects[id].def.has_type(CardType::Land))
            .collect()
    }

    /// Cards in hand, the command zone or (with flashback) the graveyard that `p` could cast
    /// right now some way, ignoring X choices.
    pub fn castable(&self, p: PlayerId) -> Vec<ObjectId> {
        let mut out: Vec<ObjectId> = Vec::new();
        for (id, _) in self.cast_ways(p) {
            if !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }

    /// Every card `p` could cast now and each way to cast it (normal, kicked, with buyback,
    /// face down). Kicked and buyback are only listed when they can be paid.
    pub fn cast_ways(&self, p: PlayerId) -> Vec<(ObjectId, CastOptions)> {
        if !self.has_priority(p) || self.split_second_on_stack() {
            return vec![];
        }
        let view = self.view();
        let mut candidates: Vec<ObjectId> = self.player(p).hand.clone();
        candidates.extend(
            self.command
                .iter()
                .copied()
                .filter(|id| self.objects[id].owner == p),
        );
        candidates.extend(
            self.player(p)
                .graveyard
                .iter()
                .copied()
                .filter(|id| self.objects[id].def.flashback_cost().is_some()),
        );
        let mut out = Vec::new();
        for id in candidates {
            let o = &self.objects[&id];
            if o.def.has_type(CardType::Land) {
                continue;
            }
            let Some(ch) = view.get(id) else { continue };
            let mut ways = vec![CastOptions::default()];
            if o.def.morph().is_some() && o.zone == Zone::Hand {
                ways.push(CastOptions {
                    face_down: true,
                    ..Default::default()
                });
            }
            if o.def.kicker_cost().is_some() {
                ways.push(CastOptions {
                    kicked: true,
                    ..Default::default()
                });
            }
            if o.def.evoke_cost().is_some() && o.zone == Zone::Hand {
                ways.push(CastOptions {
                    evoke: true,
                    ..Default::default()
                });
            }
            if o.def.buyback_cost().is_some() {
                ways.push(CastOptions {
                    buyback: true,
                    ..Default::default()
                });
            }
            for opts in ways {
                if self.can_cast_way(p, id, ch, opts) {
                    out.push((id, opts));
                }
            }
        }
        out
    }

    fn can_cast_way(
        &self,
        p: PlayerId,
        id: ObjectId,
        ch: &Characteristics,
        opts: CastOptions,
    ) -> bool {
        let o = &self.objects[&id];
        let instant =
            !opts.face_down && (ch.has_type(CardType::Instant) || ch.has_keyword(Keyword::Flash));
        if !instant && !self.sorcery_timing_ok(p) {
            return false;
        }
        if !opts.face_down {
            // At least one mode (or the spell itself) has legal targets for every slot.
            if !o.def.mode_choices().into_iter().any(|m| {
                o.def.spell_parts(m).is_some_and(|(specs, _)| {
                    specs
                        .iter()
                        .all(|s| !self.legal_targets(p, Some(id), s).is_empty())
                })
            }) {
                return false;
            }
            if let Some(c) = o.def.additional_cost() {
                if self.check_extra_cost(p, id, c).is_err() {
                    return false;
                }
            }
        }
        let cost = self.spell_mana_cost(p, id, opts, 0);
        let helpers = if opts.face_down {
            vec![]
        } else {
            self.cast_helpers(p, id, ch)
        };
        self.can_afford_with(p, &cost, &helpers)
    }

    /// Activated abilities (including mana abilities) `p` could activate: (object, ability index).
    pub fn activatable(&self, p: PlayerId) -> Vec<(ObjectId, usize)> {
        if !self.has_priority(p) {
            return vec![];
        }
        let view = self.view();
        let mut out = Vec::new();
        if self.split_second_on_stack() {
            return vec![];
        }
        let hand = self.player(p).hand.clone();
        let yard = self.player(p).graveyard.clone();
        // Where the object is: 0 battlefield, 1 hand, 2 graveyard.
        for (id, place) in self
            .battlefield
            .iter()
            .map(|id| (id, 0))
            .chain(hand.iter().map(|id| (id, 1)))
            .chain(yard.iter().map(|id| (id, 2)))
        {
            let Some(ch) = view.get(*id) else { continue };
            if ch.controller != p && place == 0 {
                continue;
            }
            let in_hand = place == 1;
            let o = &self.objects[id];
            for (i, a) in ch.abilities.iter().enumerate() {
                let (cost, sorcery, targets) = match &a.ability {
                    Ability::Mana { cost, .. } => (cost, false, &[][..]),
                    Ability::Activated {
                        cost,
                        sorcery_speed,
                        targets,
                        ..
                    } => (cost, *sorcery_speed, targets.as_slice()),
                    _ => continue,
                };
                if cost.discard_this != in_hand || cost.from_graveyard != (place == 2) {
                    continue;
                }
                if let Some(n) = cost.crew {
                    if self.crew_choice(p, *id, n).is_none() {
                        continue;
                    }
                }
                if cost.tap
                    && (o.tapped
                        || (ch.is_creature()
                            && o.summoning_sick
                            && !ch.has_keyword(Keyword::Haste)))
                {
                    continue;
                }
                if (sorcery || cost.loyalty.is_some()) && !self.sorcery_timing_ok(p) {
                    continue;
                }
                if cost.once_per_turn && o.activated_this_turn.contains(&i) {
                    continue;
                }
                if self.player(p).hand.len() < cost.discard as usize {
                    continue;
                }
                if let Some((f, n)) = &cost.sacrifice {
                    if self.cost_sacrifice_choice(p, *id, f, *n).len() < *n as usize {
                        continue;
                    }
                }
                if let Some((k, n)) = &cost.remove_counters {
                    if o.counters.get(k).copied().unwrap_or(0) < *n {
                        continue;
                    }
                }
                if let Some(l) = cost.loyalty {
                    let have = o.counters.get(&CounterKind::Loyalty).copied().unwrap_or(0) as i32;
                    if o.loyalty_used || have + l < 0 {
                        continue;
                    }
                }
                if targets
                    .iter()
                    .any(|s| self.legal_targets(p, Some(*id), s).is_empty())
                {
                    continue;
                }
                if !cost.mana.0.is_empty() && !self.can_afford(p, &cost.mana) {
                    continue;
                }
                out.push((*id, i));
            }
        }
        out
    }

    /// Face-down permanents `p` controls whose morph/disguise cost they can pay now.
    pub fn can_turn_face_up(&self, p: PlayerId) -> Vec<ObjectId> {
        if !self.has_priority(p) {
            return vec![];
        }
        let view = self.view();
        self.battlefield
            .iter()
            .copied()
            .filter(|id| {
                let o = &self.objects[id];
                o.face_down
                    && view.controller(*id) == Some(p)
                    && o.def.morph().is_some_and(|(c, _, _)| self.can_afford(p, c))
            })
            .collect()
    }

    pub fn possible_attackers(&self) -> Vec<ObjectId> {
        self.battlefield
            .iter()
            .copied()
            .filter(|id| self.could_attack(*id))
            .collect()
    }

    pub fn possible_blockers(&self, p: PlayerId) -> Vec<ObjectId> {
        let view = self.view();
        self.battlefield
            .iter()
            .copied()
            .filter(|id| {
                view.get(*id)
                    .is_some_and(|c| c.controller == p && c.is_creature())
                    && !self.objects[id].tapped
            })
            .collect()
    }
}
