//! Card definitions: printed characteristics plus the scripted abilities that make a card work.
//!
//! A `CardDef` is immutable data shared by every object made from that card. Abilities are
//! described with a small vocabulary of triggers, costs, targets and effects that the engine
//! executes. The `carddb` crate builds these from MTGJSON oracle text.

use crate::ids::PlayerId;
use crate::mana::{Color, ColorSet, ManaCost, ManaType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
}

impl CardType {
    pub fn parse(s: &str) -> Option<CardType> {
        Some(match s {
            "Artifact" => CardType::Artifact,
            "Battle" => CardType::Battle,
            "Creature" => CardType::Creature,
            "Enchantment" => CardType::Enchantment,
            "Instant" => CardType::Instant,
            "Kindred" | "Tribal" => CardType::Kindred,
            "Land" => CardType::Land,
            "Planeswalker" => CardType::Planeswalker,
            "Sorcery" => CardType::Sorcery,
            _ => return None,
        })
    }

    /// Permanent card types (CR 110.4).
    pub fn is_permanent_type(self) -> bool {
        !matches!(
            self,
            CardType::Instant | CardType::Sorcery | CardType::Kindred
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Supertype {
    Basic,
    Legendary,
    Snow,
    World,
    Ongoing,
}

impl Supertype {
    pub fn parse(s: &str) -> Option<Supertype> {
        Some(match s {
            "Basic" => Supertype::Basic,
            "Legendary" => Supertype::Legendary,
            "Snow" => Supertype::Snow,
            "World" => Supertype::World,
            "Ongoing" => Supertype::Ongoing,
            _ => return None,
        })
    }
}

/// Keyword abilities the engine implements natively (CR 702).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Keyword {
    Deathtouch,
    Defender,
    DoubleStrike,
    FirstStrike,
    Flash,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Lifelink,
    Menace,
    Reach,
    Trample,
    Vigilance,
}

impl Keyword {
    pub fn parse(s: &str) -> Option<Keyword> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "deathtouch" => Keyword::Deathtouch,
            "defender" => Keyword::Defender,
            "double strike" => Keyword::DoubleStrike,
            "first strike" => Keyword::FirstStrike,
            "flash" => Keyword::Flash,
            "flying" => Keyword::Flying,
            "haste" => Keyword::Haste,
            "hexproof" => Keyword::Hexproof,
            "indestructible" => Keyword::Indestructible,
            "lifelink" => Keyword::Lifelink,
            "menace" => Keyword::Menace,
            "reach" => Keyword::Reach,
            "trample" => Keyword::Trample,
            "vigilance" => Keyword::Vigilance,
            _ => return None,
        })
    }
}

/// Which player a filter or effect refers to, relative to the controller of the ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerRef {
    You,
    Opponents,
    Each,
    /// The player chosen as the Nth target.
    Target(usize),
    /// The controller of the Nth target object.
    ControllerOfTarget(usize),
}

/// Describes a set of objects, evaluated against characteristics as they stand at the
/// moment of evaluation (so it can change while layers are applied, CR 611.3a, 613.8a).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Filter {
    /// Object must have at least one of these card types (empty = any).
    pub any_of_types: Vec<CardType>,
    /// Object must have all of these card types.
    pub all_types: Vec<CardType>,
    pub not_types: Vec<CardType>,
    pub subtypes: Vec<String>,
    pub not_subtypes: Vec<String>,
    pub supertypes: Vec<Supertype>,
    pub not_supertypes: Vec<Supertype>,
    pub controller: Option<PlayerRef>,
    pub colors: Option<ColorSet>,
    /// Only the source object itself.
    pub this_object: bool,
    /// Exclude the source object ("other creatures").
    pub other_than_this: bool,
    pub nontoken: bool,
    pub keyword: Option<Keyword>,
}

