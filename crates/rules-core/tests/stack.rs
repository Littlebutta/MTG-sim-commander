//! CR 117 (priority), 405 (stack), 601 (casting), 603 (triggers), 608 (resolution), 704 (SBAs).

mod common;
use common::*;
use rules_core::*;

#[test]
fn commander_game_setup() {
    let g = game(4);
    for p in &g.players {
        assert_eq!(p.life, 40);
        assert_eq!(p.commanders.len(), 1);
    }
    assert_eq!(
        g.command.len(),
        4,
        "903.6: commanders start in the command zone"
    );
    // Multiplayer: nobody skips their first draw (103.8c).
    assert_eq!(g.hand(P0).len(), 8);
    assert_eq!(g.hand(P1).len(), 7);
    assert_eq!(g.step, Step::Main1);
    assert_eq!(priority(&g), P0);
}

#[test]
fn two_player_game_starting_player_skips_first_draw() {
    let g = game(2);
    assert_eq!(g.hand(P0).len(), 7);
}

#[test]
fn step_ends_only_when_all_players_pass_in_succession() {
    let mut g = game(4);
    for p in [P0, P1, P2] {
        assert_eq!(priority(&g), p);
        g.submit(p, Action::Pass).unwrap();
        assert_eq!(g.step, Step::Main1);
    }
    g.submit(P3, Action::Pass).unwrap();
    assert_eq!(g.step, Step::BeginCombat);
}

#[test]
fn only_the_player_with_priority_can_act() {
    let mut g = game(2);
    assert_eq!(g.submit(P1, Action::Pass), Err(RulesError::NotYourDecision));
}

#[test]
fn sorcery_speed_rules() {
    let mut g = game(2);
    let bear = in_hand(&mut g, P1, bears());
    add_mana(&mut g, P1, "GG");
    priority_to(&mut g, P1);
    let r = g.submit(
        P1,
        Action::Cast {
            card: bear,
            targets: vec![],
            x: 0,
        },
    );
    assert!(
        matches!(r, Err(RulesError::Illegal(_))),
        "not P1's main phase"
    );
}

#[test]
fn last_in_first_out_and_target_fizzle() {
    // P0 casts Giant Growth on their bear; P1 responds with Lightning Bolt. Bolt resolves
    // first, the bear dies to SBAs, and Giant Growth doesn't resolve (608.2b).
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    let gg = in_hand(&mut g, P0, giant_growth());
    let b = in_hand(&mut g, P1, bolt());
    add_mana(&mut g, P0, "G");
    add_mana(&mut g, P1, "R");
    cast(&mut g, P0, gg, vec![Target::Object(bear)]);
    assert_eq!(priority(&g), P0, "117.3c: caster gets priority back");
    g.submit(P0, Action::Pass).unwrap();
    cast(&mut g, P1, b, vec![Target::Object(bear)]);
    assert_eq!(g.stack.len(), 2);
    resolve_one(&mut g);
    assert!(!g.is_on_battlefield(bear), "bear died to 704.5g");
    assert_eq!(g.stack.len(), 1);
    resolve_one(&mut g);
    assert!(g
        .log
        .iter()
        .any(|l| l.contains("Giant Growth doesn't resolve")));
    assert_eq!(g.player(P0).graveyard.len(), 2);
}

#[test]
fn pump_in_response_saves_the_creature() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    let b = in_hand(&mut g, P1, bolt());
    let gg = in_hand(&mut g, P0, giant_growth());
    add_mana(&mut g, P0, "G");
    add_mana(&mut g, P1, "R");
    g.submit(P0, Action::Pass).unwrap();
    cast(&mut g, P1, b, vec![Target::Object(bear)]);
    g.submit(P1, Action::Pass).unwrap();
    cast(&mut g, P0, gg, vec![Target::Object(bear)]);
    resolve_all(&mut g);
    assert!(g.is_on_battlefield(bear));
    assert_eq!(pt(&g, bear), (5, 5));
    assert_eq!(g.object(bear).unwrap().damage, 3);
    // Damage wears off and the pump ends in cleanup.
    pass_until(&mut g, Step::Upkeep);
    assert_eq!(pt(&g, bear), (2, 2));
    assert_eq!(g.object(bear).unwrap().damage, 0);
}

#[test]
fn counterspell_counters_a_spell() {
    let mut g = game(2);
    let bear = in_hand(&mut g, P0, bears());
    let cs = in_hand(&mut g, P1, counterspell());
    add_mana(&mut g, P0, "GG");
    add_mana(&mut g, P1, "UU");
    cast(&mut g, P0, bear, vec![]);
    let spell = g.stack[0].id;
    g.submit(P0, Action::Pass).unwrap();
    cast(&mut g, P1, cs, vec![Target::Object(spell)]);
    resolve_all(&mut g);
    assert!(g
        .battlefield
        .iter()
        .all(|id| g.object(*id).unwrap().def.name != "Grizzly Bears"));
    assert_eq!(g.player(P0).graveyard.len(), 1);
}

#[test]
fn casting_auto_taps_lands_and_rejects_unpayable_costs() {
    let mut g = game(2);
    let bear = in_hand(&mut g, P0, bears());
    put(&mut g, P0, forest());
    let r = g.submit(
        P0,
        Action::Cast {
            card: bear,
            targets: vec![],
            x: 0,
        },
    );
    assert!(
        matches!(r, Err(RulesError::Illegal(_))),
        "one Forest can't pay {{1}}{{G}}"
    );
    assert!(g.stack.is_empty());
    put(&mut g, P0, forest());
    cast(&mut g, P0, bear, vec![]);
    assert_eq!(
        g.battlefield
            .iter()
            .filter(|id| g.object(**id).unwrap().tapped)
            .count(),
        2
    );
}

