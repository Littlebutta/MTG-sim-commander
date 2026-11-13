//! Commander deck import and validation (CR 903.3–903.5).
//!
//! Accepts the plain-text format Moxfield, Archidekt and MTGO export:
//!
//! ```text
//! Commander
//! 1 Atraxa, Praetors' Voice
//!
//! Deck
//! 1 Sol Ring
//! 35 Forest
//! ```
//!
//! A `*CMDR*` tag after a line also marks a commander. Set codes in parentheses and
//! collector numbers after the name are ignored.

use crate::mtgjson::CardRecord;
use rules_core::{CardType, ColorSet, Supertype};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckList {
    pub commanders: Vec<(u32, String)>,
    pub cards: Vec<(u32, String)>,
}

pub fn parse_decklist(text: &str) -> DeckList {
    let mut deck = DeckList {
        commanders: vec![],
        cards: vec![],
    };
    let mut in_commander = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let lower = line.to_lowercase();
        let header = lower.trim_end_matches(':');
        if matches!(header, "commander" | "commanders") {
            in_commander = true;
            continue;
        }
        if matches!(
            header,
            "deck" | "mainboard" | "main" | "sideboard" | "maybeboard" | "companion"
        ) {
            in_commander = false;
            continue;
        }
        let (count, rest) = match line.split_once(' ') {
            Some((n, r)) if n.trim_end_matches('x').parse::<u32>().is_ok() => {
                (n.trim_end_matches('x').parse().unwrap(), r)
            }
            _ => (1, line),
        };
        let is_cmdr = rest.contains("*CMDR*");
        let mut name = rest.replace("*CMDR*", "");
        // Drop "(SET) 123" and foil markers.
        if let Some(i) = name.find(" (") {
            name.truncate(i);
        }
        let name = name.replace("*F*", "").trim().to_string();
        if name.is_empty() {
            continue;
        }
        if in_commander || is_cmdr {
            deck.commanders.push((count, name));
        } else {
            deck.cards.push((count, name));
        }
    }
    deck
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeckProblem {
    UnknownCard(String),
    WrongSize(u32),
    NoCommander,
    TooManyCommanders,
    NotACommander(String),
    IncompatiblePartners,
    NotSingleton(String, u32),
    OutsideColorIdentity(String),
    Banned(String),
}

#[derive(Clone, Debug, Default)]
pub struct DeckReport {
    pub problems: Vec<DeckProblem>,
    /// Cards whose text the engine can't fully run yet (playable in manual mode).
    pub unsupported: Vec<String>,
    pub color_identity: ColorSet,
}

impl DeckReport {
    pub fn is_legal(&self) -> bool {
        self.problems.is_empty()
    }
}

pub struct CardIndex<'a> {
    by_name: HashMap<String, &'a CardRecord>,
}

impl<'a> CardIndex<'a> {
    pub fn new(cards: &'a [CardRecord]) -> CardIndex<'a> {
        let mut by_name = HashMap::new();
        for c in cards {
            by_name.insert(c.def.name.to_lowercase(), c);
            // Let "Fire" find "Fire // Ice" and a front face find its MDFC.
            if let Some((front, _)) = c.def.name.split_once(" // ") {
                by_name.entry(front.to_lowercase()).or_insert(c);
            }
        }
        CardIndex { by_name }
    }
    pub fn get(&self, name: &str) -> Option<&'a CardRecord> {
        self.by_name.get(&name.trim().to_lowercase()).copied()
    }
}

fn is_commander_eligible(c: &CardRecord) -> bool {
    // 903.3: legendary creature, Vehicle or Spacecraft with P/T, or text that says so.
    let d = &c.def;
    c.can_be_commander
        || (d.supertypes.contains(&Supertype::Legendary)
            && (d.has_type(CardType::Creature)
                || d.subtypes.iter().any(|s| s == "Vehicle")
                || (d.subtypes.iter().any(|s| s == "Spacecraft") && d.power.is_some())))
        || d.oracle_text.contains("can be your commander")
}

