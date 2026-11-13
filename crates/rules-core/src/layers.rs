//! The layer system (CR 613): computing every object's current characteristics from its
//! printed values plus all applicable continuous effects.
//!
//! Characteristics are never stored mutated. `compute` starts from printed values (613.1)
//! and walks layers 1a, 1b, 2, 3, 4, 5, 6, 7a, 7b, 7c, 7d. Within each layer:
//!
//! * characteristic-defining abilities apply first (613.3, 613.4a);
//! * the rest are ordered by dependency (613.8), falling back to timestamp order (613.7)
//!   and re-evaluating after every application (613.8c);
//! * an effect that started applying in an earlier layer keeps applying to the same set of
//!   objects in later layers, even if the ability that generated it was removed (613.6);
//! * static-ability effects only exist while their source has the ability at the moment the
//!   layer is applied, which is what makes e.g. Blood Moon switch off Urborg's ability (613.8a(b)).

use crate::card::{
    Ability, Amount, CardType, Filter, Keyword, Layer, ManaProduction, Modification, PlayerRef,
    Supertype,
};
use crate::card::{Cost, CounterKind};
use crate::game::Game;
use crate::ids::{ObjectId, PlayerId, Timestamp};
use crate::mana::{Color, ColorSet, ManaCost, ManaType};
use crate::state::Zone;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};

/// Identifies one ability on one object so effects it generates can be tracked across layers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbilityKey {
    /// Printed on the card (index into `CardDef::abilities`). These are "abilities generated
    /// from its rules text" for CR 305.7.
    Printed(usize),
    /// Granted by a continuous effect (hash of the effect key, index within it).
    Granted(u64, usize),
    /// Intrinsic mana ability of a basic land type (CR 305.6).
    BasicLandType(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbilityInstance {
    pub key: AbilityKey,
    pub ability: Ability,
}

/// An object's characteristics after all continuous effects (CR 109.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Characteristics {
    pub name: String,
    pub mana_cost: ManaCost,
    pub colors: ColorSet,
    pub supertypes: BTreeSet<Supertype>,
    pub types: BTreeSet<CardType>,
    pub subtypes: Vec<String>,
    pub abilities: Vec<AbilityInstance>,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub loyalty: Option<i32>,
    pub controller: PlayerId,
}

impl Characteristics {
    pub fn has_type(&self, t: CardType) -> bool {
        self.types.contains(&t)
    }
    pub fn is_creature(&self) -> bool {
        self.has_type(CardType::Creature)
    }
    pub fn has_subtype(&self, s: &str) -> bool {
        self.subtypes.iter().any(|x| x.eq_ignore_ascii_case(s))
    }
    pub fn has_keyword(&self, k: Keyword) -> bool {
        self.abilities
            .iter()
            .any(|a| a.ability == Ability::Keyword(k))
    }
    pub fn mana_value(&self) -> u32 {
        self.mana_cost.mana_value()
    }
}

/// The result of applying the layer system to the whole game.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub chars: HashMap<ObjectId, Characteristics>,
}

impl View {
    pub fn get(&self, id: ObjectId) -> Option<&Characteristics> {
        self.chars.get(&id)
    }
    pub fn controller(&self, id: ObjectId) -> Option<PlayerId> {
        self.chars.get(&id).map(|c| c.controller)
    }
}

pub const BASIC_LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

const LAND_TYPES: &[&str] = &[
    "Plains",
    "Island",
    "Swamp",
    "Mountain",
    "Forest",
    "Cave",
    "Desert",
    "Gate",
    "Lair",
    "Locus",
    "Mine",
    "Planet",
    "Power-Plant",
    "Sphere",
    "Tower",
    "Town",
    "Urza's",
];

const NONCREATURE_SUBTYPES: &[&str] = &[
    // artifact types
    "Attraction",
    "Blood",
    "Bobblehead",
    "Clue",
    "Contraption",
    "Equipment",
    "Food",
    "Fortification",
    "Gold",
    "Incubator",
    "Junk",
    "Lander",
    "Map",
    "Powerstone",
    "Spacecraft",
    "Treasure",
    "Vehicle",
    // enchantment types
    "Aura",
    "Background",
    "Cartouche",
    "Case",
    "Class",
    "Curse",
    "Role",
    "Room",
    "Rune",
    "Saga",
    "Shard",
    "Shrine",
];

