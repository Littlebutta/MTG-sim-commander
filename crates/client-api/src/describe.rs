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
                Modification::AddKeyword(k) => format!("gains {k:?}"),
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
    parts.join(", ")
}

pub fn describe_ability(a: &Ability) -> String {
    match a {
        Ability::Keyword(k) => format!("{k:?}"),
        Ability::Mana { cost: c, produces } => {
            let what = match produces {
                ManaProduction::Fixed(v) => v.iter().map(mana).collect::<String>(),
                ManaProduction::OneOf(v) => v.iter().map(mana).collect::<Vec<_>>().join(" or "),
                ManaProduction::AnyColor => "one mana of any color".into(),
            };
            format!("{}: Add {}", cost(c), what)
        }
        Ability::Activated {
            cost: c, effects, ..
        } => format!("{}: {}", cost(c), describe_effects(effects)),
        Ability::Triggered { effects, .. } => describe_effects(effects),
        Ability::Static { .. } => "Static ability".into(),
        Ability::CharacteristicPT { .. } => "Power/toughness ability".into(),
        Ability::Unsupported(t) => t.clone(),
    }
}
