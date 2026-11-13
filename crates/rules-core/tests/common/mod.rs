#![allow(dead_code)]
//! Test cards and helpers. Card scripts here mirror how the importer will describe real cards.

use rules_core::*;
use std::sync::Arc;

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);
pub const P2: PlayerId = PlayerId(2);
pub const P3: PlayerId = PlayerId(3);

pub fn forest() -> Arc<CardDef> {
    CardBuilder::new("Forest")
        .types(&[CardType::Land])
        .supertypes(&[Supertype::Basic])
        .subtypes(&["Forest"])
        .build()
}
pub fn mountain() -> Arc<CardDef> {
    CardBuilder::new("Mountain")
        .types(&[CardType::Land])
        .supertypes(&[Supertype::Basic])
        .subtypes(&["Mountain"])
        .build()
}
pub fn bears() -> Arc<CardDef> {
    CardBuilder::new("Grizzly Bears")
        .cost("{1}{G}")
        .types(&[CardType::Creature])
        .subtypes(&["Bear"])
        .pt(2, 2)
        .build()
}
pub fn vanilla(name: &str, p: i32, t: i32) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{2}")
        .types(&[CardType::Creature])
        .pt(p, t)
        .build()
}
pub fn commander_card(name: &str) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{2}{G}")
        .types(&[CardType::Creature])
        .supertypes(&[Supertype::Legendary])
        .pt(3, 3)
        .build()
}
pub fn bolt() -> Arc<CardDef> {
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
pub fn giant_growth() -> Arc<CardDef> {
    CardBuilder::new("Giant Growth")
        .cost("{G}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Permanent(Filter::creatures())],
            vec![Effect::ApplyContinuous {
                affected: Affected::Target(0),
                mods: vec![Modification::ModifyPT(3, 3)],
                duration: Duration::EndOfTurn,
            }],
        )
        .build()
}
/// "Target creature becomes 0/1 until end of turn" (layer 7b).
pub fn shrink() -> Arc<CardDef> {
    CardBuilder::new("Diminish")
        .cost("{U}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Permanent(Filter::creatures())],
            vec![Effect::ApplyContinuous {
                affected: Affected::Target(0),
                mods: vec![Modification::SetPT(0, 1)],
                duration: Duration::EndOfTurn,
            }],
        )
        .build()
}
/// "Switch target creature's power and toughness until end of turn" (layer 7d).
pub fn switch() -> Arc<CardDef> {
    CardBuilder::new("Twisted Image")
        .cost("{U}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Permanent(Filter::creatures())],
            vec![Effect::ApplyContinuous {
                affected: Affected::Target(0),
                mods: vec![Modification::SwitchPT],
                duration: Duration::EndOfTurn,
            }],
        )
        .build()
}
pub fn anthem() -> Arc<CardDef> {
    CardBuilder::new("Glorious Anthem")
        .cost("{1}{W}{W}")
        .types(&[CardType::Enchantment])
        .ability(Ability::Static {
            affected: Filter::creatures().yours(),
            mods: vec![Modification::ModifyPT(1, 1)],
        })
        .build()
}
pub fn threaten() -> Arc<CardDef> {
    CardBuilder::new("Threaten")
        .cost("{2}{R}")
        .types(&[CardType::Sorcery])
        .spell(
            vec![TargetSpec::Permanent(Filter::creatures())],
            vec![
                Effect::ApplyContinuous {
                    affected: Affected::Target(0),
                    mods: vec![
                        Modification::ControlBySourceController,
                        Modification::AddKeyword(Keyword::Haste),
                    ],
                    duration: Duration::EndOfTurn,
                },
                Effect::Untap { target: 0 },
            ],
        )
        .build()
}
pub fn blood_moon() -> Arc<CardDef> {
    CardBuilder::new("Blood Moon")
        .cost("{2}{R}")
        .types(&[CardType::Enchantment])
        .ability(Ability::Static {
            affected: Filter::of_type(CardType::Land).not_super(Supertype::Basic),
            mods: vec![Modification::SetBasicLandTypes(vec!["Mountain".into()])],
        })
        .build()
}
pub fn urborg() -> Arc<CardDef> {
    CardBuilder::new("Urborg, Tomb of Yawgmoth")
        .types(&[CardType::Land])
        .supertypes(&[Supertype::Legendary])
        .ability(Ability::Static {
            affected: Filter::of_type(CardType::Land),
            mods: vec![Modification::AddSubtypes(vec!["Swamp".into()])],
        })
        .build()
}
pub fn humility() -> Arc<CardDef> {
    CardBuilder::new("Humility")
        .cost("{2}{W}{W}")
        .types(&[CardType::Enchantment])
        .ability(Ability::Static {
            affected: Filter::creatures(),
            mods: vec![Modification::RemoveAllAbilities, Modification::SetPT(1, 1)],
        })
        .build()
}
pub fn opalescence() -> Arc<CardDef> {
    let mut f = Filter::of_type(CardType::Enchantment).other();
    f.not_subtypes = vec!["Aura".into()];
    CardBuilder::new("Opalescence")
        .cost("{2}{W}{W}")
        .types(&[CardType::Enchantment])
        .ability(Ability::Static {
            affected: f,
            mods: vec![
                Modification::AddTypes(vec![CardType::Creature]),
                Modification::SetPTToManaValue,
            ],
        })
        .build()
}
/// Power is the number of card types among cards in all graveyards, toughness that plus 1.
pub fn goyf() -> Arc<CardDef> {
    let n = Amount::CardTypesInAllGraveyards;
    CardBuilder::new("Graveyard Lurker")
        .cost("{1}{G}")
        .types(&[CardType::Creature])
        .ability(Ability::CharacteristicPT {
            power: n.clone(),
            toughness: Amount::Sum(vec![n, Amount::Fixed(1)]),
        })
        .build()
}
pub fn flyer(name: &str, p: i32, t: i32) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{2}")
        .types(&[CardType::Creature])
        .pt(p, t)
        .keyword(Keyword::Flying)
        .build()
}
pub fn with_keyword(name: &str, p: i32, t: i32, k: Keyword) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{2}")
        .types(&[CardType::Creature])
        .pt(p, t)
        .keyword(k)
        .build()
}
/// "When this creature enters, draw a card."
pub fn etb_drawer() -> Arc<CardDef> {
    CardBuilder::new("Elvish Visionary")
        .cost("{1}{G}")
        .types(&[CardType::Creature])
        .pt(1, 1)
        .ability(Ability::Triggered {
            event: TriggerEvent::EntersBattlefield(Filter::this()),
            targets: vec![],
            effects: vec![Effect::DrawCards {
                who: PlayerRef::You,
                amount: Amount::Fixed(1),
            }],
            optional: false,
        })
        .build()
}
/// "Whenever another creature dies, you gain 1 life."
pub fn blood_artist() -> Arc<CardDef> {
    CardBuilder::new("Soul Collector")
        .cost("{1}{B}")
        .types(&[CardType::Creature])
        .pt(0, 1)
        .ability(Ability::Triggered {
            event: TriggerEvent::Dies(Filter::creatures().other()),
            targets: vec![],
            effects: vec![Effect::GainLife {
                who: PlayerRef::You,
                amount: Amount::Fixed(1),
            }],
            optional: false,
        })
        .build()
}
/// "At the beginning of each upkeep, you gain 1 life."
pub fn upkeep_lifegain(name: &str) -> Arc<CardDef> {
    CardBuilder::new(name)
        .cost("{1}")
        .types(&[CardType::Artifact])
        .ability(Ability::Triggered {
            event: TriggerEvent::BeginningOfUpkeep(PlayerRef::Each),
            targets: vec![],
            effects: vec![Effect::GainLife {
                who: PlayerRef::You,
                amount: Amount::Fixed(1),
            }],
            optional: false,
        })
        .build()
}
pub fn unsummon() -> Arc<CardDef> {
    CardBuilder::new("Unsummon")
        .cost("{U}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Permanent(Filter::creatures())],
            vec![Effect::ReturnToHand { target: 0 }],
        )
        .build()
}
pub fn murder() -> Arc<CardDef> {
    CardBuilder::new("Murder")
        .cost("{1}{B}{B}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Permanent(Filter::creatures())],
            vec![Effect::Destroy { target: 0 }],
        )
        .build()
}
pub fn counterspell() -> Arc<CardDef> {
    CardBuilder::new("Counterspell")
        .cost("{U}{U}")
        .types(&[CardType::Instant])
        .spell(
            vec![TargetSpec::Spell(Filter::default())],
            vec![Effect::Counter { target: 0 }],
        )
        .build()
}

