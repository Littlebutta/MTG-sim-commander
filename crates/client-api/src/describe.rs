//! Short human-readable descriptions of scripted abilities, for ability menus and the stack.

use rules_core::*;

fn amt(a: &Amount) -> String {
    match a {
        Amount::Fixed(n) => n.to_string(),
        Amount::X => "X".into(),
        _ => "N".into(),
    }
}

fn who(p: &PlayerRef) -> &'static str {
    match p {
        PlayerRef::You => "You",
        PlayerRef::Opponents => "Each opponent",
        PlayerRef::Each => "Each player",
        PlayerRef::Target(_) => "Target player",
        PlayerRef::ControllerOfTarget(_) => "Its controller",
    }
}

fn mana(m: &ManaType) -> String {
    match m {
        ManaType::Color(c) => format!("{{{}}}", c.letter()),
        ManaType::Colorless => "{C}".into(),
    }
}

pub fn describe_effect(e: &Effect) -> String {
    match e {
        Effect::DrawCards { who: w, amount } => format!("{} draw {}", who(w), amt(amount)),
        Effect::GainLife { who: w, amount } => format!("{} gain {} life", who(w), amt(amount)),
        Effect::LoseLife { who: w, amount } => format!("{} lose {} life", who(w), amt(amount)),
        Effect::DealDamage { amount, .. } => format!("Deal {} damage", amt(amount)),
        Effect::DealDamageEach { amount, .. } => format!("Deal {} damage to each", amt(amount)),
        Effect::Destroy { .. } => "Destroy target".into(),
        Effect::DestroyAll { .. } => "Destroy all".into(),
        Effect::Exile { .. } => "Exile target".into(),
        Effect::ReturnToHand { .. } => "Return target to hand".into(),
        Effect::Counter { .. } => "Counter target spell".into(),
        Effect::Tap { .. } => "Tap target".into(),
        Effect::Untap { .. } => "Untap target".into(),
        Effect::AddCounters { amount, .. } => format!("Put {} +1/+1 counter(s)", amt(amount)),
        Effect::AddMana { mana: m } => format!("Add {}", m.iter().map(mana).collect::<String>()),
        Effect::AddManaAnyColor { .. } => "Add one mana of any color".into(),
        Effect::CreateToken { token, amount, .. } => {
            format!("Create {} {}", amt(amount), token.name)
        }
        Effect::ApplyContinuous { mods, .. } => mods
            .iter()
            .map(|m| match m {
                Modification::ModifyPT(p, t) => format!("{p:+}/{t:+} until end of turn"),
                Modification::AddKeyword(k) => format!("gains {}", k.label()),
                other => format!("{other:?}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Effect::Mill { amount, .. } => format!("Mill {}", amt(amount)),
        Effect::ReturnFromGraveyardToBattlefield { .. } => {
            "Return target card to the battlefield".into()
        }
        Effect::ReturnFromGraveyardToHand { .. } => "Return target card to hand".into(),
        Effect::Sacrifice { .. } => "Sacrifice".into(),
        Effect::Attach { .. } => "Attach to target creature".into(),
        Effect::Scry { amount } => format!("Scry {}", amt(amount)),
        Effect::Surveil { amount } => format!("Surveil {}", amt(amount)),
        Effect::SearchLibrary { count, .. } => {
            format!("Search your library for {} card(s)", count)
        }
        Effect::AddCountersEach { amount, .. } => {
            format!("Put {} +1/+1 counter(s) on each", amt(amount))
        }
        Effect::Discard { who: w, amount } => format!("{} discard {}", who(w), amt(amount)),
        Effect::Fight { .. } => "Fight target creature".into(),
        Effect::Regenerate { .. } => "Regenerate".into(),
        Effect::ReturnThisToHand => "Return this to its owner's hand".into(),
        Effect::UntapThis => "Untap this".into(),
        Effect::ControllerOfTargetGainsLifeEqualToPower { .. } => {
            "Its controller gains life equal to its power".into()
        }
    }
}

pub fn describe_effects(es: &[Effect]) -> String {
    es.iter()
        .map(describe_effect)
        .collect::<Vec<_>>()
        .join(". ")
}

fn cost(c: &Cost) -> String {
    let mut parts = Vec::new();
    if !c.mana.0.is_empty() {
        parts.push(c.mana.to_string());
    }
    if c.tap {
        parts.push("{T}".into());
    }
    if c.sacrifice_this {
        parts.push("Sacrifice this".into());
    }
    if c.pay_life > 0 {
        parts.push(format!("Pay {} life", c.pay_life));
    }
    if c.discard_this {
        parts.push("Discard this card".into());
    }
    if c.discard > 0 {
        parts.push(match c.discard {
            1 => "Discard a card".into(),
            n => format!("Discard {n} cards"),
        });
    }
    if let Some((_, n)) = &c.sacrifice {
        parts.push(match n {
            1 => "Sacrifice a permanent".into(),
            n => format!("Sacrifice {n} permanents"),
        });
    }
    if let Some((k, n)) = &c.remove_counters {
        let k = match k {
            CounterKind::PlusOnePlusOne => "+1/+1".to_string(),
            CounterKind::MinusOneMinusOne => "-1/-1".into(),
            CounterKind::Loyalty => "loyalty".into(),
            CounterKind::Other(s) => s.clone(),
        };
        parts.push(format!(
            "Remove {n} {k} counter{}",
            if *n == 1 { "" } else { "s" }
        ));
    }
    if let Some(l) = c.loyalty {
        parts.push(if l > 0 {
            format!("+{l}")
        } else {
            l.to_string()
        });
    }
    parts.join(", ")
}

pub fn describe_ability(a: &Ability) -> String {
    match a {
        Ability::Keyword(k) => k.label(),
        Ability::Mana { cost: c, produces } => {
            let what = match produces {
                ManaProduction::Fixed(v) => v.iter().map(mana).collect::<String>(),
                ManaProduction::OneOf(v) => v.iter().map(mana).collect::<Vec<_>>().join(" or "),
                ManaProduction::AnyColor => "one mana of any color".into(),
                ManaProduction::CommanderIdentity => {
                    "one mana of any color in your commander's color identity".into()
                }
            };
            format!("{}: Add {}", cost(c), what)
        }
        Ability::Activated {
            cost: c, effects, ..
        } => format!("{}: {}", cost(c), describe_effects(effects)),
        Ability::Triggered { effects, .. } => describe_effects(effects),
        Ability::Static { .. } => "Static ability".into(),
        Ability::CharacteristicPT { .. } => "Power/toughness ability".into(),
        Ability::EntersTapped => "Enters tapped".into(),
        Ability::EntersWithCounters { amount, .. } => {
            format!("Enters with {} +1/+1 counter(s)", amt(amount))
        }
        Ability::NoMaxHandSize => "No maximum hand size".into(),
        Ability::EntersTappedUnless(_) => "Enters tapped unless a condition is met".into(),
        Ability::Unsupported(t) => t.clone(),
    }
}
