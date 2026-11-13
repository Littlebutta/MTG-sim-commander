//! What one player is allowed to see, shaped for the UI.

use crate::describe::{describe_ability, describe_effects};
use rules_core::layers::Characteristics;
use rules_core::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct AbilityView {
    pub index: usize,
    pub label: String,
    pub is_mana: bool,
    pub activatable: bool,
    pub targets: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CardView {
    pub id: u32,
    pub name: String,
    pub mana_cost: String,
    pub mana_value: u32,
    /// Color letters, e.g. ["G", "W"].
    pub colors: Vec<char>,
    pub supertypes: Vec<String>,
    pub types: Vec<String>,
    pub subtypes: Vec<String>,
    pub type_line: String,
    pub text: String,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub base_power: Option<i32>,
    pub base_toughness: Option<i32>,
    pub loyalty: Option<u32>,
    pub damage: i32,
    pub counters: BTreeMap<String, u32>,
    pub keywords: Vec<String>,
    pub tapped: bool,
    pub summoning_sick: bool,
    pub controller: u8,
    pub owner: u8,
    pub is_commander: bool,
    pub is_token: bool,
    pub attacking: Option<u8>,
    pub blocking: Option<u32>,
    pub abilities: Vec<AbilityView>,
    /// Target descriptions for casting this card (empty if it doesn't target).
    pub spell_targets: Vec<String>,
    pub has_x: bool,
    pub unsupported: bool,
    pub zone: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeatView {
    pub id: u8,
    pub name: String,
    pub life: i32,
    pub poison: u32,
    pub hand_count: usize,
    pub library_count: usize,
    pub graveyard: Vec<CardView>,
    pub exile: Vec<CardView>,
    pub command: Vec<CardView>,
    /// Combat damage this player has taken from each commander (by commander name).
    pub commander_damage: BTreeMap<String, i32>,
    pub mana_pool: BTreeMap<String, u32>,
    pub color_identity: Vec<char>,
    pub is_you: bool,
    pub is_active: bool,
    pub has_priority: bool,
    pub lost: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct StackItemView {
    pub id: u32,
    pub controller: u8,
    pub name: String,
    pub is_ability: bool,
    pub text: String,
    pub targets: Vec<String>,
    pub card: Option<CardView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TriggerChoiceView {
    pub index: usize,
    pub source: String,
    pub text: String,
    pub targets: Vec<Vec<Target>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hints {
    pub playable_lands: Vec<u32>,
    pub castable: Vec<u32>,
    pub activatable: Vec<(u32, usize)>,
    pub attackers: Vec<u32>,
    pub blockers: Vec<u32>,
    /// For each castable card / activatable ability: the legal choices per target slot.
    pub cast_targets: BTreeMap<u32, Vec<Vec<Target>>>,
    pub ability_targets: BTreeMap<String, Vec<Vec<Target>>>,
    pub triggers: Vec<TriggerChoiceView>,
    pub discard_count: usize,
    pub mulligan_bottom: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlayerView {
    pub you: u8,
    pub turn: u32,
    pub active: u8,
    pub step: Step,
    pub decision: Decision,
    pub your_decision: bool,
    pub players: Vec<SeatView>,
    pub battlefield: Vec<CardView>,
    pub hand: Vec<CardView>,
    pub stack: Vec<StackItemView>,
    pub log: Vec<String>,
    pub hints: Hints,
    pub commander_return: bool,
}

fn type_names<T: std::fmt::Debug>(v: impl IntoIterator<Item = T>) -> Vec<String> {
    v.into_iter().map(|t| format!("{t:?}")).collect()
}

fn target_spec_label(s: &TargetSpec) -> String {
    match s {
        TargetSpec::Any => "any target".into(),
        TargetSpec::Player => "target player".into(),
        TargetSpec::Opponent => "target opponent".into(),
        TargetSpec::Permanent(f) if f.any_of_types == vec![CardType::Creature] => {
            "target creature".into()
        }
        TargetSpec::Permanent(_) => "target permanent".into(),
        TargetSpec::Spell(_) => "target spell".into(),
        TargetSpec::StackObject => "target spell or ability".into(),
        TargetSpec::CardInGraveyard(_) => "target card in a graveyard".into(),
    }
}

pub(crate) fn card_view(
    g: &Game,
    ch: &Characteristics,
    id: ObjectId,
    activatable: &[(ObjectId, usize)],
) -> CardView {
    let o = g.object(id).expect("object exists");
    let keywords: Vec<String> = ch
        .abilities
        .iter()
        .filter_map(|a| match &a.ability {
            Ability::Keyword(k) => Some(format!("{k:?}")),
            _ => None,
        })
        .collect();
    let abilities = ch
        .abilities
        .iter()
        .enumerate()
        .filter(|(_, a)| matches!(a.ability, Ability::Mana { .. } | Ability::Activated { .. }))
        .map(|(i, a)| AbilityView {
            index: i,
            label: describe_ability(&a.ability),
            is_mana: matches!(a.ability, Ability::Mana { .. }),
            activatable: activatable.contains(&(id, i)),
            targets: match &a.ability {
                Ability::Activated { targets, .. } => {
                    targets.iter().map(target_spec_label).collect()
                }
                _ => vec![],
            },
        })
        .collect();
    let mut type_line = String::new();
    let mut words = type_names(ch.supertypes.iter());
    words.extend(type_names(ch.types.iter()));
    type_line.push_str(&words.join(" "));
    if !ch.subtypes.is_empty() {
        type_line.push_str(" — ");
        type_line.push_str(&ch.subtypes.join(" "));
    }
    let counters = o
        .counters
        .iter()
        .map(|(k, v)| {
            let name = match k {
                CounterKind::PlusOnePlusOne => "+1/+1".to_string(),
                CounterKind::MinusOneMinusOne => "-1/-1".to_string(),
                CounterKind::Loyalty => "loyalty".to_string(),
                CounterKind::Other(s) => s.clone(),
            };
            (name, *v)
        })
        .collect();
    CardView {
        id: id.0,
        name: ch.name.clone(),
        mana_cost: ch.mana_cost.to_string(),
        mana_value: ch.mana_value(),
        colors: ch.colors.iter().map(|c| c.letter()).collect(),
        supertypes: type_names(ch.supertypes.iter()),
        types: type_names(ch.types.iter()),
        subtypes: ch.subtypes.clone(),
        type_line,
        text: o.def.oracle_text.clone(),
        power: ch.power,
        toughness: ch.toughness,
        base_power: o.def.power,
        base_toughness: o.def.toughness,
        loyalty: o.counters.get(&CounterKind::Loyalty).copied(),
        damage: o.damage,
        counters,
        keywords,
        tapped: o.tapped,
        summoning_sick: o.summoning_sick,
        controller: ch.controller.0,
        owner: o.owner.0,
        is_commander: g.is_commander(id),
        is_token: o.is_token,
        attacking: g
            .combat
            .attackers
            .iter()
            .find(|(a, _)| *a == id)
            .map(|(_, d)| d.0),
        blocking: g
            .combat
            .blockers
            .iter()
            .find(|(b, _)| *b == id)
            .map(|(_, a)| a.0),
        abilities,
        spell_targets: o.def.spell_targets.iter().map(target_spec_label).collect(),
        has_x: o.def.mana_cost.has_x(),
        unsupported: !o.def.fully_supported(),
        zone: format!("{:?}", o.zone),
    }
}

fn target_label(g: &Game, view: &View, t: &Target) -> String {
    match t {
        Target::Player(p) => g.player(*p).name.clone(),
        Target::Object(id) => view
            .get(*id)
            .map(|c| c.name.clone())
            .or_else(|| {
                g.stack.iter().find(|e| e.id == *id).map(|e| match &e.kind {
                    StackKind::Ability { source_name, .. } => format!("{source_name} ability"),
                    StackKind::Spell => "spell".into(),
                })
            })
            .unwrap_or_else(|| "(gone)".into()),
    }
}

pub(crate) fn build(g: &Game, viewer: PlayerId) -> PlayerView {
    let view = g.view();
    let activatable = g.activatable(viewer);
    let cv = |id: &ObjectId| view.get(*id).map(|ch| card_view(g, ch, *id, &activatable));
    let decision = g.decision().clone();
    let priority = match &decision {
        Decision::Priority { player } => Some(*player),
        _ => None,
    };
    let players = g
        .players
        .iter()
        .map(|p| {
            let commander_damage = p
                .commander_damage
                .iter()
                .filter_map(|(card, d)| {
                    let name = g
                        .objects
                        .values()
                        .find(|o| o.card == *card)
                        .map(|o| o.def.name.clone())?;
                    Some((name, *d))
                })
                .collect();
            let mana_pool = ManaType::all()
                .iter()
                .filter(|t| p.mana_pool.get(**t) > 0)
                .map(|t| {
                    let k = match t {
                        ManaType::Color(c) => c.letter().to_string(),
                        ManaType::Colorless => "C".into(),
                    };
                    (k, p.mana_pool.get(*t))
                })
                .collect();
            let identity = p
                .commanders
                .iter()
                .filter_map(|c| g.objects.values().find(|o| o.card == *c))
                .fold(ColorSet::EMPTY, |a, o| a.union(o.def.color_identity));
            SeatView {
                id: p.id.0,
                name: p.name.clone(),
                life: p.life,
                poison: p.poison,
                hand_count: p.hand.len(),
                library_count: p.library.len(),
                graveyard: p.graveyard.iter().filter_map(cv).collect(),
                exile: g
                    .exile
                    .iter()
                    .filter(|id| g.objects[id].owner == p.id)
                    .filter_map(cv)
                    .collect(),
                command: g
                    .command
                    .iter()
                    .filter(|id| g.objects[id].owner == p.id)
                    .filter_map(cv)
                    .collect(),
                commander_damage,
                mana_pool,
                color_identity: identity.iter().map(|c| c.letter()).collect(),
                is_you: p.id == viewer,
                is_active: p.id == g.active,
                has_priority: priority == Some(p.id),
                lost: p.has_lost || p.has_left,
            }
        })
        .collect();

    let stack = g
        .stack
        .iter()
        .rev()
        .map(|e| {
            let targets = e
                .targets
                .iter()
                .map(|t| target_label(g, &view, t))
                .collect();
            match &e.kind {
                StackKind::Spell => {
                    let card = cv(&e.id);
                    StackItemView {
                        id: e.id.0,
                        controller: e.controller.0,
                        name: card.as_ref().map(|c| c.name.clone()).unwrap_or_default(),
                        is_ability: false,
                        text: card.as_ref().map(|c| c.text.clone()).unwrap_or_default(),
                        targets,
                        card,
                    }
                }
                StackKind::Ability {
                    source_name,
                    ability,
                    ..
                } => StackItemView {
                    id: e.id.0,
                    controller: e.controller.0,
                    name: source_name.clone(),
                    is_ability: true,
                    text: describe_ability(ability),
                    targets,
                    card: None,
                },
            }
        })
        .collect();

    let mine = decision.player() == Some(viewer);
    let castable = g.castable(viewer);
    let cast_targets = castable
        .iter()
        .map(|id| {
            let specs = &g.object(*id).unwrap().def.spell_targets;
            (
                id.0,
                specs
                    .iter()
                    .map(|s| g.legal_targets(viewer, Some(*id), s))
                    .collect(),
            )
        })
        .collect();
    let ability_targets = activatable
        .iter()
        .filter_map(|(id, i)| {
            let ch = view.get(*id)?;
            match &ch.abilities[*i].ability {
                Ability::Activated { targets, .. } => Some((
                    format!("{}:{}", id.0, i),
                    targets
                        .iter()
                        .map(|s| g.legal_targets(viewer, Some(*id), s))
                        .collect(),
                )),
                _ => None,
            }
        })
        .collect();
    let triggers = if matches!(decision, Decision::PutTriggersOnStack { player, .. } if player == viewer)
    {
        g.pending_triggers_for(viewer)
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (targets, effects) = match &t.ability {
                    Ability::Triggered {
                        targets, effects, ..
                    } => (targets.clone(), effects.clone()),
                    _ => (vec![], vec![]),
                };
                TriggerChoiceView {
                    index: i,
                    source: t.source_name.clone(),
                    text: describe_effects(&effects),
                    targets: targets
                        .iter()
                        .map(|s| g.legal_targets(viewer, Some(t.source), s))
                        .collect(),
                }
            })
            .collect()
    } else {
        vec![]
    };
    let hints = Hints {
        playable_lands: g.playable_lands(viewer).iter().map(|i| i.0).collect(),
        castable: castable.iter().map(|i| i.0).collect(),
        activatable: activatable.iter().map(|(o, i)| (o.0, *i)).collect(),
        attackers: if matches!(decision, Decision::DeclareAttackers { player } if player == viewer)
        {
            g.possible_attackers().iter().map(|i| i.0).collect()
        } else {
            vec![]
        },
        blockers: if matches!(decision, Decision::DeclareBlockers { player } if player == viewer) {
            g.possible_blockers(viewer).iter().map(|i| i.0).collect()
        } else {
            vec![]
        },
        cast_targets,
        ability_targets,
        triggers,
        discard_count: match decision {
            Decision::Discard { player, count } if player == viewer => count,
            _ => 0,
        },
        mulligan_bottom: match decision {
            Decision::Mulligan {
                player,
                bottom_count_if_keep,
            } if player == viewer => bottom_count_if_keep,
            _ => 0,
        },
    };

    PlayerView {
        you: viewer.0,
        turn: g.turn,
        active: g.active.0,
        step: g.step,
        decision,
        your_decision: mine,
        players,
        battlefield: g.battlefield.iter().filter_map(cv).collect(),
        hand: g.player(viewer).hand.iter().filter_map(cv).collect(),
        stack,
        log: g.log.iter().rev().take(60).rev().cloned().collect(),
        hints,
        commander_return: g.player(viewer).return_commander_to_command_zone,
    }
}
