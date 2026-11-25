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
    Shroud,
    /// Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.
    Prowess,
    // Rules restrictions that behave like keywords: printed on a card ("~ can't block") or
    // granted by an Aura or Equipment ("enchanted creature can't attack or block").
    CantAttack,
    CantBlock,
    CantBeBlocked,
    CantBeCountered,
    Fear,
    Intimidate,
    Shadow,
    Horsemanship,
    Skulk,
    Plainswalk,
    Islandwalk,
    Swampwalk,
    Mountainwalk,
    Forestwalk,
    /// "~ can block only creatures with flying."
    CanBlockOnlyFlyers,
    /// "~ can't be blocked by more than one creature."
    CantBeBlockedByMoreThanOne,
    ProtectionFromWhite,
    ProtectionFromBlue,
    ProtectionFromBlack,
    ProtectionFromRed,
    ProtectionFromGreen,
    /// Every creature type (CR 702.73).
    Changeling,
    /// Colorless (CR 702.114); the colors themselves come from the card data.
    Devoid,
    /// "~ doesn't untap during your untap step."
    DoesntUntap,
    /// Creatures you control can tap to help pay for this spell (CR 702.51).
    Convoke,
    /// Artifacts you control can tap to pay generic mana for this spell (CR 702.126).
    Improvise,
    /// Exile cards from your graveyard to pay generic mana for this spell (CR 702.66).
    Delve,
    /// Whenever a creature you control attacks alone, it gets +1/+1 (CR 702.83).
    Exalted,
    /// Deals damage to creatures as -1/-1 counters and to players as poison (CR 702.90).
    Infect,
    /// Deals damage to creatures as -1/-1 counters (CR 702.80).
    Wither,
    /// Returns with a +1/+1 counter if it dies without one (CR 702.93).
    Undying,
    /// Returns with a -1/-1 counter if it dies without one (CR 702.79).
    Persist,
    /// Blockers without flanking get -1/-1 (CR 702.25).
    Flanking,
    /// "~ attacks each combat if able."
    AttacksEachCombat,
    /// "~ blocks each combat if able."
    BlocksEachCombat,
    /// Protection from artifacts, everything, etc. are separate; this one is from multicolored.
    ProtectionFromArtifacts,
    /// Can block an additional creature each combat.
    ExtraBlock,
    /// Split second (CR 702.61): no spells or non-mana abilities while it's on the stack.
    SplitSecond,
}

impl Keyword {
    /// The color this keyword grants protection from, if it's a protection keyword.
    pub fn protection_color(self) -> Option<Color> {
        Some(match self {
            Keyword::ProtectionFromWhite => Color::White,
            Keyword::ProtectionFromBlue => Color::Blue,
            Keyword::ProtectionFromBlack => Color::Black,
            Keyword::ProtectionFromRed => Color::Red,
            Keyword::ProtectionFromGreen => Color::Green,
            _ => return None,
        })
    }