/// A game of `n` players, each with a 99-Forest deck and a 3/3 legendary commander, starting
/// at player 0's first precombat main phase.
pub fn game(n: usize) -> Game {
    let decks = (0..n)
        .map(|i| DeckSpec {
            player_name: format!("P{i}"),
            commanders: vec![commander_card(&format!("Commander {i}"))],
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
    g
}

/// Answer every decision with "do nothing" until we reach `step` with priority and an empty stack.
pub fn pass_until(g: &mut Game, step: Step) {
    for _ in 0..10_000 {
        if g.step == step && g.stack.is_empty() && matches!(g.decision(), Decision::Priority { .. })
        {
            return;
        }
        default_answer(g);
    }
    panic!("never reached {step:?}");
}

pub fn default_answer(g: &mut Game) {
    match g.decision().clone() {
        Decision::Priority { player } => g.submit(player, Action::Pass).unwrap(),
        Decision::DeclareAttackers { player } => g.submit(player, Action::Attack(vec![])).unwrap(),
        Decision::DeclareBlockers { player } => g.submit(player, Action::Block(vec![])).unwrap(),
        Decision::Discard { player, count } => {
            let cards = g.hand(player)[..count].to_vec();
            g.submit(player, Action::Discard(cards)).unwrap()
        }
        Decision::PutTriggersOnStack { player, count } => g
            .submit(
                player,
                Action::OrderTriggers((0..count).map(|i| (i, vec![])).collect()),
            )
            .unwrap(),
        Decision::Mulligan { player, .. } => {
            g.submit(player, Action::Keep { bottom: vec![] }).unwrap()
        }
        Decision::GameOver { .. } => panic!("game over"),
    }
}

/// Everyone passes until the stack is empty.
pub fn resolve_all(g: &mut Game) {
    for _ in 0..1000 {
        if g.stack.is_empty() {
            return;
        }
        default_answer(g);
    }
    panic!("stack never emptied");
}

/// Pass priority around once per object until the top object resolves.
pub fn resolve_one(g: &mut Game) {
    let top = g.stack.last().expect("stack not empty").id;
    while g.stack.iter().any(|e| e.id == top) {
        default_answer(g);
    }
}

pub fn put(g: &mut Game, p: PlayerId, def: Arc<CardDef>) -> ObjectId {
    g.debug_create(def, p, Zone::Battlefield)
}
pub fn in_hand(g: &mut Game, p: PlayerId, def: Arc<CardDef>) -> ObjectId {
    g.debug_create(def, p, Zone::Hand)
}
pub fn add_mana(g: &mut Game, p: PlayerId, s: &str) {
    for ch in s.chars() {
        let t = match ch {
            'C' => ManaType::Colorless,
            c => ManaType::Color(Color::from_letter(c).unwrap()),
        };
        g.player_mut(p).mana_pool.add(t, 1);
    }
}
pub fn pt(g: &Game, id: ObjectId) -> (i32, i32) {
    let c = g.chars(id).unwrap();
    (c.power.unwrap(), c.toughness.unwrap())
}
pub fn priority(g: &Game) -> PlayerId {
    match g.decision() {
        Decision::Priority { player } => *player,
        d => panic!("expected priority, got {d:?}"),
    }
}
pub fn cast(g: &mut Game, p: PlayerId, card: ObjectId, targets: Vec<Target>) {
    g.submit(
        p,
        Action::Cast {
            card,
            targets,
            x: 0,
        },
    )
    .unwrap();
}
/// Give priority to `p` by having everyone before them pass (stack must not resolve).
pub fn priority_to(g: &mut Game, p: PlayerId) {
    while priority(g) != p {
        let q = priority(g);
        g.submit(q, Action::Pass).unwrap();
    }
}
