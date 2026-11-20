//! Enumerating what a player can do right now, so a client can highlight playable cards.
//! These are conveniences for the UI: `Game::submit` remains the authority on legality.

use crate::card::{Ability, CardType, CounterKind, Keyword};
use crate::engine::{plan_auto_tap, Decision};
use crate::game::Game;
use crate::ids::{ObjectId, PlayerId};
use crate::mana::ManaCost;
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

    /// Cards in hand or the command zone that `p` could cast right now, ignoring X choices.
    pub fn castable(&self, p: PlayerId) -> Vec<ObjectId> {
        if !self.has_priority(p) {
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
        candidates
            .into_iter()
            .filter(|id| {
                let o = &self.objects[id];
                if o.def.has_type(CardType::Land) {
                    return false;
                }
                let Some(ch) = view.get(*id) else {
                    return false;
                };
                let instant = ch.has_type(CardType::Instant) || ch.has_keyword(Keyword::Flash);
                if !instant && !self.sorcery_timing_ok(p) {
                    return false;
                }
                // At least one mode (or the spell itself) has legal targets for every slot.
                if !o.def.mode_choices().into_iter().any(|m| {
                    o.def.spell_parts(m).is_some_and(|(specs, _)| {
                        specs
                            .iter()
                            .all(|s| !self.legal_targets(p, Some(*id), s).is_empty())
                    })
                }) {
                    return false;
                }
                let tax = if o.zone == Zone::Command {
                    self.commander_tax(p, *id)
                } else {
                    0
                };
                self.can_afford(p, &o.def.mana_cost.with_x_and_extra(0, tax))
            })
            .collect()
    }

    /// Activated abilities (including mana abilities) `p` could activate: (object, ability index).
    pub fn activatable(&self, p: PlayerId) -> Vec<(ObjectId, usize)> {
        if !self.has_priority(p) {
            return vec![];
        }
        let view = self.view();
        let mut out = Vec::new();
        let hand = self.player(p).hand.clone();
        for (id, in_hand) in self
            .battlefield
            .iter()
            .map(|id| (id, false))
            .chain(hand.iter().map(|id| (id, true)))
        {
            let Some(ch) = view.get(*id) else { continue };
            if ch.controller != p && !in_hand {
                continue;
            }
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
                if cost.discard_this != in_hand {
                    continue;
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