fn has_kw(c: &CardRecord, kw: &str) -> bool {
    c.keywords.iter().any(|k| k.eq_ignore_ascii_case(kw)) || c.def.oracle_text.contains(kw)
}

fn partners_ok(a: &CardRecord, b: &CardRecord) -> bool {
    let partner_with = |x: &CardRecord, y: &CardRecord| {
        x.def
            .oracle_text
            .contains(&format!("Partner with {}", y.def.name))
    };
    (has_kw(a, "Partner")
        && has_kw(b, "Partner")
        && !a.def.oracle_text.contains("Partner with")
        && !b.def.oracle_text.contains("Partner with"))
        || (partner_with(a, b) && partner_with(b, a))
        || (has_kw(a, "Friends forever") && has_kw(b, "Friends forever"))
        || (has_kw(a, "Choose a Background") && b.def.subtypes.iter().any(|s| s == "Background"))
        || (has_kw(b, "Choose a Background") && a.def.subtypes.iter().any(|s| s == "Background"))
        || (has_kw(a, "Doctor's companion") && b.def.subtypes.iter().any(|s| s == "Doctor"))
        || (has_kw(b, "Doctor's companion") && a.def.subtypes.iter().any(|s| s == "Doctor"))
}

/// Validate a decklist against CR 903.5 and the Commander banned list.
pub fn validate(list: &DeckList, index: &CardIndex) -> DeckReport {
    let mut r = DeckReport::default();
    let mut commanders = Vec::new();
    for (_, name) in &list.commanders {
        match index.get(name) {
            Some(c) => commanders.push(c),
            None => r.problems.push(DeckProblem::UnknownCard(name.clone())),
        }
    }
    match commanders.len() {
        0 => r.problems.push(DeckProblem::NoCommander),
        1 | 2 => {}
        _ => r.problems.push(DeckProblem::TooManyCommanders),
    }
    for c in &commanders {
        if !is_commander_eligible(c) {
            r.problems
                .push(DeckProblem::NotACommander(c.def.name.clone()));
        }
    }
    if commanders.len() == 2 && !partners_ok(commanders[0], commanders[1]) {
        r.problems.push(DeckProblem::IncompatiblePartners);
    }
    r.color_identity = commanders
        .iter()
        .fold(ColorSet::EMPTY, |a, c| a.union(c.def.color_identity));

    let total: u32 = list
        .commanders
        .iter()
        .chain(&list.cards)
        .map(|(n, _)| n)
        .sum();
    if total != 100 {
        r.problems.push(DeckProblem::WrongSize(total));
    }
    let mut counts: HashMap<String, u32> = HashMap::new();
    for (n, name) in list.commanders.iter().chain(&list.cards) {
        let Some(c) = index.get(name) else {
            if !list.commanders.iter().any(|(_, x)| x == name) {
                r.problems.push(DeckProblem::UnknownCard(name.clone()));
            }
            continue;
        };
        *counts.entry(c.def.name.clone()).or_insert(0) += n;
        if !c.def.color_identity.is_subset_of(r.color_identity) {
            r.problems
                .push(DeckProblem::OutsideColorIdentity(c.def.name.clone()));
        }
        if c.commander_legality.as_deref() == Some("Banned") {
            r.problems.push(DeckProblem::Banned(c.def.name.clone()));
        }
        if !c.def.fully_supported() && !r.unsupported.contains(&c.def.name) {
            r.unsupported.push(c.def.name.clone());
        }
    }
    for (name, n) in counts {
        let c = index.get(&name).unwrap();
        let any_number = c.def.supertypes.contains(&Supertype::Basic)
            || c.def
                .oracle_text
                .contains("A deck can have any number of cards named");
        if n > 1 && !any_number {
            r.problems.push(DeckProblem::NotSingleton(name, n));
        }
    }
    r
}
