//! CR 903 (Commander), 506–511 (combat) and 800.4 (leaving a multiplayer game).

mod common;
use common::*;
use rules_core::*;

fn commander_of(g: &Game, p: PlayerId) -> ObjectId {
    let card = g.player(p).commanders[0];
    g.objects.values().find(|o| o.card == card).unwrap().id
}

fn attack(g: &mut Game, attacks: Vec<(ObjectId, PlayerId)>) {
    pass_until_decision(g, |d| matches!(d, Decision::DeclareAttackers { .. }));
    let p = g.active;
    g.submit(p, Action::Attack(attacks)).unwrap();
}

fn pass_until_decision(g: &mut Game, f: impl Fn(&Decision) -> bool) {
    for _ in 0..1000 {
        if f(g.decision()) {
            return;
        }
        default_answer(g);
    }
    panic!("decision never came");
}

#[test]
fn commander_tax_increases_each_cast_from_command_zone() {
    let mut g = game(2);
    let cmd = commander_of(&g, P0);
    assert_eq!(g.commander_tax(P0, cmd), 0);
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, cmd, vec![]);
    resolve_all(&mut g);
    let on_bf = commander_of(&g, P0);
    assert!(g.is_on_battlefield(on_bf));
    // Bounce it: 903.9b sends it to the command zone instead of the hand.
    let u = in_hand(&mut g, P0, unsummon());
    add_mana(&mut g, P0, "U");
    cast(&mut g, P0, u, vec![Target::Object(on_bf)]);
    resolve_all(&mut g);
    let cmd = commander_of(&g, P0);
    assert_eq!(g.object(cmd).unwrap().zone, Zone::Command);
    assert_eq!(g.commander_tax(P0, cmd), 2);
    add_mana(&mut g, P0, "GGG");
    let r = g.submit(
        P0,
        Action::Cast {
            card: cmd,
            targets: vec![],
            x: 0,
        },
    );
    assert!(matches!(r, Err(RulesError::Illegal(_))), "needs {{2}} more");
    add_mana(&mut g, P0, "CC");
    cast(&mut g, P0, cmd, vec![]);
}

#[test]
fn commander_that_dies_returns_to_command_zone_as_sba() {
    let mut g = game(2);
    let cmd = commander_of(&g, P0);
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, cmd, vec![]);
    resolve_all(&mut g);
    let cmd = commander_of(&g, P0);
    let m = in_hand(&mut g, P1, murder());
    add_mana(&mut g, P1, "BBB");
    priority_to(&mut g, P1);
    cast(&mut g, P1, m, vec![Target::Object(cmd)]);
    resolve_all(&mut g);
    assert_eq!(g.object(commander_of(&g, P0)).unwrap().zone, Zone::Command);
}

#[test]
fn commander_can_be_left_in_graveyard_if_owner_chooses() {
    let mut g = game(2);
    g.submit(P0, Action::SetCommanderReturn(false)).unwrap();
    let cmd = commander_of(&g, P0);
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, cmd, vec![]);
    resolve_all(&mut g);
    let m = in_hand(&mut g, P1, murder());
    add_mana(&mut g, P1, "BBB");
    priority_to(&mut g, P1);
    let cmd = commander_of(&g, P0);
    cast(&mut g, P1, m, vec![Target::Object(cmd)]);
    resolve_all(&mut g);
    assert_eq!(
        g.object(commander_of(&g, P0)).unwrap().zone,
        Zone::Graveyard
    );
}

#[test]
fn unblocked_attack_deals_damage_and_tracks_commander_damage() {
    let mut g = game(2);
    let cmd = commander_of(&g, P0);
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, cmd, vec![]);
    resolve_all(&mut g);
    let cmd = commander_of(&g, P0);
    g.object(cmd).unwrap();
    // Summoning sick this turn: move to next P0 turn.
    pass_until(&mut g, Step::End);
    pass_until(&mut g, Step::Main1); // P1's turn
    pass_until(&mut g, Step::End);
    pass_until(&mut g, Step::Main1); // P0's turn
    assert_eq!(g.active, P0);
    attack(&mut g, vec![(cmd, P1)]);
    pass_until(&mut g, Step::EndCombat);
    assert_eq!(g.player(P1).life, 37);
    let card = g.player(P0).commanders[0];
    assert_eq!(g.player(P1).commander_damage.get(&card), Some(&3));
}

