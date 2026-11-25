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

fn find(g: &Game, name: &str) -> ObjectId {
    *g.battlefield
        .iter()
        .find(|id| g.object(**id).unwrap().def.name == name)
        .unwrap()
}

#[test]
fn cycling_discards_from_hand_and_draws() {
    let mut g = game(2);
    let crash = in_hand(
        &mut g,
        P0,
        card(
            "Airship Crash",
            "{2}{G}",
            &[CardType::Instant],
            &[],
            "Destroy target artifact, enchantment, or creature with flying.\nCycling {2} ({2}, Discard this card: Draw a card.)",
        ),
    );
    assert!(g.activatable(P0).is_empty()); // no mana yet
    add_mana(&mut g, P0, "CC");
    let ability = g
        .activatable(P0)
        .iter()
        .find(|(o, _)| *o == crash)
        .unwrap()
        .1;
    let hand = g.hand(P0).len();
    g.submit(
        P0,
        Action::Activate {
            object: crash,
            ability,
            targets: vec![],
            x: 0,
        },
    )
    .unwrap();
    resolve_all(&mut g);
    // The cycled card left, a new card came in.
    assert_eq!(g.hand(P0).len(), hand);
    assert!(!g.hand(P0).contains(&crash));
    assert!(g
        .player(P0)
        .graveyard
        .iter()
        .any(|id| g.object(*id).unwrap().def.name == "Airship Crash"));
}

#[test]
fn modal_spells_do_the_chosen_mode() {
    let mut g = game(2);
    let bear = put(&mut g, P1, bears());
    let charm = in_hand(
        &mut g,
        P0,
        card(
            "Test Charm",
            "{R}",
            &[CardType::Instant],
            &[],
            "Choose one —\n• Target player gains 5 life.\n• Destroy target creature.",
        ),
    );
    assert_eq!(g.object(charm).unwrap().def.spell_modes.len(), 2);
    add_mana(&mut g, P0, "R");
    // A mode is required, and targets are checked against that mode.
    assert!(g
        .submit(
            P0,
            Action::Cast {
                card: charm,
                targets: vec![Target::Object(bear)],
                x: 0,
                mode: None,
                opts: Default::default(),
            }
        )
        .is_err());
    assert!(g
        .submit(
            P0,
            Action::Cast {
                card: charm,
                targets: vec![Target::Object(bear)],
                x: 0,
                mode: Some(0),
                opts: Default::default(),
            }
        )
        .is_err());
    g.submit(
        P0,
        Action::Cast {
            card: charm,
            targets: vec![Target::Object(bear)],
            x: 0,
            mode: Some(1),
            opts: Default::default(),
        },
    )
    .unwrap();
    resolve_all(&mut g);
    assert!(!g.battlefield.contains(&bear));
}

#[test]
fn regeneration_saves_a_creature_once() {
    let mut g = game(2);
    let troll = put(
        &mut g,
        P0,
        card(
            "Test Troll",
            "{1}{G}",
            &[CardType::Creature],
            &["Troll"],
            "{G}: Regenerate Test Troll.",
        ),
    );
    add_mana(&mut g, P0, "G");
    g.submit(
        P0,
        Action::Activate {
            object: troll,
            ability: 0,
            targets: vec![],
            x: 0,
        },
    )
    .unwrap();
    resolve_all(&mut g);
    for alive in [true, false] {
        let b = in_hand(&mut g, P0, bolt());
        add_mana(&mut g, P0, "R");
        cast(&mut g, P0, b, vec![Target::Object(troll)]);
        resolve_all(&mut g);
        assert_eq!(g.battlefield.contains(&troll), alive);
        if alive {
            assert!(g.object(troll).unwrap().tapped);
        }
    }
}

#[test]
fn protection_stops_targeting_and_damage_and_landwalk_stops_blocks() {
    let mut g = game(2);
    let knight = put(
        &mut g,
        P1,
        card(
            "White Knight",
            "{W}{W}",
            &[CardType::Creature],
            &["Knight"],
            "First strike\nProtection from red",
        ),
    );
    let b = in_hand(&mut g, P0, bolt());
    add_mana(&mut g, P0, "R");
    assert!(g
        .submit(
            P0,
            Action::Cast {
                card: b,
                targets: vec![Target::Object(knight)],
                x: 0,
                mode: None,
                opts: Default::default(),
            }
        )
        .is_err());

    let walker = put(
        &mut g,
        P0,
        card(
            "Forest Walker",
            "{G}",
            &[CardType::Creature],
            &[],
            "Forestwalk",
        ),
    );
    put(&mut g, P1, forest());
    let blocker = put(&mut g, P1, bears());
    while !matches!(g.decision(), Decision::DeclareAttackers { .. }) {
        default_answer(&mut g);
    }
    g.submit(P0, Action::Attack(vec![(walker, P1)])).unwrap();
    while !matches!(g.decision(), Decision::DeclareBlockers { .. }) {
        default_answer(&mut g);
    }
    assert!(g
        .submit(P1, Action::Block(vec![(blocker, walker)]))
        .is_err());
}

