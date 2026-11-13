//! CR 613: the layer system, timestamps and dependencies.

mod common;
use common::*;
use rules_core::*;

fn stored(g: &mut Game, objects: Vec<ObjectId>, mods: Vec<Modification>) {
    // Resolve an effect the way a spell would, using a throwaway instant.
    let def = CardBuilder::new("Effect")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Permanent(Filter::default())],
            vec![Effect::ApplyContinuous {
                affected: Affected::Target(0),
                mods,
                duration: Duration::EndOfTurn,
            }],
        )
        .build();
    let c = in_hand(g, P0, def);
    cast(g, P0, c, objects.into_iter().map(Target::Object).collect());
    resolve_all(g);
}

#[test]
fn layer_7b_set_applies_before_7c_modify_regardless_of_timestamp() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    // Giant Growth first (+3/+3, 7c), then "becomes 0/1" (7b) with a later timestamp.
    stored(&mut g, vec![bear], vec![Modification::ModifyPT(3, 3)]);
    stored(&mut g, vec![bear], vec![Modification::SetPT(0, 1)]);
    assert_eq!(pt(&g, bear), (3, 4), "613.4b applies before 613.4c");
}

#[test]
fn layer_7d_switch_applies_after_counters_and_pumps() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    stored(&mut g, vec![bear], vec![Modification::SwitchPT]);
    g.add_counters(bear, CounterKind::PlusOnePlusOne, 1); // 3/3 before switching
    stored(&mut g, vec![bear], vec![Modification::ModifyPT(2, 0)]); // 5/3
    assert_eq!(pt(&g, bear), (3, 5));
}

#[test]
fn control_change_happens_before_anthems_choose_what_to_pump() {
    let mut g = game(2);
    put(&mut g, P0, anthem());
    let theirs = put(&mut g, P1, bears());
    assert_eq!(pt(&g, theirs), (2, 2));
    let t = in_hand(&mut g, P0, threaten());
    add_mana(&mut g, P0, "RRR");
    cast(&mut g, P0, t, vec![Target::Object(theirs)]);
    resolve_all(&mut g);
    let c = g.chars(theirs).unwrap();
    assert_eq!(c.controller, P0, "layer 2");
    assert!(c.has_keyword(Keyword::Haste), "layer 6");
    assert_eq!(
        pt(&g, theirs),
        (3, 3),
        "P0's anthem now applies (layer 2 before 7c)"
    );
    // Until end of turn: back to its owner next turn.
    pass_until(&mut g, Step::Upkeep);
    assert_eq!(g.chars(theirs).unwrap().controller, P1);
}

#[test]
fn characteristic_defining_ability_counts_graveyards_in_layer_7a() {
    let mut g = game(2);
    let goyf = put(&mut g, P0, goyf());
    assert_eq!(pt(&g, goyf), (0, 1));
    // A creature and an instant in graveyards: 2 card types.
    g.debug_create(bears(), P1, Zone::Graveyard);
    g.debug_create(bolt(), P0, Zone::Graveyard);
    assert_eq!(pt(&g, goyf), (2, 3));
    g.add_counters(goyf, CounterKind::PlusOnePlusOne, 1);
    assert_eq!(
        pt(&g, goyf),
        (3, 4),
        "counters apply in 7c on top of the CDA"
    );
    // The CDA also works outside the battlefield (604.3).
    let in_hand = in_hand(&mut g, P0, goyf_def());
    assert_eq!(pt(&g, in_hand), (2, 3));
}

fn goyf_def() -> std::sync::Arc<CardDef> {
    common::goyf()
}

#[test]
fn blood_moon_turns_off_urborg_regardless_of_timestamps() {
    // 613.8: Urborg's effect depends on Blood Moon's (Blood Moon removes Urborg's ability),
    // so Blood Moon applies first and Urborg's effect no longer exists.
    for moon_first in [true, false] {
        let mut g = game(2);
        let (moon, urb) = if moon_first {
            let m = put(&mut g, P0, blood_moon());
            (m, put(&mut g, P1, urborg()))
        } else {
            let u = put(&mut g, P1, urborg());
            (put(&mut g, P0, blood_moon()), u)
        };
        let forest = put(&mut g, P1, forest());
        let _ = moon;
        let u = g.chars(urb).unwrap();
        assert!(
            u.has_subtype("Mountain") && !u.has_subtype("Swamp"),
            "moon_first={moon_first}: {:?}",
            u.subtypes
        );
        let f = g.chars(forest).unwrap();
        assert!(!f.has_subtype("Swamp"), "Urborg's effect never applies");
        // Urborg taps for red (305.7 + 305.6).
        let mana: Vec<_> = u.abilities.iter().map(|a| a.ability.clone()).collect();
        assert_eq!(
            mana,
            vec![Ability::Mana {
                cost: Cost::tap(),
                produces: ManaProduction::Fixed(vec![ManaType::Color(Color::Red)])
            }]
        );
    }
}

#[test]
fn urborg_alone_makes_every_land_a_swamp_with_the_mana_ability() {
    let mut g = game(2);
    put(&mut g, P1, urborg());
    let f = put(&mut g, P0, forest());
    let c = g.chars(f).unwrap();
    assert!(c.has_subtype("Forest") && c.has_subtype("Swamp"));
    assert_eq!(
        c.abilities
            .iter()
            .filter(|a| matches!(a.ability, Ability::Mana { .. }))
            .count(),
        2
    );
}

#[test]
fn humility_then_opalescence_timestamp_order() {
    // Official ruling: if Opalescence has the later timestamp, Humility is a 4/4 and other
    // creatures are 1/1.
    let mut g = game(2);
    let hum = put(&mut g, P0, humility());
    let _opal = put(&mut g, P0, opalescence());
    let bear = put(&mut g, P1, bears());
    assert_eq!(pt(&g, hum), (4, 4));
    assert_eq!(pt(&g, bear), (1, 1));
    assert!(
        g.chars(hum).unwrap().abilities.is_empty(),
        "Humility loses its own ability but keeps applying (613.6)"
    );
}

#[test]
fn opalescence_then_humility_timestamp_order() {
    // If Humility has the later timestamp, every creature including Humility is 1/1.
    let mut g = game(2);
    let _opal = put(&mut g, P0, opalescence());
    let hum = put(&mut g, P0, humility());
    let bear = put(&mut g, P1, bears());
    assert_eq!(pt(&g, hum), (1, 1));
    assert_eq!(pt(&g, bear), (1, 1));
}

#[test]
fn effects_applied_later_win_within_a_layer_by_timestamp() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    stored(&mut g, vec![bear], vec![Modification::RemoveAllAbilities]);
    stored(
        &mut g,
        vec![bear],
        vec![Modification::AddKeyword(Keyword::Flying)],
    );
    assert!(g.chars(bear).unwrap().has_keyword(Keyword::Flying));
    let other = put(&mut g, P0, bears());
    stored(
        &mut g,
        vec![other],
        vec![Modification::AddKeyword(Keyword::Flying)],
    );
    stored(&mut g, vec![other], vec![Modification::RemoveAllAbilities]);
    assert!(!g.chars(other).unwrap().has_keyword(Keyword::Flying));
}

#[test]
fn static_effects_are_not_locked_in() {
    // 611.3a: an anthem applies to creatures that arrive later; a resolved pump does not.
    let mut g = game(2);
    put(&mut g, P0, anthem());
    let late = put(&mut g, P0, bears());
    assert_eq!(pt(&g, late), (3, 3));
}
