//! Deck builder: card search, deck statistics and suggestions for the deck you're building.
//!
//! Suggestions come from what the deck is short of (lands, ramp, card draw, removal, sweepers,
//! creatures, measured against common Commander guidelines), what the commander's text is about
//! (tokens, counters, a creature type...), and a short list of well-known staples. Cards the game
//! can fully run are preferred.

use crate::view::type_names;
use carddb::{parse_decklist, CardIndex, CardRecord};
use rules_core::CardType;
use rules_core::{Color, ColorSet, Supertype};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Land,
    Ramp,
    Draw,
    Removal,
    Wipe,
    Creature,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CardSummary {
    pub name: String,
    pub mana_cost: String,
    pub mana_value: u32,
    pub type_line: String,
    /// WUBRG letters.
    pub color_identity: String,
    pub oracle_text: String,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub can_be_commander: bool,
    /// The game runs all of its rules text.
    pub supported: bool,
    pub roles: Vec<Role>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct SearchQuery {
    /// Matched against name first, then type line and rules text.
    pub text: String,
    /// Only cards inside this color identity (WUBRG letters; "" for colorless only).
    pub identity: Option<String>,
    /// Card types every result must have, e.g. ["Creature"].
    pub types: Vec<String>,
    pub commanders_only: bool,
    /// Include cards banned in Commander (adventure mode's cauldron can brew anything).
    pub include_banned: bool,
    pub max_mv: Option<u32>,
    pub limit: Option<usize>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct DeckStats {
    pub cards: u32,
    pub lands: u32,
    pub ramp: u32,
    pub draw: u32,
    pub removal: u32,
    pub wipes: u32,
    pub creatures: u32,
    /// Nonland cards by mana value 0..=6, with 7+ in the last slot.
    pub curve: [u32; 8],
    pub avg_mv: f32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Advice {
    pub role: Option<Role>,
    pub have: u32,
    pub want: u32,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub card: CardSummary,
    pub role: Option<Role>,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Recommendations {
    pub commanders: Vec<String>,
    pub identity: String,
    pub stats: DeckStats,
    pub themes: Vec<String>,
    pub advice: Vec<Advice>,
    pub picks: Vec<Suggestion>,
}

const STAPLES: &[&str] = &[
    "Sol Ring",
    "Arcane Signet",
    "Command Tower",
    "Mind Stone",
    "Fellwar Stone",
    "Thought Vessel",
    "Commander's Sphere",
    "Cultivate",
    "Kodama's Reach",
    "Rampant Growth",
    "Farseek",
    "Nature's Lore",
    "Three Visits",
    "Llanowar Elves",
    "Birds of Paradise",
    "Swords to Plowshares",
    "Path to Exile",
    "Generous Gift",
    "Beast Within",
    "Chaos Warp",
    "Counterspell",
    "Swan Song",
    "Rhystic Study",
    "Mystic Remora",
    "Cyclonic Rift",
    "Rapid Hybridization",
    "Night's Whisper",
    "Sign in Blood",
    "Phyrexian Arena",
    "Read the Bones",
    "Toxic Deluge",
    "Damnation",
    "Wrath of God",
    "Blasphemous Act",
    "Vandalblast",
    "Harmonize",
    "Beast Whisperer",
    "Guardian Project",
    "Skullclamp",
    "Swiftfoot Boots",
    "Lightning Greaves",
    "Heroic Intervention",
    "Teferi's Protection",
    "Esper Sentinel",
    "Smothering Tithe",
    "Infernal Grasp",
    "Go for the Throat",
    "Feed the Swarm",
    "Krosan Grip",
    "Nature's Claim",
    "Return to Dust",
    "Exotic Orchard",
    "Path of Ancestry",
    "Reliquary Tower",
    "Evolving Wilds",
    "Terramorphic Expanse",
    "Divination",
    "Lightning Bolt",
    "Murder",
    "Doom Blade",
];

/// What a theme looks like in a commander's text, and in a card that supports it.
const THEMES: &[(&str, &[&str], &[&str])] = &[
    ("tokens", &["token"], &["create", "token"]),
    ("+1/+1 counters", &["+1/+1 counter"], &["+1/+1 counter"]),
    ("sacrifice", &["sacrifice"], &["sacrifice"]),
    ("graveyard", &["graveyard"], &["graveyard"]),
    ("artifacts", &["artifact"], &["artifact"]),
    ("enchantments", &["enchantment"], &["enchantment"]),
    (
        "instants and sorceries",
        &["instant or sorcery", "noncreature spell"],
        &["instant or sorcery", "noncreature spell"],
    ),
    (
        "life gain",
        &["gain life", "gains life", "lifelink"],
        &["gain", "life"],
    ),
    ("flying", &["flying"], &["flying"]),
    ("lands", &["landfall", "land enters"], &["land"]),
    ("attacking", &["attacks", "combat damage"], &["attacks"]),
];

fn letters(c: ColorSet) -> String {
    Color::ALL
        .iter()
        .filter(|x| c.contains(**x))
        .map(|x| x.letter())
        .collect()
}

fn parse_identity(s: &str) -> ColorSet {
    let mut c = ColorSet::EMPTY;
    for ch in s.chars() {
        if let Some(col) = Color::from_letter(ch.to_ascii_uppercase()) {
            c.insert(col);
        }
    }
    c
}

pub fn roles(r: &CardRecord) -> Vec<Role> {
    let d = &r.def;
    let t = d.oracle_text.to_lowercase();
    let mut out = Vec::new();
    if d.has_type(CardType::Land) {
        out.push(Role::Land);
        return out;
    }
    let has = |n: &[&str]| n.iter().any(|x| t.contains(x));
    if has(&[
        "add {",
        "add one mana",
        "add two mana",
        "put a land card from your hand onto the battlefield",
    ]) || (t.contains("search your library for") && t.contains("land card"))
    {
        out.push(Role::Ramp);
    }
    if has(&[
        "draw a card",
        "draw two",
        "draw three",
        "draw cards",
        "draws a card",
        "draw x",
    ]) {
        out.push(Role::Draw);
    }
    if has(&[
        "destroy all",
        "exile all",
        "damage to each creature",
        "all creatures get -",
        "each creature gets -",
    ]) {
        out.push(Role::Wipe);
    } else if has(&[
        "destroy target",
        "exile target",
        "counter target spell",
        "return target creature to its owner's hand",
        "return target nonland permanent",
        "damage to any target",
        "damage to target creature",
        "fights target",
        "target creature gets -",
    ]) {
        out.push(Role::Removal);
    }
    if d.has_type(CardType::Creature) {
        out.push(Role::Creature);
    }
    out
}

pub fn summary(r: &CardRecord) -> CardSummary {
    let d = &r.def;
    let mut words = type_names(d.supertypes.iter());
    words.extend(type_names(d.types.iter()));
    let mut type_line = words.join(" ");
    if !d.subtypes.is_empty() {
        type_line.push_str(" — ");
        type_line.push_str(&d.subtypes.join(" "));
    }
    CardSummary {
        name: d.name.clone(),
        mana_cost: d.mana_cost.to_string(),
        mana_value: d.mana_cost.mana_value(),
        type_line,
        color_identity: letters(d.color_identity),
        oracle_text: d.oracle_text.clone(),
        power: d.power,
        toughness: d.toughness,
        can_be_commander: is_commander(r),
        supported: d.fully_supported(),
        roles: roles(r),
    }
}

fn is_commander(r: &CardRecord) -> bool {
    let d = &r.def;
    r.can_be_commander
        || (d.supertypes.contains(&Supertype::Legendary) && d.has_type(CardType::Creature))
        || d.oracle_text.contains("can be your commander")
}

fn playable_in_commander(r: &CardRecord) -> bool {
    !r.def.is_token && r.commander_legality.as_deref() != Some("Banned")
}

/// Cards matching a search, best matches first.
pub fn search(pool: &[CardRecord], q: &SearchQuery) -> Vec<CardSummary> {
    let text = q.text.trim().to_lowercase();
    let identity = q.identity.as_deref().map(parse_identity);
    let mut hits: Vec<(u8, &CardRecord)> = pool
        .iter()
        .filter(|r| playable_in_commander(r) || (q.include_banned && !r.def.is_token))
        .filter(|r| !q.commanders_only || is_commander(r))
        .filter(|r| identity.is_none_or(|id| r.def.color_identity.is_subset_of(id)))
        .filter(|r| q.max_mv.is_none_or(|m| r.def.mana_cost.mana_value() <= m))
        .filter(|r| {
            q.types.iter().all(|t| {
                let names = type_names(r.def.types.iter());
                names.iter().any(|n| n.eq_ignore_ascii_case(t))
                    || r.def.subtypes.iter().any(|s| s.eq_ignore_ascii_case(t))
            })
        })
        .filter_map(|r| {
            if text.is_empty() {
                return Some((3, r));
            }
            let name = r.def.name.to_lowercase();
            if name == text {
                Some((0, r))
            } else if name.starts_with(&text) {
                Some((1, r))
            } else if name.contains(&text) {
                Some((2, r))
            } else {
                let s = summary_text(r);
                text.split_whitespace()
                    .all(|w| s.contains(w))
                    .then_some((4, r))
            }
        })
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.def.name.cmp(&b.1.def.name)));
    hits.dedup_by(|a, b| a.1.def.name == b.1.def.name);
    hits.into_iter()
        .take(q.limit.unwrap_or(60))
        .map(|(_, r)| summary(r))
        .collect()
}

/// Summaries for the named cards that exist, in the order given.
pub fn lookup(index: &CardIndex, names: &[String]) -> Vec<CardSummary> {
    names
        .iter()
        .filter_map(|n| index.get(n))
        .map(summary)
        .collect()
}

fn summary_text(r: &CardRecord) -> String {
    let mut s = r.def.oracle_text.to_lowercase();
    for t in type_names(r.def.types.iter()) {
        s.push(' ');
        s.push_str(&t.to_lowercase());
    }
    for t in &r.def.subtypes {
        s.push(' ');
        s.push_str(&t.to_lowercase());
    }
    s
}

fn wants(stats: &DeckStats) -> [(Role, u32, &'static str); 6] {
    let lands = if stats.avg_mv > 0.0 && stats.avg_mv < 2.6 {
        35
    } else {
        37
    };
    [
        (Role::Land, lands, "lands"),
        (Role::Ramp, 10, "ramp"),
        (Role::Draw, 10, "card draw"),
        (Role::Removal, 8, "removal spells"),
        (Role::Wipe, 2, "board wipes"),
        (Role::Creature, 20, "creatures"),
    ]
}

fn have(stats: &DeckStats, r: Role) -> u32 {
    match r {
        Role::Land => stats.lands,
        Role::Ramp => stats.ramp,
        Role::Draw => stats.draw,
        Role::Removal => stats.removal,
        Role::Wipe => stats.wipes,
        Role::Creature => stats.creatures,
    }
}

/// Statistics and suggestions for a decklist.
pub fn recommend(text: &str, pool: &[CardRecord], index: &CardIndex) -> Recommendations {
    let list = parse_decklist(text);
    let commanders: Vec<&CardRecord> = list
        .commanders
        .iter()
        .filter_map(|(_, n)| index.get(n))
        .collect();
    let cards: Vec<(u32, &CardRecord)> = list
        .cards
        .iter()
        .filter_map(|(n, name)| index.get(name).map(|r| (*n, r)))
        .collect();
    let in_deck: HashSet<&str> = commanders
        .iter()
        .map(|c| c.def.name.as_str())
        .chain(cards.iter().map(|(_, c)| c.def.name.as_str()))
        .collect();

    // Statistics.
    let mut stats = DeckStats::default();
    let mut mv_total = 0u32;
    let mut nonland = 0u32;
    let mut pips: BTreeMap<char, u32> = BTreeMap::new();
    for (n, r) in commanders
        .iter()
        .map(|c| (1u32, *c))
        .chain(cards.iter().copied())
    {
        stats.cards += n;
        for role in roles(r) {
            let slot = match role {
                Role::Land => &mut stats.lands,
                Role::Ramp => &mut stats.ramp,
                Role::Draw => &mut stats.draw,
                Role::Removal => &mut stats.removal,
                Role::Wipe => &mut stats.wipes,
                Role::Creature => &mut stats.creatures,
            };
            *slot += n;
        }
        if !r.def.has_type(CardType::Land) {
            let mv = r.def.mana_cost.mana_value();
            stats.curve[(mv as usize).min(7)] += n;
            mv_total += mv * n;
            nonland += n;
            for ch in r.def.mana_cost.to_string().chars() {
                if "WUBRG".contains(ch) {
                    *pips.entry(ch).or_default() += n;
                }
            }
        }
    }
    stats.avg_mv = if nonland > 0 {
        mv_total as f32 / nonland as f32
    } else {
        0.0
    };

    let identity = commanders
        .iter()
        .fold(ColorSet::EMPTY, |a, c| a.union(c.def.color_identity));
    let mut rec = Recommendations {
        commanders: commanders.iter().map(|c| c.def.name.clone()).collect(),
        identity: letters(identity),
        stats: stats.clone(),
        ..Default::default()
    };

    // Advice: what the deck is short of.
    for (role, want, label) in wants(&stats) {
        let h = have(&stats, role);
        let text = if h + 1 < want {
            match role {
                Role::Land => format!("{h} lands. Most Commander decks run about {want}."),
                Role::Creature => {
                    format!("{h} creatures. Around {want} keeps a board to attack and block with.")
                }
                _ => format!("{}: {h} so far. Aim for about {want}.", cap(label)),
            }
        } else if role == Role::Land && h > want + 3 {
            format!("{h} lands is a lot. Around {want} leaves more room for spells.")
        } else {
            continue;
        };
        rec.advice.push(Advice {
            role: Some(role),
            have: h,
            want,
            text,
        });
    }
    let total_pips: u32 = pips.values().sum();
    if total_pips > 0 && stats.lands > 0 && identity.len() > 1 {
        let parts: Vec<String> = pips
            .iter()
            .map(|(c, n)| {
                format!(
                    "{} {}",
                    (*n as f32 / total_pips as f32 * stats.lands as f32).round() as u32,
                    color_word(*c)
                )
            })
            .collect();
        rec.advice.push(Advice {
            role: None,
            have: 0,
            want: 0,
            text: format!(
                "For your mana symbols, your {} lands should make roughly: {}.",
                stats.lands,
                parts.join(", ")
            ),
        });
    }
    if stats.cards != 100 && !commanders.is_empty() {
        rec.advice.push(Advice {
            role: None,
            have: stats.cards,
            want: 100,
            text: if stats.cards < 100 {
                format!("{} more cards to reach 100.", 100 - stats.cards)
            } else {
                format!("{} cards over 100.", stats.cards - 100)
            },
        });
    }
    if commanders.is_empty() {
        rec.advice.insert(
            0,
            Advice {
                role: None,
                have: 0,
                want: 1,
                text: "Pick a commander first. It sets your colors and what to look for.".into(),
            },
        );
        return rec;
    }

    // Themes and creature types the commander cares about.
    let ctext: String = commanders
        .iter()
        .map(|c| c.def.oracle_text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let themes: Vec<&(&str, &[&str], &[&str])> = THEMES
        .iter()
        .filter(|(_, needles, _)| needles.iter().any(|n| ctext.contains(n)))
        .collect();
    let all_types: BTreeSet<&str> = pool
        .iter()
        .filter(|r| r.def.has_type(CardType::Creature))
        .flat_map(|r| r.def.subtypes.iter().map(String::as_str))
        .collect();
    let raw: String = commanders
        .iter()
        .map(|c| c.def.oracle_text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    let tribes: BTreeSet<String> = raw
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
        .map(|w| w.trim_end_matches('s').to_string())
        .filter(|w| {
            all_types.contains(w.as_str())
                && !commanders.iter().any(|c| c.def.name.contains(w.as_str()))
        })
        .collect();
    rec.themes = themes
        .iter()
        .map(|t| t.0.to_string())
        .chain(tribes.iter().cloned())
        .collect();

    // Score every card that could go in.
    let need = |r: Role| -> f32 {
        let (_, want, _) = wants(&stats).into_iter().find(|w| w.0 == r).unwrap();
        (want.saturating_sub(have(&stats, r))) as f32 / want as f32
    };
    let staples: HashSet<&str> = STAPLES.iter().copied().collect();
    let mut scored: Vec<(f32, Option<Role>, String, &CardRecord)> = Vec::new();
    for r in pool {
        let d = &r.def;
        if in_deck.contains(d.name.as_str())
            || !playable_in_commander(r)
            || !d.color_identity.is_subset_of(identity)
            || d.supertypes.contains(&Supertype::Basic)
            || d.mana_cost.mana_value() > 7
        {
            continue;
        }
        let rs = roles(r);
        let t = d.oracle_text.to_lowercase();
        let mut score = 0.0f32;
        let mut best: Option<(f32, Role)> = None;
        for &role in &rs {
            let w = match role {
                Role::Land => {
                    // Only lands that help this deck's colors.
                    let fixes = t.contains("any color")
                        || identity
                            .iter()
                            .any(|c| t.contains(&format!("{{{}}}", c.letter())));
                    if fixes {
                        2.0
                    } else {
                        0.0
                    }
                }
                Role::Creature => 1.2,
                _ => 3.0,
            } * need(role);
            score += w;
            if best.is_none_or(|b| w > b.0) {
                best = Some((w, role));
            }
        }
        let mut reason = best.filter(|b| b.0 > 0.0).map(|(_, role)| {
            let (_, want, label) = wants(&stats).into_iter().find(|w| w.0 == role).unwrap();
            format!(
                "{}: {} so far, aim for about {want}.",
                cap(label),
                have(&stats, role)
            )
        });
        let mut matched = Vec::new();
        for (label, _, signs) in &themes {
            let hit = if *label == "instants and sorceries" {
                d.has_type(CardType::Instant)
                    || d.has_type(CardType::Sorcery)
                    || signs.iter().any(|s| t.contains(s))
            } else {
                signs.iter().all(|s| t.contains(s))
            };
            if hit {
                matched.push(label.to_string());
            }
        }
        for tribe in &tribes {
            if d.subtypes.iter().any(|s| s == tribe) || t.contains(&tribe.to_lowercase()) {
                matched.push(tribe.clone());
                score += 0.8;
            }
        }
        score += 1.1 * matched.len().min(2) as f32;
        if !matched.is_empty() && (reason.is_none() || matched.len() > 1) {
            reason = Some(format!("Fits your commander's {} theme.", matched[0]));
        }
        if staples.contains(d.name.as_str()) {
            score += 1.6;
            reason.get_or_insert_with(|| "A Commander staple.".into());
        }
        let mv = d.mana_cost.mana_value() as f32;
        if rs.contains(&Role::Ramp) && mv > 3.0 {
            score -= 1.0;
        }
        score -= (mv - 4.0).max(0.0) * 0.35;
        if !d.fully_supported() {
            score *= 0.45;
        }
        if score <= 0.3 {
            continue;
        }
        let Some(reason) = reason else { continue };
        scored.push((score, best.map(|b| b.1), reason, r));
    }
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap()
            .then_with(|| a.3.def.name.cmp(&b.3.def.name))
    });

    // A few for each thing the deck is short of, then the best of the rest.
    let mut picked: HashSet<String> = HashSet::new();
    let mut per_role: BTreeMap<Option<Role>, usize> = BTreeMap::new();
    for (_, role, reason, r) in &scored {
        if rec.picks.len() >= 30 {
            break;
        }
        let n = per_role.entry(*role).or_default();
        if *n >= 6 || !picked.insert(r.def.name.clone()) {
            continue;
        }
        *n += 1;
        rec.picks.push(Suggestion {
            card: summary(r),
            role: *role,
            reason: reason.clone(),
        });
    }
    rec
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn color_word(c: char) -> &'static str {
    match c {
        'W' => "white",
        'U' => "blue",
        'B' => "black",
        'R' => "red",
        _ => "green",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::starter_cards;

    #[test]
    fn search_finds_by_name_and_respects_identity() {
        let pool = starter_cards();
        let hits = search(
            &pool,
            &SearchQuery {
                text: "sol ring".into(),
                ..Default::default()
            },
        );
        assert_eq!(hits[0].name, "Sol Ring");
        assert!(hits[0].roles.contains(&Role::Ramp));
        let green = search(
            &pool,
            &SearchQuery {
                identity: Some("G".into()),
                limit: Some(500),
                ..Default::default()
            },
        );
        assert!(green
            .iter()
            .all(|c| c.color_identity.chars().all(|ch| ch == 'G')));
        let cmdrs = search(
            &pool,
            &SearchQuery {
                commanders_only: true,
                limit: Some(500),
                ..Default::default()
            },
        );
        assert!(!cmdrs.is_empty() && cmdrs.iter().all(|c| c.can_be_commander));
    }

    #[test]
    fn banned_cards_only_show_when_asked_for() {
        let mut pool = starter_cards();
        let ring = pool.iter_mut().find(|r| r.def.name == "Sol Ring").unwrap();
        ring.commander_legality = Some("Banned".into());
        let q = |include_banned| SearchQuery {
            text: "sol ring".into(),
            include_banned,
            ..Default::default()
        };
        assert!(search(&pool, &q(false))
            .iter()
            .all(|c| c.name != "Sol Ring"));
        assert_eq!(search(&pool, &q(true))[0].name, "Sol Ring");
    }

    #[test]
    fn recommends_what_a_thin_deck_is_missing() {
        let pool = starter_cards();
        let index = CardIndex::new(&pool);
        let (_, text) = crate::starter_decklists().into_iter().next().unwrap();
        let list = parse_decklist(&text);
        // Keep the commander and only a handful of cards.
        let mut thin = String::from("Commander\n");
        for (n, c) in &list.commanders {
            thin.push_str(&format!("{n} {c}\n"));
        }
        thin.push_str("Deck\n");
        let spells: Vec<&(u32, String)> = list
            .cards
            .iter()
            .filter(|(_, c)| !matches!(c.as_str(), "Forest" | "Plains" | "Island"))
            .take(10)
            .collect();
        for (n, c) in &spells {
            thin.push_str(&format!("{n} {c}\n"));
        }
        let rec = recommend(&thin, &pool, &index);
        assert!(!rec.picks.is_empty());
        assert!(rec.advice.iter().any(|a| a.role == Some(Role::Land)));
        let in_deck: HashSet<String> = spells.iter().map(|(_, c)| c.clone()).collect();
        let id = parse_identity(&rec.identity);
        for p in &rec.picks {
            assert!(!in_deck.contains(&p.card.name));
            assert!(
                parse_identity(&p.card.color_identity).is_subset_of(id),
                "{} outside identity",
                p.card.name
            );
        }
        // Without a commander, it asks for one.
        let none = recommend("1 Sol Ring", &pool, &index);
        assert!(none.picks.is_empty() && none.advice[0].text.contains("commander"));
    }
}
