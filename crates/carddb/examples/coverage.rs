//! How much of the card pool the oracle parser fully runs, and which wordings it misses most.
//!
//!     cargo run --release -p carddb --example coverage -- path/to/AtomicCards.json [top]
//!
//! The file is MTGJSON's AtomicCards (decompress the .xz/.gz first). Prints the share of
//! cards that are fully supported (all of them, and the Commander-legal ones), then the most
//! common unsupported lines with numbers and mana symbols folded together so one wording
//! counts once however many cards print it with different numbers.

use carddb::{convert_all, AtomicFile};
use rules_core::Ability;
use std::collections::HashMap;

fn shape(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            for d in chars.by_ref() {
                if d == '}' {
                    break;
                }
            }
            if !out.ends_with("{M}") {
                out.push_str("{M}");
            }
        } else if c.is_ascii_digit() {
            while chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                chars.next();
            }
            out.push('N');
        } else {
            out.push(c);
        }
    }
    out.to_lowercase()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: coverage <AtomicCards.json> [top]");
    let top: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(60);
    let text = std::fs::read_to_string(&path).expect("read AtomicCards");
    let file: AtomicFile = serde_json::from_str(&text).expect("parse AtomicCards");
    let records = convert_all(&file);
    let mut total = 0;
    let mut ok = 0;
    let mut legal = 0;
    let mut legal_ok = 0;
    let mut lines: HashMap<String, (usize, String)> = HashMap::new();
    // Wordings that are the only thing keeping a card from being fully supported.
    let mut only: HashMap<String, usize> = HashMap::new();
    for r in &records {
        // Un-cards, tokens and other oddities that can't be in a deck.
        if matches!(
            r.layout.as_str(),
            "token" | "emblem" | "art_series" | "double_faced_token"
        ) {
            continue;
        }
        let in_commander = r.commander_legality.as_deref() == Some("Legal");
        let unsupported: Vec<&String> = std::iter::once(&r.def)
            .chain(&r.other_faces)
            .flat_map(|d| d.abilities.iter())
            .filter_map(|a| match a {
                Ability::Unsupported(l) => Some(l),
                _ => None,
            })
            .collect();
        total += 1;
        legal += in_commander as usize;
        if unsupported.is_empty() {
            ok += 1;
            legal_ok += in_commander as usize;
        }
        if in_commander {
            if let Ok(path) = std::env::var("COVERAGE_DUMP") {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(path)
                    .unwrap();
                for l in &unsupported {
                    writeln!(f, "{}\t{}", r.def.name, l).unwrap();
                }
            }
        }
        if in_commander && unsupported.len() == 1 {
            *only.entry(shape(unsupported[0])).or_default() += 1;
        }
        if in_commander {
            for l in unsupported {
                let e = lines.entry(shape(l)).or_insert((0, r.def.name.clone()));
                e.0 += 1;
            }
        }
    }
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!(
        "All cards:            {ok} of {total} fully supported ({:.1}%)",
        pct(ok, total)
    );
    println!(
        "Commander-legal:      {legal_ok} of {legal} fully supported ({:.1}%)",
        pct(legal_ok, legal)
    );
    println!("\nMost common unsupported lines (Commander-legal cards):");
    let mut v: Vec<_> = lines.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    for (l, (n, example)) in v.into_iter().take(top) {
        println!("{n:6}  {l}   [{example}]");
    }
    println!("\nWordings that are a card's only unsupported line (fixing one unlocks that many):");
    let mut v: Vec<_> = only.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (l, n) in v.into_iter().take(top) {
        println!("{n:6}  {l}");
    }
}
