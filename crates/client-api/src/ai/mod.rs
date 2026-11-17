//! The computer opponent.
//!
//! It plays the way a careful person does: for every play it's considering, it imagines the
//! result with the real rules engine ("if I cast this and nobody responds...", "if I swing
//! with these, they'll probably block like that...") and picks what leaves it best off. It
//! remembers who has been attacking it, sizes up which opponent is the biggest threat, holds
//! instants for the right moment, and takes a human-like amount of time to decide.
//!
//! It never peeks: decisions use only the battlefield, life totals, card counts and its own
//! hand, never the contents of other hands or libraries.

mod eval;
mod sim;

pub use eval::{life_value, score, Weights};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rules_core::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Play styles, so a table of AI opponents doesn't feel like three copies of one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Personality {
    /// Attacks often, takes risks, rarely holds cards back.
    Aggressive,
    /// Sensible all-rounder.
    Balanced,
    /// Keeps blockers home, holds instants for the opponents' turns, values cards.
    Controlling,
}

impl Personality {
    pub const ALL: [Personality; 3] = [
        Personality::Aggressive,
        Personality::Balanced,
        Personality::Controlling,
    ];

    fn weights(self) -> Weights {
        match self {
            Personality::Aggressive => Weights {
                caution: 0.45,
                card: 2.2,
            },
            Personality::Balanced => Weights {
                caution: 0.8,
                card: 2.6,
            },
            Personality::Controlling => Weights {
                caution: 1.15,
                card: 3.0,
            },
        }
    }

    /// Score lost by casting an instant at sorcery speed instead of waiting.
    fn patience(self) -> f32 {
        match self {
            Personality::Aggressive => 0.3,
            Personality::Balanced => 1.2,
            Personality::Controlling => 2.5,
        }
    }

    /// How close two options must be before it's a coin flip between them.
    fn noise(self) -> f32 {
        match self {
            Personality::Aggressive => 0.6,
            Personality::Balanced => 0.35,
            Personality::Controlling => 0.25,
        }
    }
}

/// What the AI decided, and how long a person would plausibly take to decide it.
#[derive(Clone, Debug)]
pub struct Decided {
    pub action: Action,
    pub think_ms: u32,
}

/// One AI player's mind: its style, its memory of the game, and its dice.
#[derive(Clone, Debug)]
pub struct Brain {
    pub me: PlayerId,
    pub personality: Personality,
    weights: Weights,
    /// How much each opponent has hurt or targeted us lately.
    grudges: HashMap<PlayerId, f32>,
    seen_attacks: HashSet<(u32, ObjectId)>,
    seen_stack: HashSet<ObjectId>,
    last_decay_turn: u32,
    rng: ChaCha8Rng,
}

struct Option_ {
    action: Action,
    score: f32,
}

impl Brain {
    pub fn new(me: PlayerId, personality: Personality, seed: u64) -> Brain {
        Brain {
            me,
            personality,
            weights: personality.weights(),
            grudges: HashMap::new(),
            seen_attacks: HashSet::new(),
            seen_stack: HashSet::new(),
            last_decay_turn: 0,
            rng: ChaCha8Rng::seed_from_u64(seed ^ (me.0 as u64).wrapping_mul(0x9E37_79B9)),
        }
    }

    pub fn grudge(&self, p: PlayerId) -> f32 {
        self.grudges.get(&p).copied().unwrap_or(0.0)
    }

    /// Watch the table: remember who attacks us or points spells at our things.
    pub fn observe(&mut self, g: &Game) {
        let me = self.me;
        if g.active == me && g.turn != self.last_decay_turn {
            self.last_decay_turn = g.turn;
            for v in self.grudges.values_mut() {
                *v *= 0.7;
            }
        }
        let view = g.view();
        for (a, defender) in &g.combat.attackers {
            if *defender != me || !self.seen_attacks.insert((g.turn, *a)) {
                continue;
            }
            if let Some(c) = view.get(*a) {
                *self.grudges.entry(c.controller).or_insert(0.0) +=
                    2.0 + c.power.unwrap_or(0).max(0) as f32;
            }
        }
        for e in &g.stack {
            if e.controller == me || self.seen_stack.contains(&e.id) {
                continue;
            }
            let hits_me = e.targets.iter().any(|t| match t {
                Target::Player(p) => *p == me,
                Target::Object(o) => view.controller(*o) == Some(me),
            });
            if hits_me {
                self.seen_stack.insert(e.id);
                *self.grudges.entry(e.controller).or_insert(0.0) += 4.0;
            }
        }
    }