impl Filter {
    pub fn this() -> Filter {
        Filter {
            this_object: true,
            ..Default::default()
        }
    }
    pub fn of_type(t: CardType) -> Filter {
        Filter {
            any_of_types: vec![t],
            ..Default::default()
        }
    }
    pub fn creatures() -> Filter {
        Filter::of_type(CardType::Creature)
    }
    pub fn yours(mut self) -> Filter {
        self.controller = Some(PlayerRef::You);
        self
    }
    pub fn other(mut self) -> Filter {
        self.other_than_this = true;
        self
    }
    pub fn not_super(mut self, s: Supertype) -> Filter {
        self.not_supertypes.push(s);
        self
    }
    pub fn any_permanent() -> Filter {
        Filter::default()
    }
}

/// Dynamic numbers used by effects and characteristic-defining abilities.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Amount {
    Fixed(i32),
    X,
    /// Number of permanents matching the filter (controller relative to the source).
    CountPermanents(Filter),
    /// Number of card types among cards in all graveyards.
    CardTypesInAllGraveyards,
    /// Number of cards in the controller's hand.
    CardsInHand,
    /// The object's own mana value.
    ManaValueOfThis,
    Sum(Vec<Amount>),
}

impl Amount {
    pub fn fixed(n: i32) -> Amount {
        Amount::Fixed(n)
    }
}

/// What can be chosen as a target (CR 115).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetSpec {
    /// "any target": creature, player, planeswalker or battle.
    Any,
    Player,
    Opponent,
    Permanent(Filter),
    Spell(Filter),
    /// Spell or ability on the stack.
    StackObject,
    CardInGraveyard(Filter),
}

