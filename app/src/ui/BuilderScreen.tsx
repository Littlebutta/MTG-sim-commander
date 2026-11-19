import { useEffect, useMemo, useRef, useState } from "react";
import {
  builtinDecks, listDecks, lookupCards, newDeckId, recommendCards, saveDeck, searchCards,
  type CardSummary, type Recommendations, type Role, type SavedDeck,
} from "../decks";
import type { CardView } from "../engine/types";
import { CardArt, ManaCost, RulesText } from "./Card";
import { explain } from "./keywords";
import { uiScale } from "../scale";

// Deck builder: search the card pool on the left, the deck in the middle, and on the right what
// the deck is short of with cards that would help.

interface Draft {
  commanders: string[];
  cards: Map<string, number>;
}

function parse(text: string): Draft {
  const d: Draft = { commanders: [], cards: new Map() };
  let cmdr = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("//") || line.startsWith("#")) continue;
    const head = line.toLowerCase().replace(/:$/, "");
    if (head === "commander" || head === "commanders") { cmdr = true; continue; }
    if (["deck", "mainboard", "main", "sideboard", "maybeboard", "companion"].includes(head)) { cmdr = false; continue; }
    const m = line.match(/^(\d+)x?\s+(.*)$/);
    const n = m ? parseInt(m[1], 10) : 1;
    let name = (m ? m[2] : line);
    const isCmdr = name.includes("*CMDR*");
    name = name.replace("*CMDR*", "").replace("*F*", "");
    const paren = name.indexOf(" (");
    if (paren >= 0) name = name.slice(0, paren);
    name = name.trim();
    if (!name) continue;
    if (cmdr || isCmdr) d.commanders.push(name);
    else d.cards.set(name, (d.cards.get(name) ?? 0) + n);
  }
  return d;
}

function serialize(d: Draft): string {
  const lines: string[] = [];
  if (d.commanders.length) lines.push("Commander", ...d.commanders.map((c) => `1 ${c}`), "");
  lines.push("Deck", ...[...d.cards.entries()].map(([n, c]) => `${c} ${n}`));
  return lines.join("\n");
}

const isBasic = (c?: CardSummary) => !!c && c.type_line.startsWith("Basic");

const TYPE_FILTERS = ["Creature", "Instant", "Sorcery", "Artifact", "Enchantment", "Planeswalker", "Land"];
const GROUPS: [string, (c: CardSummary) => boolean][] = [
  ["Creatures", (c) => c.type_line.includes("Creature")],
  ["Planeswalkers", (c) => c.type_line.includes("Planeswalker")],
  ["Instants", (c) => c.type_line.includes("Instant")],
  ["Sorceries", (c) => c.type_line.includes("Sorcery")],
  ["Artifacts", (c) => c.type_line.includes("Artifact")],
  ["Enchantments", (c) => c.type_line.includes("Enchantment")],
  ["Lands", (c) => c.type_line.includes("Land")],
];
const ROLE_LABEL: Record<Role, string> = { Land: "Lands", Ramp: "Ramp", Draw: "Card draw", Removal: "Removal", Wipe: "Board wipes", Creature: "Creatures" };
const ROLE_SHORT: Record<Role, string> = { Land: "land", Ramp: "ramp", Draw: "draw", Removal: "removal", Wipe: "wipe", Creature: "creature" };

function Pips({ colors }: { colors: string }) {
  if (!colors) return <span className="pip pip-C" title="Colorless" />;
  return <span className="pips">{colors.split("").map((c) => <span key={c} className={`pip pip-${c}`} />)}</span>;
}

/** Where the mouse is, in the interface's own (zoomed) pixels. */
const mouse = { x: 0, y: 0 };
if (typeof window !== "undefined") {
  window.addEventListener("mousemove", (e) => {
    mouse.x = e.clientX;
    mouse.y = e.clientY;
  }, { passive: true });
}

/** A card's details beside the mouse, on whichever side keeps the pointer (and the buttons
 * under it) uncovered. */