    fn score(&self, g: &Game) -> f32 {
        score(g, self.me, &self.weights, &self.grudges)
    }

    fn think(&mut self, lo: u32, hi: u32) -> u32 {
        self.rng.gen_range(lo..=hi)
    }

    /// Pick the best option, but treat near-ties as a matter of taste.
    fn choose(&mut self, mut opts: Vec<Option_>) -> Option<Option_> {
        if opts.is_empty() {
            return None;
        }
        opts.sort_by(|a, b| b.score.total_cmp(&a.score));
        let best = opts[0].score;
        let noise = self.personality.noise();
        let close = opts.iter().take_while(|o| best - o.score <= noise).count();
        let pick = self.rng.gen_range(0..close);
        Some(opts.swap_remove(pick))
    }

    pub fn decide(&mut self, g: &Game) -> Decided {
        self.observe(g);
        match g.decision().clone() {
            Decision::Mulligan {
                bottom_count_if_keep,
                ..
            } => self.mulligan(g, bottom_count_if_keep),
            Decision::Priority { .. } => self.priority(g),
            Decision::PutTriggersOnStack { count, .. } => self.triggers(g, count),
            Decision::DeclareAttackers { .. } => self.attack(g),
            Decision::DeclareBlockers { .. } => self.block(g),
            Decision::Discard { count, .. } => self.discard(g, count),
            Decision::GameOver { .. } => Decided {
                action: Action::Pass,
                think_ms: 0,
            },
        }
    }

    // ---------------------------------------------------------------- opening hand

    fn mulligan(&mut self, g: &Game, bottom: u32) -> Decided {
        let me = self.me;
        let hand = g.hand(me);
        let land = |id: &ObjectId| {
            g.object(*id)
                .is_some_and(|o| o.def.has_type(CardType::Land))
        };
        let lands = hand.iter().filter(|id| land(id)).count();
        let cheap = hand
            .iter()
            .filter(|id| !land(id))
            .filter(|id| {
                g.object(**id)
                    .is_some_and(|o| o.def.mana_cost.mana_value() <= 3)
            })
            .count();
        let mulls = g.player(me).mulligans;
        let keep = match mulls {
            0 => (2..=5).contains(&lands) && (cheap >= 1 || lands >= 3),
            1 => (2..=5).contains(&lands),
            2 => (1..=6).contains(&lands),
            _ => true,
        };
        let think_ms = self.think(1800, 4200);
        if !keep {
            return Decided {
                action: Action::Mulligan,
                think_ms,
            };
        }
        // Bottom what this hand needs least: extra lands past four, then the most expensive.
        let mut ranked: Vec<(f32, ObjectId)> = hand
            .iter()
            .map(|id| {
                let o = g.object(*id).unwrap();
                let v = if o.def.has_type(CardType::Land) {
                    if lands > 4 {
                        1.0
                    } else {
                        9.0
                    }
                } else {
                    6.0 - o.def.mana_cost.mana_value() as f32 * 0.6
                };
                (v, *id)
            })
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
        Decided {
            action: Action::Keep {
                bottom: ranked
                    .into_iter()
                    .take(bottom as usize)
                    .map(|(_, id)| id)
                    .collect(),
            },
            think_ms,
        }
    }

    // ---------------------------------------------------------------- priority

    /// Candidate targets for one target slot, most relevant first, capped so the search stays small.
    fn target_choices(&self, g: &Game, source: Option<ObjectId>, spec: &TargetSpec) -> Vec<Target> {
        let me = self.me;
        let view = g.view();
        let legal = g.legal_targets(me, source, spec);
        let mut players: Vec<Target> = Vec::new();
        let mut theirs: Vec<(f32, Target)> = Vec::new();
        let mut mine: Vec<(f32, Target)> = Vec::new();
        for t in legal {
            match t {
                Target::Player(_) => players.push(t),
                Target::Object(id) => {
                    let v = if g.is_on_battlefield(id) {
                        eval::permanent_value(g, &view, id)
                    } else {
                        g.object(id)
                            .map(|o| o.def.mana_cost.mana_value() as f32)
                            .unwrap_or(0.0)
                    };
                    let controller = view
                        .controller(id)
                        .or_else(|| g.stack.iter().find(|e| e.id == id).map(|e| e.controller));
                    if controller == Some(me) {
                        mine.push((v, t));
                    } else {
                        theirs.push((v, t));
                    }
                }
            }
        }
        theirs.sort_by(|a, b| b.0.total_cmp(&a.0));
        mine.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out: Vec<Target> = theirs.into_iter().take(6).map(|x| x.1).collect();
        out.extend(mine.into_iter().take(3).map(|x| x.1));
        out.extend(players);
        out
    }