pub fn basic_land_mana(subtype: &str) -> Option<Color> {
    Some(match subtype {
        "Plains" => Color::White,
        "Island" => Color::Blue,
        "Swamp" => Color::Black,
        "Mountain" => Color::Red,
        "Forest" => Color::Green,
        _ => return None,
    })
}

fn is_land_type(s: &str) -> bool {
    LAND_TYPES.iter().any(|t| t.eq_ignore_ascii_case(s))
}

/// Identity of a continuous effect during computation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum EffectKey {
    Stored(u64),
    Static(ObjectId, AbilityKey),
    Cda(ObjectId),
    Counters(ObjectId),
}

impl EffectKey {
    fn hash64(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}

#[derive(Clone, Debug)]
enum Scope {
    /// Fixed objects (resolved spell effects, 611.2c).
    Locked(Vec<ObjectId>),
    /// Static ability: whatever matches right now (611.3a).
    Filter(Filter),
    SelfOnly,
}

#[derive(Clone, Debug)]
struct Candidate {
    key: EffectKey,
    timestamp: Timestamp,
    source: Option<ObjectId>,
    /// Controller of the effect when it isn't derived from the source (stored effects).
    fixed_controller: Option<PlayerId>,
    scope: Scope,
    mods: Vec<Modification>,
    is_cda: bool,
}

/// Concrete form of a modification with every dynamic value resolved; comparing these is how
/// we detect "what it does" changing for CR 613.8a(d).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Resolved {
    Mod(Modification),
    Copy(Box<Characteristics>),
    Controller(PlayerId),
    Pt(Option<i32>, Option<i32>),
    DeltaPt(i32, i32),
}

struct Ctx<'g> {
    game: &'g Game,
}

pub fn compute(game: &Game) -> View {
    let ctx = Ctx { game };
    let mut chars: HashMap<ObjectId, Characteristics> = HashMap::new();
    for (id, obj) in &game.objects {
        let d = &obj.def;
        chars.insert(
            *id,
            Characteristics {
                name: d.name.clone(),
                mana_cost: d.mana_cost.clone(),
                colors: d.colors(),
                supertypes: d.supertypes.clone(),
                types: d.types.clone(),
                subtypes: d.subtypes.clone(),
                abilities: d
                    .abilities
                    .iter()
                    .enumerate()
                    .map(|(i, a)| AbilityInstance {
                        key: AbilityKey::Printed(i),
                        ability: a.clone(),
                    })
                    .collect(),
                power: d.power,
                toughness: d.toughness,
                loyalty: d.loyalty,
                controller: obj.base_controller,
            },
        );
    }

    // Effects that have begun applying, with the objects they're locked to (613.6).
    let mut started: BTreeMap<EffectKey, (Candidate, Vec<ObjectId>)> = BTreeMap::new();

    for layer in Layer::ORDER {
        let mut cands: Vec<Candidate> = ctx.candidates(layer, &chars);
        // Continue effects that started in an earlier layer, even if their ability is gone now.
        for (key, (cand, _)) in &started {
            if cand.mods.iter().any(|m| m.layer() == layer) && !cands.iter().any(|c| &c.key == key)
            {
                cands.push(cand.clone());
            }
        }
        cands.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then(a.key.cmp(&b.key)));

        let (cdas, mut rest): (Vec<_>, Vec<_>) = cands.into_iter().partition(|c| c.is_cda);
        for c in cdas {
            ctx.apply(&c, layer, &mut chars, &mut started);
        }
        while !rest.is_empty() {
            let pick = if rest.len() == 1 || !ctx.may_have_dependencies(&rest, layer) {
                0
            } else {
                // First effect (in timestamp order) that depends on no other unapplied effect.
                // If every remaining effect is in a dependency loop, timestamp order (613.8b).
                (0..rest.len())
                    .find(|&i| {
                        !(0..rest.len()).any(|j| {
                            j != i && ctx.depends_on(&rest[i], &rest[j], layer, &chars, &started)
                        })
                    })
                    .unwrap_or(0)
            };
            let c = rest.remove(pick);
            // An effect whose generating ability vanished before it started never applies.
            if !started.contains_key(&c.key) && !ctx.exists(&c, &chars) {
                continue;
            }
            ctx.apply(&c, layer, &mut chars, &mut started);
        }

        if layer == Layer::L4 {
            add_basic_land_abilities(&mut chars);
        }
    }
    View { chars }
}