    /// How the game names it to players.
    pub fn label(self) -> String {
        match self {
            Keyword::CantAttack => "Can't attack".into(),
            Keyword::CantBlock => "Can't block".into(),
            Keyword::CantBeBlocked => "Can't be blocked".into(),
            Keyword::CantBeCountered => "Can't be countered".into(),
            Keyword::CanBlockOnlyFlyers => "Can block only flyers".into(),
            Keyword::CantBeBlockedByMoreThanOne => "Can't be blocked by more than one".into(),
            Keyword::DoesntUntap => "Doesn't untap".into(),
            Keyword::AttacksEachCombat => "Attacks each combat if able".into(),
            Keyword::BlocksEachCombat => "Blocks each combat if able".into(),
            Keyword::ProtectionFromArtifacts => "Protection from artifacts".into(),
            Keyword::ExtraBlock => "Can block an additional creature".into(),
            Keyword::SplitSecond => "Split second".into(),
            Keyword::ProtectionFromWhite => "Protection from white".into(),
            Keyword::ProtectionFromBlue => "Protection from blue".into(),
            Keyword::ProtectionFromBlack => "Protection from black".into(),
            Keyword::ProtectionFromRed => "Protection from red".into(),
            Keyword::ProtectionFromGreen => "Protection from green".into(),
            k => format!("{k:?}"),
        }
    }

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
            "shroud" => Keyword::Shroud,
            "prowess" => Keyword::Prowess,
            "fear" => Keyword::Fear,
            "intimidate" => Keyword::Intimidate,
            "shadow" => Keyword::Shadow,
            "horsemanship" => Keyword::Horsemanship,
            "skulk" => Keyword::Skulk,
            "plainswalk" => Keyword::Plainswalk,
            "islandwalk" => Keyword::Islandwalk,
            "swampwalk" => Keyword::Swampwalk,
            "mountainwalk" => Keyword::Mountainwalk,
            "forestwalk" => Keyword::Forestwalk,
            "protection from white" => Keyword::ProtectionFromWhite,
            "protection from blue" => Keyword::ProtectionFromBlue,
            "protection from black" => Keyword::ProtectionFromBlack,
            "protection from red" => Keyword::ProtectionFromRed,
            "protection from green" => Keyword::ProtectionFromGreen,
            "changeling" => Keyword::Changeling,
            "devoid" => Keyword::Devoid,
            "convoke" => Keyword::Convoke,
            "improvise" => Keyword::Improvise,
            "delve" => Keyword::Delve,
            "exalted" => Keyword::Exalted,
            "infect" => Keyword::Infect,
            "wither" => Keyword::Wither,
            "undying" => Keyword::Undying,
            "persist" => Keyword::Persist,
            "flanking" => Keyword::Flanking,
            "protection from artifacts" => Keyword::ProtectionFromArtifacts,
            "split second" => Keyword::SplitSecond,
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
#[serde(default)]
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
    /// Object must have none of these colors ("nonblack").
    #[serde(default)]
    pub not_colors: Option<ColorSet>,
    /// Only the source object itself.
    pub this_object: bool,
    /// Exclude the source object ("other creatures").
    pub other_than_this: bool,
    pub nontoken: bool,
    pub keyword: Option<Keyword>,
    /// Only the permanent the source is attached to ("enchanted creature", "equipped creature").
    pub attached_to_this: bool,
    /// Object must be untapped / tapped.
    pub tapped: Option<bool>,
    /// Must be attacking (or, with `blocking` too, attacking or blocking).
    #[serde(default)]
    pub attacking: bool,
    /// Must be blocking (or, with `attacking` too, attacking or blocking).
    #[serde(default)]
    pub blocking: bool,
    /// Power at most / at least this.
    #[serde(default)]
    pub power_at_most: Option<i32>,
    #[serde(default)]
    pub power_at_least: Option<i32>,
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
    /// Attach the source (an Aura or Equipment) to the target permanent (CR 701.3).
    Attach {
        target: usize,
    },
    /// Look at the top N cards and put any number on the bottom (CR 701.22). The choice is
    /// made automatically for now.
    Scry {
        amount: Amount,
    },
    /// Like scry, but cards go to the graveyard (CR 701.25).
    Surveil {
        amount: Amount,
    },
    /// Search your library for up to `count` cards matching the filter, put them where
    /// `dest` says, then shuffle (CR 701.23). The cards are picked automatically for now.
    SearchLibrary {
        filter: Filter,
        count: u32,
        dest: SearchDest,
    },
    /// Put counters on each permanent matching the filter.
    AddCountersEach {
        filter: Filter,
        kind: CounterKind,
        amount: Amount,
    },
    /// The player discards cards (chosen automatically for now).
    Discard {
        who: PlayerRef,
        amount: Amount,
    },
    /// The source and the target creature deal damage equal to their power to each other (CR 701.14).
    Fight {
        target: usize,
    },
    /// Its controller (of the Nth target) gains or loses life equal to that target's power.
    ControllerOfTargetGainsLifeEqualToPower {
        target: usize,
    },
    /// Give the target (or the source, if `None`) a regeneration shield (CR 701.19).
    Regenerate {
        target: Option<usize>,
    },
    /// The source permanent returns to its owner's hand.
    ReturnThisToHand,
    /// Untap the source permanent.
    UntapThis,
    /// Undying, persist, "return it to the battlefield": the source card, if it's still in the
    /// graveyard, returns under its owner's control.
    ReturnThisToBattlefield {
        counter: Option<CounterKind>,
    },
    /// Exile the source (a permanent, or a spell as it finishes resolving).
    ExileThis,
    /// Sacrifice the source permanent (evoke).
    SacrificeThis,
    /// "You become the monarch" (CR 724).
    BecomeMonarch,
    /// "Draw a card at the beginning of the next turn's upkeep."
    DrawAtNextUpkeep,
    /// "Sacrifice ~ unless you pay {N}" (echo, upkeep costs). Paid automatically if possible.
    SacrificeThisUnlessPay(ManaCost),
    /// Unearth (CR 702.84): return the source card from the graveyard with haste; exile it at
    /// the next end step or if it would leave.
    Unearth,
    /// "If this spell was kicked, ..." (otherwise the second list, for "... instead").
    IfKicked {
        then: Vec<Effect>,
        otherwise: Vec<Effect>,
    },
    /// Ward (CR 702.21): counter that spell or ability unless its controller pays.
    WardCounter {
        stack_object: crate::ids::ObjectId,
        cost: WardCost,
    },
    /// "Counter target spell unless its controller pays {N}."
    CounterUnlessPays {
        target: usize,
        mana: ManaCost,
    },
}

