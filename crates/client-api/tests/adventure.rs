//! Adventure mode content: every built-in card the adventure hands out runs in full, and every
//! deck in the adventure (starter decks and every duelist) is a legal adventure deck.

use carddb::{convert_all, AtomicFile, CardIndex};
use client_api::adventure::{check_adventure_deck, resolve_adventure_deck};
use client_api::starter_cards;

#[test]
fn adventure_cards_are_fully_supported() {
    let file: AtomicFile =
        serde_json::from_str(include_str!("../data/adventure-cards.json")).unwrap();
    let cards = convert_all(&file);
    let bad: Vec<String> = cards
        .iter()
        .filter(|c| !c.def.fully_supported())
        .map(|c| format!("{}: {:?}", c.def.name, c.def.abilities))
        .collect();
    assert!(
        bad.is_empty(),
        "unsupported adventure cards:\n{}",
        bad.join("\n")
    );
}

#[test]
fn four_copies_at_most_and_basics_are_free() {
    let cards = starter_cards();
    let index = CardIndex::new(&cards);
    let ok = "4 Shock\n4 Lightning Bolt\n32 Mountain";
    assert!(check_adventure_deck(ok, &index).playable);
    let five = "5 Shock\n35 Mountain";
    let c = check_adventure_deck(five, &index);
    assert!(!c.playable && c.problems[0].contains("at most 4"));
    let small = "4 Shock\n10 Mountain";
    assert!(!check_adventure_deck(small, &index).playable);
    let deck = resolve_adventure_deck("Burn", ok, &index).unwrap();
    assert!(deck.commanders.is_empty());
    assert_eq!(deck.cards.len(), 40);
}

#[test]
fn every_adventure_deck_is_legal_and_fully_supported() {
    let cards = starter_cards();
    let index = CardIndex::new(&cards);
    let decks: serde_json::Value =
        serde_json::from_str(include_str!("../../../app/src/adventure/data/decks.json")).unwrap();
    let mut all: Vec<(String, String)> = Vec::new();
    for (k, v) in decks["starters"].as_object().unwrap() {
        all.push((k.clone(), v["text"].as_str().unwrap().to_string()));
    }
    for (k, v) in decks["duelists"].as_object().unwrap() {
        all.push((k.clone(), v.as_str().unwrap().to_string()));
    }
    assert!(all.len() >= 6 + 37);
    for (k, text) in all {
        let c = check_adventure_deck(&text, &index);
        assert!(c.playable, "{k}: {c:?}");
        assert!(c.unsupported.is_empty(), "{k}: {:?}", c.unsupported);
        assert_eq!(c.card_count, 40, "{k}");
    }
}

/// Two computer duelists play an adventure duel (no commanders, 20 life, a boss with more life)
/// to the end.
#[test]
fn an_adventure_duel_plays_to_the_end() {
    use client_api::{Brain, Personality, SeatSpec, Session};
    use rules_core::{Decision, PlayerId};
    let cards = starter_cards();
    let index = CardIndex::new(&cards);
    let decks: serde_json::Value =
        serde_json::from_str(include_str!("../../../app/src/adventure/data/decks.json")).unwrap();
    let d = |k: &str| {
        resolve_adventure_deck(k, decks["duelists"][k].as_str().unwrap(), &index).unwrap()
    };
    for (seed, a, b) in [
        (1, "tam", "pyrrhus"),
        (2, "sela", "gearlock"),
        (3, "morvane", "regent"),
    ] {
        let seats = vec![
            SeatSpec {
                name: a.into(),
                deck: d(a),
                ai: None,
                life: None,
                skill: None,
            },
            SeatSpec {
                name: b.into(),
                deck: d(b),
                ai: Some(Personality::Controlling),
                life: Some(30),
                skill: None,
            },
        ];
        let mut s = Session::new_game_with(seed, seats, 20);
        assert_eq!(s.game.players[0].life, 20);
        assert_eq!(s.game.players[1].life, 30);
        s.brains
            .push(Brain::new(PlayerId(0), Personality::Aggressive, seed));
        s.run_bots();
        assert!(
            matches!(s.game.decision(), Decision::GameOver { .. }),
            "{a} v {b} stuck at {:?} on turn {}",
            s.game.decision(),
            s.game.turn
        );
        eprintln!("{a} v {b}: {:?} on turn {}", s.game.decision(), s.game.turn);
    }
}