#[test]
fn checklands_enter_untapped_with_the_right_land() {
    let mut g = game(2);
    let text =
        "This land enters tapped unless you control a Forest or an Island.\n{T}: Add {G} or {U}.";
    let first = put(
        &mut g,
        P0,
        card("Hinterland Harbor", "", &[CardType::Land], &[], text),
    );
    assert!(g.object(first).unwrap().tapped);
    put(&mut g, P0, forest());
    let second = put(
        &mut g,
        P0,
        card("Hinterland Harbor", "", &[CardType::Land], &[], text),
    );
    assert!(!g.object(second).unwrap().tapped);
}

#[test]
fn cast_and_combat_triggers_and_sacrifice_costs() {
    let mut g = game(2);
    let hand = g.hand(P0).len();
    let seer = in_hand(
        &mut g,
        P0,
        card(
            "Cast Seer",
            "{1}",
            &[CardType::Creature],
            &[],
            "When you cast this spell, draw a card.",
        ),
    );
    add_mana(&mut g, P0, "C");
    cast(&mut g, P0, seer, vec![]);
    resolve_all(&mut g);
    // Seer went from hand to the battlefield, and its cast trigger drew a card.
    assert_eq!(g.hand(P0).len(), hand + 1);

    let altar = put(
        &mut g,
        P0,
        card(
            "Test Altar",
            "{1}",
            &[CardType::Artifact],
            &[],
            "Sacrifice a creature: You gain 2 life.\nAt the beginning of combat on your turn, you gain 1 life.",
        ),
    );
    let life = g.player(P0).life;
    g.submit(
        P0,
        Action::Activate {
            object: altar,
            ability: 0,
            targets: vec![],
            x: 0,
        },
    )
    .unwrap();
    resolve_all(&mut g);
    assert_eq!(g.player(P0).life, life + 2);
    assert!(g.battlefield.contains(&find(&g, "Test Altar")));
    assert!(!g
        .battlefield
        .iter()
        .any(|id| g.object(*id).unwrap().def.name == "Cast Seer"));
    pass_until(&mut g, Step::BeginCombat);
    assert_eq!(g.player(P0).life, life + 3);
}

/// Resolve the stack, including triggers waiting to be put on it.
fn settle(g: &mut Game) {
    for _ in 0..1000 {
        if g.stack.is_empty() && !matches!(g.decision(), Decision::PutTriggersOnStack { .. }) {
            return;
        }
        default_answer(g);
    }
    panic!("never settled");
}

fn cast_with(g: &mut Game, p: PlayerId, card: ObjectId, targets: Vec<Target>, opts: CastOptions) {
    g.submit(
        p,
        Action::Cast {
            card,
            targets,
            x: 0,
            mode: None,
            opts,
        },
    )
    .unwrap();
}

#[test]
fn flashback_casts_from_the_graveyard_and_exiles_it() {
    let mut g = game(2);
    let life = g.player(P1).life;
    let spell = in_hand(
        &mut g,
        P0,
        card(
            "Test Flare",
            "{R}",
            &[CardType::Sorcery],
            &[],
            "Test Flare deals 2 damage to any target.\nFlashback {1}{R}",
        ),
    );
    add_mana(&mut g, P0, "R");
    cast(&mut g, P0, spell, vec![Target::Player(P1)]);
    resolve_all(&mut g);
    let in_yard = *g.player(P0).graveyard.last().unwrap();
    assert_eq!(g.object(in_yard).unwrap().def.name, "Test Flare");
    assert!(g.castable(P0).is_empty());
    add_mana(&mut g, P0, "RR");
    assert!(g.castable(P0).contains(&in_yard));
    cast(&mut g, P0, in_yard, vec![Target::Player(P1)]);
    resolve_all(&mut g);
    assert_eq!(g.player(P1).life, life - 4);
    assert!(g
        .exile
        .iter()
        .any(|id| g.object(*id).unwrap().def.name == "Test Flare"));
}

