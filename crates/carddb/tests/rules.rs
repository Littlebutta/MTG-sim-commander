//! Cards scripted from their oracle text, played through the rules engine: Auras, Equipment,
//! planeswalkers, land searches and the other wordings the card support pass added.

#[path = "../../rules-core/tests/common/mod.rs"]
mod common;
use common::*;
use rules_core::*;
use std::sync::Arc;

fn card(name: &str, cost: &str, types: &[CardType], subtypes: &[&str], text: &str) -> Arc<CardDef> {
    let mut d = CardDef {
        name: name.into(),
        mana_cost: ManaCost::parse(cost).unwrap(),
        types: types.iter().copied().collect(),
        subtypes: subtypes.iter().map(|s| s.to_string()).collect(),
        oracle_text: text.into(),
        ..Default::default()
    };
    if types.contains(&CardType::Creature) {
        d.power = Some(2);
        d.toughness = Some(2);
    }
    if types.contains(&CardType::Planeswalker) {
        d.loyalty = Some(3);
    }
    carddb::oracle::script(&mut d, name, text);
    assert!(d.fully_supported(), "{name}: {:?}", d.abilities);
    Arc::new(d)
}

fn has(g: &Game, id: ObjectId, k: Keyword) -> bool {
    g.chars(id).unwrap().has_keyword(k)
}

#[test]
fn an_aura_enters_attached_to_its_target_and_leaves_with_it() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    let aura = in_hand(
        &mut g,
        P0,
        card(
            "Rancor Lite",
            "{G}",
            &[CardType::Enchantment],
            &["Aura"],
            "Enchant creature\nEnchanted creature gets +2/+0 and has trample.",
        ),
    );
    add_mana(&mut g, P0, "G");
    cast(&mut g, P0, aura, vec![Target::Object(bear)]);
    resolve_all(&mut g);
    assert_eq!(pt(&g, bear), (4, 2));
    assert!(has(&g, bear, Keyword::Trample));
    // The creature dies; the Aura goes to the graveyard as a state-based action.
    let bolt = in_hand(&mut g, P0, bolt());
    add_mana(&mut g, P0, "R");
    cast(&mut g, P0, bolt, vec![Target::Object(bear)]);
    resolve_all(&mut g);
    assert!(!g
        .battlefield
        .iter()
        .any(|id| g.object(*id).unwrap().def.name == "Rancor Lite"));
}

#[test]
fn pacifism_stops_attacks_and_blocks() {
    let mut g = game(2);
    let bear = put(&mut g, P1, bears());
    let aura = in_hand(
        &mut g,
        P0,
        card(
            "Pacifism",
            "{1}{W}",
            &[CardType::Enchantment],
            &["Aura"],
            "Enchant creature\nEnchanted creature can't attack or block.",
        ),
    );
    add_mana(&mut g, P0, "WW");
    cast(&mut g, P0, aura, vec![Target::Object(bear)]);
    resolve_all(&mut g);
    assert!(has(&g, bear, Keyword::CantAttack) && has(&g, bear, Keyword::CantBlock));
    let mine = put(&mut g, P0, vanilla("Ogre", 3, 3));
    while !matches!(g.decision(), Decision::DeclareAttackers { .. }) {
        default_answer(&mut g);
    }
    g.submit(P0, Action::Attack(vec![(mine, P1)])).unwrap();
    while !matches!(g.decision(), Decision::DeclareBlockers { .. }) {
        default_answer(&mut g);
    }
    assert!(g.submit(P1, Action::Block(vec![(bear, mine)])).is_err());
}

