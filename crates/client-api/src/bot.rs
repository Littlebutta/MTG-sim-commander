//! A simple practice opponent. It plays lands, casts what it can afford (biggest first),
//! attacks when it's safe-ish, and blocks to trade or chump when life is low. It's meant to
//! keep a solo test game moving, not to play well.

use rules_core::*;

pub fn bot_action(g: &Game, me: PlayerId) -> Action {
    match g.decision().clone() {
        Decision::Mulligan { .. } => {
            let bottom: Vec<ObjectId> = match g.decision() {
                Decision::Mulligan {
                    bottom_count_if_keep,
                    ..
                } => g
                    .hand(me)
                    .iter()
                    .rev()
                    .take(*bottom_count_if_keep as usize)
                    .copied()
                    .collect(),
                _ => vec![],
            };
            Action::Keep { bottom }
        }
        Decision::Priority { .. } => priority(g, me),
        Decision::PutTriggersOnStack { count, .. } => {
            let trigs = g.pending_triggers_for(me);
            Action::OrderTriggers(
                (0..count)
                    .map(|i| {
                        let targets = trigs[i]
                            .targets()
                            .iter()
                            .filter_map(|s| pick_target(g, me, Some(trigs[i].source), s))
                            .collect::<Vec<_>>();
                        let targets = if targets.len() == trigs[i].targets().len() {
                            targets
                        } else {
                            vec![]
                        };
                        (i, targets)
                    })
                    .collect(),
            )
        }
        Decision::DeclareAttackers { .. } => attack(g, me),
        Decision::DeclareBlockers { .. } => block(g, me),
        Decision::Discard { count, .. } => {
            let view = g.view();
            let mut hand = g.hand(me).to_vec();
            hand.sort_by_key(|id| {
                std::cmp::Reverse(view.get(*id).map(|c| c.mana_value()).unwrap_or(0))
            });
            Action::Discard(hand.into_iter().take(count).collect())
        }
        Decision::GameOver { .. } => Action::Pass,
    }
}

fn opponents(g: &Game, me: PlayerId) -> Vec<PlayerId> {
    g.players_in_game()
        .into_iter()
        .filter(|p| *p != me)
        .collect()
}

fn weakest_opponent(g: &Game, me: PlayerId) -> Option<PlayerId> {
    opponents(g, me)
        .into_iter()
        .min_by_key(|p| g.player(*p).life)
}

fn pick_target(
    g: &Game,
    me: PlayerId,
    source: Option<ObjectId>,
    spec: &TargetSpec,
) -> Option<Target> {
    let view = g.view();
    let legal = g.legal_targets(me, source, spec);
    let mine = |t: &Target| match t {
        Target::Player(p) => *p == me,
        Target::Object(id) => view.controller(*id) == Some(me),
    };
    let power = |t: &Target| match t {
        Target::Object(id) => view.get(*id).and_then(|c| c.power).unwrap_or(0),
        _ => 0,
    };
    // Helpful effects go on our own stuff; everything else at the opponents' best creature or face.
    let helpful = matches!(spec, TargetSpec::Permanent(f) if f.controller == Some(PlayerRef::You));
    if helpful {
        return legal.into_iter().filter(mine).max_by_key(power);
    }
    let theirs: Vec<Target> = legal.into_iter().filter(|t| !mine(t)).collect();
    theirs
        .iter()
        .filter(|t| matches!(t, Target::Object(_)))
        .max_by_key(|t| power(t))
        .copied()
        .or_else(|| {
            let w = weakest_opponent(g, me)?;
            theirs.iter().find(|t| **t == Target::Player(w)).copied()
        })
        .or_else(|| theirs.first().copied())
}