function CardTip({ card }: { card: CardSummary }) {
  // Follow the mouse, so moving onto a row's buttons never leaves the tip on top of them.
  const [, redraw] = useState(0);
  useEffect(() => {
    let raf = 0;
    const move = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => redraw((n) => n + 1));
    };
    window.addEventListener("mousemove", move, { passive: true });
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("mousemove", move);
    };
  }, []);
  const z = uiScale() || 1;
  const vw = window.innerWidth / z;
  const vh = window.innerHeight / z;
  const x = mouse.x / z;
  const y = mouse.y / z;
  const W = 300;
  const left = x > vw / 2 ? Math.max(8, x - W - 36) : Math.min(vw - W - 8, x + 36);
  const top = Math.max(8, Math.min(vh - 280, y - 60));
  return (
    <div className="b-tip" style={{ left, top, right: "auto", bottom: "auto" }}>
      <div className="b-tip-head"><strong>{card.name}</strong><ManaCost cost={card.mana_cost} /></div>
      <div className="b-tip-type">{card.type_line}</div>
      <div className="b-tip-text">{card.oracle_text || <span className="muted">No rules text.</span>}</div>
      {card.power !== null && <div className="b-tip-pt">{card.power}/{card.toughness}</div>}
      {!card.supported && <div className="b-tip-warn">Some of this card's text isn't supported in games yet.</div>}
    </div>
  );
}

/** Just enough of a card view to paint art for a card that isn't in a game. */
function asView(c: CardSummary): CardView {
  const [left, right = ""] = c.type_line.split(" — ");
  const words = left.split(" ");
  const colors = (c.mana_cost.match(/\{([WUBRG])\}/g) ?? []).map((m) => m[1]).filter((v, i, a) => a.indexOf(v) === i);
  return {
    id: 0, name: c.name, mana_cost: c.mana_cost, mana_value: c.mana_value, colors, type_line: c.type_line, text: c.oracle_text,
    types: words.filter((w) => /^[A-Z]/.test(w) && !["Legendary", "Basic", "Snow"].includes(w)),
    supertypes: words.filter((w) => ["Legendary", "Basic", "Snow"].includes(w)), subtypes: right.split(" ").filter(Boolean),
    power: c.power, toughness: c.toughness, keywords: [], abilities: [],
  } as unknown as CardView;
}

/** Right-click a card: read it in a window. */
function CardWindow({ card, onClose, inDeck, onAdd, onRemove }: {
  card: CardSummary; onClose: () => void; inDeck: boolean; onAdd: () => void; onRemove: () => void;
}) {
  const kws = card.oracle_text.split("\n").flatMap((l) => l.split(/,\s*/)).map((w) => w.trim()).filter((w) => /^[A-Z][a-z]+( [a-z]+)?$/.test(w));
  const terms = explain(kws, card.oracle_text);
  return (
    <div className="scrim b-window-scrim" onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }}>
      <div className="b-window" onClick={(e) => e.stopPropagation()}>
        <div className="b-window-art"><CardArt card={asView(card)} glyph={72} /></div>
        <div className="b-window-body">
          <div className="preview-head"><span className="preview-name">{card.name}</span><ManaCost cost={card.mana_cost} size="md" /></div>
          <div className="preview-type">{card.type_line}</div>
          <div className="preview-text">{card.oracle_text ? <RulesText text={card.oracle_text} /> : <p className="muted">No rules text.</p>}</div>
          {card.power !== null && <div className="b-window-pt">{card.power}/{card.toughness}</div>}
          {card.roles.length > 0 && <div className="b-window-roles">{card.roles.map((r) => <span key={r} className={`b-role role-${r}`}>{ROLE_SHORT[r]}</span>)}</div>}
          {terms.length > 0 && (
            <dl className="glossary">{terms.map(([k, v]) => <div key={k}><dt>{k}</dt><dd><RulesText text={v} /></dd></div>)}</dl>
          )}
          {!card.supported && <p className="warn small">Some of this card's text isn't supported in games yet; those parts do nothing.</p>}
          <div className="modal-actions">
            {inDeck ? <button className="btn" onClick={onRemove}>Remove from deck</button> : <button className="btn" onClick={onAdd}>Add to deck</button>}
            <button className="btn btn-primary" onClick={onClose}>Close</button>
          </div>
        </div>
      </div>
    </div>
  );
}

/** Rough share of a Commander deck's nonland cards at each mana value (0–1, 2, 3, 4, 5, 6, 7+). */
const CURVE_TARGET = [0.08, 0.22, 0.22, 0.18, 0.13, 0.09, 0.08];

