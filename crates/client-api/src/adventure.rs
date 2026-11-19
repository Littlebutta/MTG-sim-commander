//! Adventure mode decks. Adventure duels are one-on-one games at 20 life with no commander,
//! and only one deck-building rule: at most four copies of any card (basic lands are
//! unlimited). A deck also needs enough cards to last a duel.

use crate::decks::ResolvedDeck;
use carddb::{parse_decklist, CardIndex};
use rules_core::Supertype;
use serde::{Deserialize, Serialize};

/// Most copies of one card an adventure deck may hold.
pub const MAX_COPIES: u32 = 4;
/// Fewest cards an adventure deck may hold.
pub const MIN_CARDS: u32 = 40;
/// Starting life in an adventure duel.
pub const STARTING_LIFE: i32 = 20;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdventureDeckCheck {
    pub card_count: u32,
    /// Ready to duel with.
    pub playable: bool,
    /// Problems in plain words.
    pub problems: Vec<String>,
    pub unknown: Vec<String>,
    /// Cards whose rules text the engine can't fully run.
    pub unsupported: Vec<String>,
}

/// Every (count, name) in a list, commanders included: adventure decks have no command zone.
fn entries(text: &str) -> Vec<(u32, String)> {
    let list = parse_decklist(text);
    list.commanders.into_iter().chain(list.cards).collect()
}

pub fn check_adventure_deck(text: &str, index: &CardIndex) -> AdventureDeckCheck {
    let mut r = AdventureDeckCheck::default();
    let mut counts: Vec<(String, u32, bool)> = Vec::new();
    for (n, name) in entries(text) {
        r.card_count += n;
        let Some(c) = index.get(&name) else {
            if !r.unknown.contains(&name) {
                r.unknown.push(name);
            }
            continue;
        };
        if !c.def.fully_supported() && !r.unsupported.contains(&c.def.name) {
            r.unsupported.push(c.def.name.clone());
        }
        let basic = c.def.supertypes.contains(&Supertype::Basic);
        match counts.iter_mut().find(|(x, _, _)| *x == c.def.name) {
            Some(e) => e.1 += n,
            None => counts.push((c.def.name.clone(), n, basic)),
        }
    }
    for (name, n, basic) in &counts {
        if !basic && *n > MAX_COPIES {
            r.problems.push(format!(
                "{n} copies of {name}; a deck may carry at most {MAX_COPIES}"
            ));
        }
    }
    if r.card_count < MIN_CARDS {
        r.problems.push(format!(
            "{} cards; a deck needs at least {MIN_CARDS} to last a duel",
            r.card_count
        ));
    }
    r.playable = r.unknown.is_empty() && r.problems.is_empty();
    r
}

/// Turn an adventure decklist into card definitions. Fails if it can't be played.
pub fn resolve_adventure_deck(
    name: &str,
    text: &str,
    index: &CardIndex,
) -> Result<ResolvedDeck, String> {
    let check = check_adventure_deck(text, index);
    if !check.unknown.is_empty() {
        return Err(format!("Unknown cards: {}", check.unknown.join(", ")));
    }
    if let Some(p) = check.problems.first() {
        return Err(p.clone());
    }
    Ok(ResolvedDeck {
        name: name.to_string(),
        commanders: Vec::new(),
        cards: entries(text)
            .iter()
            .flat_map(|(count, n)| {
                let d = index.get(n).map(|c| c.def.clone());
                (0..*count).filter_map(move |_| d.clone())
            })
            .collect(),
    })
}