#[test]
fn equipment_attaches_for_its_equip_cost_and_moves_on() {
    let mut g = game(2);
    let a = put(&mut g, P0, bears());
    let b = put(&mut g, P0, vanilla("Ogre", 3, 3));
    let sword = put(
        &mut g,
        P0,
        card(
            "Short Sword",
            "{1}",
            &[CardType::Artifact],
            &["Equipment"],
            "Equipped creature gets +1/+1.\nEquip {1}",
        ),
    );
    add_mana(&mut g, P0, "C");
    g.submit(
        P0,
        Action::Activate {
            object: sword,
            ability: 1,
            targets: vec![Target::Object(a)],
            x: 0,
        },
    )
    .unwrap();
    resolve_all(&mut g);
    assert_eq!(pt(&g, a), (3, 3));
    add_mana(&mut g, P0, "C");
    g.submit(
        P0,
        Action::Activate {
            object: sword,
            ability: 1,
            targets: vec![Target::Object(b)],
            x: 0,
        },
    )
    .unwrap();
    resolve_all(&mut g);
    assert_eq!(pt(&g, a), (2, 2));
    assert_eq!(pt(&g, b), (4, 4));
    // Equip is sorcery-speed: not with something on the stack.
    let growth = in_hand(&mut g, P0, giant_growth());
    add_mana(&mut g, P0, "GC");
    cast(&mut g, P0, growth, vec![Target::Object(a)]);
    assert!(g
        .submit(
            P0,
            Action::Activate {
                object: sword,
                ability: 1,
                targets: vec![Target::Object(a)],
                x: 0
            }
        )
        .is_err());
}

#[test]
fn planeswalkers_enter_with_loyalty_and_use_one_ability_a_turn() {
    let mut g = game(2);
    let walker = in_hand(
        &mut g,
        P0,
        card(
            "Garruk Lite",
            "{2}{G}{G}",
            &[CardType::Planeswalker],
            &["Garruk"],
            "+1: You gain 2 life.\n−1: Create a 3/3 green Beast creature token.",
        ),
    );
    add_mana(&mut g, P0, "GGGG");
    cast(&mut g, P0, walker, vec![]);
    resolve_all(&mut g);
    let w = *g
        .battlefield
        .iter()
        .find(|id| g.object(**id).unwrap().def.name == "Garruk Lite")
        .unwrap();
    assert_eq!(
        g.object(w).unwrap().counters.get(&CounterKind::Loyalty),
        Some(&3)
    );
    g.submit(
        P0,
        Action::Activate {
            object: w,
            ability: 1,
            targets: vec![],
            x: 0,
        },
    )
    .unwrap();
    assert_eq!(
        g.object(w).unwrap().counters.get(&CounterKind::Loyalty),
        Some(&2)
    );
    resolve_all(&mut g);
    assert!(g
        .battlefield
        .iter()
        .any(|id| g.object(*id).unwrap().def.name.contains("Beast")));
    assert!(g
        .submit(
            P0,
            Action::Activate {
                object: w,
                ability: 0,
                targets: vec![],
                x: 0
            }
        )
        .is_err());
}

#[test]
fn land_searches_tapped_lands_and_painlands() {
    let mut g = game(2);
    let n = g.player(P0).library.len();
    let cultivate = in_hand(
        &mut g,
        P0,
        card(
            "Cultivate",
            "{2}{G}",
            &[CardType::Sorcery],
            &[],
            "Search your library for up to two basic land cards, reveal those cards, put one onto the battlefield tapped and the other into your hand, then shuffle.",
        ),
    );
    let lands_before = g.battlefield.len();
    let hand_before = g.hand(P0).len();
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, cultivate, vec![]);
    resolve_all(&mut g);
    assert_eq!(g.player(P0).library.len(), n - 2);
    assert_eq!(g.battlefield.len(), lands_before + 1);
    assert_eq!(g.hand(P0).len(), hand_before); // the spell left, a land came in
    let fetched = *g.battlefield.last().unwrap();
    assert!(g.object(fetched).unwrap().tapped);

    let gate = put(
        &mut g,
        P0,
        card(
            "Dimir Guildgate",
            "",
            &[CardType::Land],
            &["Gate"],
            "This land enters tapped.\n{T}: Add {U} or {B}.",
        ),
    );
    assert!(g.object(gate).unwrap().tapped);

    let pain = put(
        &mut g,
        P0,
        card(
            "Llanowar Wastes",
            "",
            &[CardType::Land],
            &[],
            "{T}: Add {C}.\n{T}: Add {B} or {G}. This land deals 1 damage to you.",
        ),
    );
    let life = g.player(P0).life;
    g.submit(
        P0,
        Action::ActivateMana {
            object: pain,
            ability: 1,
            color: Some(Color::Green),
        },
    )
    .unwrap();
    assert_eq!(g.player(P0).life, life - 1);
}

