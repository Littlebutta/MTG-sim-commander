//! Mana symbols, costs and mana pools (CR 106, 107.4, 202).

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl Color {
    pub const ALL: [Color; 5] = [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ];

    pub fn from_letter(c: char) -> Option<Color> {
        Some(match c.to_ascii_uppercase() {
            'W' => Color::White,
            'U' => Color::Blue,
            'B' => Color::Black,
            'R' => Color::Red,
            'G' => Color::Green,
            _ => return None,
        })
    }

    pub fn letter(self) -> char {
        match self {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        }
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A set of colors stored as a bitmask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorSet(u8);

impl ColorSet {
    pub const EMPTY: ColorSet = ColorSet(0);

    pub fn of(colors: &[Color]) -> ColorSet {
        let mut s = ColorSet::EMPTY;
        for c in colors {
            s.insert(*c);
        }
        s
    }
    pub fn insert(&mut self, c: Color) {
        self.0 |= c.bit();
    }
    pub fn contains(self, c: Color) -> bool {
        self.0 & c.bit() != 0
    }
    pub fn union(self, o: ColorSet) -> ColorSet {
        ColorSet(self.0 | o.0)
    }
    pub fn is_subset_of(self, o: ColorSet) -> bool {
        self.0 & !o.0 == 0
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn len(self) -> u32 {
        self.0.count_ones()
    }
    pub fn iter(self) -> impl Iterator<Item = Color> {
        Color::ALL.into_iter().filter(move |c| self.contains(*c))
    }
}

/// One mana symbol (CR 107.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(Color),
    /// {C}: must be paid with colorless mana.
    Colorless,
    /// {W/U}
    Hybrid(Color, Color),
    /// {2/W}
    TwoHybrid(Color),
    /// {W/P}
    Phyrexian(Color),
    /// {W/U/P}
    PhyrexianHybrid(Color, Color),
    X,
    /// {S}: paid with mana from a snow source. Treated as generic for payment until snow tracking lands.
    Snow,
}

impl ManaSymbol {
    /// Contribution to mana value (CR 202.3).
    pub fn mana_value(self) -> u32 {
        match self {
            ManaSymbol::Generic(n) => n,
            ManaSymbol::TwoHybrid(_) => 2,
            ManaSymbol::X => 0,
            _ => 1,
        }
    }

    pub fn colors(self) -> ColorSet {
        match self {
            ManaSymbol::Colored(c) | ManaSymbol::TwoHybrid(c) | ManaSymbol::Phyrexian(c) => {
                ColorSet::of(&[c])
            }
            ManaSymbol::Hybrid(a, b) | ManaSymbol::PhyrexianHybrid(a, b) => ColorSet::of(&[a, b]),
            _ => ColorSet::EMPTY,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ManaCost(pub Vec<ManaSymbol>);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid mana cost: {0}")]
pub struct ManaParseError(pub String);

impl ManaCost {
    pub fn parse(s: &str) -> Result<ManaCost, ManaParseError> {
        let mut out = Vec::new();
        let mut rest = s.trim();
        while !rest.is_empty() {
            let open = rest.find('{').ok_or_else(|| ManaParseError(s.into()))?;
            if open != 0 {
                return Err(ManaParseError(s.into()));
            }
            let close = rest.find('}').ok_or_else(|| ManaParseError(s.into()))?;
            let sym = &rest[1..close];
            out.push(parse_symbol(sym).ok_or_else(|| ManaParseError(s.into()))?);
            rest = rest[close + 1..].trim_start();
        }
        Ok(ManaCost(out))
    }

    pub fn generic(n: u32) -> ManaCost {
        if n == 0 {
            ManaCost::default()
        } else {
            ManaCost(vec![ManaSymbol::Generic(n)])
        }
    }

    pub fn mana_value(&self) -> u32 {
        self.0.iter().map(|s| s.mana_value()).sum()
    }

    pub fn colors(&self) -> ColorSet {
        self.0
            .iter()
            .fold(ColorSet::EMPTY, |a, s| a.union(s.colors()))
    }

    pub fn has_x(&self) -> bool {
        self.0.contains(&ManaSymbol::X)
    }

    /// The cost with X replaced by a number and extra generic mana added (commander tax etc.).
    pub fn with_x_and_extra(&self, x: u32, extra_generic: u32) -> ManaCost {
        let mut syms: Vec<ManaSymbol> = self
            .0
            .iter()
            .map(|s| {
                if *s == ManaSymbol::X {
                    ManaSymbol::Generic(x)
                } else {
                    *s
                }
            })
            .collect();
        if extra_generic > 0 {
            syms.push(ManaSymbol::Generic(extra_generic));
        }
        ManaCost(
            syms.into_iter()
                .filter(|s| *s != ManaSymbol::Generic(0))
                .collect(),
        )
    }
}

fn parse_symbol(sym: &str) -> Option<ManaSymbol> {
    let up = sym.to_ascii_uppercase();
    if let Ok(n) = up.parse::<u32>() {
        return Some(ManaSymbol::Generic(n));
    }
    let parts: Vec<&str> = up.split('/').collect();
    let col = |p: &str| {
        p.chars()
            .next()
            .filter(|_| p.len() == 1)
            .and_then(Color::from_letter)
    };
    Some(match parts.as_slice() {
        ["X"] | ["Y"] | ["Z"] => ManaSymbol::X,
        ["C"] => ManaSymbol::Colorless,
        ["S"] => ManaSymbol::Snow,
        [c] => ManaSymbol::Colored(col(c)?),
        ["2", c] => ManaSymbol::TwoHybrid(col(c)?),
        [c, "P"] => ManaSymbol::Phyrexian(col(c)?),
        [a, b] => ManaSymbol::Hybrid(col(a)?, col(b)?),
        [a, b, "P"] => ManaSymbol::PhyrexianHybrid(col(a)?, col(b)?),
        _ => return None,
    })
}

impl fmt::Display for ManaCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for s in &self.0 {
            match s {
                ManaSymbol::Generic(n) => write!(f, "{{{n}}}")?,
                ManaSymbol::Colored(c) => write!(f, "{{{}}}", c.letter())?,
                ManaSymbol::Colorless => write!(f, "{{C}}")?,
                ManaSymbol::Hybrid(a, b) => write!(f, "{{{}/{}}}", a.letter(), b.letter())?,
                ManaSymbol::TwoHybrid(c) => write!(f, "{{2/{}}}", c.letter())?,
                ManaSymbol::Phyrexian(c) => write!(f, "{{{}/P}}", c.letter())?,
                ManaSymbol::PhyrexianHybrid(a, b) => {
                    write!(f, "{{{}/{}/P}}", a.letter(), b.letter())?
                }
                ManaSymbol::X => write!(f, "{{X}}")?,
                ManaSymbol::Snow => write!(f, "{{S}}")?,
            }
        }
        Ok(())
    }
}

/// A kind of mana in a pool: one of the five colors or colorless (CR 106.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ManaType {
    Color(Color),
    Colorless,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManaPool {
    /// W, U, B, R, G, C
    amounts: [u32; 6],
}

fn idx(t: ManaType) -> usize {
    match t {
        ManaType::Color(c) => c as usize,
        ManaType::Colorless => 5,
    }
}

const ALL_TYPES: [ManaType; 6] = [
    ManaType::Color(Color::White),
    ManaType::Color(Color::Blue),
    ManaType::Color(Color::Black),
    ManaType::Color(Color::Red),
    ManaType::Color(Color::Green),
    ManaType::Colorless,
];

/// What a payment did besides spending mana.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaymentResult {
    pub life_paid: u32,
}

impl ManaPool {
    pub fn add(&mut self, t: ManaType, n: u32) {
        self.amounts[idx(t)] += n;
    }
    pub fn get(&self, t: ManaType) -> u32 {
        self.amounts[idx(t)]
    }
    pub fn total(&self) -> u32 {
        self.amounts.iter().sum()
    }
    pub fn empty(&mut self) {
        self.amounts = [0; 6];
    }
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// Try to pay `cost` (X already substituted) from this pool. On success the pool is
    /// debited. Phyrexian symbols are paid with mana when possible, otherwise with 2 life
    /// if `can_pay_life` allows it. Uses a small exhaustive search so hybrid choices never
    /// strand a payment that a smarter choice could make.
    pub fn try_pay(&mut self, cost: &ManaCost, life_available: i32) -> Option<PaymentResult> {
        let mut generic = 0u32;
        let mut specific: Vec<ManaSymbol> = Vec::new();
        for s in &cost.0 {
            match s {
                ManaSymbol::Generic(n) => generic += n,
                ManaSymbol::Snow => generic += 1,
                ManaSymbol::X => {}
                other => specific.push(*other),
            }
        }
        // Most constrained first.
        specific.sort_by_key(|s| match s {
            ManaSymbol::Colored(_) | ManaSymbol::Colorless => 0,
            ManaSymbol::Hybrid(..) => 1,
            ManaSymbol::TwoHybrid(_) => 2,
            _ => 3,
        });
        let mut best: Option<([u32; 6], u32)> = None;
        search(
            &specific,
            0,
            self.amounts,
            generic,
            0,
            life_available,
            &mut best,
        );
        let (rest, life) = best?;
        self.amounts = rest;
        Some(PaymentResult { life_paid: life })
    }
}

fn pay_generic(mut pool: [u32; 6], mut n: u32) -> Option<[u32; 6]> {
    // Spend colorless first, then the colors we have the most of.
    let take = pool[5].min(n);
    pool[5] -= take;
    n -= take;
    while n > 0 {
        let (i, &amt) = pool[..5].iter().enumerate().max_by_key(|(_, a)| **a)?;
        if amt == 0 {
            return None;
        }
        pool[i] -= 1;
        n -= 1;
    }
    Some(pool)
}

fn search(
    syms: &[ManaSymbol],
    i: usize,
    pool: [u32; 6],
    generic: u32,
    life: u32,
    life_available: i32,
    best: &mut Option<([u32; 6], u32)>,
) {
    if best.as_ref().is_some_and(|(_, l)| *l == 0) {
        return;
    }
    if i == syms.len() {
        if let Some(rest) = pay_generic(pool, generic) {
            if best.as_ref().map_or(true, |(_, l)| life < *l) {
                *best = Some((rest, life));
            }
        }
        return;
    }
    let try_type = |t: ManaType, best: &mut Option<([u32; 6], u32)>| {
        let mut p = pool;
        if p[idx(t)] > 0 {
            p[idx(t)] -= 1;
            search(syms, i + 1, p, generic, life, life_available, best);
        }
    };
    match syms[i] {
        ManaSymbol::Colored(c) => try_type(ManaType::Color(c), best),
        ManaSymbol::Colorless => try_type(ManaType::Colorless, best),
        ManaSymbol::Hybrid(a, b) => {
            try_type(ManaType::Color(a), best);
            try_type(ManaType::Color(b), best);
        }
        ManaSymbol::TwoHybrid(c) => {
            try_type(ManaType::Color(c), best);
            search(syms, i + 1, pool, generic + 2, life, life_available, best);
        }
        ManaSymbol::Phyrexian(c) => {
            try_type(ManaType::Color(c), best);
            if (life as i32 + 2) <= life_available {
                search(syms, i + 1, pool, generic, life + 2, life_available, best);
            }
        }
        ManaSymbol::PhyrexianHybrid(a, b) => {
            try_type(ManaType::Color(a), best);
            try_type(ManaType::Color(b), best);
            if (life as i32 + 2) <= life_available {
                search(syms, i + 1, pool, generic, life + 2, life_available, best);
            }
        }
        _ => search(syms, i + 1, pool, generic, life, life_available, best),
    }
}

impl ManaType {
    pub fn all() -> [ManaType; 6] {
        ALL_TYPES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_mana_value() {
        let c = ManaCost::parse("{2}{G}{G}").unwrap();
        assert_eq!(c.mana_value(), 4);
        assert_eq!(c.to_string(), "{2}{G}{G}");
        let h = ManaCost::parse("{X}{W/U}{2/B}{G/P}").unwrap();
        assert_eq!(h.mana_value(), 4);
        assert!(h.has_x());
        assert_eq!(h.colors().len(), 4);
    }

    #[test]
    fn hybrid_payment_finds_working_assignment() {
        let mut p = ManaPool::default();
        p.add(ManaType::Color(Color::White), 1);
        p.add(ManaType::Color(Color::Blue), 1);
        // {W/U}{W}: greedy W for the hybrid would fail, search must use U.
        let cost = ManaCost::parse("{W/U}{W}").unwrap();
        assert!(p.try_pay(&cost, 0).is_some());
        assert!(p.is_empty());
    }

    #[test]
    fn phyrexian_prefers_mana_then_life() {
        let mut p = ManaPool::default();
        let cost = ManaCost::parse("{G/P}").unwrap();
        assert_eq!(p.clone().try_pay(&cost, 1), None);
        assert_eq!(p.try_pay(&cost, 20).unwrap().life_paid, 2);
    }
}
