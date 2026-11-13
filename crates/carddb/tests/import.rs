use carddb::*;
use rules_core::{Ability, CardType, Color, ManaProduction, ManaType};

fn sample() -> Vec<CardRecord> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/AtomicCards.sample.json"
    ))
    .unwrap();
    let file: AtomicFile = serde_json::from_str(&text).unwrap();
    convert_all(&file)
}

#[test]
fn converts_mtgjson_atomic_cards() {
    let cards = sample();
    let idx = CardIndex::new(&cards);
    let atraxa = idx.get("Atraxa, Praetors' Voice").unwrap();
    assert!(atraxa.can_be_commander);
    assert_eq!(atraxa.def.power, Some(4));
    assert_eq!(atraxa.def.color_identity.len(), 4);
    assert!(
        !atraxa.def.fully_supported(),
        "proliferate isn't scripted yet, so it's flagged"
    );
    let sol = idx.get("sol ring").unwrap();
    assert_eq!(
        sol.def.abilities[0],
        Ability::Mana {
            cost: rules_core::Cost::tap(),
            produces: ManaProduction::Fixed(vec![ManaType::Colorless, ManaType::Colorless])
        }
    );
    let bolt = idx.get("Lightning Bolt").unwrap();
    assert!(bolt.def.fully_supported() && bolt.def.has_type(CardType::Instant));
    let fire = idx.get("Fire").unwrap();
    assert_eq!(fire.def.name, "Fire // Ice");
    assert_eq!(fire.other_faces.len(), 1);
    assert!(idx
        .get("Forest")
        .unwrap()
        .def
        .color_identity
        .contains(Color::Green));
}

#[test]
fn update_check_compares_versions() {
    let old = Meta {
        date: "2026-11-01".into(),
        version: "5.3.0+20261101".into(),
    };
    let new = Meta {
        date: "2026-11-12".into(),
        version: "5.3.0+20261112".into(),
    };
    assert!(needs_update(None, &new));
    assert!(needs_update(Some(&old), &new));
    assert!(!needs_update(Some(&new), &new));
    let meta: carddb::mtgjson::MetaFile =
        serde_json::from_str(r#"{"meta":{"date":"2026-11-12","version":"5.3.0+20261112"},"data":{"date":"2026-11-12","version":"5.3.0+20261112"}}"#).unwrap();
    assert_eq!(meta.data, new);
}

#[test]
fn decklist_parsing_and_validation() {
    let cards = sample();
    let idx = CardIndex::new(&cards);
    let text = "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n1 Sol Ring (C21) 263\n1x Swords to Plowshares\n1 Lightning Bolt\n1 Mana Crypt\n2 Sol Ring\n93 Forest\n";
    let list = parse_decklist(text);
    assert_eq!(
        list.commanders,
        vec![(1, "Atraxa, Praetors' Voice".to_string())]
    );
    let report = validate(&list, &idx);
    assert!(report
        .problems
        .contains(&DeckProblem::OutsideColorIdentity("Lightning Bolt".into())));
    assert!(report
        .problems
        .contains(&DeckProblem::Banned("Mana Crypt".into())));
    assert!(report
        .problems
        .contains(&DeckProblem::NotSingleton("Sol Ring".into(), 3)));
    assert!(!report
        .problems
        .iter()
        .any(|p| matches!(p, DeckProblem::NotSingleton(n, _) if n == "Forest")));
    assert!(!report
        .problems
        .iter()
        .any(|p| matches!(p, DeckProblem::WrongSize(_))));
    assert!(report
        .unsupported
        .contains(&"Swords to Plowshares".to_string()));
}

#[test]
fn cmdr_tag_marks_commander() {
    let list = parse_decklist("1 Atraxa, Praetors' Voice *CMDR*\n99 Forest");
    assert_eq!(list.commanders.len(), 1);
    assert_eq!(list.cards, vec![(99, "Forest".to_string())]);
}