/** What the mana curve and colors say about a deck, in plain words. */
function curveAdvice(curve: number[], avg: number, pips: Record<string, number>, sources: Record<string, number>, small = false): string[] {
  const out: string[] = [];
  const buckets = [curve[0] + curve[1], curve[2], curve[3], curve[4], curve[5], curve[6], curve[7]];
  const total = buckets.reduce((a, b) => a + b, 0);
  if (total < (small ? 6 : 10)) return ["Add more cards to see advice about your mana curve."];
  const names = ["0–1", "2", "3", "4", "5", "6", "7+"];
  const diffs = buckets.map((n, i) => n - CURVE_TARGET[i] * total);
  // Too few cheap-to-mid spells, or too many expensive ones, are what hurt; the reverse is fine.
  const step = small ? 2 : 3;
  const low = diffs.map((d, i) => [d, i] as const).filter(([d, i]) => i >= 1 && i <= 3 && d <= -step).sort((a, b) => a[0] - b[0]);
  const high = diffs.map((d, i) => [d, i] as const).filter(([d, i]) => i >= 4 && d >= step).sort((a, b) => b[0] - a[0]);
  if (avg >= 4) out.push(`Average mana value is ${avg.toFixed(2)}: a slow deck. Cheaper spells and more ramp will help you keep up.`);
  else if (avg > 0 && avg <= 2.4) out.push(`Average mana value is ${avg.toFixed(2)}: a fast, low curve. You can run 1–2 fewer lands.`);
  else out.push(`Average mana value is ${avg.toFixed(2)}, a comfortable middle${small ? "" : " for Commander"}.`);
  const top = buckets[5] + buckets[6];
  if (top > total * 0.25) out.push(`${top} cards cost 6 or more. That's top-heavy; trim a few or add ramp.`);
  if (low.length) out.push(`Light at ${low.map(([d, i]) => `${names[i]} mana (about ${Math.round(-d)} short)`).join(", ")}.`);
  if (high.length) out.push(`Heavy at ${high.map(([d, i]) => `${names[i]} mana (about ${Math.round(d)} extra)`).join(", ")}.`);
  if (buckets[1] < (small ? 5 : 8)) out.push(`Two-mana plays are what let you do something on turn 2; most decks want ${small ? "6" : "10"} or more.`);
  const pipTotal = Object.values(pips).reduce((a, b) => a + b, 0);
  const srcTotal = Object.values(sources).reduce((a, b) => a + b, 0);
  if (pipTotal > 0 && srcTotal > 0) {
    for (const [c, n] of Object.entries(pips)) {
      const share = n / pipTotal;
      const have = (sources[c] ?? 0) / srcTotal;
      if (share - have > 0.12) out.push(`${Math.round(share * 100)}% of your colored mana symbols are ${COLOR_NAME[c]}, but only ${Math.round(have * 100)}% of your lands make it.`);
    }
  }
  return out;
}
const COLOR_NAME: Record<string, string> = { W: "white", U: "blue", B: "black", R: "red", G: "green" };

function ExportWindow({ name, text, onClose }: { name: string; text: string; onClose: () => void }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  const download = () => {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
    a.download = `${name.replace(/[^\w\- ]+/g, "").trim() || "deck"}.txt`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  };
  return (
    <div className="scrim b-window-scrim" onClick={onClose}>
      <div className="modal wide b-export" onClick={(e) => e.stopPropagation()}>
        <h2>Export decklist</h2>
        <p className="muted">Plain text that Moxfield, Archidekt and other deck sites can import.</p>
        <textarea className="field b-export-text" readOnly value={text} onFocus={(e) => e.currentTarget.select()} />
        <div className="modal-actions">
          <button className="btn" onClick={download}>Download .txt</button>
          <button className="btn" onClick={copy}>{copied ? "Copied" : "Copy"}</button>
          <button className="btn btn-primary" onClick={onClose}>Done</button>
        </div>
      </div>
    </div>
  );
}

/** Building from a collection instead of the whole card pool (adventure mode): no commander, a
 * copy limit, and only cards you own (basic lands are free). Changes are handed back as they
 * happen; there is no separate save. */