#[test]
fn kicker_costs_more_and_does_more() {
    let mut g = game(2);
    let life = g.player(P1).life;
    let text = "Kicker {2}\nTest Bolt deals 2 damage to any target. If this spell was kicked, it deals 5 damage instead.";
    let a = in_hand(
        &mut g,
        P0,
        card("Test Bolt", "{R}", &[CardType::Instant], &[], text),
    );
    add_mana(&mut g, P0, "R");
    // Can't afford the kicker with one mana.
    assert!(!g.cast_ways(P0).contains(&(
        a,
        CastOptions {
            kicked: true,
            ..Default::default()
        }
    )));
    cast(&mut g, P0, a, vec![Target::Player(P1)]);
    resolve_all(&mut g);
    assert_eq!(g.player(P1).life, life - 2);
    let b = in_hand(
        &mut g,
        P0,
        card("Test Bolt", "{R}", &[CardType::Instant], &[], text),
    );
    add_mana(&mut g, P0, "RCC");
    cast_with(
        &mut g,
        P0,
        b,
        vec![Target::Player(P1)],
        CastOptions {
            kicked: true,
            ..Default::default()
        },
    );
    resolve_all(&mut g);
    assert_eq!(g.player(P1).life, life - 7);

    // "If it was kicked, it enters with counters."
    let c = in_hand(
        &mut g,
        P0,
        card(
            "Test Kavu",
            "{G}",
            &[CardType::Creature],
            &[],
            "Kicker {1}\nIf Test Kavu was kicked, it enters with two +1/+1 counters on it.",
        ),
    );
    add_mana(&mut g, P0, "GC");
    cast_with(
        &mut g,
        P0,
        c,
        vec![],
        CastOptions {
            kicked: true,
            ..Default::default()
        },
    );
    resolve_all(&mut g);
    assert_eq!(pt(&g, find(&g, "Test Kavu")), (4, 4));
}

#[test]
fn crew_turns_a_vehicle_into_a_creature() {
    let mut g = game(2);
    let mut d = (*card(
        "Test Cart",
        "{2}",
        &[CardType::Artifact],
        &["Vehicle"],
        "Trample\nCrew 2",
    ))
    .clone();
    d.power = Some(4);
    d.toughness = Some(4);
    let cart = put(&mut g, P0, Arc::new(d));
    assert!(!g.chars(cart).unwrap().is_creature());
    // A 1/1 alone can't crew 2.
    let small = put(&mut g, P0, vanilla("Squire", 1, 1));
    assert!(g.activatable(P0).iter().all(|(o, _)| *o != cart));
    let bear = put(&mut g, P0, bears());
    let ability = g
        .activatable(P0)
        .iter()
        .find(|(o, _)| *o == cart)
        .unwrap()
        .1;
    g.submit(
        P0,
        Action::Activate {
            object: cart,
            ability,
            targets: vec![],
            x: 0,
        },
    )
    .unwrap();
    resolve_all(&mut g);
    assert!(g.chars(cart).unwrap().is_creature());
    assert_eq!(pt(&g, cart), (4, 4));
    // The bear (just enough power) tapped, the squire didn't.
    assert!(g.object(bear).unwrap().tapped);
    assert!(!g.object(small).unwrap().tapped);
}

#[test]
fn ward_counters_unless_paid() {
    let mut g = game(2);
    let warded = put(
        &mut g,
        P1,
        card(
            "Warded Bear",
            "{1}{G}",
            &[CardType::Creature],
            &[],
            "Ward {2}",
        ),
    );
    // No mana left over to pay ward: the bolt is countered.
    let b = in_hand(&mut g, P0, bolt());
    add_mana(&mut g, P0, "R");
    cast(&mut g, P0, b, vec![Target::Object(warded)]);
    resolve_all(&mut g);
    assert!(g.battlefield.contains(&warded));
    // With two more mana it gets paid.
    let b = in_hand(&mut g, P0, bolt());
    add_mana(&mut g, P0, "RCC");
    cast(&mut g, P0, b, vec![Target::Object(warded)]);
    resolve_all(&mut g);
    assert!(!g.battlefield.contains(&warded));
}

#[test]
fn morph_is_a_hidden_two_two_until_turned_face_up() {
    let mut g = game(2);
    let m = in_hand(
        &mut g,
        P0,
        card(
            "Test Morph",
            "{4}{G}{G}",
            &[CardType::Creature],
            &["Beast"],
            "Trample\nMorph {2}{G}\nWhen Test Morph is turned face up, you gain 3 life.",
        ),
    );
    add_mana(&mut g, P0, "CCC");
    // Only the face-down way is affordable.
    assert_eq!(
        g.cast_ways(P0)
            .iter()
            .filter(|(c, _)| *c == m)
            .map(|(_, o)| o.face_down)
            .collect::<Vec<_>>(),
        vec![true]
    );
    cast_with(
        &mut g,
        P0,
        m,
        vec![],
        CastOptions {
            face_down: true,
            ..Default::default()
        },
    );
    resolve_all(&mut g);
    let id = *g
        .battlefield
        .iter()
        .find(|id| g.object(**id).unwrap().face_down)
        .unwrap();
    let ch = g.chars(id).unwrap();
    assert_eq!(
        (ch.power, ch.toughness, ch.name.as_str()),
        (Some(2), Some(2), "")
    );
    assert!(!ch.has_keyword(Keyword::Trample));
    assert!(!g.log.iter().any(|l| l.contains("Test Morph")));
    let life = g.player(P0).life;
    add_mana(&mut g, P0, "GCC");
    g.submit(P0, Action::TurnFaceUp(id)).unwrap();
    resolve_all(&mut g);
    assert_eq!(pt(&g, id), (2, 2)); // the card() helper's 2/2
    assert!(g.chars(id).unwrap().has_keyword(Keyword::Trample));
    assert_eq!(g.player(P0).life, life + 3);
}

