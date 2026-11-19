//! Turning oracle text into executable card scripts.
//!
//! This covers common templated wording. Anything it doesn't recognise becomes
//! `Ability::Unsupported(line)`, which marks the card as needing a hand-written script (or
//! manual play) rather than silently doing the wrong thing.

use rules_core::*;
use std::sync::Arc;

/// Fill `def.abilities` / `def.spell_*` from oracle text.
pub fn script(def: &mut CardDef, name: &str, text: &str) {
    let text = normalize(text, name);
    let is_spell = def.has_type(CardType::Instant) || def.has_type(CardType::Sorcery);
    let mut spell_ok = true;
    let mut spell_targets = Vec::new();
    let mut spell_effects = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let line = strip_ability_word(line);
        // "Enchant creature": an Aura spell targets what it will enchant (CR 303.4a).
        if let Some(what) = line.strip_prefix("Enchant ") {
            match target_spec(&format!("target {}", what.to_lowercase())) {
                Some(t) if !is_spell && def.spell_targets.is_empty() => def.spell_targets = vec![t],
                _ => def.abilities.push(Ability::Unsupported(line.to_string())),
            }
            continue;
        }
        if is_spell {
            if let Some(a) = parse_self_rule(&line.to_lowercase()) {
                def.abilities.push(a);
                continue;
            }
            match parse_effects(line, &mut spell_targets) {
                Some(mut es) => spell_effects.append(&mut es),
                None => {
                    spell_ok = false;
                    def.abilities.push(Ability::Unsupported(line.to_string()));
                }
            }
            continue;
        }
        match parse_ability_line(line) {
            Some(mut abilities) => def.abilities.append(&mut abilities),
            None => def.abilities.push(Ability::Unsupported(line.to_string())),
        }
    }
    if is_spell && spell_ok {
        def.spell_targets = spell_targets;
        def.spell_effects = spell_effects;
    }
}

/// "Landfall — Whenever a land..." reads the same without the ability word (CR 207.2c).
fn strip_ability_word(line: &str) -> &str {
    if let Some((word, rest)) = line.split_once(" — ") {
        let plain = word
            .chars()
            .all(|c| c.is_alphabetic() || c == ' ' || c == '\'' || c == '-');
        if plain && word.split(' ').count() <= 4 && !word.starts_with("Choose") {
            return rest;
        }
    }
    line
}

