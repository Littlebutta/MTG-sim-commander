import { useEffect, useMemo, useState } from "react";
import {
  builtinDecks, listDecks, lookupCards, newDeckId, recommendCards, saveDeck, searchCards,
  type CardSummary, type Recommendations, type Role, type SavedDeck,
} from "../decks";
import { ManaCost } from "./Card";

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

function CardTip({ card }: { card: CardSummary }) {
  return (
    <div className="b-tip">
      <div className="b-tip-head"><strong>{card.name}</strong><ManaCost cost={card.mana_cost} /></div>
      <div className="b-tip-type">{card.type_line}</div>
      <div className="b-tip-text">{card.oracle_text || <span className="muted">No rules text.</span>}</div>
      {card.power !== null && <div className="b-tip-pt">{card.power}/{card.toughness}</div>}
      {!card.supported && <div className="b-tip-warn">Some of this card's text isn't supported in games yet.</div>}
    </div>
  );
}

export function BuilderScreen({ onBack }: { onBack: () => void }) {
  const [decks, setDecks] = useState<SavedDeck[]>([]);
  const [deck, setDeck] = useState<SavedDeck>(() => ({ id: newDeckId(), name: "New deck", text: "", updated: 0 }));
  const [draft, setDraft] = useState<Draft>({ commanders: [], cards: new Map() });
  const [saved, setSaved] = useState(true);
  const [info, setInfo] = useState<Map<string, CardSummary>>(new Map());
  const [query, setQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [fitColors, setFitColors] = useState(true);
  const [results, setResults] = useState<CardSummary[]>([]);
  const [rec, setRec] = useState<Recommendations | null>(null);
  const [tip, setTip] = useState<CardSummary | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([listDecks(), builtinDecks()]).then(([m, b]) => setDecks([...m, ...b]));
  }, []);

  const text = useMemo(() => serialize(draft), [draft]);
  const needCommander = draft.commanders.length === 0;
  const identity = rec?.identity ?? "";

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
  }, [query, typeFilter, fitColors, identity, needCommander]);

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
      if (n > 0 && !isBasic(c)) return;
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
  const meters: [Role, number, number][] = stats
    ? [["Land", stats.lands, stats.avg_mv > 0 && stats.avg_mv < 2.6 ? 35 : 37], ["Ramp", stats.ramp, 10], ["Draw", stats.draw, 10], ["Removal", stats.removal, 8], ["Wipe", stats.wipes, 2], ["Creature", stats.creatures, 20]]
    : [];
  const inDeck = (n: string) => draft.commanders.includes(n) || draft.cards.has(n);

  const resultRow = (c: CardSummary, extra?: React.ReactNode) => (
    <div key={c.name} className={`b-row ${inDeck(c.name) && !isBasic(c) ? "in" : ""}`} onMouseEnter={() => setTip(c)} onMouseLeave={() => setTip(null)}>
      <span className="b-mv"><ManaCost cost={c.mana_cost} /></span>
      <span className="b-name">
        <span>{c.name}</span>
        <span className="b-type">{c.type_line}{!c.supported && <span className="b-partial" title="Some of this card's text isn't supported in games yet"> · partly supported</span>}</span>
        {extra}
      </span>
      {needCommander && c.can_be_commander ? (
        <button className="btn b-add" onClick={() => makeCommander(c)}>Commander</button>
      ) : inDeck(c.name) && !isBasic(c) ? (
        <span className="b-check" title="In your deck">✓</span>
      ) : (
        <button className="btn b-add" onClick={() => add(c)} title="Add to deck">+</button>
      )}
    </div>
  );

  return (
    <div className="screen builder-screen">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← Back</button>
        <h2>Deck builder</h2>
        <span />
      </header>
      <div className="builder-layout">
        {/* Search */}
        <section className="b-col b-search">
          <input className="field" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={needCommander ? "Search for a commander…" : "Search cards by name, type or text"} autoFocus />
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
            {results.length === 0 && <p className="muted small">No cards match.</p>}
          </div>
        </section>

        {/* Deck */}
        <section className="b-col b-deck">
          <div className="b-deck-head">
            <select className="field" value={decks.some((d) => d.id === deck.id) ? deck.id : ""} onChange={(e) => open(e.target.value)} aria-label="Open a deck">
              <option value="" disabled>{saved ? "Open a deck…" : "Unsaved deck"}</option>
              <option value="new">New deck</option>
              <optgroup label="Your decks">{decks.filter((d) => !d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</optgroup>
              <optgroup label="Copy a built-in deck">{decks.filter((d) => d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</optgroup>
            </select>
            <input className="field b-deck-name" value={deck.name} onChange={(e) => { setDeck({ ...deck, name: e.target.value }); setSaved(false); }} aria-label="Deck name" />
            <span className={`check-count ${count === 100 ? "ok" : ""}`}>{count}/100</span>
            <button className="btn btn-primary" disabled={saved} onClick={save}>{saved ? "Saved" : "Save"}</button>
          </div>
          <div className="b-list b-decklist">
            <div className="b-group">
              <div className="menu-label">Commander</div>
              {draft.commanders.length === 0 && <p className="muted small">Search on the left and pick a commander to start.</p>}
              {draft.commanders.map((n) => (
                <div key={n} className="b-row cmdr" onMouseEnter={() => info.get(n) && setTip(info.get(n)!)} onMouseLeave={() => setTip(null)}>
                  <span className="b-mv">{info.get(n) && <ManaCost cost={info.get(n)!.mana_cost} />}</span>
                  <span className="b-name"><span>{n}</span><span className="b-type">{info.get(n)?.type_line}</span></span>
                  <button className="ghost b-qty" onClick={() => removeCommander(n)} title="Remove">✕</button>
                </div>
              ))}
            </div>
            {grouped.map(([label, rows]) => (
              <div key={label} className="b-group">
                <div className="menu-label">{label} <span className="muted">{rows.reduce((a, r) => a + r[1], 0)}</span></div>
                {rows.map(([n, q]) => {
                  const c = info.get(n);
                  return (
                    <div key={n} className="b-row" onMouseEnter={() => c && setTip(c)} onMouseLeave={() => setTip(null)}>
                      <span className="b-mv">{c && <ManaCost cost={c.mana_cost} />}</span>
                      <span className="b-name"><span>{q > 1 ? `${q}× ` : ""}{n}</span></span>
                      <span className="b-qtys">
                        <button className="ghost b-qty" onClick={() => remove(n)} title="Remove one">−</button>
                        {isBasic(c) && <button className="ghost b-qty" onClick={() => c && add(c)} title="Add one">+</button>}
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
              <div className="b-curve" aria-label="Mana curve">
                {stats.curve.map((n, i) => (
                  <div key={i} className="b-bar" title={`${n} card${n === 1 ? "" : "s"} at ${i === 7 ? "7+" : i} mana`}>
                    <span className="b-bar-n">{n || ""}</span>
                    <span className="b-bar-fill" style={{ height: `${(n / curveMax) * 100}%` }} />
                    <span className="b-bar-x">{i === 7 ? "7+" : i}</span>
                  </div>
                ))}
              </div>
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
          {rec && rec.themes.length > 0 && (
            <div className="b-themes"><span className="muted small">Your commander likes</span> {rec.themes.map((t) => <span key={t} className="chip">{t}</span>)}</div>
          )}
          {rec && rec.advice.length > 0 && (
            <ul className="b-tips">{rec.advice.map((a) => <li key={a.text}>{a.text}</li>)}</ul>
          )}
          {rec && rec.picks.length > 0 && (
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
      {tip && <CardTip card={tip} />}
      {message && <div className="toast">{message}</div>}
    </div>
  );
}