/// CR 305.6: a land with a basic land type has the matching intrinsic mana ability.
fn add_basic_land_abilities(chars: &mut HashMap<ObjectId, Characteristics>) {
    for c in chars.values_mut() {
        if !c.has_type(CardType::Land) {
            continue;
        }
        for t in BASIC_LAND_TYPES {
            if c.has_subtype(t)
                && !c
                    .abilities
                    .iter()
                    .any(|a| a.key == AbilityKey::BasicLandType(t.into()))
            {
                let color = basic_land_mana(t).unwrap();
                c.abilities.push(AbilityInstance {
                    key: AbilityKey::BasicLandType(t.into()),
                    ability: Ability::Mana {
                        cost: Cost::tap(),
                        produces: ManaProduction::Fixed(vec![ManaType::Color(color)]),
                    },
                });
            }
        }
    }
}

impl<'g> Ctx<'g> {
    fn candidates(
        &self,
        layer: Layer,
        chars: &HashMap<ObjectId, Characteristics>,
    ) -> Vec<Candidate> {
        let g = self.game;
        let mut out = Vec::new();
        for e in &g.effects {
            let mods: Vec<_> = e.mods.clone();
            if mods.iter().any(|m| m.layer() == layer) {
                out.push(Candidate {
                    key: EffectKey::Stored(e.id),
                    timestamp: e.timestamp,
                    source: e.source,
                    fixed_controller: Some(e.controller),
                    scope: Scope::Locked(e.objects.clone()),
                    mods,
                    is_cda: false,
                });
            }
        }
        for id in &g.battlefield {
            let Some(c) = chars.get(id) else { continue };
            let obj = &g.objects[id];
            for inst in &c.abilities {
                if let Ability::Static { affected, mods } = &inst.ability {
                    if mods.iter().any(|m| m.layer() == layer) {
                        out.push(Candidate {
                            key: EffectKey::Static(*id, inst.key.clone()),
                            // 613.7a: same timestamp as the object it's on.
                            timestamp: obj.timestamp,
                            source: Some(*id),
                            fixed_controller: None,
                            scope: Scope::Filter(affected.clone()),
                            mods: mods.clone(),
                            is_cda: false,
                        });
                    }
                }
            }
            if layer == Layer::L7c && !obj.counters.is_empty() {
                let n = |k: &CounterKind| *obj.counters.get(k).unwrap_or(&0) as i32;
                let delta = n(&CounterKind::PlusOnePlusOne) - n(&CounterKind::MinusOneMinusOne);
                if delta != 0 {
                    out.push(Candidate {
                        key: EffectKey::Counters(*id),
                        timestamp: obj.counter_timestamp,
                        source: Some(*id),
                        fixed_controller: None,
                        scope: Scope::SelfOnly,
                        mods: vec![Modification::ModifyPT(delta, delta)],
                        is_cda: false,
                    });
                }
            }
        }
        if layer == Layer::L7a {
            // CDAs function in all zones (604.3).
            for (id, c) in chars {
                for inst in &c.abilities {
                    if let Ability::CharacteristicPT { power, toughness } = &inst.ability {
                        out.push(Candidate {
                            key: EffectKey::Cda(*id),
                            timestamp: g.objects[id].timestamp,
                            source: Some(*id),
                            fixed_controller: None,
                            scope: Scope::SelfOnly,
                            mods: vec![Modification::ModifyPTBy(power.clone(), toughness.clone())],
                            is_cda: true,
                        });
                    }
                }
            }
        }
        out
    }

