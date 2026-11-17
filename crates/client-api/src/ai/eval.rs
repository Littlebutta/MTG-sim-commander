//! How good a game position is for one player.
//!
//! The AI only reads what that player could see at a real table: the battlefield, life
//! totals, commander damage and how many cards each player holds. It never looks at what the
//! cards in other hands or any library are.

use rules_core::*;
use std::collections::HashMap;

/// Score for a player who has left the game.
const OUT: f32 = -60.0;
pub const WIN: f32 = 100_000.0;

/// Tuning that differs between personalities.
#[derive(Clone, Copy, Debug)]
pub struct Weights {
    /// How much the AI fears the crack-back after it taps its creatures.
    pub caution: f32,
    /// Value of one card in hand.
    pub card: f32,
}

/// Life is worth more the less of it you have.
pub fn life_value(life: i32) -> f32 {
    let l = life as f32;
    if l <= 0.0 {
        return -40.0;
    }
    let low = l.min(10.0) * 2.2;
    let mid = (l - 10.0).clamp(0.0, 20.0) * 0.9;
    let high = (l - 30.0).max(0.0) * 0.35;
    low + mid + high
}

/// Power/toughness from effects that wear off this turn; those don't count as lasting value.
fn temporary_pt(g: &Game, id: ObjectId) -> (i32, i32) {
    let mut p = 0;
    let mut t = 0;
    for e in &g.effects {
        if e.duration == Duration::Permanent || !e.objects.contains(&id) {
            continue;
        }
        for m in &e.mods {
            if let Modification::ModifyPT(dp, dt) = m {
                p += dp;
                t += dt;
            }
        }
    }
    (p, t)
}

/// Lasting value of one permanent to its controller.
pub fn permanent_value(g: &Game, view: &View, id: ObjectId) -> f32 {
    let Some(c) = view.get(id) else { return 0.0 };
    let mv = c.mana_value() as f32;
    if c.is_creature() {
        let (tp, tt) = temporary_pt(g, id);
        let p = (c.power.unwrap_or(0) - tp).max(0) as f32;
        let t = (c.toughness.unwrap_or(0) - tt).max(0) as f32;
        let o = g.object(id);
        let damage = o.map(|o| o.damage).unwrap_or(0) as f32;
        if t - damage <= 0.0 && t > 0.0 {
            return 0.0; // about to die
        }
        let mut v = 1.0 + 1.5 * p + 1.0 * t + 0.25 * mv;
        let kw = |k: Keyword, bonus: f32| {
            if c.has_keyword(k) {
                bonus + 0.25 * p
            } else {
                0.0
            }
        };
        v += kw(Keyword::Flying, 1.2)
            + kw(Keyword::Trample, 0.3)
            + kw(Keyword::Menace, 0.6)
            + kw(Keyword::DoubleStrike, 1.0)
            + kw(Keyword::FirstStrike, 0.6)
            + kw(Keyword::Lifelink, 0.5)
            + if c.has_keyword(Keyword::Deathtouch) {
                2.0
            } else {
                0.0
            }
            + if c.has_keyword(Keyword::Indestructible) {
                3.0
            } else {
                0.0
            }
            + if c.has_keyword(Keyword::Hexproof) {
                1.0
            } else {
                0.0
            }
            + if c.has_keyword(Keyword::Vigilance) {
                0.5
            } else {
                0.0
            }
            + if c.has_keyword(Keyword::Reach) {
                0.3
            } else {
                0.0
            };
        if c.has_keyword(Keyword::Defender) {
            v -= 0.8 * p;
        }
        // Other abilities (mana, triggers, activated) are worth something too.
        v += 0.6
            * c.abilities
                .iter()
                .filter(|a| !matches!(a.ability, Ability::Keyword(_)))
                .count() as f32;
        if g.is_commander(id) {
            v += 2.0;
        }
        v
    } else if c.has_type(CardType::Land) {
        1.4
    } else {
        // Artifacts, enchantments, planeswalkers: roughly what was paid for them.
        1.5 + 0.7 * mv
    }
}

/// Everything a player has going for them, ignoring opponents.
pub fn player_value(g: &Game, view: &View, p: PlayerId, w: &Weights) -> f32 {
    let pl = g.player(p);
    if pl.has_lost || pl.has_left {
        return OUT;
    }
    let mut v = life_value(pl.life);
    let worst_cmdr = pl.commander_damage.values().copied().max().unwrap_or(0) as f32;
    v -= worst_cmdr * worst_cmdr / 25.0;
    v -= pl.poison as f32 * 3.0;
    v += (pl.hand.len().min(8) as f32) * w.card;
    let mut lands = 0;
    for id in &g.battlefield {
        if view.controller(*id) != Some(p) {
            continue;
        }
        let is_land = view
            .get(*id)
            .is_some_and(|c| c.has_type(CardType::Land) && !c.is_creature());
        if is_land {
            lands += 1;
            // Mana matters most early; the tenth land is worth little.
            v += if lands <= 7 { 1.4 } else { 0.4 };
        } else {
            v += permanent_value(g, view, *id);
        }
    }
    v
}