#[test]
fn play_land_once_per_turn() {
    let mut g = game(2);
    let lands: Vec<ObjectId> = g.hand(P0).to_vec();
    g.submit(P0, Action::PlayLand(lands[0])).unwrap();
    let r = g.submit(P0, Action::PlayLand(lands[1]));
    assert!(matches!(r, Err(RulesError::Illegal(_))));
}

#[test]
fn etb_trigger_uses_the_stack() {
    let mut g = game(2);
    let v = in_hand(&mut g, P0, etb_drawer());
    add_mana(&mut g, P0, "GG");
    cast(&mut g, P0, v, vec![]);
    let hand_before = g.hand(P0).len();
    resolve_one(&mut g); // creature spell resolves; trigger goes on the stack
    assert_eq!(g.stack.len(), 1);
    assert!(matches!(g.stack[0].kind, StackKind::Ability { .. }));
    resolve_one(&mut g);
    assert_eq!(g.hand(P0).len(), hand_before + 1);
}

#[test]
fn dies_trigger_looks_back_in_time() {
    // Both creatures die at once; the dying Soul Collector still sees the other die (603.10a).
    let mut g = game(2);
    put(&mut g, P0, blood_artist());
    put(&mut g, P0, bears());
    let wrath = CardBuilder::new("Wrath")
        .cost("{W}")
        .types(&[CardType::Sorcery])
        .spell(
            vec![],
            vec![Effect::DestroyAll {
                filter: Filter::creatures(),
            }],
        )
        .build();
    let w = in_hand(&mut g, P0, wrath);
    add_mana(&mut g, P0, "W");
    cast(&mut g, P0, w, vec![]);
    resolve_all(&mut g);
    assert_eq!(g.player(P0).life, 41);
}

#[test]
fn simultaneous_triggers_go_on_the_stack_in_apnap_order() {
    // 603.3b: the active player's triggers go on the stack first, so the non-active player's
    // resolve first.
    let mut g = game(2);
    put(&mut g, P0, upkeep_lifegain("Ivory Cup"));
    put(&mut g, P1, upkeep_lifegain("Ebony Cup"));
    pass_until(&mut g, Step::End);
    // Advance into P1's upkeep with both triggers on the stack.
    while g.step != Step::Upkeep {
        default_answer(&mut g);
    }
    assert_eq!(g.active, P1);
    assert_eq!(g.stack.len(), 2);
    let names: Vec<String> = g
        .stack
        .iter()
        .map(|e| match &e.kind {
            StackKind::Ability { source_name, .. } => source_name.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(
        names,
        vec!["Ebony Cup", "Ivory Cup"],
        "bottom = active player's (P1's)"
    );
}

#[test]
fn players_order_their_own_simultaneous_triggers() {
    let mut g = game(2);
    put(&mut g, P0, upkeep_lifegain("Cup A"));
    put(&mut g, P0, upkeep_lifegain("Cup B"));
    pass_until(&mut g, Step::End);
    loop {
        if let Decision::PutTriggersOnStack { player, count } = g.decision().clone() {
            assert_eq!((player, count), (P0, 2));
            g.submit(P0, Action::OrderTriggers(vec![(1, vec![]), (0, vec![])]))
                .unwrap();
            break;
        }
        default_answer(&mut g);
    }
    assert_eq!(g.stack.len(), 2);
}

#[test]
fn legend_rule() {
    let mut g = game(2);
    let a = put(&mut g, P0, urborg());
    let b = put(&mut g, P0, urborg());
    g.submit(P0, Action::Pass).unwrap(); // SBAs run before P1 gets priority
    assert!(!g.is_on_battlefield(a));
    assert!(g.is_on_battlefield(b));
}

#[test]
fn zero_toughness_creature_dies() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    g.add_counters(bear, CounterKind::MinusOneMinusOne, 2);
    g.submit(P0, Action::Pass).unwrap();
    assert!(!g.is_on_battlefield(bear));
}

#[test]
fn plus_and_minus_counters_annihilate() {
    let mut g = game(2);
    let bear = put(&mut g, P0, bears());
    g.add_counters(bear, CounterKind::PlusOnePlusOne, 2);
    g.add_counters(bear, CounterKind::MinusOneMinusOne, 1);
    g.submit(P0, Action::Pass).unwrap();
    let o = g.object(bear).unwrap();
    assert_eq!(o.counters.get(&CounterKind::PlusOnePlusOne), Some(&1));
    assert_eq!(o.counters.get(&CounterKind::MinusOneMinusOne), None);
}

#[test]
fn drawing_from_empty_library_loses() {
    let mut g = game(2);
    let lib = g.player(P1).library.clone();
    for c in lib {
        g.move_object(c, Zone::Exile, None);
    }
    pass_until(&mut g, Step::End);
    while !matches!(g.decision(), Decision::GameOver { .. }) {
        default_answer(&mut g);
    }
    assert_eq!(g.active, P1, "lost in their own draw step");
    assert!(g.player(P1).has_lost);
    assert_eq!(g.decision(), &Decision::GameOver { winner: Some(P0) });
}

#[test]
fn deterministic_with_the_same_seed() {
    let a = game(4);
    let b = game(4);
    let names = |g: &Game| -> Vec<String> {
        g.hand(P2)
            .iter()
            .map(|id| g.object(*id).unwrap().def.name.clone())
            .collect()
    };
    assert_eq!(names(&a), names(&b));
    assert_eq!(a.player(P3).library, b.player(P3).library);
}
