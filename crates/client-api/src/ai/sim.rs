//! "What happens if I do this?" — play an action forward on a copy of the game.
//!
//! Other players are assumed to pass priority, and their combat choices come from the simple
//! heuristic bot, which only uses information on the table. That is how a person reasons
//! about a play too: "if nobody responds, this resolves, and then..."

use crate::bot::bot_action;
use rules_core::*;

fn in_combat(step: Step) -> bool {
    matches!(
        step,
        Step::BeginCombat
            | Step::DeclareAttackers
            | Step::DeclareBlockers
            | Step::FirstStrikeDamage
            | Step::CombatDamage
    )
}

/// After `me` acted, let things play out to a quiet point: the stack resolves (everyone else
/// passes), and if we're in combat, combat finishes too.
pub fn settle(g: &mut Game) {
    let turn = g.turn;
    let through_combat = in_combat(g.step) && g.step != Step::BeginCombat;
    for _ in 0..400 {
        let d = g.decision().clone();
        let Some(p) = d.player() else { return };
        if g.turn != turn {
            return;
        }
        let quiet = g.stack.is_empty() && g.pending_triggers.is_empty();
        if quiet && !(through_combat && in_combat(g.step)) {
            return;
        }
        let action = match d {
            Decision::Priority { .. } => Action::Pass,
            // Our own later choices in a line are made the simple way.
            _ => bot_action(g, p),
        };
        if g.submit(p, action).is_err() && g.submit(p, Action::Pass).is_err() {
            let _ = g.submit(p, Action::Concede);
        }
    }
}

/// Copy the game, apply `action` for `me`, and settle. `None` if the action is illegal.
pub fn try_action(g: &Game, me: PlayerId, action: Action) -> Option<Game> {
    let mut sim = g.clone();
    sim.submit(me, action).ok()?;
    settle(&mut sim);
    Some(sim)
}