    /// Every way to fill the target slots, up to a limit.
    fn target_combos(
        &self,
        g: &Game,
        source: Option<ObjectId>,
        specs: &[TargetSpec],
    ) -> Vec<Vec<Target>> {
        let mut combos: Vec<Vec<Target>> = vec![vec![]];
        for spec in specs {
            let choices = self.target_choices(g, source, spec);
            let mut next = Vec::new();
            for c in &combos {
                for t in &choices {
                    if c.contains(t) {
                        continue;
                    }
                    let mut n = c.clone();
                    n.push(*t);
                    next.push(n);
                    if next.len() >= 30 {
                        break;
                    }
                }
            }
            combos = next;
        }
        combos
    }

    fn max_x(&self, g: &Game, card: ObjectId) -> u32 {
        let o = g.object(card).unwrap();
        let tax = if o.zone == Zone::Command {
            g.commander_tax(self.me, card)
        } else {
            0
        };
        (1..=20)
            .rev()
            .find(|x| g.can_afford(self.me, &o.def.mana_cost.with_x_and_extra(*x, tax)))
            .unwrap_or(0)
    }

    /// Is now a sensible moment to consider instant-speed plays?
    fn instant_window(&self, g: &Game) -> bool {
        if !g.stack.is_empty() {
            return true;
        }
        if g.active == self.me {
            return g.step.is_main() || g.step == Step::DeclareBlockers;
        }
        matches!(
            g.step,
            Step::DeclareAttackers | Step::DeclareBlockers | Step::End
        )
    }

    fn priority(&mut self, g: &Game) -> Decided {
        let me = self.me;
        let my_turn = g.active == me;
        let sorcery_time = my_turn && g.step.is_main() && g.stack.is_empty();

        // Lands first: pick the one that lets us cast the most.
        let lands = g.playable_lands(me);
        if !lands.is_empty() {
            let mut best: Option<(usize, ObjectId)> = None;
            let mut seen = HashSet::new();
            for l in lands {
                let name = g.object(l).unwrap().def.name.clone();
                if !seen.insert(name) {
                    continue;
                }
                let mut sim = g.clone();
                if sim.submit(me, Action::PlayLand(l)).is_err() {
                    continue;
                }
                let n = sim.castable(me).len() * 10
                    + usize::from(!sim.object(l).is_some_and(|o| o.tapped));
                if best.is_none_or(|b| n > b.0) {
                    best = Some((n, l));
                }
            }
            if let Some((_, l)) = best {
                let think_ms = self.think(500, 1200);
                return Decided {
                    action: Action::PlayLand(l),
                    think_ms,
                };
            }
        }

        let base = {
            let mut sim = g.clone();
            if sim.submit(me, Action::Pass).is_ok() {
                sim::settle(&mut sim);
            }
            self.score(&sim)
        };
        let view = g.view();
        let can_attack_now = my_turn
            && g.step == Step::Main1
            && g.battlefield
                .iter()
                .any(|id| ready_to_attack(g, &view, me, *id));

        let mut opts: Vec<Option_> = Vec::new();
        let window = self.instant_window(g);
        for card in g.castable(me) {
            let o = g.object(card).unwrap();
            let def = o.def.clone();
            let instant = def.has_type(CardType::Instant)
                || view
                    .get(card)
                    .is_some_and(|c| c.has_keyword(Keyword::Flash));
            if !instant && !sorcery_time {
                continue;
            }
            if instant && !window {
                continue;
            }
            // Creatures without haste wait for after combat, like most players do.
            if can_attack_now
                && def.has_type(CardType::Creature)
                && !def.abilities.contains(&Ability::Keyword(Keyword::Haste))
            {
                continue;
            }
            let xs: Vec<u32> = if def.mana_cost.has_x() {
                let m = self.max_x(g, card);
                if m == 0 {
                    continue;
                }
                vec![m]
            } else {
                vec![0]
            };
            let penalty = if instant && sorcery_time {
                self.personality.patience()
            } else {
                0.0
            };
            for x in xs {
                for targets in self.target_combos(g, Some(card), &def.spell_targets) {
                    let action = Action::Cast { card, targets, x };
                    if let Some(sim) = sim::try_action(g, me, action.clone()) {
                        let score = self.score(&sim) - penalty;
                        opts.push(Option_ { action, score });
                    }
                }
            }
        }
        for (obj, idx) in g.activatable(me) {
            let Some(ch) = view.get(obj) else { continue };
            let Ability::Activated {
                targets,
                sorcery_speed,
                ..
            } = &ch.abilities[idx].ability
            else {
                continue;
            };
            if !*sorcery_speed && !window {
                continue;
            }
            for t in self.target_combos(g, Some(obj), targets) {
                let action = Action::Activate {
                    object: obj,
                    ability: idx,
                    targets: t,
                    x: 0,
                };
                if let Some(sim) = sim::try_action(g, me, action.clone()) {
                    let score = self.score(&sim);
                    opts.push(Option_ { action, score });
                }
            }
        }

        // Only act if it's clearly better than doing nothing.
        opts.retain(|o| o.score > base + 0.4);
        let considered = !opts.is_empty();
        match self.choose(opts) {
            Some(o) => {
                let mv = match &o.action {
                    Action::Cast { card, .. } => g
                        .object(*card)
                        .map(|c| c.def.mana_cost.mana_value())
                        .unwrap_or(0),
                    _ => 1,
                };
                let think_ms = if g.stack.is_empty() {
                    self.think(900, 2000) + 120 * mv
                } else {
                    self.think(700, 1500)
                };
                Decided {
                    action: o.action,
                    think_ms,
                }
            }
            None => {
                let think_ms = if considered
                    || (!g.stack.is_empty() && g.stack.last().is_some_and(|e| e.controller != me))
                {
                    self.think(300, 900)
                } else if sorcery_time {
                    self.think(250, 700)
                } else {
                    0
                };
                Decided {
                    action: Action::Pass,
                    think_ms,
                }
            }
        }
    }