    /// Whether the ability generating this effect currently exists (613.8a(b)).
    fn exists(&self, c: &Candidate, chars: &HashMap<ObjectId, Characteristics>) -> bool {
        match &c.key {
            EffectKey::Stored(_) | EffectKey::Counters(_) => true,
            EffectKey::Static(obj, key) => chars.get(obj).is_some_and(|ch| {
                ch.abilities
                    .iter()
                    .any(|a| &a.key == key && matches!(a.ability, Ability::Static { .. }))
            }),
            EffectKey::Cda(obj) => chars.get(obj).is_some_and(|ch| {
                ch.abilities
                    .iter()
                    .any(|a| matches!(a.ability, Ability::CharacteristicPT { .. }))
            }),
        }
    }

    fn effect_controller(
        &self,
        c: &Candidate,
        chars: &HashMap<ObjectId, Characteristics>,
    ) -> PlayerId {
        c.fixed_controller
            .or_else(|| c.source.and_then(|s| chars.get(&s).map(|x| x.controller)))
            .unwrap_or(PlayerId(0))
    }

    fn affected(
        &self,
        c: &Candidate,
        chars: &HashMap<ObjectId, Characteristics>,
        started: &BTreeMap<EffectKey, (Candidate, Vec<ObjectId>)>,
    ) -> Vec<ObjectId> {
        if let Some((_, locked)) = started.get(&c.key) {
            return locked.clone();
        }
        match &c.scope {
            Scope::Locked(v) => v
                .iter()
                .copied()
                .filter(|id| chars.contains_key(id))
                .collect(),
            Scope::SelfOnly => c.source.into_iter().collect(),
            Scope::Filter(f) => {
                let ctl = self.effect_controller(c, chars);
                let mut v: Vec<ObjectId> = self
                    .game
                    .battlefield
                    .iter()
                    .copied()
                    .filter(|id| matches_filter(self.game, f, *id, chars, c.source, ctl))
                    .collect();
                v.sort();
                v
            }
        }
    }

    fn resolve(
        &self,
        c: &Candidate,
        layer: Layer,
        obj: ObjectId,
        chars: &HashMap<ObjectId, Characteristics>,
    ) -> Vec<Resolved> {
        let ctl = self.effect_controller(c, chars);
        c.mods
            .iter()
            .filter(|m| m.layer() == layer)
            .map(|m| match m {
                Modification::BecomeCopyOf(other) => match chars.get(other) {
                    Some(o) => Resolved::Copy(Box::new(o.clone())),
                    None => Resolved::Mod(m.clone()),
                },
                Modification::ControlBySourceController => Resolved::Controller(ctl),
                Modification::GainControl(p) => Resolved::Controller(*p),
                Modification::SetPTToManaValue => {
                    let mv = chars.get(&obj).map(|x| x.mana_value() as i32).unwrap_or(0);
                    Resolved::Pt(Some(mv), Some(mv))
                }
                Modification::ModifyPTBy(p, t) => {
                    let src = if c.is_cda { Some(obj) } else { c.source };
                    let (p, t) = (
                        eval_amount(self.game, p, src, ctl, obj, chars),
                        eval_amount(self.game, t, src, ctl, obj, chars),
                    );
                    // A characteristic-defining ability sets the values (604.3, 613.4a).
                    if c.is_cda {
                        Resolved::Pt(Some(p), Some(t))
                    } else {
                        Resolved::DeltaPt(p, t)
                    }
                }
                other => Resolved::Mod(other.clone()),
            })
            .collect()
    }

    /// Cheap pre-check: only some combinations of effects can possibly depend on each other.
    fn may_have_dependencies(&self, cands: &[Candidate], layer: Layer) -> bool {
        match layer {
            // Fixed +N/+N modifications commute and can't change each other's scope.
            Layer::L7c => cands.iter().any(|c| {
                matches!(c.scope, Scope::Filter(_))
                    || c.mods
                        .iter()
                        .any(|m| matches!(m, Modification::ModifyPTBy(..)))
            }),
            _ => true,
        }
    }

