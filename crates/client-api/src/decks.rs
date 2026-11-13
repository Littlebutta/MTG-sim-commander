//! Player decks: checking a pasted decklist against the card pool and turning it into the
//! card definitions a game is built from.

use carddb::{parse_decklist, validate, CardIndex, DeckProblem};
use rules_core::{CardDef, Color};
use serde::{Deserialize, Serialize};

/// What the deck screen shows about a decklist.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeckCheck {
    /// Commander name(s) as found in the card pool.
    pub commanders: Vec<String>,
    /// Total cards including commanders.
    pub card_count: u32,
    /// Color identity as WUBRG letters, e.g. "WG".
    pub color_identity: String,
    /// Follows every Commander deck-building rule.
    pub legal: bool,
    /// Can be played at all: every card is known and there's a commander. Casual games allow
    /// decks that break the other rules (wrong size, duplicates...), shown as warnings.
    pub playable: bool,
    /// Rule problems, in plain English.
    pub problems: Vec<String>,
    /// Card names that aren't in the card pool.
    pub unknown: Vec<String>,
    /// Cards whose rules text the engine can't run yet; their unsupported parts do nothing.
    pub unsupported: Vec<String>,
}

/// A deck turned into card definitions, ready to seat in a game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedDeck {
    pub name: String,
    pub commanders: Vec<CardDef>,
    /// The other cards, one entry per copy.
    pub cards: Vec<CardDef>,
}

fn describe(p: &DeckProblem) -> String {
    match p {
        DeckProblem::UnknownCard(n) => format!("{n} isn't a card we know"),
        DeckProblem::WrongSize(n) => {
            format!("The deck has {n} cards; Commander decks have exactly 100")
        }
        DeckProblem::NoCommander => {
            "No commander: put it under a \"Commander\" heading or add *CMDR* after its name".into()
        }
        DeckProblem::TooManyCommanders => "More than two commanders".into(),
        DeckProblem::NotACommander(n) => format!("{n} can't be a commander"),
        DeckProblem::IncompatiblePartners => "Those two commanders can't be partners".into(),
        DeckProblem::NotSingleton(n, c) => format!("{c} copies of {n}; only one is allowed"),
        DeckProblem::OutsideColorIdentity(n) => {
            format!("{n} is outside your commander's color identity")
        }
        DeckProblem::Banned(n) => format!("{n} is banned in Commander"),
    }
}

/// Check a pasted decklist.
pub fn check_deck(text: &str, index: &CardIndex) -> DeckCheck {
    let list = parse_decklist(text);
    let report = validate(&list, index);
    let mut unknown: Vec<String> = Vec::new();
    for p in &report.problems {
        if let DeckProblem::UnknownCard(n) = p {
            if !unknown.contains(n) {
                unknown.push(n.clone());
            }
        }
    }
    let commanders: Vec<String> = list
        .commanders
        .iter()
        .filter_map(|(_, n)| index.get(n).map(|c| c.def.name.clone()))
        .collect();
    let playable = unknown.is_empty() && !commanders.is_empty() && commanders.len() <= 2;
    let color_identity = Color::ALL
        .iter()
        .filter(|c| report.color_identity.contains(**c))
        .map(|c| c.letter())
        .collect();
    let mut problems: Vec<String> = Vec::new();
    for p in report
        .problems
        .iter()
        .filter(|p| !matches!(p, DeckProblem::UnknownCard(_)))
    {
        let s = describe(p);
        if !problems.contains(&s) {
            problems.push(s);
        }
    }
    DeckCheck {
        commanders,
        card_count: list
            .commanders
            .iter()
            .chain(&list.cards)
            .map(|(n, _)| n)
            .sum(),
        color_identity,
        legal: report.is_legal(),
        playable,
        problems,
        unknown,
        unsupported: report.unsupported,
    }
}

/// Turn a decklist into card definitions. Fails if it can't be played at all.
pub fn resolve_deck(name: &str, text: &str, index: &CardIndex) -> Result<ResolvedDeck, String> {
    let check = check_deck(text, index);
    if !check.unknown.is_empty() {
        return Err(format!("Unknown cards: {}", check.unknown.join(", ")));
    }
    if !check.playable {
        return Err(check
            .problems
            .first()
            .cloned()
            .unwrap_or_else(|| "This deck can't be played".into()));
    }
    let list = parse_decklist(text);
    let get = |n: &str| index.get(n).map(|c| c.def.clone());
    Ok(ResolvedDeck {
        name: name.to_string(),
        commanders: list.commanders.iter().filter_map(|(_, n)| get(n)).collect(),
        cards: list
            .cards
            .iter()
            .flat_map(|(count, n)| {
                let d = get(n);
                (0..*count).filter_map(move |_| d.clone())
            })
            .collect(),
    })
}