/// How dangerous each opponent looks to `me`, as weights that sum to 1.
pub fn threat_weights(
    g: &Game,
    view: &View,
    me: PlayerId,
    w: &Weights,
    grudges: &HashMap<PlayerId, f32>,
) -> Vec<(PlayerId, f32)> {
    let opps: Vec<PlayerId> = g
        .players
        .iter()
        .map(|p| p.id)
        .filter(|p| *p != me && g.in_game(*p))
        .collect();
    if opps.is_empty() {
        return vec![];
    }
    let values: Vec<f32> = opps
        .iter()
        .map(|p| player_value(g, view, *p, w).max(1.0))
        .collect();
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let raw: Vec<f32> = opps
        .iter()
        .zip(&values)
        .map(|(p, v)| {
            let grudge = grudges.get(p).copied().unwrap_or(0.0);
            let life = g.player(*p).life;
            1.0 + 0.6 * (v / mean - 1.0).max(-0.5)
                + (0.06 * grudge).min(1.2)
                + if life <= 12 { 0.35 } else { 0.0 }
        })
        .collect();
    let total: f32 = raw.iter().sum();
    opps.into_iter()
        .zip(raw)
        .map(|(p, r)| (p, r / total))
        .collect()
}

/// Damage `me` should expect on the opponents' turns, given who can still block.
fn exposure(g: &Game, view: &View, me: PlayerId) -> (f32, i32) {
    let mut blockers: Vec<(i32, bool)> = g
        .battlefield
        .iter()
        .filter_map(|id| {
            let c = view.get(*id)?;
            let o = g.object(*id)?;
            (c.controller == me && c.is_creature() && !o.tapped).then(|| {
                (
                    c.toughness.unwrap_or(0),
                    c.has_keyword(Keyword::Flying) || c.has_keyword(Keyword::Reach),
                )
            })
        })
        .collect();
    blockers.sort_by_key(|b| std::cmp::Reverse(b.0));
    let mut expected = 0.0;
    let mut worst = 0;
    for opp in g.players_in_game().into_iter().filter(|p| *p != me) {
        let mut attackers: Vec<(i32, bool)> = g
            .battlefield
            .iter()
            .filter_map(|id| {
                let c = view.get(*id)?;
                (c.controller == opp && c.is_creature() && !c.has_keyword(Keyword::Defender))
                    .then(|| (c.power.unwrap_or(0).max(0), c.has_keyword(Keyword::Flying)))
            })
            .collect();
        attackers.sort_by_key(|a| std::cmp::Reverse(a.0));
        let mut free = blockers.clone();
        let mut through = 0;
        for (power, flying) in attackers {
            if let Some(i) = free.iter().position(|b| !flying || b.1) {
                free.remove(i);
            } else {
                through += power;
            }
        }
        worst = worst.max(through);
        // Not every opponent swings at you every turn.
        expected += through as f32 * 0.45;
    }
    (expected, worst)
}

/// The number the AI maximises: its own position minus its opponents', weighted by threat,
/// minus the risk of being attacked back.
pub fn score(g: &Game, me: PlayerId, w: &Weights, grudges: &HashMap<PlayerId, f32>) -> f32 {
    if let Decision::GameOver { winner } = g.decision() {
        return match winner {
            Some(p) if *p == me => WIN,
            _ => -WIN,
        };
    }
    if !g.in_game(me) {
        return -WIN;
    }
    let view = g.view();
    let mine = player_value(g, &view, me, w);
    let mut theirs = 0.0;
    for (p, weight) in threat_weights(g, &view, me, w, grudges) {
        theirs += weight * player_value(g, &view, p, w);
    }
    // Opponents who are already out still count as a big gain for having removed them.
    let out = g
        .players
        .iter()
        .filter(|p| p.id != me && !g.in_game(p.id))
        .count() as f32;
    let (expected, worst) = exposure(g, &view, me);
    let life = g.player(me).life;
    let mut risk = life_value(life) - life_value(life - expected.round() as i32);
    if worst >= life {
        risk += 35.0;
    }
    mine - theirs + out * 25.0 - w.caution * risk
}