    /// CR 613.8a: does `a` depend on `b`?
    fn depends_on(
        &self,
        a: &Candidate,
        b: &Candidate,
        layer: Layer,
        chars: &HashMap<ObjectId, Characteristics>,
        started: &BTreeMap<EffectKey, (Candidate, Vec<ObjectId>)>,
    ) -> bool {
        if a.is_cda != b.is_cda {
            return false;
        }
        if !started.contains_key(&b.key) && !self.exists(b, chars) {
            return false;
        }
        let mut after = chars.clone();
        let mut scratch = started.clone();
        self.apply(b, layer, &mut after, &mut scratch);

        let exists_before = started.contains_key(&a.key) || self.exists(a, chars);
        let exists_after = started.contains_key(&a.key) || self.exists(a, &after);
        if exists_before != exists_after {
            return true;
        }
        if !exists_before {
            return false;
        }
        let aff_before = self.affected(a, chars, started);
        let aff_after = self.affected(a, &after, started);
        if aff_before != aff_after {
            return true;
        }
        aff_before
            .iter()
            .any(|o| self.resolve(a, layer, *o, chars) != self.resolve(a, layer, *o, &after))
    }

    fn apply(
        &self,
        c: &Candidate,
        layer: Layer,
        chars: &mut HashMap<ObjectId, Characteristics>,
        started: &mut BTreeMap<EffectKey, (Candidate, Vec<ObjectId>)>,
    ) {
        let targets = self.affected(c, chars, started);
        let resolved: Vec<(ObjectId, Vec<Resolved>)> = targets
            .iter()
            .map(|o| (*o, self.resolve(c, layer, *o, chars)))
            .collect();
        let key_hash = c.key.hash64();
        for (obj, rs) in resolved {
            let Some(ch) = chars.get_mut(&obj) else {
                continue;
            };
            for (i, r) in rs.into_iter().enumerate() {
                apply_one(ch, r, key_hash, i);
            }
        }
        started
            .entry(c.key.clone())
            .or_insert_with(|| (c.clone(), targets));
    }
}

fn apply_one(ch: &mut Characteristics, r: Resolved, key_hash: u64, idx: usize) {
    use Modification as M;
    match r {
        Resolved::Copy(o) => {
            // 707.2: copiable values; control is not copied.
            let ctl = ch.controller;
            *ch = *o;
            ch.controller = ctl;
        }
        Resolved::Controller(p) => ch.controller = p,
        Resolved::Pt(p, t) => {
            ch.power = p;
            ch.toughness = t;
        }
        Resolved::DeltaPt(p, t) => {
            ch.power = Some(ch.power.unwrap_or(0) + p);
            ch.toughness = Some(ch.toughness.unwrap_or(0) + t);
        }
        Resolved::Mod(m) => match m {
            M::AddTypes(ts) => ch.types.extend(ts),
            M::RemoveTypes(ts) => ts.iter().for_each(|t| {
                ch.types.remove(t);
            }),
            M::AddSubtypes(ss) => {
                for s in ss {
                    if !ch.has_subtype(&s) {
                        ch.subtypes.push(s);
                    }
                }
            }
            M::AddSupertypes(ss) => ch.supertypes.extend(ss),
            M::SetBasicLandTypes(ss) => {
                if ch.has_type(CardType::Land) {
                    // 305.7: lose old land types and abilities from rules text.
                    ch.subtypes.retain(|s| !is_land_type(s));
                    ch.subtypes.extend(ss);
                    ch.abilities.retain(|a| {
                        !matches!(a.key, AbilityKey::Printed(_) | AbilityKey::BasicLandType(_))
                    });
                }
            }
            M::SetCreatureTypes(ss) => {
                ch.subtypes.retain(|s| {
                    is_land_type(s)
                        || NONCREATURE_SUBTYPES
                            .iter()
                            .any(|n| n.eq_ignore_ascii_case(s))
                });
                ch.subtypes.extend(ss);
            }
            M::SetColors(c) => ch.colors = c,
            M::AddColors(c) => ch.colors = ch.colors.union(c),
            M::AddAbility(a) => ch.abilities.push(AbilityInstance {
                key: AbilityKey::Granted(key_hash, idx),
                ability: *a,
            }),
            M::AddKeyword(k) => ch.abilities.push(AbilityInstance {
                key: AbilityKey::Granted(key_hash, idx),
                ability: Ability::Keyword(k),
            }),
            M::RemoveKeyword(k) => ch.abilities.retain(|a| a.ability != Ability::Keyword(k)),
            M::RemoveAllAbilities => ch.abilities.clear(),
            M::SetPT(p, t) => {
                ch.power = Some(p);
                ch.toughness = Some(t);
            }
            M::SetBasePower(p) => ch.power = Some(p),
            M::ModifyPT(p, t) => {
                if let (Some(pp), Some(tt)) = (ch.power, ch.toughness) {
                    ch.power = Some(pp + p);
                    ch.toughness = Some(tt + t);
                }
            }
            M::SwitchPT => std::mem::swap(&mut ch.power, &mut ch.toughness),
            M::BecomeCopyOf(_)
            | M::GainControl(_)
            | M::ControlBySourceController
            | M::SetPTToManaValue
            | M::ModifyPTBy(..) => {}
        },
    }
}