export interface CollectionBuild {
  title: string;
  name: string;
  cards: Record<string, number>;
  owned: Record<string, number>;
  basics: string[];
  maxCopies: number;
  minCards: number;
  backLabel: string;
  onChange: (name: string, cards: Record<string, number>) => void;
  /** A line under the deck: is it ready to play? */
  status?: React.ReactNode;
}

export function BuilderScreen({ onBack, collection }: { onBack: () => void; collection?: CollectionBuild }) {
  const adv = collection;
  const [decks, setDecks] = useState<SavedDeck[]>([]);
  const [deck, setDeck] = useState<SavedDeck>(() => ({ id: newDeckId(), name: adv?.name ?? "New deck", text: "", updated: 0 }));
  const [draft, setDraft] = useState<Draft>(() => ({ commanders: [], cards: new Map(adv ? Object.entries(adv.cards).filter(([, n]) => n > 0) : []) }));
  const [pool, setPool] = useState<CardSummary[]>([]);
  const [saved, setSaved] = useState(true);
  const [info, setInfo] = useState<Map<string, CardSummary>>(new Map());
  const [query, setQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [fitColors, setFitColors] = useState(!adv);
  const [results, setResults] = useState<CardSummary[]>([]);
  const [rec, setRec] = useState<Recommendations | null>(null);
  const [tip, setTip] = useState<CardSummary | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [opened, setOpened] = useState<CardSummary | null>(null);
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    if (adv) return;
    Promise.all([listDecks(), builtinDecks()]).then(([m, b]) => setDecks([...m, ...b]));
  }, [adv]);

  // Collection building: the cards you own (and the basics) are the whole search pool.
  const ownedKey = adv ? Object.keys(adv.owned).filter((n) => adv.owned[n] > 0).sort().join("|") : "";
  useEffect(() => {
    if (!adv) return;
    const names = [...ownedKey.split("|").filter(Boolean), ...adv.basics];
    lookupCards(names).then((cards) => {
      setPool(cards);
      setInfo((old) => {
        const m = new Map(old);
        for (const c of cards) m.set(c.name, c);
        return m;
      });
    }).catch((e) => setMessage(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ownedKey]);

  // Hand every change back.
  const advRef = useRef(adv);
  advRef.current = adv;
  const firstRef = useRef(true);
  useEffect(() => {
    if (firstRef.current) {
      firstRef.current = false;
      return;
    }
    advRef.current?.onChange(deck.name, Object.fromEntries(draft.cards));
  }, [draft, deck.name]);

  const text = useMemo(() => serialize(draft), [draft]);
  const needCommander = !adv && draft.commanders.length === 0;
  // Without a commander, a deck's colors are the colors of what's in it.
  const deckIdentity = useMemo(() => {
    const set = new Set<string>();
    for (const n of draft.cards.keys()) for (const c of info.get(n)?.color_identity ?? "") set.add(c);
    return "WUBRG".split("").filter((c) => set.has(c)).join("");
  }, [draft, info]);
  const identity = adv ? deckIdentity : rec?.identity ?? "";
  /** Copies of a card the deck may hold. */
  const allowed = (c: CardSummary) => (isBasic(c) ? Infinity : adv ? Math.min(adv.maxCopies, adv.owned[c.name] ?? 0) : 1);

  // Card details for everything in the deck.
  useEffect(() => {
    const names = [...draft.commanders, ...draft.cards.keys()].filter((n) => !info.has(n));
    if (!names.length) return;
    lookupCards(names).then((found) => {
      setInfo((old) => {
        const m = new Map(old);
        for (const c of found) m.set(c.name, c);
        return m;
      });
    });
  }, [draft, info]);

  // Stats and suggestions, a moment after each change.
  useEffect(() => {
    const t = setTimeout(() => recommendCards(text).then(setRec).catch((e) => setMessage(String(e))), 200);
    return () => clearTimeout(t);
  }, [text]);

  // Search as you type.
  useEffect(() => {
    if (adv) {
      const q = query.trim().toLowerCase();
      const words = q.split(/\s+/).filter(Boolean);
      const idSet = new Set(identity.split(""));
      const hits = pool
        .filter((c) => !typeFilter || c.type_line.includes(typeFilter))
        .filter((c) => !fitColors || !identity || c.color_identity.split("").every((x) => idSet.has(x)))
        .filter((c) => {
          if (!q) return true;
          const name = c.name.toLowerCase();
          if (name.includes(q)) return true;
          const all = `${name} ${c.type_line} ${c.oracle_text}`.toLowerCase();
          return words.every((w) => all.includes(w));
        })
        .sort((a, b) => Number(isBasic(a)) - Number(isBasic(b)) || a.mana_value - b.mana_value || a.name.localeCompare(b.name));
      setResults(hits);
      return;
    }
    const t = setTimeout(() => {
      searchCards({
        text: query,
        commanders_only: needCommander && !query && !typeFilter,
        identity: !needCommander && fitColors ? identity : null,
        types: typeFilter ? [typeFilter] : [],
        limit: 80,
      }).then(setResults).catch((e) => setMessage(String(e)));
    }, 150);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, typeFilter, fitColors, identity, needCommander, pool]);

  useEffect(() => {
    if (!message) return;
    const t = setTimeout(() => setMessage(null), 2400);
    return () => clearTimeout(t);
  }, [message]);

  const change = (f: (d: Draft) => void) => {
    const d: Draft = { commanders: [...draft.commanders], cards: new Map(draft.cards) };
    f(d);
    setDraft(d);
    setSaved(false);
  };
  const remember = (c: CardSummary) => setInfo((m) => (m.has(c.name) ? m : new Map(m).set(c.name, c)));
  const add = (c: CardSummary) => {
    remember(c);
    if (needCommander && c.can_be_commander) return makeCommander(c);
    change((d) => {
      const n = d.cards.get(c.name) ?? 0;
      if (n >= allowed(c)) return;
      d.cards.set(c.name, n + 1);
    });
  };
  const makeCommander = (c: CardSummary) => {
    remember(c);
    change((d) => {
      d.cards.delete(c.name);
      if (d.commanders.length >= 2 || (d.commanders.length === 1 && !/partner|background|friends forever/i.test(c.oracle_text + info.get(d.commanders[0])?.oracle_text))) d.commanders = [];
      if (!d.commanders.includes(c.name)) d.commanders.push(c.name);
    });
  };
  const remove = (name: string) => change((d) => {
    const n = d.cards.get(name) ?? 0;
    if (n <= 1) d.cards.delete(name);
    else d.cards.set(name, n - 1);
  });
  const removeCommander = (name: string) => change((d) => { d.commanders = d.commanders.filter((c) => c !== name); });

  const open = (id: string) => {
    if (id === "new") {
      setDeck({ id: newDeckId(), name: "New deck", text: "", updated: 0 });
      setDraft({ commanders: [], cards: new Map() });
      setSaved(true);
      return;
    }
    const d = decks.find((x) => x.id === id);
    if (!d) return;
    // Built-in decks open as a copy.
    setDeck(d.builtin ? { id: newDeckId(), name: `${d.name} (copy)`, text: d.text, updated: 0 } : d);
    setDraft(parse(d.text));
    setSaved(!d.builtin);
  };
  const save = async () => {
    const d = { ...deck, name: deck.name.trim() || "Untitled deck", text };
    await saveDeck(d);
    setDeck(d);
    setSaved(true);
    setDecks([...(await listDecks()), ...(await builtinDecks())]);
    setMessage("Saved to your decks");
  };

  const count = draft.commanders.length + [...draft.cards.values()].reduce((a, b) => a + b, 0);
  const grouped = useMemo(() => {
    const out: [string, [string, number][]][] = [];
    const left = new Map(draft.cards);
    for (const [label, test] of GROUPS) {
      const rows = [...left.entries()].filter(([n]) => { const c = info.get(n); return c && test(c); });
      rows.forEach(([n]) => left.delete(n));
      if (rows.length) out.push([label, rows.sort((a, b) => (info.get(a[0])!.mana_value - info.get(b[0])!.mana_value) || a[0].localeCompare(b[0]))]);
    }
    if (left.size) out.push(["Other", [...left.entries()]]);
    return out;
  }, [draft, info]);

  const stats = rec?.stats;
  const curveMax = Math.max(1, ...(stats?.curve ?? [1]));
  const size = Math.max(adv?.minCards ?? 40, count) / 40;
  const meters: [Role, number, number][] = stats && adv
    ? [["Land", stats.lands, Math.round(17 * size)], ["Ramp", stats.ramp, Math.round(2 * size)], ["Draw", stats.draw, Math.round(3 * size)], ["Removal", stats.removal, Math.round(4 * size)], ["Creature", stats.creatures, Math.round(15 * size)]]
    : stats
    ? [["Land", stats.lands, stats.avg_mv > 0 && stats.avg_mv < 2.6 ? 35 : 37], ["Ramp", stats.ramp, 10], ["Draw", stats.draw, 10], ["Removal", stats.removal, 8], ["Wipe", stats.wipes, 2], ["Creature", stats.creatures, 20]]
    : [];
  const inDeck = (n: string) => draft.commanders.includes(n) || draft.cards.has(n);
  // Colored mana symbols in the deck's spells, and lands that can make each color.
  const { pips, sources } = useMemo(() => {
    const pips: Record<string, number> = {};
    const sources: Record<string, number> = {};
    for (const [n, q] of [...draft.cards.entries(), ...draft.commanders.map((c) => [c, 1] as [string, number])]) {
      const c = info.get(n);
      if (!c) continue;
      if (c.type_line.includes("Land")) {
        for (const col of c.color_identity.split("")) if (col) sources[col] = (sources[col] ?? 0) + q;
      } else {
        for (const m of c.mana_cost.match(/\{([WUBRG])\}/g) ?? []) pips[m[1]] = (pips[m[1]] ?? 0) + q;
      }
    }
    return { pips, sources };
  }, [draft, info]);
  const curveTips = stats ? curveAdvice(stats.curve, stats.avg_mv, pips, sources, !!adv) : [];
  const countOf = (n: string) => draft.cards.get(n) ?? 0;
  const maxed = (c: CardSummary) => countOf(c.name) >= allowed(c);
  // Suggestions from your own cards: ones that fill a role the deck is short of.
  const short = new Set(meters.filter(([, have, want]) => have < want - 1).map(([r]) => r));
  const ownedPicks = adv
    ? pool
        .filter((c) => !isBasic(c) && !maxed(c) && c.roles.some((r) => short.has(r)))
        .filter((c) => !identity || c.color_identity.split("").every((x) => identity.includes(x)))
        .slice(0, 30)
        .map((c) => ({ card: c, role: c.roles.find((r) => short.has(r)) ?? null, reason: countOf(c.name) ? `${countOf(c.name)} in your deck` : "In your collection" }))
    : [];
  const nonland = stats ? stats.curve.reduce((a, b) => a + b, 0) : 0;
  const targetAt = (i: number) => (i <= 1 ? CURVE_TARGET[0] / 2 : CURVE_TARGET[i - 1]) * nonland;
  const curveTop = Math.max(curveMax, ...Array.from({ length: 8 }, (_, i) => targetAt(i)));

  const resultRow = (c: CardSummary, extra?: React.ReactNode) => (
    <div key={c.name} className={`b-row ${maxed(c) ? "in" : ""}`} onMouseEnter={() => setTip(c)} onMouseLeave={() => setTip(null)}
      onContextMenu={(e) => { e.preventDefault(); setTip(null); setOpened(c); }}>
      <span className="b-mv"><ManaCost cost={c.mana_cost} /></span>
      <span className="b-name">
        <span>{c.name}</span>
        <span className="b-type">{adv && !isBasic(c) && <span className="b-owned">{countOf(c.name)}/{Math.min(adv.maxCopies, adv.owned[c.name] ?? 0)} · </span>}{c.type_line}{!c.supported && <span className="b-partial" title="Some of this card's text isn't supported in games yet"> · partly supported</span>}</span>
        {extra}
      </span>
      {needCommander && c.can_be_commander ? (
        <button className="btn b-add" onClick={() => makeCommander(c)}>Commander</button>
      ) : maxed(c) ? (
        <span className="b-check" title="In your deck">✓</span>
      ) : (
        <button className="btn b-add" onClick={() => add(c)} title="Add to deck">+</button>
      )}
    </div>
  );

  return (
    <div className="screen builder-screen">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← {adv?.backLabel ?? "Back"}</button>
        <h2>{adv?.title ?? "Deck builder"}</h2>
        <span />
      </header>
      <div className="builder-layout">
        {/* Search */}
        <section className="b-col b-search">
          <input className="field" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={needCommander ? "Search for a commander…" : adv ? "Search your cards by name, type or text" : "Search cards by name, type or text"} autoFocus={!adv} />
          <div className="b-filters">
            {TYPE_FILTERS.map((t) => (
              <button key={t} className={`chip-btn ${typeFilter === t ? "on" : ""}`} onClick={() => setTypeFilter(typeFilter === t ? null : t)}>{t}</button>
            ))}
          </div>
          {!needCommander && (
            <label className="toggle"><input type="checkbox" checked={fitColors} onChange={(e) => setFitColors(e.target.checked)} /> Only cards in my colors <Pips colors={identity} /></label>
          )}
          <div className="b-list">
            {needCommander && !query && !typeFilter && <div className="menu-label">Commanders</div>}
            {results.map((c) => resultRow(c))}
            {results.length === 0 && <p className="muted small">{adv && pool.length <= adv.basics.length ? "No cards yet. Explore, duel, and look in the tall grass." : "No cards match."}</p>}
          </div>
        </section>

        {/* Deck */}
        <section className="b-col b-deck">
          <div className="b-deck-head">
            {!adv && <select className="field" value={decks.some((d) => d.id === deck.id) ? deck.id : ""} onChange={(e) => open(e.target.value)} aria-label="Open a deck">
              <option value="" disabled>{saved ? "Open a deck…" : "Unsaved deck"}</option>
              <option value="new">New deck</option>
              <optgroup label="Your decks">{decks.filter((d) => !d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</optgroup>
              <optgroup label="Copy a built-in deck">{decks.filter((d) => d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</optgroup>
            </select>}
            <input className="field b-deck-name" value={deck.name} onChange={(e) => { setDeck({ ...deck, name: e.target.value }); setSaved(false); }} aria-label="Deck name" />
            {adv ? (
              <span className={`check-count ${count >= adv.minCards ? "ok" : ""}`} title={`At least ${adv.minCards} cards`}>{count} cards</span>
            ) : (
              <span className={`check-count ${count === 100 ? "ok" : ""}`}>{count}/100</span>
            )}
            <button className="btn" onClick={() => setExporting(true)} disabled={count === 0} title="Copy or download the decklist as text">Export</button>
            {!adv && <button className="btn btn-primary" disabled={saved} onClick={save}>{saved ? "Saved" : "Save"}</button>}
          </div>
          {adv?.status}
          <div className="b-list b-decklist">
            {!adv && <div className="b-group">
              <div className="menu-label">Commander</div>
              {draft.commanders.length === 0 && <p className="muted small">Search on the left and pick a commander to start.</p>}
              {draft.commanders.map((n) => (
                <div key={n} className="b-row cmdr" onMouseEnter={() => info.get(n) && setTip(info.get(n)!)} onMouseLeave={() => setTip(null)}
                  onContextMenu={(e) => { e.preventDefault(); const c = info.get(n); if (c) { setTip(null); setOpened(c); } }}>
                  <span className="b-mv">{info.get(n) && <ManaCost cost={info.get(n)!.mana_cost} />}</span>
                  <span className="b-name"><span>{n}</span><span className="b-type">{info.get(n)?.type_line}</span></span>
                  <button className="ghost b-qty" onClick={() => removeCommander(n)} title="Remove">✕</button>
                </div>
              ))}
            </div>}
            {grouped.map(([label, rows]) => (
              <div key={label} className="b-group">
                <div className="menu-label">{label} <span className="muted">{rows.reduce((a, r) => a + r[1], 0)}</span></div>
                {rows.map(([n, q]) => {
                  const c = info.get(n);
                  return (
                    <div key={n} className="b-row" onMouseEnter={() => c && setTip(c)} onMouseLeave={() => setTip(null)}
                      onContextMenu={(e) => { e.preventDefault(); if (c) { setTip(null); setOpened(c); } }}>
                      <span className="b-mv">{c && <ManaCost cost={c.mana_cost} />}</span>
                      <span className="b-name"><span>{q > 1 ? `${q}× ` : ""}{n}</span></span>
                      <span className="b-qtys">
                        <button className="ghost b-qty" onClick={() => remove(n)} title="Remove one">−</button>
                        {c && !maxed(c) && (adv || isBasic(c)) && <button className="ghost b-qty" onClick={() => add(c)} title="Add one">+</button>}
                      </span>
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </section>

        {/* Stats and suggestions */}
        <section className="b-col b-advice">
          {stats && (
            <div className="b-stats">
              <div className="menu-label b-curve-label">Mana curve <span className="muted">avg {stats.avg_mv.toFixed(2)} · {nonland} spells</span></div>
              <div className="b-curve" aria-label="Mana curve">
                {stats.curve.map((n, i) => (
                  <div key={i} className="b-bar" title={`${n} card${n === 1 ? "" : "s"} at ${i === 7 ? "7+" : i} mana (typical: about ${Math.round(targetAt(i))})`}>
                    <span className="b-bar-n">{n || ""}</span>
                    <span className="b-bar-fill" style={{ height: `${(n / curveTop) * 100}%` }} />
                    {nonland >= 10 && <span className="b-bar-target" style={{ bottom: `${16 + (targetAt(i) / curveTop) * 78}px` }} />}
                    <span className="b-bar-x">{i === 7 ? "7+" : i}</span>
                  </div>
                ))}
              </div>
              {Object.keys(pips).length > 0 && (
                <div className="b-colors">
                  {Object.entries(pips).map(([c, n]) => (
                    <span key={c} className="b-color" title={`${n} ${COLOR_NAME[c]} symbols in spells, ${sources[c] ?? 0} lands make ${COLOR_NAME[c]}`}>
                      <span className={`pip pip-${c}`} /> {n} <span className="muted">symbols · {sources[c] ?? 0} sources</span>
                    </span>
                  ))}
                </div>
              )}
              {curveTips.length > 0 && <ul className="b-tips b-curve-tips">{curveTips.map((t) => <li key={t}>{t}</li>)}</ul>}
              <div className="b-meters">
                {meters.map(([r, have, want]) => (
                  <div key={r} className={`b-meter ${have >= want - 1 ? "ok" : ""}`}>
                    <span>{ROLE_LABEL[r]}</span>
                    <span className="b-meter-track"><span style={{ width: `${Math.min(100, (have / want) * 100)}%` }} /></span>
                    <span className="b-meter-n">{have}<span className="muted">/{want}</span></span>
                  </div>
                ))}
              </div>
            </div>
          )}
          {!adv && rec && rec.themes.length > 0 && (
            <div className="b-themes"><span className="muted small">Your commander likes</span> {rec.themes.map((t) => <span key={t} className="chip">{t}</span>)}</div>
          )}
          {!adv && rec && rec.advice.length > 0 && (
            <ul className="b-tips">{rec.advice.map((a) => <li key={a.text}>{a.text}</li>)}</ul>
          )}
          {adv && ownedPicks.length > 0 && (
            <>
              <div className="menu-label">From your collection</div>
              <div className="b-list">
                {ownedPicks.map((p) => resultRow(p.card, (
                  <span className="b-reason">{p.role && <span className={`b-role role-${p.role}`}>{ROLE_SHORT[p.role]}</span>}{p.reason}</span>
                )))}
              </div>
            </>
          )}
          {!adv && rec && rec.picks.length > 0 && (
            <>
              <div className="menu-label">Suggestions</div>
              <div className="b-list">
                {rec.picks.filter((p) => !inDeck(p.card.name)).map((p) => resultRow(p.card, (
                  <span className="b-reason">{p.role && <span className={`b-role role-${p.role}`}>{ROLE_SHORT[p.role]}</span>}{p.reason}</span>
                )))}
              </div>
            </>
          )}
        </section>
      </div>
      {tip && !opened && <CardTip card={tip} />}
      {opened && (
        <CardWindow card={opened} onClose={() => setOpened(null)} inDeck={adv ? maxed(opened) && countOf(opened.name) > 0 : inDeck(opened.name)}
          onAdd={() => { add(opened); setOpened(null); }}
          onRemove={() => { if (draft.commanders.includes(opened.name)) removeCommander(opened.name); else remove(opened.name); setOpened(null); }} />
      )}
      {exporting && <ExportWindow name={deck.name} text={text} onClose={() => setExporting(false)} />}
      {message && <div className="toast">{message}</div>}
    </div>
  );
}

