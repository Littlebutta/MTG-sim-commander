//! Reading MTGJSON's AtomicCards file (https://mtgjson.com/data-models/card/card-atomic/).
//!
//! AtomicCards maps each card name to its faces with oracle (not printing-specific) data,
//! which is exactly what a rules engine needs.

use crate::oracle;
use rules_core::{CardDef, CardType, Color, ColorSet, ManaCost, Supertype};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};

pub const META_URL: &str = "https://mtgjson.com/api/v5/Meta.json";
pub const ATOMIC_CARDS_URL: &str = "https://mtgjson.com/api/v5/AtomicCards.json.xz";
pub const ATOMIC_CARDS_SHA256_URL: &str = "https://mtgjson.com/api/v5/AtomicCards.json.xz.sha256";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, serde::Serialize)]
pub struct Meta {
    pub date: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct MetaFile {
    pub data: Meta,
}

#[derive(Debug, Deserialize)]
pub struct AtomicFile {
    pub meta: Meta,
    pub data: HashMap<String, Vec<AtomicCard>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AtomicCard {
    pub name: String,
    pub face_name: Option<String>,
    pub side: Option<String>,
    pub layout: String,
    pub mana_cost: Option<String>,
    pub colors: Vec<String>,
    pub color_identity: Vec<String>,
    pub color_indicator: Option<Vec<String>>,
    pub types: Vec<String>,
    pub supertypes: Vec<String>,
    pub subtypes: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub text: Option<String>,
    pub keywords: Vec<String>,
    pub legalities: HashMap<String, String>,
    pub leadership_skills: Option<LeadershipSkills>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LeadershipSkills {
    pub commander: bool,
}

/// Is `remote` newer than what we have? MTGJSON versions look like "5.3.0+20261112".
pub fn needs_update(local: Option<&Meta>, remote: &Meta) -> bool {
    match local {
        None => true,
        Some(l) => {
            (remote.date.as_str(), remote.version.as_str()) > (l.date.as_str(), l.version.as_str())
        }
    }
}

fn colors(v: &[String]) -> ColorSet {
    let cs: Vec<Color> = v
        .iter()
        .filter_map(|s| s.chars().next().and_then(Color::from_letter))
        .collect();
    ColorSet::of(&cs)
}

fn num(s: &Option<String>) -> Option<i32> {
    s.as_deref().and_then(|x| x.parse().ok())
}

/// Card-level data the deck builder needs in addition to the playable definition.
#[derive(Clone, Debug)]
pub struct CardRecord {
    pub def: CardDef,
    /// Other faces of multi-faced cards (back face of a DFC, adventure half...).
    pub other_faces: Vec<CardDef>,
    pub commander_legality: Option<String>,
    pub can_be_commander: bool,
    pub keywords: Vec<String>,
    pub layout: String,
}

pub fn convert_face(c: &AtomicCard) -> CardDef {
    let name = c.face_name.clone().unwrap_or_else(|| c.name.clone());
    let mana_cost = c
        .mana_cost
        .as_deref()
        .and_then(|m| ManaCost::parse(m).ok())
        .unwrap_or_default();
    let types: BTreeSet<CardType> = c.types.iter().filter_map(|t| CardType::parse(t)).collect();
    let supertypes: BTreeSet<Supertype> = c
        .supertypes
        .iter()
        .filter_map(|t| Supertype::parse(t))
        .collect();
    let text = c.text.clone().unwrap_or_default();
    let mut def = CardDef {
        name: name.clone(),
        color_indicator: c.color_indicator.as_ref().map(|v| colors(v)),
        mana_cost,
        supertypes,
        types,
        subtypes: c.subtypes.clone(),
        power: num(&c.power),
        toughness: num(&c.toughness),
        loyalty: num(&c.loyalty),
        oracle_text: text.clone(),
        color_identity: colors(&c.color_identity),
        ..Default::default()
    };
    if c.color_indicator.is_none()
        && !c.colors.is_empty()
        && def.mana_cost.colors() != colors(&c.colors)
    {
        def.color_indicator = Some(colors(&c.colors));
    }
    oracle::script(&mut def, &name, &text);
    def
}

/// Convert every card in an AtomicCards file.
pub fn convert_all(file: &AtomicFile) -> Vec<CardRecord> {
    let mut out = Vec::with_capacity(file.data.len());
    let mut names: Vec<&String> = file.data.keys().collect();
    names.sort();
    for name in names {
        let faces = &file.data[name];
        let Some(front) = faces
            .iter()
            .find(|f| f.side.as_deref().unwrap_or("a") == "a")
        else {
            continue;
        };
        let def = convert_face(front);
        let mut def = def;
        // Full card name ("A // B") is what decklists use for split/MDFC cards.
        def.name = name.clone();
        let other_faces = faces
            .iter()
            .filter(|f| !std::ptr::eq(*f, front))
            .map(convert_face)
            .collect();
        let can_be_commander = front
            .leadership_skills
            .as_ref()
            .is_some_and(|l| l.commander);
        out.push(CardRecord {
            def,
            other_faces,
            commander_legality: front.legalities.get("commander").cloned(),
            can_be_commander,
            keywords: front.keywords.clone(),
            layout: front.layout.clone(),
        });
    }
    out
}
