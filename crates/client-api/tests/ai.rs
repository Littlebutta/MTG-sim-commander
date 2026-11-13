//! The computer player should make the plays a sensible person would.

use client_api::*;
use rules_core::*;
use std::sync::Arc;

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

fn land(name: &str, sub: &str) -> Arc<CardDef> {
    CardBuilder::new(name)
        .types(&[CardType::Land])
        .supertypes(&[Supertype::Basic])
        .subtypes(&[sub])
        .build()
}
fn creature(name: &str, p: i32, t: i32) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{3}")
        .types(&[CardType::Creature])
        .pt(p, t)
        .build()
}
fn flyer(name: &str, p: i32, t: i32) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{3}")
        .types(&[CardType::Creature])
        .pt(p, t)
        .keyword(Keyword::Flying)
        .build()
}
fn bolt() -> Arc<CardDef> {
    CardBuilder::new("Lightning Bolt")
        .cost("{R}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Any],
            vec![Effect::DealDamage {
                target: 0,
                amount: Amount::Fixed(3),
            }],
        )
        .build()
}
fn commander(name: &str) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{9}")
        .types(&[CardType::Creature])
        .supertypes(&[Supertype::Legendary])
        .pt(1, 1)
        .build()
}

/// A game where nobody has anything yet; tests then put cards where they want them.
fn table(players: usize, first: usize) -> Game {
    let decks = (0..players)
        .map(|i| DeckSpec {
            player_name: format!("P{i}"),
            commanders: vec![commander(&format!("Commander {i}"))],
            cards: (0..99).map(|_| land("Wastes", "Wastes")).collect(),
        })
        .collect();
    let g = Game::new_commander(
        GameConfig {
            seed: 7,
            starting_player: Some(first),
            skip_mulligans: true,
            starting_hand: 0,
            ..Default::default()
        },
        decks,
    );
    g
}

/// Pass priority for everyone until `stop` says we're there.
fn advance(g: &mut Game, stop: impl Fn(&Game) -> bool) {
    for _ in 0..500 {
        if stop(g) {
            return;
        }
        let d = g.decision().clone();
        let p = d.player().expect("game still going");
        let a = match d {
            Decision::DeclareAttackers { .. } => Action::Attack(vec![]),
            Decision::DeclareBlockers { .. } => Action::Block(vec![]),
            _ => Action::Pass,
        };
        g.submit(p, a).unwrap();
    }
    panic!("never reached the expected point");
}

fn brain(p: PlayerId) -> Brain {
    Brain::new(p, Personality::Balanced, 1)
}

#[test]
fn goes_for_the_kill() {
    let mut g = table(3, 0);
    let a = g.debug_create(creature("Ogre", 3, 3), P0, Zone::Battlefield);
    let b = g.debug_create(creature("Ogre", 3, 3), P0, Zone::Battlefield);
    g.debug_set_life(P1, 6);
    g.debug_create(creature("Big", 6, 6), P2, Zone::Battlefield);
    advance(
        &mut g,
        |g| matches!(g.decision(), Decision::DeclareAttackers { player } if *player == P0),
    );
    let d = brain(P0).decide(&g);
    let Action::Attack(list) = d.action else {
        panic!()
    };
    assert!(
        list.contains(&(a, P1)) && list.contains(&(b, P1)),
        "{list:?}"
    );
}

#[test]
fn does_not_attack_into_a_bad_block() {
    let mut g = table(2, 0);
    g.debug_create(creature("Bear", 2, 2), P0, Zone::Battlefield);
    g.debug_create(creature("Wall", 4, 4), P1, Zone::Battlefield);
    advance(
        &mut g,
        |g| matches!(g.decision(), Decision::DeclareAttackers { player } if *player == P0),
    );
    let d = brain(P0).decide(&g);
    assert_eq!(d.action, Action::Attack(vec![]));
}