    // ---------------------------------------------------------------- combat

    fn eval_attack(&self, g: &Game, attack: &[(ObjectId, PlayerId)]) -> Option<f32> {
        sim::try_action(g, self.me, Action::Attack(attack.to_vec())).map(|s| self.score(&s))
    }

    fn attack(&mut self, g: &Game) -> Decided {
        let me = self.me;
        let view = g.view();
        let opps: Vec<PlayerId> = g
            .players_in_game()
            .into_iter()
            .filter(|p| *p != me)
            .collect();
        let mut attackers = g.possible_attackers();
        attackers
            .sort_by_key(|id| std::cmp::Reverse(view.get(*id).and_then(|c| c.power).unwrap_or(0)));
        let mut plan: Vec<(ObjectId, PlayerId)> = Vec::new();
        let mut plan_score = self.eval_attack(g, &plan).unwrap_or(f32::MIN);
        for a in &attackers {
            let mut opts = vec![Option_ {
                action: Action::Attack(plan.clone()),
                score: plan_score,
            }];
            for o in &opps {
                let mut p = plan.clone();
                p.push((*a, *o));
                if let Some(s) = self.eval_attack(g, &p) {
                    opts.push(Option_ {
                        action: Action::Attack(p),
                        score: s,
                    });
                }
            }
            if let Some(best) = self.choose(opts) {
                if let Action::Attack(p) = best.action {
                    plan = p;
                    plan_score = best.score;
                }
            }
        }
        // Also consider sending everything at one player: sometimes that's lethal.
        for o in &opps {
            let all: Vec<(ObjectId, PlayerId)> = attackers.iter().map(|a| (*a, *o)).collect();
            if let Some(s) = self.eval_attack(g, &all) {
                if s > plan_score + 1.0 {
                    plan = all;
                    plan_score = s;
                }
            }
        }
        let think_ms = if attackers.is_empty() {
            0
        } else {
            self.think(1100, 2400) + 250 * plan.len() as u32
        };
        Decided {
            action: Action::Attack(plan),
            think_ms,
        }
    }

    fn eval_block(&self, g: &Game, blocks: &[(ObjectId, ObjectId)]) -> Option<f32> {
        sim::try_action(g, self.me, Action::Block(blocks.to_vec())).map(|s| self.score(&s))
    }

