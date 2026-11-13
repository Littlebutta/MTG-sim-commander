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
        if is_spell {
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
    ] {
        s = s.replace(t, "~").replace(&capitalize(t), "~");
    }
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
    if let Some(ks) = parse_keyword_line(line) {
        return Some(ks);
    }
    let l = line.to_lowercase();
    // Triggered abilities.
    for (prefix, event) in [
        (
            "when ~ enters the battlefield, ",
            TriggerEvent::EntersBattlefield(Filter::this()),
        ),
        (
            "when ~ enters, ",
            TriggerEvent::EntersBattlefield(Filter::this()),
        ),
        (
            "whenever ~ attacks, ",
            TriggerEvent::Attacks(Filter::this()),
        ),
        ("when ~ dies, ", TriggerEvent::Dies(Filter::this())),
        (
            "whenever ~ deals combat damage to a player, ",
            TriggerEvent::DealsCombatDamageToPlayer(Filter::this()),
        ),
        (
            "whenever another creature you control dies, ",
            TriggerEvent::Dies(Filter::creatures().yours().other()),
        ),
        (
            "whenever another creature dies, ",
            TriggerEvent::Dies(Filter::creatures().other()),
        ),
        (
            "whenever a creature you control dies, ",
            TriggerEvent::Dies(Filter::creatures().yours()),
        ),
        (
            "whenever another creature you control enters, ",
            TriggerEvent::EntersBattlefield(Filter::creatures().yours().other()),
        ),
        (
            "whenever another creature enters, ",
            TriggerEvent::EntersBattlefield(Filter::creatures().other()),
        ),
        (
            "at the beginning of your upkeep, ",
            TriggerEvent::BeginningOfUpkeep(PlayerRef::You),
        ),
        (
            "at the beginning of each upkeep, ",
            TriggerEvent::BeginningOfUpkeep(PlayerRef::Each),
        ),
        (
            "at the beginning of your end step, ",
            TriggerEvent::BeginningOfEndStep(PlayerRef::You),
        ),
        (
            "whenever you cast a spell, ",
            TriggerEvent::YouCastSpell(Filter::default()),
        ),
        (
            "whenever you cast an instant or sorcery spell, ",
            TriggerEvent::YouCastSpell(Filter {
                any_of_types: vec![CardType::Instant, CardType::Sorcery],
                ..Default::default()
            }),
        ),
        (
            "whenever you cast a creature spell, ",
            TriggerEvent::YouCastSpell(Filter::creatures()),
        ),
    ] {
        if let Some(rest) = l.strip_prefix(prefix) {
            let (optional, rest) = match rest.strip_prefix("you may ") {
                Some(r) => (true, r),
                None => (false, rest),
            };
            let mut targets = Vec::new();
            let effects = parse_effects(rest, &mut targets)?;
            return Some(vec![Ability::Triggered {
                event,
                targets,
                effects,
                optional,
            }]);
        }
    }
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
    let parts: Vec<&str> = line.trim_end_matches('.').split(", ").collect();
    let ks: Option<Vec<Keyword>> = parts.iter().map(|p| Keyword::parse(p)).collect();
    ks.map(|v| v.into_iter().map(Ability::Keyword).collect())
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
    for (prefix, filter) in [
        (
            "other creatures you control ",
            Filter::creatures().yours().other(),
        ),
        ("creatures you control ", Filter::creatures().yours()),
        ("all creatures ", Filter::creatures()),
        (
            "creatures your opponents control ",
            Filter {
                controller: Some(PlayerRef::Opponents),
                ..Filter::creatures()
            },
        ),
    ] {
        if let Some(rest) = l.strip_prefix(prefix) {
            let mut mods = Vec::new();
            for clause in rest.split(" and ") {
                if let Some(pt) = clause.strip_prefix("get ") {
                    let (p, t) = parse_pt_mod(pt)?;
                    mods.push(Modification::ModifyPT(p, t));
                } else if let Some(k) = clause.strip_prefix("have ") {
                    mods.push(Modification::AddKeyword(Keyword::parse(k)?));
                } else {
                    return None;
                }
            }
            return Some(Ability::Static {
                affected: filter,
                mods,
            });
        }
    }
    None
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
        "target creature card from your graveyard" => {
            TargetSpec::CardInGraveyard(Filter::creatures().yours())
        }
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
        for part in sentence
            .split(", then ")
            .flat_map(|p| p.split(" and you ").map(str::to_string).collect::<Vec<_>>())
        {
            out.push(parse_effect(part.trim(), targets)?);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_effect(s: &str, targets: &mut Vec<TargetSpec>) -> Option<Effect> {
    let s = s.strip_prefix("you ").unwrap_or(s);
    let words: Vec<&str> = s.split_whitespace().collect();
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
        if to == "each creature" {
            return Some(Effect::DealDamageEach {
                filter: Filter::creatures(),
                players: None,
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
        if let Some(t) = rest.strip_suffix(" to the battlefield") {
            return Some(Effect::ReturnFromGraveyardToBattlefield {
                target: push_target(targets, t)?,
            });
        }
    }
    // "target creature gets +3/+3 until end of turn"
    if let Some((who, rest)) = s.split_once(" gets ") {
        let (pt, dur) = rest.split_once(" until end of turn")?;
        if !dur.is_empty() {
            return None;
        }
        let (p, t) = parse_pt_mod(pt)?;
        let affected = if who == "~" {
            Affected::ThisObject
        } else {
            Affected::Target(push_target(targets, who)?)
        };
        return Some(Effect::ApplyContinuous {
            affected,
            mods: vec![Modification::ModifyPT(p, t)],
            duration: Duration::EndOfTurn,
        });
    }
    if let Some((who, rest)) = s.split_once(" gains ") {
        let k = Keyword::parse(rest.strip_suffix(" until end of turn")?)?;
        let affected = if who == "~" {
            Affected::ThisObject
        } else {
            Affected::Target(push_target(targets, who)?)
        };
        return Some(Effect::ApplyContinuous {
            affected,
            mods: vec![Modification::AddKeyword(k)],
            duration: Duration::EndOfTurn,
        });
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
        let m = parse_mana_production(&format!("add {rest}"))?;
        return match m {
            ManaProduction::Fixed(v) => Some(Effect::AddMana { mana: v }),
            ManaProduction::AnyColor => Some(Effect::AddManaAnyColor { amount: 1 }),
            ManaProduction::OneOf(_) => None,
        };
    }
    None
}

fn parse_token(rest: &str) -> Option<Effect> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let n = amount(words.first()?)?;
    let (p, t) = parse_pt_mod(words.get(1)?)?;
    let token_pos = words.iter().position(|w| *w == "token" || *w == "tokens")?;
    let creature_pos = words.iter().position(|w| *w == "creature")?;
    if creature_pos + 1 != token_pos {
        return None;
    }
    let mut colors = Vec::new();
    let mut subtypes = Vec::new();
    for w in &words[2..creature_pos] {
        match *w {
            "white" => colors.push(Color::White),
            "blue" => colors.push(Color::Blue),
            "black" => colors.push(Color::Black),
            "red" => colors.push(Color::Red),
            "green" => colors.push(Color::Green),
            "colorless" | "and" => {}
            other => subtypes.push(capitalize(other)),
        }
    }
    let mut b = CardBuilder::new(&format!("{} Token", subtypes.join(" ")))
        .types(&[CardType::Creature])
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
}