#[test]
fn twenty_one_commander_damage_loses() {
    let mut g = game(3);
    let card = g.player(P0).commanders[0];
    g.player_mut(P2).commander_damage.insert(card, 21);
    g.submit(P0, Action::Pass).unwrap();
    assert!(g.player(P2).has_lost);
    assert!(!g.player(P1).has_lost);
    assert!(
        matches!(g.decision(), Decision::Priority { .. }),
        "the game goes on with two players"
    );
}

#[test]
fn summoning_sick_creatures_cant_attack_but_haste_can() {
    let mut g = game(2);
    let sick = g.debug_create(bears(), P0, Zone::Battlefield);
    g.objects.get_mut(&sick).unwrap().summoning_sick = true;
    let hasty = g.debug_create(
        with_keyword("Raging Goblin", 1, 1, Keyword::Haste),
        P0,
        Zone::Battlefield,
    );
    g.objects.get_mut(&hasty).unwrap().summoning_sick = true;
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareAttackers { .. }));
    assert!(g.submit(P0, Action::Attack(vec![(sick, P1)])).is_err());
    g.submit(P0, Action::Attack(vec![(hasty, P1)])).unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert_eq!(g.player(P1).life, 39);
}

#[test]
fn flying_can_only_be_blocked_by_flying_or_reach() {
    let mut g = game(2);
    let bird = put(&mut g, P0, flyer("Bird", 1, 1));
    let bear = put(&mut g, P1, bears());
    let spider = put(&mut g, P1, with_keyword("Spider", 1, 3, Keyword::Reach));
    attack(&mut g, vec![(bird, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    assert!(g.submit(P1, Action::Block(vec![(bear, bird)])).is_err());
    g.submit(P1, Action::Block(vec![(spider, bird)])).unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert!(!g.is_on_battlefield(bird));
    assert_eq!(g.player(P1).life, 40);
}

#[test]
fn first_strike_kills_before_regular_damage() {
    let mut g = game(2);
    let knight = put(
        &mut g,
        P0,
        with_keyword("Knight", 2, 2, Keyword::FirstStrike),
    );
    let bear = put(&mut g, P1, bears());
    attack(&mut g, vec![(knight, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    g.submit(P1, Action::Block(vec![(bear, knight)])).unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert!(g.is_on_battlefield(knight));
    assert!(!g.is_on_battlefield(bear));
    assert_eq!(g.object(knight).unwrap().damage, 0);
}

#[test]
fn double_strike_hits_twice() {
    let mut g = game(2);
    let ds = put(
        &mut g,
        P0,
        with_keyword("Duelist", 2, 2, Keyword::DoubleStrike),
    );
    attack(&mut g, vec![(ds, P1)]);
    pass_until(&mut g, Step::EndCombat);
    assert_eq!(g.player(P1).life, 36);
}

#[test]
fn trample_and_deathtouch_assign_one_lethal_damage() {
    let mut g = game(2);
    let mut def = (*with_keyword("Wurm", 6, 6, Keyword::Trample)).clone();
    def.abilities.push(Ability::Keyword(Keyword::Deathtouch));
    let wurm = put(&mut g, P0, std::sync::Arc::new(def));
    let wall = put(&mut g, P1, vanilla("Wall", 0, 5));
    attack(&mut g, vec![(wurm, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    g.submit(P1, Action::Block(vec![(wall, wurm)])).unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert!(!g.is_on_battlefield(wall));
    assert_eq!(g.player(P1).life, 35);
}

#[test]
fn lifelink_gains_life() {
    let mut g = game(2);
    let ll = put(&mut g, P0, with_keyword("Cleric", 3, 3, Keyword::Lifelink));
    attack(&mut g, vec![(ll, P1)]);
    pass_until(&mut g, Step::EndCombat);
    assert_eq!(g.player(P0).life, 43);
}

#[test]
fn menace_needs_two_blockers() {
    let mut g = game(2);
    let m = put(&mut g, P0, with_keyword("Brute", 3, 3, Keyword::Menace));
    let a = put(&mut g, P1, bears());
    let b = put(&mut g, P1, bears());
    attack(&mut g, vec![(m, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    assert!(g.submit(P1, Action::Block(vec![(a, m)])).is_err());
    g.submit(P1, Action::Block(vec![(a, m), (b, m)])).unwrap();
}

#[test]
fn attacking_multiple_players_and_each_defender_blocks() {
    let mut g = game(3);
    let x = put(&mut g, P0, bears());
    let y = put(&mut g, P0, bears());
    let blocker = put(&mut g, P2, vanilla("Wall", 0, 4));
    attack(&mut g, vec![(x, P1), (y, P2)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    assert_eq!(
        g.decision(),
        &Decision::DeclareBlockers { player: P1 },
        "APNAP order"
    );
    g.submit(P1, Action::Block(vec![])).unwrap();
    assert_eq!(g.decision(), &Decision::DeclareBlockers { player: P2 });
    g.submit(P2, Action::Block(vec![(blocker, y)])).unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert_eq!(g.player(P1).life, 38);
    assert_eq!(g.player(P2).life, 40);
}

#[test]
fn conceding_player_leaves_and_their_cards_go_with_them() {
    let mut g = game(4);
    let theirs = put(&mut g, P2, bears());
    // P0 steals one of P1's creatures; when P0 leaves, that control effect ends (800.4a).
    let stolen = put(&mut g, P1, bears());
    let t = in_hand(&mut g, P0, threaten());
    add_mana(&mut g, P0, "RRR");
    cast(&mut g, P0, t, vec![Target::Object(stolen)]);
    resolve_all(&mut g);
    assert_eq!(g.chars(stolen).unwrap().controller, P0);
    g.submit(P2, Action::Concede).unwrap();
    assert!(
        g.object(theirs).is_none(),
        "800.4a: objects P2 owns leave the game"
    );
    g.submit(P0, Action::Concede).unwrap();
    assert_eq!(
        g.chars(stolen).unwrap().controller,
        P1,
        "control effects of a leaving player end"
    );
    assert_eq!(g.players_in_game(), vec![P1, P3]);
    // The turn continues without an active player and the game goes on (800.4j).
    pass_until(&mut g, Step::End);
    pass_until(&mut g, Step::Main1);
    assert_eq!(g.active, P1);
}

#[test]
fn attacker_divides_damage_among_several_blockers() {
    let mut g = game(2);
    let giant = put(&mut g, P0, vanilla("Giant", 5, 5));
    let a = put(&mut g, P1, bears());
    let b = put(&mut g, P1, vanilla("Wall", 0, 4));
    attack(&mut g, vec![(giant, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    g.submit(P1, Action::Block(vec![(a, giant), (b, giant)]))
        .unwrap();
    pass_until_decision(&mut g, |d| matches!(d, Decision::AssignCombatDamage { .. }));
    let Decision::AssignCombatDamage {
        player,
        power,
        trample,
        blockers,
        ..
    } = g.decision().clone()
    else {
        unreachable!()
    };
    assert_eq!((player, power, trample), (P0, 5, false));
    assert_eq!(blockers, vec![(a, 2), (b, 4)]);
    // Can't send damage to the player without trample, and must assign all of it.
    assert!(g
        .submit(
            P0,
            Action::AssignDamage(vec![(Target::Object(a), 2), (Target::Player(P1), 3)])
        )
        .is_err());
    assert!(g
        .submit(P0, Action::AssignDamage(vec![(Target::Object(a), 2)]))
        .is_err());
    // Any division is fine: here everything goes to the wall.
    g.submit(P0, Action::AssignDamage(vec![(Target::Object(b), 5)]))
        .unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert!(g.is_on_battlefield(a));
    assert!(!g.is_on_battlefield(b));
}

#[test]
fn deathtouch_trample_needs_only_one_per_blocker() {
    let mut g = game(2);
    let mut def = (*with_keyword("Wurm", 6, 6, Keyword::Trample)).clone();
    def.abilities.push(Ability::Keyword(Keyword::Deathtouch));
    let wurm = put(&mut g, P0, std::sync::Arc::new(def));
    let a = put(&mut g, P1, vanilla("Wall", 0, 5));
    let b = put(&mut g, P1, vanilla("Wall", 0, 5));
    attack(&mut g, vec![(wurm, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    g.submit(P1, Action::Block(vec![(a, wurm), (b, wurm)]))
        .unwrap();
    pass_until_decision(&mut g, |d| matches!(d, Decision::AssignCombatDamage { .. }));
    let Decision::AssignCombatDamage {
        blockers,
        suggested,
        ..
    } = g.decision().clone()
    else {
        unreachable!()
    };
    assert_eq!(blockers, vec![(a, 1), (b, 1)]);
    assert_eq!(
        suggested,
        vec![
            (Target::Object(a), 1),
            (Target::Object(b), 1),
            (Target::Player(P1), 4)
        ]
    );
    // Lethal to only one blocker isn't enough to trample over.
    assert!(g
        .submit(
            P0,
            Action::AssignDamage(vec![(Target::Object(a), 1), (Target::Player(P1), 5)])
        )
        .is_err());
    g.submit(P0, Action::AssignDamage(suggested)).unwrap();
    pass_until(&mut g, Step::EndCombat);
    assert!(!g.is_on_battlefield(a) && !g.is_on_battlefield(b));
    assert_eq!(g.player(P1).life, 36);
}

#[test]
fn unspent_mana_can_be_taken_back() {
    let mut g = game(2);
    let forest = put(&mut g, P0, forest());
    pass_until(&mut g, Step::Main1);
    let ability = 0;
    g.submit(
        P0,
        Action::ActivateMana {
            object: forest,
            ability,
            color: None,
        },
    )
    .unwrap();
    assert!(g.object(forest).unwrap().tapped);
    assert_eq!(g.undoable_mana(P0), vec![forest]);
    g.submit(P0, Action::UndoMana(forest)).unwrap();
    assert!(!g.object(forest).unwrap().tapped);
    assert_eq!(g.player(P0).mana_pool.total(), 0);
    // Once you do something else, the mana is committed.
    g.submit(
        P0,
        Action::ActivateMana {
            object: forest,
            ability,
            color: None,
        },
    )
    .unwrap();
    g.submit(P0, Action::Pass).unwrap();
    assert!(g.undoable_mana(P0).is_empty());
}

fn kill_commander_and_wait_for_question(g: &mut Game) -> ObjectId {
    g.submit(P0, Action::SetCommanderAsk(true)).unwrap();
    let cmd = commander_of(g, P0);
    add_mana(g, P0, "GGG");
    cast(g, P0, cmd, vec![]);
    resolve_all(g);
    let m = in_hand(g, P1, murder());
    add_mana(g, P1, "BBB");
    priority_to(g, P1);
    let cmd = commander_of(g, P0);
    cast(g, P1, m, vec![Target::Object(cmd)]);
    for _ in 0..10 {
        if let Decision::CommanderZone { player, zone, .. } = g.decision().clone() {
            assert_eq!(player, P0);
            assert_eq!(zone, Zone::Graveyard);
            return commander_of(g, P0);
        }
        let p = priority(g);
        g.submit(p, Action::Pass).unwrap();
    }
    panic!("the owner was never asked");
}

#[test]
fn owner_is_asked_where_a_dead_commander_goes_and_can_keep_it_in_the_graveyard() {
    let mut g = game(2);
    kill_commander_and_wait_for_question(&mut g);
    g.submit(P0, Action::CommanderToCommandZone(false)).unwrap();
    assert_eq!(
        g.object(commander_of(&g, P0)).unwrap().zone,
        Zone::Graveyard
    );
    // The question isn't repeated.
    assert!(matches!(g.decision(), Decision::Priority { .. }));
    let p = priority(&g);
    g.submit(p, Action::Pass).unwrap();
    assert!(!matches!(g.decision(), Decision::CommanderZone { .. }));
}

#[test]
fn owner_is_asked_and_can_send_a_dead_commander_to_the_command_zone() {
    let mut g = game(2);
    kill_commander_and_wait_for_question(&mut g);
    g.submit(P0, Action::CommanderToCommandZone(true)).unwrap();
    assert_eq!(g.object(commander_of(&g, P0)).unwrap().zone, Zone::Command);
}

#[test]
fn owner_is_asked_when_a_commander_is_returned_to_hand_and_can_keep_it_there() {
    let mut g = game(2);
    g.submit(P0, Action::SetCommanderAsk(true)).unwrap();
    let cmd = commander_of(&g, P0);
    add_mana(&mut g, P0, "GGG");
    cast(&mut g, P0, cmd, vec![]);
    resolve_all(&mut g);
    let on_bf = commander_of(&g, P0);
    let u = in_hand(&mut g, P0, unsummon());
    add_mana(&mut g, P0, "U");
    cast(&mut g, P0, u, vec![Target::Object(on_bf)]);
    for _ in 0..10 {
        if matches!(g.decision(), Decision::CommanderZone { .. }) {
            break;
        }
        let p = priority(&g);
        g.submit(p, Action::Pass).unwrap();
    }
    assert!(
        matches!(g.decision(), Decision::CommanderZone { player, zone, .. } if *player == P0 && *zone == Zone::Hand)
    );
    g.submit(P0, Action::CommanderToCommandZone(false)).unwrap();
    assert_eq!(g.object(commander_of(&g, P0)).unwrap().zone, Zone::Hand);
    assert!(matches!(g.decision(), Decision::Priority { .. }));
}

#[test]
fn only_flying_or_reach_can_block_a_flyer() {
    let mut g = game(2);
    let bird = put(&mut g, P0, flyer("Bird", 1, 1));
    let bear = put(&mut g, P1, bears());
    let spider = put(&mut g, P1, with_keyword("Spider", 1, 3, Keyword::Reach));
    let hawk = put(&mut g, P1, flyer("Hawk", 1, 1));
    attack(&mut g, vec![(bird, P1)]);
    pass_until_decision(&mut g, |d| matches!(d, Decision::DeclareBlockers { .. }));
    assert!(g.blockable_attackers(P1, bear).is_empty());
    assert_eq!(g.blockable_attackers(P1, spider), vec![bird]);
    assert_eq!(g.blockable_attackers(P1, hawk), vec![bird]);
    assert!(g.submit(P1, Action::Block(vec![(bear, bird)])).is_err());
    // A double block doesn't sneak a ground creature in either.
    assert!(g
        .submit(P1, Action::Block(vec![(spider, bird), (bear, bird)]))
        .is_err());
    g.submit(P1, Action::Block(vec![(spider, bird)])).unwrap();
}

fn graveyard_spell(name: &str, f: Filter, effect: Effect) -> std::sync::Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{B}")
        .types(&[CardType::Sorcery])
        .spell(vec![TargetSpec::CardInGraveyard(f)], vec![effect])
        .build()
}

#[test]
fn spells_can_target_cards_in_any_graveyard() {
    let mut g = game(2);
    let mine = g.debug_create(bears(), P0, Zone::Graveyard);
    let theirs = g.debug_create(vanilla("Ogre", 3, 3), P1, Zone::Graveyard);
    pass_until(&mut g, Step::Main1);

    // "your graveyard" sees only your own cards; "a graveyard" sees both.
    let raise = in_hand(
        &mut g,
        P0,
        graveyard_spell(
            "Raise",
            Filter::creatures().yours(),
            Effect::ReturnFromGraveyardToHand { target: 0 },
        ),
    );
    let spec = TargetSpec::CardInGraveyard(Filter::creatures().yours());
    assert_eq!(
        g.legal_targets(P0, Some(raise), &spec),
        vec![Target::Object(mine)]
    );
    let any = TargetSpec::CardInGraveyard(Filter::creatures());
    assert_eq!(g.legal_targets(P0, Some(raise), &any).len(), 2);

    add_mana(&mut g, P0, "B");
    cast(&mut g, P0, raise, vec![Target::Object(mine)]);
    resolve_all(&mut g);
    assert!(g
        .hand(P0)
        .iter()
        .any(|id| g.object(*id).unwrap().def.name == "Grizzly Bears"));

    // Reanimate an opponent's creature under your control.
    let rean = in_hand(
        &mut g,
        P0,
        graveyard_spell(
            "Reanimate",
            Filter::creatures(),
            Effect::ReturnFromGraveyardToBattlefield { target: 0 },
        ),
    );
    add_mana(&mut g, P0, "B");
    cast(&mut g, P0, rean, vec![Target::Object(theirs)]);
    resolve_all(&mut g);
    let view = g.view();
    assert!(g.battlefield.iter().any(|id| {
        let o = g.object(*id).unwrap();
        o.def.name == "Ogre" && o.owner == P1 && view.get(*id).unwrap().controller == P0
    }));

    // Exile a card straight out of a graveyard.
    let dead = g.debug_create(bears(), P1, Zone::Graveyard);
    let purge = in_hand(
        &mut g,
        P0,
        graveyard_spell("Purge", Filter::default(), Effect::Exile { target: 0 }),
    );
    add_mana(&mut g, P0, "B");
    cast(&mut g, P0, purge, vec![Target::Object(dead)]);
    resolve_all(&mut g);
    assert!(g.player(P1).graveyard.is_empty());
    assert_eq!(g.exile.len(), 1);
}