/// Strip reminder text and refer to the card itself as "~".
pub fn normalize(text: &str, name: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0;
    for ch in text.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    let mut s = out.replace(name, "~");
    // Legendary short names: "Urborg, Tomb of Yawgmoth" is "Urborg" in its own text.
    if let Some((short, _)) = name.split_once(',') {
        s = s.replace(short, "~");
    }
    for t in [
        "this creature",
        "this artifact",
        "this enchantment",
        "this land",
        "this permanent",
        "this spell",
        "this aura",
        "this equipment",
        "this vehicle",
        "this card",
        "this aura",
        "this Aura",
        "this Equipment",
        "this Vehicle",
    ] {
        s = s.replace(t, "~").replace(&capitalize(t), "~");
    }
    // Older wording, same rules meaning.
    s = s
        .replace(" enters the battlefield", " enters")
        .replace("−", "-");
    s.lines()
        .map(|l| l.trim().trim_end_matches(' ').to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn number(w: &str) -> Option<i32> {
    Some(match w {
        "a" | "an" | "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "x" => return None,
        n => n.parse().ok()?,
    })
}

fn amount(w: &str) -> Option<Amount> {
    if w == "x" {
        Some(Amount::X)
    } else {
        number(w).map(Amount::Fixed)
    }
}

fn parse_ability_line(line: &str) -> Option<Vec<Ability>> {
    let l = line.to_lowercase();
    if let Some(a) = parse_self_rule(&l) {
        return Some(vec![a]);
    }
    // "Equip {2}" (CR 702.6a): attach to target creature you control, as a sorcery.
    if let Some(c) = l.strip_prefix("equip ").filter(|c| c.starts_with('{')) {
        return Some(vec![Ability::Activated {
            cost: Cost {
                mana: ManaCost::parse(c).ok()?,
                ..Default::default()
            },
            targets: vec![TargetSpec::Permanent(Filter::creatures().yours())],
            effects: vec![Effect::Attach { target: 0 }],
            sorcery_speed: true,
        }]);
    }
    if let Some(ks) = parse_keyword_line(line) {
        return Some(ks);
    }
    // Loyalty abilities: "+1: ...", "-2: ...", "0: ...".
    if let Some((n, effect)) = l.split_once(": ") {
        let signed = n.starts_with('+') || n.starts_with('-') || n == "0";
        if signed {
            let loyalty: i32 = n.trim_start_matches('+').parse().ok()?;
            let mut targets = Vec::new();
            let effects = parse_effects(effect, &mut targets)?;
            return Some(vec![Ability::Activated {
                cost: Cost {
                    loyalty: Some(loyalty),
                    ..Default::default()
                },
                targets,
                effects,
                sorcery_speed: true,
            }]);
        }
    }
    if let Some(abilities) = parse_trigger(&l) {
        return Some(abilities);
    }
    parse_ability_rest(&l)
}

/// Rules about the card itself: "~ can't block", "~ enters tapped", "~ can't be countered".
fn parse_self_rule(l: &str) -> Option<Ability> {
    let l = l.trim_end_matches('.');
    Some(match l {
        "~ can't block" => Ability::Keyword(Keyword::CantBlock),
        "~ can't attack" => Ability::Keyword(Keyword::CantAttack),
        "~ can't be blocked" => Ability::Keyword(Keyword::CantBeBlocked),
        "~ can't be countered" => Ability::Keyword(Keyword::CantBeCountered),
        "~ enters tapped" => Ability::EntersTapped,
        "you have no maximum hand size" => Ability::NoMaxHandSize,
        _ => {
            let rest = l.strip_prefix("~ enters with ")?;
            let (n, rest) = rest.split_once(' ')?;
            let kind = match rest {
                "+1/+1 counter on it" | "+1/+1 counters on it" => CounterKind::PlusOnePlusOne,
                _ => return None,
            };
            Ability::EntersWithCounters {
                kind,
                amount: amount(n)?,
            }
        }
    })
}

/// The start of a triggered ability ("whenever ~ attacks, ") and what it triggers on. Some
/// wordings stand for two triggers ("whenever ~ enters or attacks").
fn trigger_prefix(l: &str) -> Option<(Vec<TriggerEvent>, &str)> {
    let creature_of = |f: Filter| Filter {
        any_of_types: vec![CardType::Creature],
        ..f
    };
    let lands_yours = Filter::of_type(CardType::Land).yours();
    let opp = |f: Filter| Filter {
        controller: Some(PlayerRef::Opponents),
        ..f
    };
    let nontoken = |mut f: Filter| {
        f.nontoken = true;
        f
    };
    let table: Vec<(&str, Vec<TriggerEvent>)> = vec![
        (
            "when ~ enters, ",
            vec![TriggerEvent::EntersBattlefield(Filter::this())],
        ),
        (
            "whenever ~ enters, ",
            vec![TriggerEvent::EntersBattlefield(Filter::this())],
        ),
        (
            "whenever ~ enters or attacks, ",
            vec![
                TriggerEvent::EntersBattlefield(Filter::this()),
                TriggerEvent::Attacks(Filter::this()),
            ],
        ),
        (
            "when ~ enters or dies, ",
            vec![
                TriggerEvent::EntersBattlefield(Filter::this()),
                TriggerEvent::Dies(Filter::this()),
            ],
        ),
        (
            "whenever ~ attacks, ",
            vec![TriggerEvent::Attacks(Filter::this())],
        ),
        ("when ~ dies, ", vec![TriggerEvent::Dies(Filter::this())]),
        (
            "whenever ~ deals combat damage to a player, ",
            vec![TriggerEvent::DealsCombatDamageToPlayer(Filter::this())],
        ),
        (
            "whenever a creature you control deals combat damage to a player, ",
            vec![TriggerEvent::DealsCombatDamageToPlayer(
                Filter::creatures().yours(),
            )],
        ),
        (
            "whenever equipped creature deals combat damage to a player, ",
            vec![TriggerEvent::DealsCombatDamageToPlayer(Filter {
                attached_to_this: true,
                ..Default::default()
            })],
        ),
        (
            "whenever enchanted creature deals combat damage to a player, ",
            vec![TriggerEvent::DealsCombatDamageToPlayer(Filter {
                attached_to_this: true,
                ..Default::default()
            })],
        ),
        (
            "whenever a creature you control attacks, ",
            vec![TriggerEvent::Attacks(Filter::creatures().yours())],
        ),
        (
            "whenever equipped creature attacks, ",
            vec![TriggerEvent::Attacks(Filter {
                attached_to_this: true,
                ..Default::default()
            })],
        ),
        (
            "whenever another creature you control dies, ",
            vec![TriggerEvent::Dies(Filter::creatures().yours().other())],
        ),
        (
            "whenever another nontoken creature you control dies, ",
            vec![TriggerEvent::Dies(nontoken(
                Filter::creatures().yours().other(),
            ))],
        ),
        (
            "whenever a nontoken creature you control dies, ",
            vec![TriggerEvent::Dies(nontoken(Filter::creatures().yours()))],
        ),
        (
            "whenever another creature dies, ",
            vec![TriggerEvent::Dies(Filter::creatures().other())],
        ),
        (
            "whenever another nontoken creature dies, ",
            vec![TriggerEvent::Dies(nontoken(Filter::creatures().other()))],
        ),
        (
            "whenever a creature you control dies, ",
            vec![TriggerEvent::Dies(Filter::creatures().yours())],
        ),
        (
            "whenever a creature an opponent controls dies, ",
            vec![TriggerEvent::Dies(opp(Filter::creatures()))],
        ),
        (
            "whenever another creature you control enters, ",
            vec![TriggerEvent::EntersBattlefield(
                Filter::creatures().yours().other(),
            )],
        ),
        (
            "whenever another nontoken creature you control enters, ",
            vec![TriggerEvent::EntersBattlefield(nontoken(
                Filter::creatures().yours().other(),
            ))],
        ),
        (
            "whenever a nontoken creature you control enters, ",
            vec![TriggerEvent::EntersBattlefield(nontoken(
                Filter::creatures().yours(),
            ))],
        ),
        (
            "whenever a creature you control enters, ",
            vec![TriggerEvent::EntersBattlefield(Filter::creatures().yours())],
        ),
        (
            "whenever a creature enters under your control, ",
            vec![TriggerEvent::EntersBattlefield(Filter::creatures().yours())],
        ),
        (
            "whenever another creature enters, ",
            vec![TriggerEvent::EntersBattlefield(Filter::creatures().other())],
        ),
        (
            "whenever a creature enters, ",
            vec![TriggerEvent::EntersBattlefield(Filter::creatures())],
        ),
        (
            "whenever a creature an opponent controls enters, ",
            vec![TriggerEvent::EntersBattlefield(opp(Filter::creatures()))],
        ),
        (
            "whenever a land you control enters, ",
            vec![TriggerEvent::EntersBattlefield(lands_yours.clone())],
        ),
        (
            "whenever a land enters under your control, ",
            vec![TriggerEvent::EntersBattlefield(lands_yours.clone())],
        ),
        (
            "whenever an artifact you control enters, ",
            vec![TriggerEvent::EntersBattlefield(
                Filter::of_type(CardType::Artifact).yours(),
            )],
        ),
        (
            "whenever an enchantment you control enters, ",
            vec![TriggerEvent::EntersBattlefield(
                Filter::of_type(CardType::Enchantment).yours(),
            )],
        ),
        (
            "at the beginning of your upkeep, ",
            vec![TriggerEvent::BeginningOfUpkeep(PlayerRef::You)],
        ),
        (
            "at the beginning of each upkeep, ",
            vec![TriggerEvent::BeginningOfUpkeep(PlayerRef::Each)],
        ),
        (
            "at the beginning of each player's upkeep, ",
            vec![TriggerEvent::BeginningOfUpkeep(PlayerRef::Each)],
        ),
        (
            "at the beginning of each opponent's upkeep, ",
            vec![TriggerEvent::BeginningOfUpkeep(PlayerRef::Opponents)],
        ),
        (
            "at the beginning of your end step, ",
            vec![TriggerEvent::BeginningOfEndStep(PlayerRef::You)],
        ),
        (
            "at the beginning of each end step, ",
            vec![TriggerEvent::BeginningOfEndStep(PlayerRef::Each)],
        ),
        (
            "whenever you cast a spell, ",
            vec![TriggerEvent::YouCastSpell(Filter::default())],
        ),
        (
            "whenever you cast an instant or sorcery spell, ",
            vec![TriggerEvent::YouCastSpell(Filter {
                any_of_types: vec![CardType::Instant, CardType::Sorcery],
                ..Default::default()
            })],
        ),
        (
            "whenever you cast a noncreature spell, ",
            vec![TriggerEvent::YouCastSpell(Filter {
                not_types: vec![CardType::Creature],
                ..Default::default()
            })],
        ),
        (
            "whenever you cast a creature spell, ",
            vec![TriggerEvent::YouCastSpell(Filter::creatures())],
        ),
        (
            "whenever you cast an artifact spell, ",
            vec![TriggerEvent::YouCastSpell(Filter::of_type(
                CardType::Artifact,
            ))],
        ),
        (
            "whenever you cast an enchantment spell, ",
            vec![TriggerEvent::YouCastSpell(Filter::of_type(
                CardType::Enchantment,
            ))],
        ),
    ];
    let _ = creature_of;
    for (prefix, events) in table {
        if let Some(rest) = l.strip_prefix(prefix) {
            return Some((events, rest));
        }
    }
    // "whenever you cast a goblin spell", "whenever another elf you control enters"
    for (pre, post) in [
        ("whenever you cast a ", " spell, "),
        ("whenever you cast an ", " spell, "),
    ] {
        if let Some(r) = l.strip_prefix(pre) {
            if let Some((what, rest)) = r.split_once(post) {
                if !what.contains(' ') && !what.starts_with("non") {
                    let f = Filter {
                        subtypes: vec![capitalize(what)],
                        ..Default::default()
                    };
                    return Some((vec![TriggerEvent::YouCastSpell(f)], rest));
                }
            }
        }
    }
    for (pre, other) in [
        ("whenever another ", true),
        ("whenever a ", false),
        ("whenever an ", false),
    ] {
        if let Some(r) = l.strip_prefix(pre) {
            for (post, enters) in [
                (" you control enters, ", true),
                (" you control dies, ", false),
            ] {
                if let Some((what, rest)) = r.split_once(post) {
                    let mut f = group_singular(what)?.yours();
                    f.other_than_this = other;
                    let ev = if enters {
                        TriggerEvent::EntersBattlefield(f)
                    } else {
                        TriggerEvent::Dies(f)
                    };
                    return Some((vec![ev], rest));
                }
            }
        }
    }
    None
}

fn parse_trigger(l: &str) -> Option<Vec<Ability>> {
    let (events, rest) = trigger_prefix(l)?;
    let (optional, rest) = match rest.strip_prefix("you may ") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let mut targets = Vec::new();
    let effects = parse_effects(rest, &mut targets)?;
    Some(
        events
            .into_iter()
            .map(|event| Ability::Triggered {
                event,
                targets: targets.clone(),
                effects: effects.clone(),
                optional,
            })
            .collect(),
    )
}

fn parse_ability_rest(l: &str) -> Option<Vec<Ability>> {
    let l = l.to_string();
    // Static anthems.
    if let Some(a) = parse_static(&l) {
        return Some(vec![a]);
    }
    // Activated abilities: "<cost>: <effect>".
    if let Some((cost, effect)) = l.split_once(": ") {
        let cost = parse_cost(cost)?;
        if let Some(m) = parse_mana_production(effect) {
            return Some(vec![Ability::Mana { cost, produces: m }]);
        }
        if let Some((add, hurt)) = effect.split_once(". ~ deals ") {
            if let (Some(m), Some(n)) = (
                parse_mana_production(add),
                hurt.strip_suffix(" damage to you.").and_then(number),
            ) {
                let mut cost = cost;
                cost.deals_damage_to_you = n as u32;
                return Some(vec![Ability::Mana { cost, produces: m }]);
            }
        }
        let (effect, sorcery_speed) = match effect.strip_suffix(" activate only as a sorcery.") {
            Some(e) => (e, true),
            None => (effect, false),
        };
        let mut targets = Vec::new();
        let effects = parse_effects(effect, &mut targets)?;
        return Some(vec![Ability::Activated {
            cost,
            targets,
            effects,
            sorcery_speed,
        }]);
    }
    None
}

fn parse_keyword_line(line: &str) -> Option<Vec<Ability>> {
    let parts: Vec<&str> = line
        .trim_end_matches('.')
        .split([',', ';'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let known: Vec<Option<Keyword>> = parts.iter().map(|p| Keyword::parse(p)).collect();
    if known.iter().all(Option::is_some) {
        return Some(known.into_iter().flatten().map(Ability::Keyword).collect());
    }
    // A keyword list with one we don't know ("Flying, ward {2}"): keep the ones we do, so the
    // creature doesn't lose flying along with the rest, and flag only the unknown part.
    let listlike = known.iter().any(Option::is_some)
        && parts
            .iter()
            .all(|p| p.split_whitespace().count() <= 4 && !p.contains('~') && !p.contains(':'));
    if !listlike {
        return None;
    }
    Some(
        parts
            .iter()
            .zip(known)
            .map(|(p, k)| match k {
                Some(k) => Ability::Keyword(k),
                None => Ability::Unsupported(capitalize(p)),
            })
            .collect(),
    )
}

fn parse_cost(s: &str) -> Option<Cost> {
    let mut cost = Cost::default();
    let mut mana = String::new();
    for part in s.split(", ") {
        let p = part.trim();
        if p == "{t}" {
            cost.tap = true;
        } else if p.starts_with('{') {
            mana.push_str(p);
        } else if p == "sacrifice ~" {
            cost.sacrifice_this = true;
        } else if let Some(n) = p.strip_prefix("pay ").and_then(|r| r.strip_suffix(" life")) {
            cost.pay_life = number(n)? as u32;
        } else {
            return None;
        }
    }
    if !mana.is_empty() {
        cost.mana = ManaCost::parse(&mana).ok()?;
    }
    Some(cost)
}

fn mana_symbol(s: &str) -> Option<ManaType> {
    match s.trim() {
        "{c}" => Some(ManaType::Colorless),
        x if x.len() == 3 && x.starts_with('{') => {
            Color::from_letter(x.chars().nth(1)?).map(ManaType::Color)
        }
        _ => None,
    }
}

fn parse_mana_production(s: &str) -> Option<ManaProduction> {
    let rest = s.strip_prefix("add ")?.trim_end_matches('.');
    if rest == "one mana of any color" {
        return Some(ManaProduction::AnyColor);
    }
    if rest == "one mana of any color in your commander's color identity" {
        return Some(ManaProduction::CommanderIdentity);
    }
    if rest.contains(" or ") {
        let opts: Option<Vec<ManaType>> = rest
            .replace(", or ", ", ")
            .replace(" or ", ", ")
            .split(", ")
            .map(mana_symbol)
            .collect();
        return opts.map(ManaProduction::OneOf);
    }
    let mut v = Vec::new();
    let mut r = rest;
    while let Some(end) = r.find('}') {
        v.push(mana_symbol(&r[..=end])?);
        r = &r[end + 1..];
    }
    if v.is_empty() || !r.trim().is_empty() {
        return None;
    }
    Some(ManaProduction::Fixed(v))
}

fn parse_pt_mod(s: &str) -> Option<(i32, i32)> {
    let (p, t) = s.split_once('/')?;
    Some((
        p.trim_start_matches('+').parse().ok()?,
        t.trim_start_matches('+').parse().ok()?,
    ))
}

fn parse_static(l: &str) -> Option<Ability> {
    let l = l.trim_end_matches('.');
    // "You control enchanted creature." (Control Magic)
    if l == "you control enchanted creature" || l == "you control enchanted permanent" {
        return Some(Ability::Static {
            affected: attached(),
            mods: vec![Modification::ControlBySourceController],
        });
    }
    for (subject, verb) in split_subject(l) {
        let affected = if matches!(
            subject,
            "enchanted creature"
                | "equipped creature"
                | "enchanted permanent"
                | "enchanted land"
                | "enchanted artifact"
        ) {
            attached()
        } else if subject == "~" {
            Filter::this()
        } else if let Some(f) = group_plural(subject) {
            f
        } else {
            continue;
        };
        let mods = parse_grants(verb)?;
        return Some(Ability::Static { affected, mods });
    }
    None
}

fn attached() -> Filter {
    Filter {
        attached_to_this: true,
        ..Default::default()
    }
}

/// Split "creatures you control get +1/+1" at each verb that could start the predicate.
fn split_subject(l: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    for v in [" gets ", " get ", " has ", " have ", " can't "] {
        let mut from = 0;
        while let Some(i) = l[from..].find(v) {
            let at = from + i;
            out.push((&l[..at], &l[at + 1..]));
            from = at + 1;
        }
    }
    out.sort_by_key(|(s, _)| s.len());
    out
}

/// "gets +2/+2 and has trample and lifelink", "has first strike, vigilance, and lifelink",
/// "can't attack or block", "can't be blocked and has shroud".
fn parse_grants(rest: &str) -> Option<Vec<Modification>> {
    let mut mods = Vec::new();
    let mut verb = "";
    for item in rest.split(" and ").flat_map(|x| x.split(", ")) {
        let item = item.trim().trim_end_matches(',').trim();
        let item = item.strip_prefix("and ").unwrap_or(item);
        if item.is_empty() {
            continue;
        }
        let (v, what) = match item.split_once(' ') {
            Some((v @ ("gets" | "get" | "has" | "have"), w)) => (v, w),
            _ => (verb, item),
        };
        verb = v;
        match v {
            "gets" | "get" => {
                let (p, t) = parse_pt_mod(what)?;
                mods.push(Modification::ModifyPT(p, t));
            }
            "has" | "have" => mods.extend(
                restriction(what)
                    .or_else(|| Some(vec![Keyword::parse(what)?]))?
                    .into_iter()
                    .map(Modification::AddKeyword),
            ),
            _ => mods.extend(restriction(what)?.into_iter().map(Modification::AddKeyword)),
        }
    }
    if mods.is_empty() {
        None
    } else {
        Some(mods)
    }
}

fn restriction(s: &str) -> Option<Vec<Keyword>> {
    Some(match s {
        "can't attack or block" => vec![Keyword::CantAttack, Keyword::CantBlock],
        "can't block" => vec![Keyword::CantBlock],
        "can't attack" => vec![Keyword::CantAttack],
        "can't be blocked" => vec![Keyword::CantBeBlocked],
        _ => return None,
    })
}

fn singular_type(w: &str) -> Option<CardType> {
    Some(match w {
        "creature" => CardType::Creature,
        "artifact" => CardType::Artifact,
        "enchantment" => CardType::Enchantment,
        "land" => CardType::Land,
        "planeswalker" => CardType::Planeswalker,
        "battle" => CardType::Battle,
        "instant" => CardType::Instant,
        "sorcery" => CardType::Sorcery,
        _ => return None,
    })
}

fn plural_type(w: &str) -> Option<CardType> {
    singular_type(w.strip_suffix('s')?)
}

const NOT_SUBTYPES: &[&str] = &[
    "attacking",
    "blocking",
    "another",
    "other",
    "target",
    "each",
    "all",
    "enchanted",
    "equipped",
    "chosen",
    "that",
    "this",
    "token",
    "tokens",
    "card",
    "cards",
    "spell",
    "spells",
    "with",
    "without",
    "your",
    "you",
    "control",
    "controls",
    "an",
    "a",
    "the",
    "of",
    "and",
    "or",
    "not",
    "it",
    "them",
    "those",
    "up",
    "to",
    "one",
    "two",
    "three",
    "x",
    "~",
    "player",
    "opponent",
    "opponents",
    "permanent",
    "permanents",
    "nontoken",
    "tapped",
    "untapped",
    "legendary",
    "nonlegendary",
    "basic",
    "nonbasic",
    "snow",
    "colorless",
    "multicolored",
    "monocolored",
    "face-down",
    "modified",
    "historic",
    "outlaw",
];

/// Apply qualifier words ("nonblack", "tapped", "zombie", "artifact") to a filter.
fn qualify(f: &mut Filter, words: &[&str]) -> Option<()> {
    for w in words {
        let w = w.trim_end_matches(',');
        if let Some(c) = color_word(w) {
            f.colors = Some(ColorSet::of(&[c]));
        } else if let Some(t) = singular_type(w) {
            f.all_types.push(t);
        } else {
            match w {
                "nontoken" => f.nontoken = true,
                "tapped" => f.tapped = Some(true),
                "untapped" => f.tapped = Some(false),
                "legendary" => f.supertypes.push(Supertype::Legendary),
                "nonlegendary" => f.not_supertypes.push(Supertype::Legendary),
                "basic" => f.supertypes.push(Supertype::Basic),
                "nonbasic" => f.not_supertypes.push(Supertype::Basic),
                "snow" => f.supertypes.push(Supertype::Snow),
                _ => {
                    let neg = w.strip_prefix("non").map(|x| x.trim_start_matches('-'));
                    if let Some(n) = neg {
                        if let Some(c) = color_word(n) {
                            let mut cs = f.not_colors.unwrap_or_default();
                            cs.insert(c);
                            f.not_colors = Some(cs);
                            continue;
                        }
                        if let Some(t) = singular_type(n) {
                            f.not_types.push(t);
                            continue;
                        }
                        if n.chars().all(|c| c.is_alphabetic()) && !NOT_SUBTYPES.contains(&n) {
                            f.not_subtypes.push(capitalize(n));
                            continue;
                        }
                        return None;
                    }
                    if w.chars().all(|c| c.is_alphabetic() || c == '\'')
                        && !NOT_SUBTYPES.contains(&w)
                    {
                        f.subtypes.push(capitalize(w));
                    } else {
                        return None;
                    }
                }
            }
        }
    }
    Some(())
}

/// Split off "you control" / "an opponent controls" and return the controller it names.
fn controller_suffix(s: &str) -> (&str, Option<PlayerRef>) {
    for (suffix, who) in [
        (" you control", PlayerRef::You),
        (" you don't control", PlayerRef::Opponents),
        (" an opponent controls", PlayerRef::Opponents),
        (" your opponents control", PlayerRef::Opponents),
        (" each opponent controls", PlayerRef::Opponents),
    ] {
        if let Some(r) = s.strip_suffix(suffix) {
            return (r, Some(who));
        }
    }
    (s, None)
}

/// One kind of permanent, singular: "creature", "zombie", "nontoken artifact creature", "land".
fn group_singular(what: &str) -> Option<Filter> {
    let (what, ctl) = controller_suffix(what);
    let words: Vec<&str> = what.split_whitespace().collect();
    let (last, quals) = words.split_last()?;
    let mut f = Filter::default();
    if let Some(t) = singular_type(last) {
        f.any_of_types.push(t);
    } else if *last != "permanent" {
        if NOT_SUBTYPES.contains(last) || !last.chars().all(|c| c.is_alphabetic()) {
            return None;
        }
        f.subtypes.push(capitalize(last));
    }
    qualify(&mut f, quals)?;
    if ctl.is_some() {
        f.controller = ctl;
    }
    Some(f)
}

/// A group of permanents: "creatures you control", "other elf creatures you control",
/// "all creatures", "nonland permanents", "artifacts and enchantments" isn't one.
fn group_plural(who: &str) -> Option<Filter> {
    let who = who.strip_prefix("all ").unwrap_or(who);
    let (who, other) = match who.strip_prefix("other ") {
        Some(r) => (r, true),
        None => (who, false),
    };
    let (who, ctl) = controller_suffix(who);
    let words: Vec<&str> = who.split_whitespace().collect();
    let (last, quals) = words.split_last()?;
    let mut f = Filter::default();
    if let Some(t) = plural_type(last) {
        f.any_of_types.push(t);
    } else if *last != "permanents" {
        if *last == "tokens" || !last.ends_with('s') {
            return None;
        }
        let one = if let Some(stem) = last.strip_suffix("ves") {
            format!("{stem}f")
        } else {
            last.trim_end_matches('s').to_string()
        };
        if NOT_SUBTYPES.contains(&one.as_str()) {
            return None;
        }
        f.subtypes.push(capitalize(&one));
    }
    qualify(&mut f, quals)?;
    f.controller = ctl;
    f.other_than_this = other;
    Some(f)
}

fn group_filter(who: &str) -> Option<Filter> {
    group_plural(who)
}

/// "target nonartifact, nonblack creature", "target creature or planeswalker", "target
/// artifact, enchantment, or land", "another target creature you control", "target noncreature
/// spell", "target creature with flying".
fn general_target(phrase: &str) -> Option<TargetSpec> {
    let rest = phrase
        .strip_prefix("target ")
        .or_else(|| phrase.strip_prefix("another target "))?;
    let other = phrase.starts_with("another ");
    let (rest, keyword) = match rest.split_once(" with ") {
        Some((r, k)) => (r, Some(Keyword::parse(k)?)),
        None => (rest, None),
    };
    let (rest, spell) = match rest.strip_suffix(" spell") {
        Some(r) => (r, true),
        None => (rest, false),
    };
    let (rest, ctl) = controller_suffix(rest);
    // Split alternatives, gluing "non-x," qualifier lists back onto what follows.
    let flat = rest
        .replace(", or ", "|")
        .replace(" or ", "|")
        .replace(", ", "|");
    let mut alts: Vec<String> = Vec::new();
    let mut carry = String::new();
    for a in flat.split('|') {
        let a = a.trim();
        if a.starts_with("non") && !a.contains(' ') && singular_type(a).is_none() {
            carry.push_str(a);
            carry.push(' ');
            continue;
        }
        alts.push(format!("{carry}{a}"));
        carry.clear();
    }
    if !carry.is_empty() {
        return None;
    }
    let mut f = if alts.len() == 1 {
        if alts[0] == "permanent" {
            Filter::default()
        } else {
            group_singular(&alts[0])?
        }
    } else {
        let types: Option<Vec<CardType>> = alts.iter().map(|a| singular_type(a)).collect();
        Filter {
            any_of_types: types?,
            ..Default::default()
        }
    };
    f.keyword = keyword.or(f.keyword);
    if ctl.is_some() {
        f.controller = ctl;
    }
    f.other_than_this = other;
    Some(if spell {
        TargetSpec::Spell(f)
    } else {
        TargetSpec::Permanent(f)
    })
}

/// Parse "target X" phrases, allocating target indices.
fn target_spec(phrase: &str) -> Option<TargetSpec> {
    let f = |types: &[CardType]| Filter {
        any_of_types: types.to_vec(),
        ..Default::default()
    };
    Some(match phrase {
        "any target" => TargetSpec::Any,
        "target player" => TargetSpec::Player,
        "target opponent" => TargetSpec::Opponent,
        "target creature" => TargetSpec::Permanent(Filter::creatures()),
        "target creature you control" => TargetSpec::Permanent(Filter::creatures().yours()),
        "target creature an opponent controls" | "target creature you don't control" => {
            TargetSpec::Permanent(Filter {
                controller: Some(PlayerRef::Opponents),
                ..Filter::creatures()
            })
        }
        "target artifact" => TargetSpec::Permanent(f(&[CardType::Artifact])),
        "target enchantment" => TargetSpec::Permanent(f(&[CardType::Enchantment])),
        "target artifact or enchantment" => {
            TargetSpec::Permanent(f(&[CardType::Artifact, CardType::Enchantment]))
        }
        "target land" => TargetSpec::Permanent(f(&[CardType::Land])),
        "target planeswalker" => TargetSpec::Permanent(f(&[CardType::Planeswalker])),
        "target permanent" => TargetSpec::Permanent(Filter::default()),
        "target nonland permanent" => TargetSpec::Permanent(Filter {
            not_types: vec![CardType::Land],
            ..Default::default()
        }),
        "target spell" => TargetSpec::Spell(Filter::default()),
        "target creature spell" => TargetSpec::Spell(Filter::creatures()),
        "target noncreature spell" => TargetSpec::Spell(Filter {
            not_types: vec![CardType::Creature],
            ..Default::default()
        }),
        "target instant or sorcery spell" => {
            TargetSpec::Spell(f(&[CardType::Instant, CardType::Sorcery]))
        }
        "target artifact or creature" => {
            TargetSpec::Permanent(f(&[CardType::Artifact, CardType::Creature]))
        }
        _ => return graveyard_card(phrase).or_else(|| general_target(phrase)),
    })
}

/// "target creature card from your graveyard", "target card from a graveyard", "target
/// instant or sorcery card in an opponent's graveyard".
fn graveyard_card(phrase: &str) -> Option<TargetSpec> {
    let rest = phrase.strip_prefix("target ")?;
    let (kind, whose) = rest
        .split_once(" card from ")
        .or_else(|| rest.split_once(" card in "))
        .or_else(|| rest.strip_prefix("card from ").map(|w| ("", w)))
        .or_else(|| rest.strip_prefix("card in ").map(|w| ("", w)))?;
    let mut f = match kind {
        "" => Filter::default(),
        "creature" => Filter::creatures(),
        "artifact" => Filter::of_type(CardType::Artifact),
        "enchantment" => Filter::of_type(CardType::Enchantment),
        "instant" => Filter::of_type(CardType::Instant),
        "sorcery" => Filter::of_type(CardType::Sorcery),
        "land" => Filter::of_type(CardType::Land),
        "planeswalker" => Filter::of_type(CardType::Planeswalker),
        "instant or sorcery" => Filter {
            any_of_types: vec![CardType::Instant, CardType::Sorcery],
            ..Default::default()
        },
        "artifact or creature" => Filter {
            any_of_types: vec![CardType::Artifact, CardType::Creature],
            ..Default::default()
        },
        "artifact or enchantment" => Filter {
            any_of_types: vec![CardType::Artifact, CardType::Enchantment],
            ..Default::default()
        },
        "permanent" => Filter {
            not_types: vec![CardType::Instant, CardType::Sorcery],
            ..Default::default()
        },
        "noncreature" => Filter {
            not_types: vec![CardType::Creature],
            ..Default::default()
        },
        "nonland" => Filter {
            not_types: vec![CardType::Land],
            ..Default::default()
        },
        _ => return None,
    };
    match whose {
        "your graveyard" => f.controller = Some(PlayerRef::You),
        "an opponent's graveyard" => f.controller = Some(PlayerRef::Opponents),
        "a graveyard" | "any graveyard" | "a single graveyard" => {}
        _ => return None,
    }
    Some(TargetSpec::CardInGraveyard(f))
}

fn color_word(w: &str) -> Option<Color> {
    Some(match w {
        "white" => Color::White,
        "blue" => Color::Blue,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        _ => return None,
    })
}

fn push_target(targets: &mut Vec<TargetSpec>, phrase: &str) -> Option<usize> {
    targets.push(target_spec(phrase)?);
    Some(targets.len() - 1)
}

/// Parse one or more sentences of effects.
pub fn parse_effects(s: &str, targets: &mut Vec<TargetSpec>) -> Option<Vec<Effect>> {
    let s = s.to_lowercase();
    let mut out = Vec::new();
    for sentence in s
        .split(". ")
        .map(|x| x.trim().trim_end_matches('.'))
        .filter(|x| !x.is_empty())
    {
        // The engine has no regeneration, so "can't be regenerated" never changes anything.
        if matches!(
            sentence,
            "they can't be regenerated" | "it can't be regenerated"
        ) {
            continue;
        }
        // A search's "then shuffle" is part of the search itself.
        if sentence == "shuffle"
            && out
                .iter()
                .any(|e| matches!(e, Effect::SearchLibrary { .. }))
        {
            continue;
        }
        if let Some(e) = parse_search(sentence) {
            out.push(e);
            continue;
        }
        let mut subject: Option<PlayerRef> = None;
        for part in sentence.split(", then ") {
            if part == "shuffle"
                && out
                    .iter()
                    .any(|e| matches!(e, Effect::SearchLibrary { .. }))
            {
                continue;
            }
            out.append(&mut parse_clause(part.trim(), targets, &mut subject)?);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// One clause, which may join two effects with "and" ("you draw a card and you lose 1 life",
/// "target player draws two cards and loses 2 life").
fn parse_clause(
    s: &str,
    targets: &mut Vec<TargetSpec>,
    subject: &mut Option<PlayerRef>,
) -> Option<Vec<Effect>> {
    let mark = targets.len();
    if let Some(e) = parse_player_effect(s, targets, subject) {
        return Some(vec![e]);
    }
    targets.truncate(mark);
    if let Some(e) = parse_effect(s, targets) {
        return Some(vec![e]);
    }
    targets.truncate(mark);
    // Try each " and " as the join between two effects.
    let mut from = 0;
    while let Some(i) = s[from..].find(" and ") {
        let at = from + i;
        let (a, b) = (&s[..at], &s[at + 5..]);
        let saved = *subject;
        if let Some(mut first) = parse_clause(a, targets, subject) {
            if let Some(mut second) = parse_clause(b, targets, subject) {
                first.append(&mut second);
                return Some(first);
            }
        }
        targets.truncate(mark);
        *subject = saved;
        from = at + 5;
    }
    None
}

/// Effects on a player: draw, gain or lose life, mill. The subject carries over to a following
/// clause with no subject of its own ("target player draws two cards and loses 2 life").
fn parse_player_effect(
    s: &str,
    targets: &mut Vec<TargetSpec>,
    subject: &mut Option<PlayerRef>,
) -> Option<Effect> {
    let (who, rest) = if let Some(r) = s.strip_prefix("you ") {
        (PlayerRef::You, r)
    } else if let Some(r) = s.strip_prefix("target player ") {
        targets.push(TargetSpec::Player);
        (PlayerRef::Target(targets.len() - 1), r)
    } else if let Some(r) = s.strip_prefix("target opponent ") {
        targets.push(TargetSpec::Opponent);
        (PlayerRef::Target(targets.len() - 1), r)
    } else if let Some(r) = s.strip_prefix("each opponent ") {
        (PlayerRef::Opponents, r)
    } else if let Some(r) = s.strip_prefix("each player ") {
        (PlayerRef::Each, r)
    } else if let Some(r) = s.strip_prefix("each other player ") {
        (PlayerRef::Opponents, r)
    } else if let Some(r) = s
        .strip_prefix("its controller ")
        .or_else(|| s.strip_prefix("that creature's controller "))
        .or_else(|| s.strip_prefix("that permanent's controller "))
    {
        // The controller of what the previous sentence targeted.
        (
            PlayerRef::ControllerOfTarget(targets.len().checked_sub(1)?),
            r,
        )
    } else {
        (subject.unwrap_or(PlayerRef::You), s)
    };
    let verb = |r: &str, v: &str| {
        r.strip_prefix(&format!("{v}s "))
            .or_else(|| r.strip_prefix(&format!("{v} ")))
            .map(str::to_string)
    };
    let e = if rest == "gains life equal to its power" {
        match who {
            PlayerRef::ControllerOfTarget(t) => {
                Effect::ControllerOfTargetGainsLifeEqualToPower { target: t }
            }
            _ => return None,
        }
    } else if let Some(r) = verb(rest, "draw") {
        let n = r.strip_suffix(" cards").or(r.strip_suffix(" card"))?;
        Effect::DrawCards {
            who,
            amount: amount(n)?,
        }
    } else if let Some(r) = verb(rest, "gain") {
        Effect::GainLife {
            who,
            amount: amount(r.strip_suffix(" life")?)?,
        }
    } else if let Some(r) = verb(rest, "lose") {
        Effect::LoseLife {
            who,
            amount: amount(r.strip_suffix(" life")?)?,
        }
    } else if let Some(r) = verb(rest, "sacrifice") {
        let what = r.strip_prefix("a ").or_else(|| r.strip_prefix("an "))?;
        Effect::Sacrifice {
            who,
            filter: group_singular(what)?,
        }
    } else if let Some(r) = verb(rest, "discard") {
        Effect::Discard {
            who,
            amount: amount(r.strip_suffix(" cards").or(r.strip_suffix(" card"))?)?,
        }
    } else if let Some(r) = verb(rest, "create") {
        if let PlayerRef::ControllerOfTarget(_) = who {
            match parse_token(&r)? {
                Effect::CreateToken { token, amount, .. } => Effect::CreateToken {
                    token,
                    amount,
                    controller: who,
                },
                _ => return None,
            }
        } else {
            return None;
        }
    } else if let Some(r) = verb(rest, "scry") {
        if who != PlayerRef::You {
            return None;
        }
        Effect::Scry {
            amount: amount(&r)?,
        }
    } else if let Some(r) = verb(rest, "surveil") {
        if who != PlayerRef::You {
            return None;
        }
        Effect::Surveil {
            amount: amount(&r)?,
        }
    } else if let Some(r) = verb(rest, "mill") {
        Effect::Mill {
            who,
            amount: amount(r.strip_suffix(" cards").or(r.strip_suffix(" card"))?)?,
        }
    } else {
        return None;
    };
    *subject = Some(who);
    Some(e)
}

fn parse_effect(s: &str, targets: &mut Vec<TargetSpec>) -> Option<Effect> {
    let s = s.strip_prefix("you ").unwrap_or(s);
    // In a permanent's own ability, "it" is the permanent itself.
    let owned;
    let s = match s.strip_prefix("it deals ") {
        Some(r) => {
            owned = format!("~ deals {r}");
            owned.as_str()
        }
        None => s,
    };
    let words: Vec<&str> = s.split_whitespace().collect();
    if let Some(n) = s.strip_prefix("scry ") {
        return Some(Effect::Scry { amount: amount(n)? });
    }
    if let Some(n) = s.strip_prefix("surveil ") {
        return Some(Effect::Surveil { amount: amount(n)? });
    }
    if s == "investigate" {
        return parse_token("a clue token");
    }
    if let Some(t) = s.strip_prefix("~ fights ") {
        return Some(Effect::Fight {
            target: push_target(targets, t)?,
        });
    }
    if let Some(t) = s.strip_prefix("attach ~ to ") {
        return Some(Effect::Attach {
            target: push_target(targets, t)?,
        });
    }
    // "put a +1/+1 counter on each creature you control"
    if let Some(r) = s.strip_prefix("put ") {
        if let Some((n, on)) = r
            .split_once(" +1/+1 counters on each ")
            .or_else(|| r.split_once(" +1/+1 counter on each "))
        {
            return Some(Effect::AddCountersEach {
                filter: group_singular(on)?,
                kind: CounterKind::PlusOnePlusOne,
                amount: amount(n)?,
            });
        }
    }
    if let Some(who) = s.strip_prefix("destroy all ") {
        return Some(Effect::DestroyAll {
            filter: group_plural(who)?,
        });
    }
    // draw
    if let Some(rest) = s.strip_prefix("draw ") {
        let n = rest.split_whitespace().next()?;
        if rest.ends_with("card") || rest.ends_with("cards") {
            return Some(Effect::DrawCards {
                who: PlayerRef::You,
                amount: amount(n)?,
            });
        }
    }
    if let Some(rest) = s.strip_prefix("gain ") {
        let n = rest.strip_suffix(" life")?;
        return Some(Effect::GainLife {
            who: PlayerRef::You,
            amount: amount(n)?,
        });
    }
    if let Some(rest) = s.strip_prefix("lose ") {
        return Some(Effect::LoseLife {
            who: PlayerRef::You,
            amount: amount(rest.strip_suffix(" life")?)?,
        });
    }
    if let Some(rest) = s.strip_prefix("each opponent loses ") {
        return Some(Effect::LoseLife {
            who: PlayerRef::Opponents,
            amount: amount(rest.strip_suffix(" life")?)?,
        });
    }
    if let Some(rest) = s.strip_prefix("~ deals ") {
        let (n, to) = rest.split_once(" damage to ")?;
        let amt = amount(n)?;
        if to == "each opponent" {
            return Some(Effect::DealDamageEach {
                filter: Filter {
                    this_object: true,
                    other_than_this: true,
                    ..Default::default()
                },
                players: Some(PlayerRef::Opponents),
                amount: amt,
            });
        }
        let each = match to {
            "each creature" => Some((Filter::creatures(), None)),
            "each creature and each player"
            | "each creature and each planeswalker and each player" => {
                Some((Filter::creatures(), Some(PlayerRef::Each)))
            }
            "each player" => Some((
                Filter {
                    this_object: true,
                    other_than_this: true,
                    ..Default::default()
                },
                Some(PlayerRef::Each),
            )),
            "each opponent and each creature your opponents control" => Some((
                Filter {
                    controller: Some(PlayerRef::Opponents),
                    ..Filter::creatures()
                },
                Some(PlayerRef::Opponents),
            )),
            _ => to
                .strip_prefix("each ")
                .and_then(group_singular)
                .map(|f| (f, None)),
        };
        if let Some((filter, players)) = each {
            return Some(Effect::DealDamageEach {
                filter,
                players,
                amount: amt,
            });
        }
        let t = push_target(targets, to)?;
        return Some(Effect::DealDamage {
            target: t,
            amount: amt,
        });
    }
    if s == "destroy all creatures" {
        return Some(Effect::DestroyAll {
            filter: Filter::creatures(),
        });
    }
    if let Some(rest) = s.strip_prefix("destroy ") {
        return Some(Effect::Destroy {
            target: push_target(targets, rest)?,
        });
    }
    if let Some(rest) = s.strip_prefix("exile ") {
        return Some(Effect::Exile {
            target: push_target(targets, rest)?,
        });
    }
    if let Some(rest) = s.strip_prefix("counter ") {
        return Some(Effect::Counter {
            target: push_target(targets, rest)?,
        });
    }
    if let Some(rest) = s.strip_prefix("tap ") {
        return Some(Effect::Tap {
            target: push_target(targets, rest)?,
        });
    }
    if let Some(rest) = s.strip_prefix("untap ") {
        return Some(Effect::Untap {
            target: push_target(targets, rest)?,
        });
    }
    if let Some(rest) = s.strip_prefix("return ") {
        if let Some(t) = rest
            .strip_suffix(" to its owner's hand")
            .filter(|t| t.contains("graveyard"))
        {
            return Some(Effect::ReturnFromGraveyardToHand {
                target: push_target(targets, t)?,
            });
        }
        if let Some(t) = rest.strip_suffix(" to its owner's hand") {
            return Some(Effect::ReturnToHand {
                target: push_target(targets, t)?,
            });
        }
        if let Some(t) = rest.strip_suffix(" to your hand") {
            return Some(Effect::ReturnFromGraveyardToHand {
                target: push_target(targets, t)?,
            });
        }
        if let Some(t) = rest
            .strip_suffix(" to the battlefield under your control")
            .or_else(|| rest.strip_suffix(" to the battlefield"))
        {
            return Some(Effect::ReturnFromGraveyardToBattlefield {
                target: push_target(targets, t)?,
            });
        }
    }
    // "put target creature card from a graveyard onto the battlefield under your control"
    if let Some(t) = s.strip_prefix("put ").and_then(|r| {
        r.strip_suffix(" onto the battlefield under your control")
            .or_else(|| r.strip_suffix(" onto the battlefield"))
    }) {
        if t.contains("graveyard") {
            return Some(Effect::ReturnFromGraveyardToBattlefield {
                target: push_target(targets, t)?,
            });
        }
    }
    // "target creature gets +3/+3 until end of turn", "creatures you control get +2/+1 and
    // gain trample until end of turn", "~ gains flying until end of turn".
    if let Some(e) = parse_until_eot(s, targets) {
        return Some(e);
    }
    // "put a +1/+1 counter on ~" / "put two +1/+1 counters on target creature"
    if words.first() == Some(&"put") && s.contains("+1/+1 counter") {
        let n = amount(words.get(1)?)?;
        let on = s.split(" on ").nth(1)?;
        let target = if on == "~" {
            None
        } else {
            Some(push_target(targets, on)?)
        };
        return Some(Effect::AddCounters {
            target,
            kind: CounterKind::PlusOnePlusOne,
            amount: n,
        });
    }
    // "create a 1/1 green Saproling creature token" / "... with flying"
    if let Some(rest) = s.strip_prefix("create ") {
        return parse_token(rest);
    }
    if let Some(rest) = s.strip_prefix("mill ") {
        return Some(Effect::Mill {
            who: PlayerRef::You,
            amount: amount(rest.strip_suffix(" cards").or(rest.strip_suffix(" card"))?)?,
        });
    }
    if let Some(rest) = s.strip_prefix("add ") {
        if let Some(n) = rest.strip_suffix(" mana of any one color").and_then(number) {
            return Some(Effect::AddManaAnyColor { amount: n as u32 });
        }
        let m = parse_mana_production(&format!("add {rest}"))?;
        return match m {
            ManaProduction::Fixed(v) => Some(Effect::AddMana { mana: v }),
            ManaProduction::AnyColor => Some(Effect::AddManaAnyColor { amount: 1 }),
            ManaProduction::OneOf(_) | ManaProduction::CommanderIdentity => None,
        };
    }
    None
}

fn parse_until_eot(s: &str, targets: &mut Vec<TargetSpec>) -> Option<Effect> {
    let body = s.strip_suffix(" until end of turn")?;
    let mut split = None;
    for v in [" gets ", " get ", " gains ", " gain "] {
        if let Some(i) = body.find(v) {
            if split.is_none_or(|(j, _): (usize, &str)| i < j) {
                split = Some((i, v));
            }
        }
    }
    let (i, v) = split?;
    let who = &body[..i];
    let rest = &body[i + v.len()..];
    let mut mods = Vec::new();
    let (pt, kws) = if v.contains("get") {
        match rest.split_once(" and ") {
            Some((pt, k)) => (
                Some(pt),
                Some(
                    k.strip_prefix("gains ")
                        .or_else(|| k.strip_prefix("gain "))?,
                ),
            ),
            None => (Some(rest), None),
        }
    } else {
        (None, Some(rest))
    };
    if let Some(pt) = pt {
        let (p, t) = parse_pt_mod(pt)?;
        mods.push(Modification::ModifyPT(p, t));
    }
    if let Some(k) = kws {
        for kw in k.split(" and ").flat_map(|x| x.split(", ")) {
            mods.push(Modification::AddKeyword(Keyword::parse(kw)?));
        }
    }
    let affected = if who == "~" || who == "it" {
        Affected::ThisObject
    } else if let Some(f) = group_filter(who) {
        Affected::Filter(f)
    } else {
        Affected::Target(push_target(targets, who)?)
    };
    Some(Effect::ApplyContinuous {
        affected,
        mods,
        duration: Duration::EndOfTurn,
    })
}

/// "search your library for a basic land card, put it onto the battlefield tapped, then
/// shuffle", "search your library for up to two basic land cards, reveal those cards, put one
/// onto the battlefield tapped and the other into your hand, then shuffle", "search your library
/// for a card, put that card into your hand, then shuffle".
fn parse_search(s: &str) -> Option<Effect> {
    let r = s.strip_prefix("search your library for ")?;
    let r = r
        .strip_suffix(", then shuffle")
        .or_else(|| r.strip_suffix(". then shuffle"))
        .unwrap_or(r);
    let (count, r) = if let Some(r) = r.strip_prefix("up to ") {
        let (n, r) = r.split_once(' ')?;
        (number(n)? as u32, r)
    } else {
        let r = r.strip_prefix("a ").or_else(|| r.strip_prefix("an "))?;
        (1, r)
    };
    let (what, r) = r
        .split_once(" cards, ")
        .or_else(|| r.split_once(" card, "))
        .map(|(w, r)| (w, r))
        .or_else(|| r.strip_prefix("card, ").map(|r| ("", r)))
        .or_else(|| r.strip_prefix("cards, ").map(|r| ("", r)))?;
    let filter = if what.is_empty() {
        Filter::default()
    } else if what.contains(", ") || what.contains(" or ") {
        // "plains, island, swamp, or mountain"
        let names: Vec<String> = what
            .replace(", or ", ", ")
            .replace(" or ", ", ")
            .split(", ")
            .map(capitalize)
            .collect();
        if !names
            .iter()
            .all(|n| ["Plains", "Island", "Swamp", "Mountain", "Forest"].contains(&n.as_str()))
        {
            return None;
        }
        Filter {
            subtypes: names,
            ..Default::default()
        }
    } else {
        group_singular(what)?
    };
    let r = r
        .strip_prefix("reveal it, ")
        .or_else(|| r.strip_prefix("reveal that card, "))
        .or_else(|| r.strip_prefix("reveal them, "))
        .or_else(|| r.strip_prefix("reveal those cards, "))
        .unwrap_or(r);
    let dest = match r {
        "put it onto the battlefield tapped"
        | "put that card onto the battlefield tapped"
        | "put them onto the battlefield tapped"
        | "put those cards onto the battlefield tapped" => SearchDest::Battlefield { tapped: true },
        "put it onto the battlefield"
        | "put that card onto the battlefield"
        | "put them onto the battlefield"
        | "put those cards onto the battlefield" => SearchDest::Battlefield { tapped: false },
        "put it into your hand"
        | "put that card into your hand"
        | "put them into your hand"
        | "put those cards into your hand" => SearchDest::Hand,
        "put one onto the battlefield tapped and the other into your hand" => {
            SearchDest::BattlefieldTappedThenHand
        }
        _ => return None,
    };
    Some(Effect::SearchLibrary {
        filter,
        count,
        dest,
    })
}

/// Treasure, Food, Clue and Blood tokens (CR 111.10).
fn predefined_token(kind: &str) -> Option<Arc<CardDef>> {
    let sac = |mana: &str, tap: bool| Cost {
        mana: ManaCost::parse(mana).unwrap_or_default(),
        tap,
        sacrifice_this: true,
        ..Default::default()
    };
    let ability = match kind {
        "treasure" => Ability::Mana {
            cost: sac("", true),
            produces: ManaProduction::AnyColor,
        },
        "food" => Ability::Activated {
            cost: sac("{2}", true),
            targets: vec![],
            effects: vec![Effect::GainLife {
                who: PlayerRef::You,
                amount: Amount::Fixed(3),
            }],
            sorcery_speed: false,
        },
        "clue" => Ability::Activated {
            cost: sac("{2}", false),
            targets: vec![],
            effects: vec![Effect::DrawCards {
                who: PlayerRef::You,
                amount: Amount::Fixed(1),
            }],
            sorcery_speed: false,
        },
        _ => return None,
    };
    let name = capitalize(kind);
    Some(
        CardBuilder::new(&name)
            .types(&[CardType::Artifact])
            .subtypes(&[name.as_str()])
            .ability(ability)
            .token()
            .build(),
    )
}

fn parse_token(rest: &str) -> Option<Effect> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let n = amount(words.first()?)?;
    if words.len() == 3 && matches!(words[2], "token" | "tokens") {
        return Some(Effect::CreateToken {
            token: predefined_token(words[1])?,
            amount: n,
            controller: PlayerRef::You,
        });
    }
    let (p, t) = parse_pt_mod(words.get(1)?)?;
    let token_pos = words.iter().position(|w| *w == "token" || *w == "tokens")?;
    let creature_pos = words.iter().position(|w| *w == "creature")?;
    if creature_pos + 1 != token_pos {
        return None;
    }
    let artifact = words[2..creature_pos].contains(&"artifact");
    let mut colors = Vec::new();
    let mut subtypes = Vec::new();
    for w in &words[2..creature_pos] {
        match *w {
            "white" => colors.push(Color::White),
            "blue" => colors.push(Color::Blue),
            "black" => colors.push(Color::Black),
            "red" => colors.push(Color::Red),
            "green" => colors.push(Color::Green),
            "colorless" | "and" | "artifact" => {}
            other => subtypes.push(capitalize(other)),
        }
    }
    let mut b = CardBuilder::new(&format!("{} Token", subtypes.join(" ")))
        .types(if artifact {
            &[CardType::Artifact, CardType::Creature][..]
        } else {
            &[CardType::Creature][..]
        })
        .pt(p, t)
        .colors(&colors)
        .token();
    let subs: Vec<&str> = subtypes.iter().map(|s| s.as_str()).collect();
    b = b.subtypes(&subs);
    let after = words[token_pos + 1..].join(" ");
    if let Some(k) = after.strip_prefix("with ") {
        for kw in k.split(" and ") {
            b = b.keyword(Keyword::parse(kw)?);
        }
    } else if !after.is_empty() {
        return None;
    }
    let token: Arc<CardDef> = b.build();
    Some(Effect::CreateToken {
        token,
        amount: n,
        controller: PlayerRef::You,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(name: &str, types: &[CardType], text: &str) -> CardDef {
        let mut d = CardDef {
            name: name.into(),
            types: types.iter().copied().collect(),
            ..Default::default()
        };
        script(&mut d, name, text);
        d
    }

    #[test]
    fn keywords_and_mana_abilities() {
        let d = def(
            "Birds of Paradise",
            &[CardType::Creature],
            "Flying\n{T}: Add one mana of any color.",
        );
        assert_eq!(d.abilities[0], Ability::Keyword(Keyword::Flying));
        assert!(matches!(
            d.abilities[1],
            Ability::Mana {
                produces: ManaProduction::AnyColor,
                ..
            }
        ));
        let d = def(
            "Selesnya Signet",
            &[CardType::Artifact],
            "{1}, {T}: Add {G}{W}.",
        );
        assert!(d.fully_supported());
        let d = def("Temple", &[CardType::Land], "{T}: Add {W} or {U}.");
        assert!(
            matches!(&d.abilities[0], Ability::Mana { produces: ManaProduction::OneOf(v), .. } if v.len() == 2)
        );
    }

    #[test]
    fn graveyard_targets() {
        for (text, mine) in [
            ("Return target creature card from your graveyard to your hand.", true),
            ("Return target card from your graveyard to your hand.", true),
            ("Return target artifact card from your graveyard to the battlefield.", true),
            ("Put target creature card from a graveyard onto the battlefield under your control.", false),
            ("Exile target card from a graveyard.", false),
            ("Exile target creature card from an opponent's graveyard.", false),
            ("Return target instant or sorcery card from your graveyard to your hand.", true),
        ] {
            let d = def("Test Rite", &[CardType::Sorcery], text);
            assert!(d.fully_supported(), "{text}");
            assert!(
                matches!(&d.spell_targets[0], TargetSpec::CardInGraveyard(f) if (f.controller == Some(PlayerRef::You)) == mine),
                "{text}"
            );
        }
    }

    #[test]
    fn a_keyword_list_with_an_unknown_keyword_keeps_the_known_ones() {
        for text in [
            "Flying, ward {2}",
            "Flying; protection from red",
            "Flying, first strike, landwalk",
        ] {
            let d = def("Test Drake", &[CardType::Creature], text);
            assert!(
                d.abilities.contains(&Ability::Keyword(Keyword::Flying)),
                "{text}"
            );
            assert!(!d.fully_supported(), "{text}");
        }
    }

    #[test]
    fn basic_land_reminder_text_is_dropped() {
        let d = def("Forest", &[CardType::Land], "({T}: Add {G}.)");
        assert!(
            d.abilities.is_empty(),
            "the intrinsic ability comes from the Forest subtype (305.6)"
        );
    }

    #[test]
    fn spells() {
        let d = def(
            "Lightning Bolt",
            &[CardType::Instant],
            "Lightning Bolt deals 3 damage to any target.",
        );
        assert_eq!(d.spell_targets, vec![TargetSpec::Any]);
        assert_eq!(
            d.spell_effects,
            vec![Effect::DealDamage {
                target: 0,
                amount: Amount::Fixed(3)
            }]
        );
        let d = def("Divination", &[CardType::Sorcery], "Draw two cards.");
        assert!(d.fully_supported());
        let d = def(
            "Counterspell",
            &[CardType::Instant],
            "Counter target spell.",
        );
        assert_eq!(d.spell_effects, vec![Effect::Counter { target: 0 }]);
        let d = def(
            "Giant Growth",
            &[CardType::Instant],
            "Target creature gets +3/+3 until end of turn.",
        );
        assert!(d.fully_supported() && d.spell_targets.len() == 1);
    }

    #[test]
    fn triggers_and_statics() {
        let d = def(
            "Elvish Visionary",
            &[CardType::Creature],
            "When this creature enters, draw a card.",
        );
        assert!(matches!(
            d.abilities[0],
            Ability::Triggered {
                event: TriggerEvent::EntersBattlefield(_),
                ..
            }
        ));
        let d = def(
            "Glorious Anthem",
            &[CardType::Enchantment],
            "Creatures you control get +1/+1.",
        );
        assert!(matches!(d.abilities[0], Ability::Static { .. }));
        let d = def(
            "Raise the Alarm",
            &[CardType::Instant],
            "Create two 1/1 white Soldier creature tokens.",
        );
        assert!(d.fully_supported(), "{:?}", d.abilities);
    }

    #[test]
    fn unknown_text_is_flagged_not_guessed() {
        let d = def(
            "Oddity",
            &[CardType::Creature],
            "Whenever the moon is full, swap hands with an opponent.",
        );
        assert!(!d.fully_supported());
    }

    #[test]
    fn player_subjects_carry_across_and() {
        let d = def(
            "Sign in Blood",
            &[CardType::Sorcery],
            "Target player draws two cards and loses 2 life.",
        );
        assert_eq!(d.spell_targets, vec![TargetSpec::Player]);
        assert_eq!(
            d.spell_effects,
            vec![
                Effect::DrawCards {
                    who: PlayerRef::Target(0),
                    amount: Amount::Fixed(2)
                },
                Effect::LoseLife {
                    who: PlayerRef::Target(0),
                    amount: Amount::Fixed(2)
                },
            ]
        );
        let d = def(
            "Overseer",
            &[CardType::Creature],
            "When this creature enters, you gain 1 life and draw a card.",
        );
        assert!(d.fully_supported());
    }

    #[test]
    fn colored_and_keyword_targets_and_group_pumps() {
        let d = def(
            "Doom Blade",
            &[CardType::Instant],
            "Destroy target nonblack creature.",
        );
        match &d.spell_targets[0] {
            TargetSpec::Permanent(f) => {
                assert_eq!(f.not_colors, Some(ColorSet::of(&[Color::Black])))
            }
            t => panic!("{t:?}"),
        }
        let d = def(
            "Plummet",
            &[CardType::Instant],
            "Destroy target creature with flying.",
        );
        assert!(
            matches!(&d.spell_targets[0], TargetSpec::Permanent(f) if f.keyword == Some(Keyword::Flying))
        );
        let d = def(
            "Overrun",
            &[CardType::Sorcery],
            "Creatures you control get +3/+3 and gain trample until end of turn.",
        );
        assert!(matches!(
            &d.spell_effects[0],
            Effect::ApplyContinuous { affected: Affected::Filter(_), mods, .. } if mods.len() == 2
        ));
        let d = def(
            "Kavu",
            &[CardType::Creature],
            "When this creature enters, it deals 4 damage to target creature.",
        );
        assert!(d.fully_supported());
        let d = def(
            "Wrath",
            &[CardType::Sorcery],
            "Destroy all creatures. They can't be regenerated.",
        );
        assert!(d.fully_supported());
    }

    fn supported(name: &str, types: &[CardType], text: &str) -> CardDef {
        let d = def(name, types, text);
        let bad: Vec<_> = d
            .abilities
            .iter()
            .filter(|a| matches!(a, Ability::Unsupported(_)))
            .collect();
        assert!(bad.is_empty(), "{name}: {bad:?}");
        d
    }

    #[test]
    fn auras_and_equipment() {
        use CardType::*;
        let d = supported(
            "Rancor Lite",
            &[Enchantment],
            "Enchant creature\nEnchanted creature gets +2/+0 and has trample.",
        );
        assert_eq!(
            d.spell_targets,
            vec![TargetSpec::Permanent(Filter::creatures())]
        );
        assert!(matches!(&d.abilities[0], Ability::Static { affected, mods }
            if affected.attached_to_this && mods.len() == 2));
        let d = supported(
            "Pacifism",
            &[Enchantment],
            "Enchant creature\nEnchanted creature can't attack or block.",
        );
        assert!(matches!(&d.abilities[0], Ability::Static { mods, .. }
            if mods.contains(&Modification::AddKeyword(Keyword::CantAttack))));
        supported(
            "Daybreak Coronet Lite",
            &[Enchantment],
            "Enchant creature\nEnchanted creature gets +3/+3 and has first strike, vigilance, and lifelink.",
        );
        supported(
            "Control Magic",
            &[Enchantment],
            "Enchant creature\nYou control enchanted creature.",
        );
        let d = supported(
            "Loxodon Warhammer",
            &[Artifact],
            "Equipped creature gets +3/+0 and has trample and lifelink.\nEquip {3}",
        );
        assert!(
            matches!(&d.abilities[1], Ability::Activated { effects, sorcery_speed: true, .. }
            if effects == &vec![Effect::Attach { target: 0 }])
        );
        supported(
            "Whispersilk Cloak",
            &[Artifact],
            "Equipped creature can't be blocked and has shroud.\nEquip {2}",
        );
    }

    #[test]
    fn lands_ramp_and_rules_about_the_card_itself() {
        use CardType::*;
        let d = supported(
            "Jungle Hollow",
            &[Land],
            "This land enters tapped.\nWhen this land enters, you gain 1 life.\n{T}: Add {B} or {G}.",
        );
        assert_eq!(d.abilities[0], Ability::EntersTapped);
        let d = supported(
            "Command Tower",
            &[Land],
            "{T}: Add one mana of any color in your commander's color identity.",
        );
        assert!(matches!(
            &d.abilities[0],
            Ability::Mana {
                produces: ManaProduction::CommanderIdentity,
                ..
            }
        ));
        let d = supported(
            "Llanowar Wastes",
            &[Land],
            "{T}: Add {C}.\n{T}: Add {B} or {G}. This land deals 1 damage to you.",
        );
        assert!(
            matches!(&d.abilities[1], Ability::Mana { cost, .. } if cost.deals_damage_to_you == 1)
        );
        let d = supported(
            "Cultivate",
            &[Sorcery],
            "Search your library for up to two basic land cards, reveal those cards, put one onto the battlefield tapped and the other into your hand, then shuffle.",
        );
        assert!(matches!(
            &d.spell_effects[0],
            Effect::SearchLibrary {
                count: 2,
                dest: SearchDest::BattlefieldTappedThenHand,
                ..
            }
        ));
        supported(
            "Evolving Wilds",
            &[Land],
            "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
        );
        supported(
            "Farseek",
            &[Sorcery],
            "Search your library for a Plains, Island, Swamp, or Mountain card, put it onto the battlefield tapped, then shuffle.",
        );
        supported(
            "Reliquary Tower",
            &[Land],
            "You have no maximum hand size.\n{T}: Add {C}.",
        );
        supported("Goblin Piker", &[Creature], "This creature can't block.");
        supported(
            "Dovin's Veto",
            &[Instant],
            "This spell can't be countered.\nCounter target noncreature spell.",
        );
        let d = supported(
            "Walking Ballista Lite",
            &[Artifact, Creature],
            "This creature enters with X +1/+1 counters on it.\n{4}: Put a +1/+1 counter on this creature.",
        );
        assert!(matches!(
            &d.abilities[0],
            Ability::EntersWithCounters {
                amount: Amount::X,
                ..
            }
        ));
    }

    #[test]
    fn triggers_targets_and_effects() {
        use CardType::*;
        let d = supported(
            "Lotus Cobra",
            &[Creature],
            "Landfall — Whenever a land you control enters, add one mana of any color.",
        );
        assert!(
            matches!(&d.abilities[0], Ability::Triggered { event: TriggerEvent::EntersBattlefield(f), .. }
            if f.any_of_types == vec![Land])
        );
        let d = supported(
            "Grave Titan",
            &[Creature],
            "Deathtouch\nWhenever this creature enters or attacks, create two 2/2 black Zombie creature tokens.",
        );
        assert_eq!(d.abilities.len(), 3);
        supported(
            "Shriekmaw",
            &[Creature],
            "When this creature enters, destroy target nonartifact, nonblack creature.",
        );
        supported(
            "Acidic Slime",
            &[Creature],
            "When this creature enters, destroy target artifact, enchantment, or land.",
        );
        supported(
            "Hero's Downfall",
            &[Instant],
            "Destroy target creature or planeswalker.",
        );
        supported(
            "Cast Down",
            &[Instant],
            "Destroy target nonlegendary creature.",
        );
        supported(
            "Swift Response",
            &[Instant],
            "Destroy target tapped creature.",
        );
        supported(
            "Planar Cleansing",
            &[Sorcery],
            "Destroy all nonland permanents.",
        );
        supported(
            "Pyroclasm",
            &[Sorcery],
            "Pyroclasm deals 2 damage to each creature.",
        );
        supported(
            "Volcanic Fallout",
            &[Instant],
            "Volcanic Fallout deals 2 damage to each creature and each player.",
        );
        supported("Preordain", &[Sorcery], "Scry 2, then draw a card.");
        supported("Mind Rot", &[Sorcery], "Target player discards two cards.");
        supported(
            "Diabolic Edict",
            &[Instant],
            "Target player sacrifices a creature.",
        );
        supported(
            "Fleshbag Marauder",
            &[Creature],
            "When this creature enters, each player sacrifices a creature.",
        );
        supported(
            "Faithless Looting",
            &[Sorcery],
            "Draw two cards, then discard two cards.",
        );
        supported(
            "Thraben Inspector",
            &[Creature],
            "When this creature enters, investigate.",
        );
        supported(
            "Prosperous Innkeeper",
            &[Creature],
            "When this creature enters, create a Treasure token.",
        );
        supported(
            "Bake into a Pie",
            &[Instant],
            "Destroy target creature. Create a Food token.",
        );
        let d = supported(
            "Beast Within",
            &[Instant],
            "Destroy target permanent. Its controller creates a 3/3 green Beast creature token.",
        );
        assert!(matches!(
            &d.spell_effects[1],
            Effect::CreateToken {
                controller: PlayerRef::ControllerOfTarget(0),
                ..
            }
        ));
        supported(
            "Swords to Plowshares",
            &[Instant],
            "Exile target creature. Its controller gains life equal to its power.",
        );
        supported("Cathars' Crusade", &[Enchantment], "Whenever a creature you control enters, put a +1/+1 counter on each creature you control.");
        supported(
            "Elvish Lord",
            &[Creature],
            "Other Elf creatures you control get +1/+1.",
        );
        supported(
            "Zombie Lord",
            &[Creature],
            "Other Zombies you control get +1/+1 and have deathtouch.",
        );
        supported("Monastery Swiftspear", &[Creature], "Haste\nProwess");
        supported(
            "Undead Augur",
            &[Creature],
            "Whenever another Zombie you control dies, you draw a card and you lose 1 life.",
        );
        let d = supported(
            "Garruk Wildspeaker",
            &[Planeswalker],
            "+1: Untap target land.\n−1: Create a 3/3 green Beast creature token.\n−4: Creatures you control get +3/+3 and gain trample until end of turn.",
        );
        assert!(
            matches!(&d.abilities[1], Ability::Activated { cost, .. } if cost.loyalty == Some(-1))
        );
    }
}