/// Does object `id` match `f`, evaluated against `chars` as they currently stand?
pub fn matches_filter(
    game: &Game,
    f: &Filter,
    id: ObjectId,
    chars: &HashMap<ObjectId, Characteristics>,
    source: Option<ObjectId>,
    you: PlayerId,
) -> bool {
    let Some(c) = chars.get(&id) else {
        return false;
    };
    if f.this_object && source != Some(id) {
        return false;
    }
    if f.other_than_this && source == Some(id) {
        return false;
    }
    if !f.any_of_types.is_empty() && !f.any_of_types.iter().any(|t| c.has_type(*t)) {
        return false;
    }
    if !f.all_types.iter().all(|t| c.has_type(*t)) || f.not_types.iter().any(|t| c.has_type(*t)) {
        return false;
    }
    if !f.subtypes.is_empty() && !f.subtypes.iter().any(|s| c.has_subtype(s)) {
        return false;
    }
    if f.not_subtypes.iter().any(|s| c.has_subtype(s)) {
        return false;
    }
    if !f.supertypes.iter().all(|s| c.supertypes.contains(s))
        || f.not_supertypes.iter().any(|s| c.supertypes.contains(s))
    {
        return false;
    }
    if let Some(cs) = f.colors {
        if !cs.iter().any(|col| c.colors.contains(col)) {
            return false;
        }
    }
    if let Some(k) = f.keyword {
        if !c.has_keyword(k) {
            return false;
        }
    }
    if f.nontoken && game.objects.get(&id).is_some_and(|o| o.is_token) {
        return false;
    }
    match f.controller {
        Some(PlayerRef::You) if c.controller != you => return false,
        Some(PlayerRef::Opponents) if c.controller == you => return false,
        _ => {}
    }
    true
}

pub fn eval_amount(
    game: &Game,
    a: &Amount,
    source: Option<ObjectId>,
    you: PlayerId,
    this: ObjectId,
    chars: &HashMap<ObjectId, Characteristics>,
) -> i32 {
    match a {
        Amount::Fixed(n) => *n,
        Amount::X => 0,
        Amount::CountPermanents(f) => game
            .battlefield
            .iter()
            .filter(|id| matches_filter(game, f, **id, chars, source, you))
            .count() as i32,
        Amount::CardTypesInAllGraveyards => {
            let mut types = BTreeSet::new();
            for p in &game.players {
                for id in &p.graveyard {
                    if let Some(c) = chars.get(id) {
                        types.extend(c.types.iter().copied());
                    }
                }
            }
            types.len() as i32
        }
        Amount::CardsInHand => game.player(you).hand.len() as i32,
        Amount::ManaValueOfThis => chars.get(&this).map(|c| c.mana_value() as i32).unwrap_or(0),
        Amount::Sum(v) => v
            .iter()
            .map(|x| eval_amount(game, x, source, you, this, chars))
            .sum(),
    }
}

impl Game {
    /// Current characteristics of everything in the game (CR 613). Cached per state version.
    pub fn view(&self) -> std::sync::Arc<View> {
        let mut cache = self.view_cache.lock().unwrap();
        if let Some((v, view)) = cache.as_ref() {
            if *v == self.version {
                return view.clone();
            }
        }
        let view = std::sync::Arc::new(compute(self));
        *cache = Some((self.version, view.clone()));
        view
    }

    pub fn chars(&self, id: ObjectId) -> Option<Characteristics> {
        self.view().get(id).cloned()
    }

    pub fn is_on_battlefield(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.zone == Zone::Battlefield)
    }
}