#[test]
fn command_tower_makes_only_commander_colors() {
    let mut cmd = (*commander_card("Golgari Boss")).clone();
    cmd.color_identity = ColorSet::of(&[Color::Black, Color::Green]);
    let decks = (0..2)
        .map(|i| DeckSpec {
            player_name: format!("P{i}"),
            commanders: vec![Arc::new(cmd.clone())],
            cards: (0..99).map(|_| forest()).collect(),
        })
        .collect();
    let mut g = Game::new_commander(
        GameConfig {
            seed: 7,
            starting_player: Some(0),
            skip_mulligans: true,
            ..Default::default()
        },
        decks,
    );
    pass_until(&mut g, Step::Main1);
    let tower = put(
        &mut g,
        P0,
        card(
            "Command Tower",
            "",
            &[CardType::Land],
            &[],
            "{T}: Add one mana of any color in your commander's color identity.",
        ),
    );
    assert!(g
        .submit(
            P0,
            Action::ActivateMana {
                object: tower,
                ability: 0,
                color: Some(Color::Red)
            }
        )
        .is_err());
    g.submit(
        P0,
        Action::ActivateMana {
            object: tower,
            ability: 0,
            color: Some(Color::Black),
        },
    )
    .unwrap();
    assert_eq!(g.player(P0).mana_pool.get(ManaType::Color(Color::Black)), 1);
}

#[test]
fn swords_gives_life_to_the_exiled_creatures_controller_and_beast_within_a_token() {
    let mut g = game(2);
    let ogre = put(&mut g, P1, vanilla("Ogre", 3, 3));
    let swords = in_hand(
        &mut g,
        P0,
        card(
            "Swords to Plowshares",
            "{W}",
            &[CardType::Instant],
            &[],
            "Exile target creature. Its controller gains life equal to its power.",
        ),
    );
    let life = g.player(P1).life;
    add_mana(&mut g, P0, "W");
    cast(&mut g, P0, swords, vec![Target::Object(ogre)]);
    resolve_all(&mut g);
    assert_eq!(g.player(P1).life, life + 3);

    let bear = put(&mut g, P1, bears());
    let within = in_hand(
        &mut g,
        P0,
        card(
            "Beast Within",
            "{2}{G}",
            &[CardType::Instant],
            &[],
            "Destroy target permanent. Its controller creates a 3/3 green Beast creature token.",
        ),
    );
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, within, vec![Target::Object(bear)]);
    resolve_all(&mut g);
    let view = g.view();
    assert!(g.battlefield.iter().any(|id| {
        g.object(*id).unwrap().def.name.contains("Beast") && view.get(*id).unwrap().controller == P1
    }));
}

#[test]
fn prowess_pumps_on_noncreature_spells() {
    let mut g = game(2);
    let monk = put(
        &mut g,
        P0,
        card(
            "Swiftspear",
            "{R}",
            &[CardType::Creature],
            &["Monk"],
            "Haste\nProwess",
        ),
    );
    let bolt = in_hand(&mut g, P0, bolt());
    add_mana(&mut g, P0, "R");
    cast(&mut g, P0, bolt, vec![Target::Player(P1)]);
    resolve_all(&mut g);
    assert_eq!(pt(&g, monk), (3, 3));
}