fn priority(g: &Game, me: PlayerId) -> Action {
    // Respond to nothing; act only on our own turn with an empty stack, plus burn at end of turn.
    if let Some(land) = g.playable_lands(me).first() {
        return Action::PlayLand(*land);
    }
    let view = g.view();
    let mut castable = g.castable(me);
    castable
        .sort_by_key(|id| std::cmp::Reverse(view.get(*id).map(|c| c.mana_value()).unwrap_or(0)));
    for card in castable {
        let def = &g.object(card).unwrap().def;
        // Don't counter or pump randomly; those need context the bot doesn't track.
        if def
            .spell_effects
            .iter()
            .any(|e| matches!(e, Effect::Counter { .. } | Effect::ApplyContinuous { .. }))
        {
            continue;
        }
        if !g.stack.is_empty() {
            continue;
        }
        let targets: Vec<Target> = def
            .spell_targets
            .iter()
            .filter_map(|s| pick_target(g, me, Some(card), s))
            .collect();
        if targets.len() != def.spell_targets.len() {
            continue;
        }
        // Don't wipe our own board.
        if def
            .spell_effects
            .iter()
            .any(|e| matches!(e, Effect::DestroyAll { .. }))
        {
            let mine = g
                .battlefield
                .iter()
                .filter(|id| {
                    view.get(**id)
                        .is_some_and(|c| c.controller == me && c.is_creature())
                })
                .count();
            let theirs = g
                .battlefield
                .iter()
                .filter(|id| {
                    view.get(**id)
                        .is_some_and(|c| c.controller != me && c.is_creature())
                })
                .count();
            if theirs <= mine + 1 {
                continue;
            }
        }
        return Action::Cast {
            card,
            targets,
            x: 0,
        };
    }
    // Use non-mana activated abilities that deal damage or grow us, on our turn.
    if g.active == me && g.stack.is_empty() {
        for (obj, idx) in g.activatable(me) {
            let ch = view.get(obj).unwrap();
            if let Ability::Activated {
                targets, effects, ..
            } = &ch.abilities[idx].ability
            {
                if !effects
                    .iter()
                    .any(|e| matches!(e, Effect::DealDamage { .. } | Effect::AddCounters { .. }))
                {
                    continue;
                }
                let ts: Vec<Target> = targets
                    .iter()
                    .filter_map(|s| pick_target(g, me, Some(obj), s))
                    .collect();
                if ts.len() == targets.len() {
                    return Action::Activate {
                        object: obj,
                        ability: idx,
                        targets: ts,
                        x: 0,
                    };
                }
            }
        }
    }
    Action::Pass
}

fn attack(g: &Game, me: PlayerId) -> Action {
    let view = g.view();
    let Some(target) = weakest_opponent(g, me) else {
        return Action::Attack(vec![]);
    };
    let their_best_blocker = g
        .battlefield
        .iter()
        .filter_map(|id| view.get(*id))
        .filter(|c| c.controller == target && c.is_creature())
        .map(|c| c.power.unwrap_or(0))
        .max()
        .unwrap_or(0);
    let attackers = g
        .possible_attackers()
        .into_iter()
        .filter(|id| {
            let c = view.get(*id).unwrap();
            // Attack if it survives the biggest blocker, or if it flies.
            c.toughness.unwrap_or(0) > their_best_blocker || c.has_keyword(Keyword::Flying)
        })
        .map(|id| (id, target))
        .collect();
    Action::Attack(attackers)
}

fn block(g: &Game, me: PlayerId) -> Action {
    let view = g.view();
    let mut available = g.possible_blockers(me);
    let mut blocks = Vec::new();
    let incoming: Vec<ObjectId> = g
        .combat
        .attackers
        .iter()
        .filter(|(_, d)| *d == me)
        .map(|(a, _)| *a)
        .collect();
    let total: i32 = incoming
        .iter()
        .filter_map(|a| view.get(*a))
        .map(|c| c.power.unwrap_or(0))
        .sum();
    let desperate = total >= g.player(me).life;
    for a in incoming {
        let ac = view.get(a).unwrap();
        if ac.has_keyword(Keyword::Menace) {
            continue;
        }
        let flying = ac.has_keyword(Keyword::Flying);
        let pick = available.iter().copied().find(|b| {
            let bc = view.get(*b).unwrap();
            let can = !flying || bc.has_keyword(Keyword::Flying) || bc.has_keyword(Keyword::Reach);
            let kills = bc.power.unwrap_or(0) >= ac.toughness.unwrap_or(0);
            let survives = bc.toughness.unwrap_or(0) > ac.power.unwrap_or(0);
            can && (survives || kills || desperate)
        });
        if let Some(b) = pick {
            available.retain(|x| *x != b);
            blocks.push((b, a));
        }
    }
    Action::Block(blocks)
}