fn blocks_at_life(life: i32) -> Vec<(ObjectId, ObjectId)> {
    let mut g = table(2, 1);
    g.debug_set_life(P0, life);
    g.debug_create(creature("Squire", 1, 1), P0, Zone::Battlefield);
    let big = g.debug_create(creature("Giant", 6, 6), P1, Zone::Battlefield);
    advance(
        &mut g,
        |g| matches!(g.decision(), Decision::DeclareAttackers { player } if *player == P1),
    );
    g.submit(P1, Action::Attack(vec![(big, P0)])).unwrap();
    advance(
        &mut g,
        |g| matches!(g.decision(), Decision::DeclareBlockers { player } if *player == P0),
    );
    let Action::Block(b) = brain(P0).decide(&g).action else {
        panic!()
    };
    b
}

#[test]
fn chump_blocks_only_when_it_matters() {
    assert!(blocks_at_life(40).is_empty(), "no need to chump at 40 life");
    assert_eq!(blocks_at_life(5).len(), 1, "must chump at 5 life");
}

#[test]
fn kills_the_biggest_threat_at_end_of_turn() {
    let mut g = table(2, 1);
    g.debug_create(land("Mountain", "Mountain"), P0, Zone::Battlefield);
    let bolt = g.debug_create(bolt(), P0, Zone::Hand);
    let drake = g.debug_create(flyer("Drake", 3, 3), P1, Zone::Battlefield);
    g.debug_create(creature("Rat", 1, 1), P1, Zone::Battlefield);
    advance(&mut g, |g| {
        g.step == Step::End
            && matches!(g.decision(), Decision::Priority { player } if *player == P0)
    });
    let d = brain(P0).decide(&g);
    assert_eq!(
        d.action,
        Action::Cast {
            card: bolt,
            targets: vec![Target::Object(drake)],
            x: 0
        }
    );
}

#[test]
fn holds_removal_when_there_is_nothing_worth_killing() {
    let mut g = table(4, 1);
    g.debug_create(land("Mountain", "Mountain"), P0, Zone::Battlefield);
    g.debug_create(bolt(), P0, Zone::Hand);
    advance(&mut g, |g| {
        g.step == Step::End
            && matches!(g.decision(), Decision::Priority { player } if *player == P0)
    });
    assert_eq!(brain(P0).decide(&g).action, Action::Pass);
}

/// Play a 2-player game between a thinking AI (seat `ai`) and the simple bot.
fn ai_vs_basic(seed: u64, ai: usize) -> bool {
    let cards = starter_cards();
    let index = carddb::CardIndex::new(&cards);
    let decks = starter_decklists();
    // Same two decks every game; the AI swaps seats so it plays both.
    let pick = [&decks[1], &decks[2]];
    let seats = (0..2)
        .map(|i| {
            let (name, text) = pick[(i + ai) % 2];
            SeatSpec {
                name: format!("seat {i}"),
                deck: resolve_deck(name, text, &index).unwrap(),
                ai: (i == ai).then_some(Personality::Balanced),
            }
        })
        .collect();
    let mut s = Session::new_game(seed, seats);
    for _ in 0..20_000 {
        s.run_bots();
        match s.game.decision().clone() {
            Decision::GameOver { winner } => return winner == Some(PlayerId(ai as u8)),
            d => {
                let p = d.player().unwrap();
                if s.game.turn > 60 {
                    s.submit(p, Action::Concede).unwrap();
                } else {
                    let a = bot_action(&s.game, p);
                    if s.submit(p, a).is_err() {
                        // The simple bot occasionally picks an illegal target; it just passes then.
                        s.submit(p, Action::Pass).unwrap();
                    }
                }
            }
        }
    }
    false
}

#[test]
fn beats_the_simple_bot() {
    let games = 12;
    let wins = (0..games)
        .filter(|i| ai_vs_basic(100 + *i as u64, (*i % 2) as usize))
        .count();
    eprintln!("AI won {wins} of {games}");
    assert!(wins * 3 >= games * 2, "won {wins} of {games}");
}

#[test]
fn four_ai_players_finish_a_game() {
    let mut s = Session::new_practice(11, 4);
    // Seat 0 is a person in practice games; let a fourth brain play it.
    s.brains.push(Brain::new(P0, Personality::Aggressive, 3));
    s.run_bots();
    assert!(
        matches!(s.game.decision(), Decision::GameOver { .. }),
        "stuck at {:?} on turn {}",
        s.game.decision(),
        s.game.turn
    );
}