#[test]
fn convoke_taps_creatures_to_pay() {
    let mut g = game(2);
    let a = put(&mut g, P0, bears());
    let b = put(&mut g, P0, bears());
    let spell = in_hand(
        &mut g,
        P0,
        card(
            "Test Convoke",
            "{2}{G}",
            &[CardType::Sorcery],
            &[],
            "Convoke\nYou gain 4 life.",
        ),
    );
    add_mana(&mut g, P0, "G");
    assert!(g.castable(P0).contains(&spell));
    let life = g.player(P0).life;
    cast(&mut g, P0, spell, vec![]);
    resolve_all(&mut g);
    assert_eq!(g.player(P0).life, life + 4);
    assert!(g.object(a).unwrap().tapped && g.object(b).unwrap().tapped);
}

#[test]
fn infect_poisons_and_undying_returns() {
    let mut g = game(2);
    let mut d = (*card("Test Infector", "{B}", &[CardType::Creature], &[], "Infect")).clone();
    d.power = Some(3);
    let inf = put(&mut g, P0, Arc::new(d));
    let blocker = put(&mut g, P1, vanilla("Wall", 0, 4));
    while !matches!(g.decision(), Decision::DeclareAttackers { .. }) {
        default_answer(&mut g);
    }
    g.submit(P0, Action::Attack(vec![(inf, P1)])).unwrap();
    while !matches!(g.decision(), Decision::DeclareBlockers { .. }) {
        default_answer(&mut g);
    }
    g.submit(P1, Action::Block(vec![])).unwrap();
    pass_until(&mut g, Step::Main2);
    assert_eq!(g.player(P1).poison, 3);
    let _ = blocker;

    let ghoul = put(
        &mut g,
        P0,
        card("Test Ghoul", "{B}", &[CardType::Creature], &[], "Undying"),
    );
    let b = in_hand(&mut g, P0, bolt());
    add_mana(&mut g, P0, "R");
    cast(&mut g, P0, b, vec![Target::Object(ghoul)]);
    resolve_all(&mut g);
    let back = find(&g, "Test Ghoul");
    assert_eq!(pt(&g, back), (3, 3));
}

#[test]
fn evoke_monarch_toxic_and_shocklands() {
    let mut g = game(2);
    // Evoke: cheap, the ETB still happens, then it's sacrificed.
    let ele = in_hand(
        &mut g,
        P0,
        card(
            "Test Elemental",
            "{3}{W}{W}",
            &[CardType::Creature],
            &[],
            "When Test Elemental enters, you become the monarch.\nEvoke {W}",
        ),
    );
    add_mana(&mut g, P0, "W");
    cast_with(
        &mut g,
        P0,
        ele,
        vec![],
        CastOptions {
            evoke: true,
            ..Default::default()
        },
    );
    settle(&mut g);
    assert_eq!(g.monarch, Some(P0));
    assert!(!g
        .battlefield
        .iter()
        .any(|id| g.object(*id).unwrap().def.name == "Test Elemental"));
    // The monarch draws at their end step.
    let hand = g.hand(P0).len();
    pass_until(&mut g, Step::End);
    assert_eq!(g.hand(P0).len(), hand + 1);

    // Shockland: plenty of life, so it's paid and the land enters untapped.
    let life = g.player(P1).life;
    let shock = put(
        &mut g,
        P1,
        card(
            "Test Shock",
            "",
            &[CardType::Land],
            &[],
            "As Test Shock enters, you may pay 2 life. If you don't, it enters tapped.\n{T}: Add {R} or {G}.",
        ),
    );
    assert!(!g.object(shock).unwrap().tapped);
    assert_eq!(g.player(P1).life, life - 2);

    // Toxic: P1 attacks P0 on their turn and takes the crown.
    let viper = put(
        &mut g,
        P1,
        card("Test Viper", "{G}", &[CardType::Creature], &[], "Toxic 2"),
    );
    while !(g.active == P1 && matches!(g.decision(), Decision::DeclareAttackers { .. })) {
        default_answer(&mut g);
    }
    g.submit(P1, Action::Attack(vec![(viper, P0)])).unwrap();
    pass_until(&mut g, Step::Main2);
    assert_eq!(g.player(P0).poison, 2);
    assert_eq!(g.monarch, Some(P1));
}