    fn block(&mut self, g: &Game) -> Decided {
        let me = self.me;
        let view = g.view();
        let mut incoming: Vec<ObjectId> = g
            .combat
            .attackers
            .iter()
            .filter(|(_, d)| *d == me)
            .map(|(a, _)| *a)
            .collect();
        incoming
            .sort_by_key(|id| std::cmp::Reverse(view.get(*id).and_then(|c| c.power).unwrap_or(0)));
        let mut free = g.possible_blockers(me);
        let mut plan: Vec<(ObjectId, ObjectId)> = Vec::new();
        let mut plan_score = self.eval_block(g, &plan).unwrap_or(f32::MIN);
        for a in &incoming {
            let mut opts = vec![Option_ {
                action: Action::Block(plan.clone()),
                score: plan_score,
            }];
            let mut tries: Vec<Vec<ObjectId>> = free.iter().map(|b| vec![*b]).collect();
            for (i, b1) in free.iter().enumerate() {
                for b2 in free.iter().skip(i + 1).take(3) {
                    tries.push(vec![*b1, *b2]);
                }
            }
            for t in tries {
                let mut p = plan.clone();
                p.extend(t.iter().map(|b| (*b, *a)));
                if let Some(s) = self.eval_block(g, &p) {
                    opts.push(Option_ {
                        action: Action::Block(p),
                        score: s,
                    });
                }
            }
            if let Some(best) = self.choose(opts) {
                if let Action::Block(p) = best.action {
                    plan = p;
                    plan_score = best.score;
                }
            }
            free.retain(|b| !plan.iter().any(|(x, _)| x == b));
        }
        let think_ms = if incoming.is_empty() {
            0
        } else {
            self.think(1000, 2300) + 200 * plan.len() as u32
        };
        Decided {
            action: Action::Block(plan),
            think_ms,
        }
    }

    // ---------------------------------------------------------------- other choices

    fn triggers(&mut self, g: &Game, count: usize) -> Decided {
        let me = self.me;
        let trigs = g.pending_triggers_for(me);
        let mut chosen: Vec<Vec<Target>> = trigs
            .iter()
            .map(|t| {
                let specs = t.targets();
                self.target_combos(g, Some(t.source), specs)
                    .into_iter()
                    .next()
                    .unwrap_or_default()
            })
            .collect();
        for i in 0..count.min(trigs.len()) {
            let specs = trigs[i].targets();
            if specs.is_empty() {
                continue;
            }
            let mut opts = Vec::new();
            for combo in self.target_combos(g, Some(trigs[i].source), specs) {
                let mut c = chosen.clone();
                c[i] = combo;
                let action = Action::OrderTriggers((0..count).map(|j| (j, c[j].clone())).collect());
                if let Some(sim) = sim::try_action(g, me, action.clone()) {
                    opts.push(Option_ {
                        action,
                        score: self.score(&sim),
                    });
                }
            }
            opts.sort_by(|a, b| b.score.total_cmp(&a.score));
            if let Some(Option_ {
                action: Action::OrderTriggers(list),
                ..
            }) = opts.into_iter().next()
            {
                chosen[i] = list[i].1.clone();
            }
        }
        let think_ms = self.think(500, 1200);
        Decided {
            action: Action::OrderTriggers((0..count).map(|j| (j, chosen[j].clone())).collect()),
            think_ms,
        }
    }

    fn discard(&mut self, g: &Game, count: usize) -> Decided {
        let me = self.me;
        let view = g.view();
        let lands_out = g
            .battlefield
            .iter()
            .filter(|id| {
                view.get(**id)
                    .is_some_and(|c| c.controller == me && c.has_type(CardType::Land))
            })
            .count() as f32;
        let mut ranked: Vec<(f32, ObjectId)> = g
            .hand(me)
            .iter()
            .map(|id| {
                let d = &g.object(*id).unwrap().def;
                let v = if d.has_type(CardType::Land) {
                    if lands_out >= 6.0 {
                        1.0
                    } else {
                        8.0
                    }
                } else {
                    let mv = d.mana_cost.mana_value() as f32;
                    // Keep what we can cast soon.
                    6.0 - (mv - lands_out - 1.0).max(0.0) * 1.5 + mv * 0.3
                };
                (v, *id)
            })
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
        Decided {
            action: Action::Discard(ranked.into_iter().take(count).map(|x| x.1).collect()),
            think_ms: self.think(900, 1800),
        }
    }
}

/// Could this creature of ours attack this turn?
fn ready_to_attack(g: &Game, view: &View, me: PlayerId, id: ObjectId) -> bool {
    let (Some(c), Some(o)) = (view.get(id), g.object(id)) else {
        return false;
    };
    c.controller == me
        && c.is_creature()
        && !o.tapped
        && !c.has_keyword(Keyword::Defender)
        && (!o.summoning_sick || c.has_keyword(Keyword::Haste))
}