/// What a ward ability asks for.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WardCost {
    Mana(ManaCost),
    Life(u32),
    Discard,
}

/// The condition of an "enters tapped unless" land, checked as it enters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryCondition {
    /// You control a land with one of these subtypes.
    ControlLandType(Vec<String>),
    /// You control at most N other lands.
    AtMostOtherLands(u32),
    /// You control at least N other lands.
    AtLeastOtherLands(u32),
    /// You control at least N basic lands.
    AtLeastBasicLands(u32),
    /// You have two or more opponents.
    TwoOrMoreOpponents,
    /// A player has N or less life.
    PlayerAtOrBelowLife(i32),
    /// You control a permanent of this type.
    ControlType(CardType),
}

/// Where searched-for cards go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SearchDest {
    Hand,
    Battlefield {
        tapped: bool,
    },
    /// The first onto the battlefield tapped, the rest into your hand (Cultivate).
    BattlefieldTappedThenHand,
    /// On top of the library after shuffling.
    TopOfLibrary,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Affected {
    Target(usize),
    ThisObject,
    Filter(Filter),
    ControllerOfThis,
    /// A specific object (abilities the engine creates, like flanking's).
    Object(crate::ids::ObjectId),
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
    /// Loyalty abilities (CR 606): add (or remove, if negative) loyalty counters. Once each
    /// turn per permanent, at sorcery speed.
    #[serde(default)]
    pub loyalty: Option<i32>,
    /// Painlands: "{T}: Add {U} or {B}. This land deals 1 damage to you." Not a cost, but it
    /// always comes with the mana, so it rides along here.
    #[serde(default)]
    pub deals_damage_to_you: u32,
    /// "Activate only once each turn."
    #[serde(default)]
    pub once_per_turn: bool,
    /// Discard this many cards (chosen automatically for now).
    #[serde(default)]
    pub discard: u32,
    /// Sacrifice this many permanents matching the filter (chosen automatically for now).
    #[serde(default)]
    pub sacrifice: Option<(Filter, u32)>,
    /// Remove counters from the source.
    #[serde(default)]
    pub remove_counters: Option<(CounterKind, u32)>,
    /// Cycling and similar: the ability works from your hand, and discarding the card pays it.
    #[serde(default)]
    pub discard_this: bool,
    /// Crew N (CR 702.122): tap untapped creatures you control with total power N or more.
    #[serde(default)]
    pub crew: Option<u32>,
    /// The ability works from your graveyard and exiling the card pays it (unearth, scavenge
    /// and "{2}: Return ~ from your graveyard to your hand").
    #[serde(default)]
    pub from_graveyard: bool,
    /// Exile the card from the graveyard as part of the cost.
    #[serde(default)]
    pub exile_this: bool,
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
    /// "Whenever you gain life"
    YouGainLife,
    /// "At the beginning of combat on your turn" / "of each combat".
    BeginningOfCombat(PlayerRef),
    /// "At the beginning of your precombat main phase" / "first main phase".
    BeginningOfFirstMain(PlayerRef),
    /// "When ~ leaves the battlefield" (looks back in time, CR 603.10a).
    LeavesBattlefield(Filter),
    /// "Whenever you attack" (once per combat, CR 508.3).
    YouAttack,
    /// "When you cast this spell" (triggers from the stack).
    CastThis,
    /// "Whenever an opponent casts a spell" / "Whenever a player casts a spell".
    PlayerCastsSpell(PlayerRef, Filter),
    /// "When ~ is turned face up" (CR 708.8).
    TurnedFaceUp,
    /// "Whenever [filter] blocks".
    Blocks(Filter),
    /// "Whenever [filter] becomes blocked" (once per combat, CR 509.3c).
    BecomesBlocked(Filter),
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
    /// "~ enters tapped."
    EntersTapped,
    /// "~ enters with N +1/+1 counters on it." (X uses the spell's X.)
    EntersWithCounters {
        kind: CounterKind,
        amount: Amount,
    },
    /// "You have no maximum hand size."
    NoMaxHandSize,
    /// "~ enters tapped unless ..." (checklands, fastlands, slowlands, battlelands...).
    EntersTappedUnless(EntryCondition),
    /// Flashback (CR 702.34): cast from your graveyard for this cost, then exile it.
    Flashback(ManaCost),
    /// Kicker (CR 702.33): an optional additional cost.
    Kicker(ManaCost),
    /// Morph / megamorph / disguise (CR 702.37, 702.168): cast face down for {3}, turn face
    /// up for this cost. Disguised permanents have ward {2} while face down.
    Morph {
        cost: ManaCost,
        megamorph: bool,
        disguise: bool,
    },
    /// Ward (CR 702.21).
    Ward(WardCost),
    /// "As an additional cost to cast this spell, sacrifice a creature / discard a card."
    AdditionalCost(Cost),
    /// Buyback (CR 702.27): an optional additional cost; the spell returns to your hand.
    Buyback(ManaCost),
    /// Echo (CR 702.30): at your next upkeep, sacrifice it unless you pay this.
    Echo(ManaCost),
    /// "If ~ was kicked, it enters with N +1/+1 counters on it."
    EntersWithCountersIfKicked {
        kind: CounterKind,
        amount: u32,
    },
    /// "~ can't be blocked by creatures with power N or less."
    CantBeBlockedByPowerAtMost(i32),
    /// Toxic N (CR 702.164): combat damage to a player also gives N poison counters.
    Toxic(u32),
    /// Evoke (CR 702.74): cast for this cost instead; sacrificed when it enters.
    Evoke(ManaCost),
    /// "As ~ enters, you may pay N life. If you don't, it enters tapped."
    EntersTappedUnlessPayLife(u32),
    /// "This spell costs {1} less to cast for each artifact you control" (affinity etc.).
    CostReduction(Amount),
    /// Text the engine can't execute yet; kept so the UI can show it and flag the card.
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManaProduction {
    Fixed(Vec<ManaType>),
    OneOf(Vec<ManaType>),
    AnyColor,
    /// One mana of any color in your commander's color identity.
    CommanderIdentity,
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
    /// Modal spells ("Choose one —", CR 700.2): when not empty, the caster picks one of these
    /// and it replaces `spell_targets` / `spell_effects`.
    #[serde(default)]
    pub spell_modes: Vec<SpellMode>,
    pub oracle_text: String,
    /// Color identity for Commander deck building (CR 903.4), precomputed by the importer.
    pub color_identity: ColorSet,
    pub is_token: bool,
}

/// One mode of a modal spell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpellMode {
    pub text: String,
    pub targets: Vec<TargetSpec>,
    pub effects: Vec<Effect>,
}