/// The executable effect vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effect {
    DrawCards {
        who: PlayerRef,
        amount: Amount,
    },
    GainLife {
        who: PlayerRef,
        amount: Amount,
    },
    LoseLife {
        who: PlayerRef,
        amount: Amount,
    },
    DealDamage {
        target: usize,
        amount: Amount,
    },
    DealDamageEach {
        filter: Filter,
        players: Option<PlayerRef>,
        amount: Amount,
    },
    Destroy {
        target: usize,
    },
    DestroyAll {
        filter: Filter,
    },
    Exile {
        target: usize,
    },
    ReturnToHand {
        target: usize,
    },
    Counter {
        target: usize,
    },
    Tap {
        target: usize,
    },
    Untap {
        target: usize,
    },
    AddCounters {
        target: Option<usize>,
        kind: CounterKind,
        amount: Amount,
    },
    AddMana {
        mana: Vec<ManaType>,
    },
    /// Add one mana of any color (choice made by the player paying; auto-chosen when paying a cost).
    AddManaAnyColor {
        amount: u32,
    },
    CreateToken {
        token: Arc<CardDef>,
        amount: Amount,
        controller: PlayerRef,
    },
    /// Creates a continuous effect from a resolving spell/ability (CR 611.2). The affected set
    /// is locked in at creation (611.2c) when it uses a filter rather than a target.
    ApplyContinuous {
        affected: Affected,
        mods: Vec<Modification>,
        duration: Duration,
    },
    Mill {
        who: PlayerRef,
        amount: Amount,
    },
    ReturnFromGraveyardToBattlefield {
        target: usize,
    },
    ReturnFromGraveyardToHand {
        target: usize,
    },
    Sacrifice {
        who: PlayerRef,
        filter: Filter,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Affected {
    Target(usize),
    ThisObject,
    Filter(Filter),
    ControllerOfThis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Duration {
    EndOfTurn,
    Permanent,
    EndOfCombat,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CounterKind {
    PlusOnePlusOne,
    MinusOneMinusOne,
    Loyalty,
    Other(String),
}

/// A modification applied by a continuous effect. Each variant belongs to exactly one layer
/// (CR 613.1), which is what `layer()` reports.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modification {
    // Layer 1a
    BecomeCopyOf(crate::ids::ObjectId),
    // Layer 2
    GainControl(PlayerId),
    /// Control goes to whoever controls the effect's source (e.g. a control Aura).
    ControlBySourceController,
    // Layer 3: text-changing effects (not yet scripted by any card).
    // Layer 4
    AddTypes(Vec<CardType>),
    RemoveTypes(Vec<CardType>),
    AddSubtypes(Vec<String>),
    AddSupertypes(Vec<Supertype>),
    /// Sets a land's subtype to basic land types, with the consequences of CR 305.7.
    SetBasicLandTypes(Vec<String>),
    /// Sets creature types, removing old ones (e.g. "becomes a 1/1 Frog").
    SetCreatureTypes(Vec<String>),
    // Layer 5
    SetColors(ColorSet),
    AddColors(ColorSet),
    // Layer 6
    AddAbility(Box<Ability>),
    AddKeyword(Keyword),
    RemoveKeyword(Keyword),
    RemoveAllAbilities,
    // Layer 7a: handled through `Ability::CharacteristicPT`
    // Layer 7b
    SetPT(i32, i32),
    SetPTToManaValue,
    SetBasePower(i32),
    // Layer 7c
    ModifyPT(i32, i32),
    /// +N/+N where N is counted at the time of application.
    ModifyPTBy(Amount, Amount),
    // Layer 7d
    SwitchPT,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Layer {
    L1a,
    L1b,
    L2,
    L3,
    L4,
    L5,
    L6,
    L7a,
    L7b,
    L7c,
    L7d,
}

impl Layer {
    pub const ORDER: [Layer; 11] = [
        Layer::L1a,
        Layer::L1b,
        Layer::L2,
        Layer::L3,
        Layer::L4,
        Layer::L5,
        Layer::L6,
        Layer::L7a,
        Layer::L7b,
        Layer::L7c,
        Layer::L7d,
    ];
}

impl Modification {
    pub fn layer(&self) -> Layer {
        use Modification::*;
        match self {
            BecomeCopyOf(_) => Layer::L1a,
            GainControl(_) | ControlBySourceController => Layer::L2,
            AddTypes(_) | RemoveTypes(_) | AddSubtypes(_) | AddSupertypes(_)
            | SetBasicLandTypes(_) | SetCreatureTypes(_) => Layer::L4,
            SetColors(_) | AddColors(_) => Layer::L5,
            AddAbility(_) | AddKeyword(_) | RemoveKeyword(_) | RemoveAllAbilities => Layer::L6,
            SetPT(..) | SetPTToManaValue | SetBasePower(_) => Layer::L7b,
            ModifyPT(..) | ModifyPTBy(..) => Layer::L7c,
            SwitchPT => Layer::L7d,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cost {
    pub mana: ManaCost,
    pub tap: bool,
    pub sacrifice_this: bool,
    pub pay_life: u32,
}

impl Cost {
    pub fn tap() -> Cost {
        Cost {
            tap: true,
            ..Default::default()
        }
    }
    pub fn mana(m: &str) -> Cost {
        Cost {
            mana: ManaCost::parse(m).expect("valid mana cost"),
            ..Default::default()
        }
    }
    pub fn and_tap(mut self) -> Cost {
        self.tap = true;
        self
    }
}

/// Events that can trigger abilities (CR 603).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerEvent {
    /// "When/Whenever [filter] enters" (filter may be `this`).
    EntersBattlefield(Filter),
    /// "When/Whenever [filter] dies" (leaves-the-battlefield trigger that looks back in time, 603.10a).
    Dies(Filter),
    /// "Whenever you cast a [filter] spell"
    YouCastSpell(Filter),
    /// "At the beginning of [your/each] upkeep"
    BeginningOfUpkeep(PlayerRef),
    /// "At the beginning of [your/each] end step"
    BeginningOfEndStep(PlayerRef),
    /// "Whenever [filter] attacks"
    Attacks(Filter),
    /// "Whenever [filter] deals combat damage to a player"
    DealsCombatDamageToPlayer(Filter),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    Keyword(Keyword),
    /// Activated mana ability (CR 605): doesn't use the stack.
    Mana {
        cost: Cost,
        produces: ManaProduction,
    },
    Activated {
        cost: Cost,
        targets: Vec<TargetSpec>,
        effects: Vec<Effect>,
        sorcery_speed: bool,
    },
    Triggered {
        event: TriggerEvent,
        targets: Vec<TargetSpec>,
        effects: Vec<Effect>,
        optional: bool,
    },
    /// Static ability generating a continuous effect (CR 604, 611.3).
    Static {
        affected: Filter,
        mods: Vec<Modification>,
    },
    /// Characteristic-defining ability setting power/toughness (CR 604.3, layer 7a).
    CharacteristicPT {
        power: Amount,
        toughness: Amount,
    },
    /// Text the engine can't execute yet; kept so the UI can show it and flag the card.
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManaProduction {
    Fixed(Vec<ManaType>),
    OneOf(Vec<ManaType>),
    AnyColor,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardDef {
    pub name: String,
    pub mana_cost: ManaCost,
    /// Color indicator / explicit colors; when empty colors come from the mana cost (CR 202.2).
    pub color_indicator: Option<ColorSet>,
    pub supertypes: BTreeSet<Supertype>,
    pub types: BTreeSet<CardType>,
    pub subtypes: Vec<String>,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub loyalty: Option<i32>,
    pub abilities: Vec<Ability>,
    /// Instructions performed when this instant/sorcery resolves.
    pub spell_targets: Vec<TargetSpec>,
    pub spell_effects: Vec<Effect>,
    pub oracle_text: String,
    /// Color identity for Commander deck building (CR 903.4), precomputed by the importer.
    pub color_identity: ColorSet,
    pub is_token: bool,
}

impl CardDef {
    pub fn colors(&self) -> ColorSet {
        self.color_indicator
            .unwrap_or_else(|| self.mana_cost.colors())
    }
    pub fn has_type(&self, t: CardType) -> bool {
        self.types.contains(&t)
    }
    pub fn is_permanent(&self) -> bool {
        self.types.iter().any(|t| t.is_permanent_type())
    }
    /// True when every ability is something the engine can execute.
    pub fn fully_supported(&self) -> bool {
        !self
            .abilities
            .iter()
            .any(|a| matches!(a, Ability::Unsupported(_)))
    }
}

/// Fluent builder used by tests and hand-scripted cards.
pub struct CardBuilder(CardDef);

impl CardBuilder {
    pub fn new(name: &str) -> CardBuilder {
        CardBuilder(CardDef {
            name: name.into(),
            ..Default::default()
        })
    }
    pub fn cost(mut self, c: &str) -> Self {
        self.0.mana_cost = ManaCost::parse(c).expect("valid cost");
        self
    }
    pub fn types(mut self, ts: &[CardType]) -> Self {
        self.0.types.extend(ts.iter().copied());
        self
    }
    pub fn supertypes(mut self, ts: &[Supertype]) -> Self {
        self.0.supertypes.extend(ts.iter().copied());
        self
    }
    pub fn subtypes(mut self, ts: &[&str]) -> Self {
        self.0.subtypes.extend(ts.iter().map(|s| s.to_string()));
        self
    }
    pub fn pt(mut self, p: i32, t: i32) -> Self {
        self.0.power = Some(p);
        self.0.toughness = Some(t);
        self
    }
    pub fn colors(mut self, cs: &[Color]) -> Self {
        self.0.color_indicator = Some(ColorSet::of(cs));
        self
    }
    pub fn ability(mut self, a: Ability) -> Self {
        self.0.abilities.push(a);
        self
    }
    pub fn keyword(self, k: Keyword) -> Self {
        self.ability(Ability::Keyword(k))
    }
    pub fn spell(mut self, targets: Vec<TargetSpec>, effects: Vec<Effect>) -> Self {
        self.0.spell_targets = targets;
        self.0.spell_effects = effects;
        self
    }
    pub fn token(mut self) -> Self {
        self.0.is_token = true;
        self
    }
    pub fn build(mut self) -> Arc<CardDef> {
        if self.0.color_identity.is_empty() {
            self.0.color_identity = self.0.colors();
        }
        Arc::new(self.0)
    }
}