impl CardDef {
    /// Targets and effects of the spell as cast in `mode` (ignored for non-modal spells).
    /// None if the spell is modal and the mode doesn't exist.
    pub fn spell_parts(&self, mode: Option<usize>) -> Option<(&[TargetSpec], &[Effect])> {
        if self.spell_modes.is_empty() {
            return Some((&self.spell_targets, &self.spell_effects));
        }
        let m = self.spell_modes.get(mode?)?;
        Some((&m.targets, &m.effects))
    }

    pub fn flashback_cost(&self) -> Option<&ManaCost> {
        self.abilities.iter().find_map(|a| match a {
            Ability::Flashback(c) => Some(c),
            _ => None,
        })
    }

    pub fn evoke_cost(&self) -> Option<&ManaCost> {
        self.abilities.iter().find_map(|a| match a {
            Ability::Evoke(c) => Some(c),
            _ => None,
        })
    }

    pub fn kicker_cost(&self) -> Option<&ManaCost> {
        self.abilities.iter().find_map(|a| match a {
            Ability::Kicker(c) => Some(c),
            _ => None,
        })
    }

    pub fn buyback_cost(&self) -> Option<&ManaCost> {
        self.abilities.iter().find_map(|a| match a {
            Ability::Buyback(c) => Some(c),
            _ => None,
        })
    }

    pub fn additional_cost(&self) -> Option<&Cost> {
        self.abilities.iter().find_map(|a| match a {
            Ability::AdditionalCost(c) => Some(c),
            _ => None,
        })
    }

    /// Morph cost, megamorph, disguise.
    pub fn morph(&self) -> Option<(&ManaCost, bool, bool)> {
        self.abilities.iter().find_map(|a| match a {
            Ability::Morph {
                cost,
                megamorph,
                disguise,
            } => Some((cost, *megamorph, *disguise)),
            _ => None,
        })
    }

    /// The modes a caster can pick from: `[None]` for a normal spell.
    pub fn mode_choices(&self) -> Vec<Option<usize>> {
        if self.spell_modes.is_empty() {
            vec![None]
        } else {
            (0..self.spell_modes.len()).map(Some).collect()
        }
    }
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
