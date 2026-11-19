// Adventure mode's windows: the journal (spellbook, satchel, journey), the cauldron, spoils
// after a duel, finding things, and the duel introduction.

import { useEffect, useMemo, useRef, useState } from "react";
import { lookupCards, searchCards, type CardSummary } from "../decks";
import type { CardView } from "../engine/types";
import { Card, CardPreview } from "../ui/Card";
import { checkAdventureDeck, type AdventureDeckCheck } from "./api";
import { personSprite } from "./art";
import { BASICS, BOSS_IDS, DUELIST_IDS, ENDGAME_IDS, PEOPLE, REAGENTS, SIGILS, isBasic, listText, type Person } from "./content";
import { formatTime, type Save } from "./save";
import { WORLD } from "./world";

/** Enough of a card view to draw a card that isn't in a game. */
export function asView(c: CardSummary): CardView {
  const [left, right = ""] = c.type_line.split(" — ");
  const words = left.split(" ");
  const colors = (c.mana_cost.match(/\{([WUBRG])\}/g) ?? []).map((m) => m[1]).filter((v, i, a) => a.indexOf(v) === i);
  const supers = ["Legendary", "Basic", "Snow"];
  return {
    id: 0, name: c.name, mana_cost: c.mana_cost, mana_value: c.mana_value, colors, supertypes: words.filter((w) => supers.includes(w)),
    types: words.filter((w) => /^[A-Z]/.test(w) && !supers.includes(w)), subtypes: right.split(" ").filter(Boolean), type_line: c.type_line,
    text: c.oracle_text, power: c.power, toughness: c.toughness, base_power: c.power, base_toughness: c.toughness, loyalty: null, damage: 0,
    counters: {}, keywords: [], tapped: false, summoning_sick: false, controller: 0, owner: 0, is_commander: false, is_token: false,
    abilities: [], attacking: null, blocking: null, zone: "Hand", unsupported: !c.supported,
  } as unknown as CardView;
}

/** Call `fn` when one of `keys` is pressed, starting `delay` ms after mounting (so the key
 * press that opened a window doesn't also close it). */
export function useKeys(keys: string[], fn: () => void, delay = 0) {
  const ref = useRef(fn);
  ref.current = fn;
  const k = keys.join("|");
  useEffect(() => {
    const list = k.split("|");
    let armed = delay === 0;
    const t = setTimeout(() => (armed = true), delay);
    const key = (e: KeyboardEvent) => {
      if (armed && list.includes(e.key)) {
        e.preventDefault();
        ref.current();
      }
    };
    window.addEventListener("keydown", key);
    return () => {
      clearTimeout(t);
      window.removeEventListener("keydown", key);
    };
  }, [k, delay]);
}

const summaryCache = new Map<string, CardSummary>();
/** Card summaries for names, cached for the session. */
export function useSummaries(names: string[]): Map<string, CardSummary> {
  const key = [...new Set(names)].sort().join("|");
  const [, bump] = useState(0);
  useEffect(() => {
    const missing = key.split("|").filter((n) => n && !summaryCache.has(n));
    if (missing.length === 0) return;
    let live = true;
    lookupCards(missing).then((list) => {
      for (const s of list) summaryCache.set(s.name, s);
      if (live) bump((n) => n + 1);
    }).catch(() => {});
    return () => {
      live = false;
    };
  }, [key]);
  return summaryCache;
}

export function Portrait({ person, size = 96, flip = false }: { person: Person | { look: Person["look"] }; size?: number; flip?: boolean }) {
  const url = useMemo(() => personSprite(person.look, flip ? "left" : "down", 0).toDataURL(), [person, flip]);
  return <img className="adv-portrait" src={url} width={size} height={size} alt="" draggable={false} />;
}

function colorKey(c: CardSummary): string {
  if (c.type_line.includes("Land")) return "L";
  const id = c.color_identity;
  if (!id) return "C";
  return id.length > 1 ? "M" : id;
}

// ---------------------------------------------------------------- spellbook (deck)

const FILTERS: { id: string; label: string }[] = [
  { id: "all", label: "All" }, { id: "W", label: "White" }, { id: "U", label: "Blue" }, { id: "B", label: "Black" },
  { id: "R", label: "Red" }, { id: "G", label: "Green" }, { id: "M", label: "Gold" }, { id: "C", label: "Colorless" },
];

function Spellbook({ save, onChange }: { save: Save; onChange: () => void }) {
  const owned = Object.keys(save.collection).filter((n) => save.collection[n] > 0);
  const names = [...owned, ...Object.keys(save.deck), ...BASICS];
  const sums = useSummaries(names);
  const [filter, setFilter] = useState("all");
  const [text, setText] = useState("");
  const [hover, setHover] = useState<CardSummary | null>(null);
  const [check, setCheck] = useState<AdventureDeckCheck | null>(null);
  const deck = save.deck;
  const deckText = listText(deck);
  useEffect(() => {
    let live = true;
    checkAdventureDeck(deckText).then((c) => live && setCheck(c)).catch(() => {});
    return () => {
      live = false;
    };
  }, [deckText]);

  const count = Object.values(deck).reduce((a, b) => a + b, 0);
  const canAdd = (n: string) => isBasic(n) || (deck[n] ?? 0) < Math.min(4, save.collection[n] ?? 0);
  const add = (n: string) => {
    if (!canAdd(n)) return;
    deck[n] = (deck[n] ?? 0) + 1;
    onChange();
  };
  const remove = (n: string) => {
    if (!deck[n]) return;
    deck[n] -= 1;
    if (deck[n] <= 0) delete deck[n];
    onChange();
  };

  const shown = owned
    .map((n) => sums.get(n))
    .filter((s): s is CardSummary => !!s)
    .filter((s) => filter === "all" || colorKey(s) === filter)
    .filter((s) => !text || s.name.toLowerCase().includes(text.toLowerCase()) || s.type_line.toLowerCase().includes(text.toLowerCase()) || s.oracle_text.toLowerCase().includes(text.toLowerCase()))
    .sort((a, b) => a.mana_value - b.mana_value || a.name.localeCompare(b.name));

  const groups: [string, (s: CardSummary) => boolean][] = [
    ["Creatures", (s) => s.type_line.includes("Creature")],
    ["Spells", (s) => !s.type_line.includes("Creature") && !s.type_line.includes("Land")],
    ["Lands", (s) => s.type_line.includes("Land")],
  ];
  const curve = Array(8).fill(0);
  for (const [n, k] of Object.entries(deck)) {
    const s = sums.get(n);
    if (s && !s.type_line.includes("Land")) curve[Math.min(7, s.mana_value)] += k;
  }
  const peak = Math.max(1, ...curve);

  return (
    <div className="adv-book">
      <div className="adv-book-left">
        <div className="adv-filters">
          {FILTERS.map((f) => (
            <button key={f.id} className={`adv-chip ${filter === f.id ? "on" : ""}`} onClick={() => setFilter(f.id)}>{f.label}</button>
          ))}
          <input className="adv-search" placeholder="Search your cards" value={text} onChange={(e) => setText(e.target.value)} />
        </div>
        <div className="adv-collection">
          {shown.length === 0 && <p className="adv-muted">No cards here yet. Explore, duel, and look in the tall grass.</p>}
          {shown.map((s) => {
            const inDeck = deck[s.name] ?? 0;
            const have = save.collection[s.name] ?? 0;
            return (
              <div key={s.name} className={`adv-coll-card ${canAdd(s.name) ? "" : "maxed"}`} onMouseEnter={() => setHover(s)} onMouseLeave={() => setHover(null)}>
                <Card card={asView(s)} size="hand" onClick={() => add(s.name)} onInspect={() => remove(s.name)} />
                <div className="adv-coll-count">
                  <span title="In your deck / owned">{inDeck}/{have}</span>
                  {inDeck > 0 && <button className="adv-mini" onClick={() => remove(s.name)} title="Take one out">−</button>}
                  {canAdd(s.name) && <button className="adv-mini" onClick={() => add(s.name)} title="Put one in">+</button>}
                </div>
              </div>
            );
          })}
        </div>
        <p className="adv-muted small">Click a card to put a copy in your deck, right-click to take one out. A deck may carry up to four of any card, and as many basic lands as you like.</p>
      </div>
      <div className="adv-book-right">
        <div className="adv-deck-head">
          <input className="adv-deckname" value={save.deckName} onChange={(e) => { save.deckName = e.target.value; onChange(); }} aria-label="Deck name" />
          <span className={`adv-count ${count >= 40 ? "ok" : ""}`}>{count} cards</span>
        </div>
        <div className="adv-curve" title="Mana curve">
          {curve.map((n, i) => (
            <div key={i} className="adv-curve-col">
              <div className="adv-curve-bar" style={{ height: `${(n / peak) * 100}%` }} />
              <span>{i === 7 ? "7+" : i}</span>
            </div>
          ))}
        </div>
        {check && (
          <div className={`adv-status ${check.playable ? "ok" : "bad"}`}>
            {check.playable ? "Ready for the road." : check.problems[0] ?? (check.unknown.length ? `Unknown: ${check.unknown.join(", ")}` : "Not ready yet.")}
          </div>
        )}
        <div className="adv-decklist">
          {groups.map(([label, test]) => {
            const rows = Object.entries(deck).filter(([n]) => {
              const s = sums.get(n);
              return s ? test(s) && !isBasic(n) : label === "Spells" && !isBasic(n);
            });
            if (rows.length === 0) return null;
            return (
              <div key={label}>
                <div className="adv-deck-group">{label} · {rows.reduce((a, [, k]) => a + k, 0)}</div>
                {rows.sort((a, b) => (sums.get(a[0])?.mana_value ?? 0) - (sums.get(b[0])?.mana_value ?? 0)).map(([n, k]) => (
                  <div key={n} className="adv-deck-row" onMouseEnter={() => sums.get(n) && setHover(sums.get(n)!)} onMouseLeave={() => setHover(null)} onClick={() => remove(n)} title="Click to take one out">
                    <span className="adv-deck-n">{k}</span>
                    <span className="adv-deck-name">{n}</span>
                    <span className="adv-deck-cost">{sums.get(n)?.mana_cost.replace(/[{}]/g, "")}</span>
                  </div>
                ))}
              </div>
            );
          })}
          <div className="adv-deck-group">Basic lands · {BASICS.reduce((a, b) => a + (deck[b] ?? 0), 0)}</div>
          <div className="adv-basics">
            {BASICS.map((b) => (
              <div key={b} className="adv-basic">
                <span className={`adv-land l-${b}`} />
                <span className="adv-basic-name">{b}</span>
                <button className="adv-mini" onClick={() => remove(b)}>−</button>
                <span className="adv-deck-n">{deck[b] ?? 0}</span>
                <button className="adv-mini" onClick={() => add(b)}>+</button>
              </div>
            ))}
          </div>
        </div>
      </div>
      {hover && (
        <div className="adv-hover"><CardPreview card={asView(hover)} /></div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- satchel

function Satchel({ save }: { save: Save }) {
  const items = Object.entries(save.reagents).filter(([, n]) => n > 0);
  return (
    <div className="adv-satchel">
      <h3>Sigils</h3>
      <div className="adv-sigils">
        {Object.entries(SIGILS).map(([id, s]) => (
          <div key={id} className={`adv-sigil ${save.sigils.includes(id) ? "have" : ""}`} style={{ "--sig": s.color } as React.CSSProperties}>
            <span className="adv-sigil-gem" />
            <span>{save.sigils.includes(id) ? s.name : "???"}</span>
          </div>
        ))}
      </div>
      <h3>Rare reagents</h3>
      {items.length === 0 && <p className="adv-muted">Nothing yet. Rare things hide in far corners, in dungeon chests, and in the hands of the Vale's old powers.</p>}
      <div className="adv-reagents">
        {items.map(([id, n]) => {
          const r = REAGENTS[id];
          return (
            <div key={id} className="adv-reagent">
              <span className="adv-reagent-gem" style={{ background: r?.color }} />
              <div>
                <strong>{r?.name ?? id}{n > 1 ? ` ×${n}` : ""}</strong>
                <p className="adv-muted">{r?.text}</p>
              </div>
            </div>
          );
        })}
      </div>
      {items.length > 0 && <p className="adv-muted small">Ysolde, who keeps the tower by the crossroads, can brew any spell from one of these.</p>}
    </div>
  );
}

// ---------------------------------------------------------------- journey

const TOTAL_CARDS = Object.values(WORLD.maps).reduce((a, m) => a + m.objects.filter((o) => o.kind === "card").length, 0);

function Journey({ save }: { save: Save }) {
  const beat = (ids: string[]) => ids.filter((id) => save.flags[`beat:${id}`]).length;
  const found = Object.keys(save.flags).filter((k) => k.startsWith("got:") && /:(ow\d+|barrow\d|sanctum\d|cinder\d|grotto\d|hollow\d)$/.test(k)).length;
  const rival = (id: string) => {
    const p = PEOPLE[id];
    const met = save.flags[`met:${id}`] || save.flags[`beat:${id}`];
    return (
      <div key={id} className={`adv-rival ${save.flags[`beat:${id}`] ? "beat" : ""}`}>
        {met ? <Portrait person={p} size={32} /> : <span className="adv-rival-unknown">?</span>}
        <span>{met ? p.name : "???"}</span>
        {save.flags[`beat:${id}`] && <span className="adv-check">✓</span>}
      </div>
    );
  };
  return (
    <div className="adv-journey">
      <div className="adv-stats">
        <div><strong>{formatTime(save.playtime)}</strong><span>on the road</span></div>
        <div><strong>{save.wins}</strong><span>duels won</span></div>
        <div><strong>{save.losses}</strong><span>duels lost</span></div>
        <div><strong>{found}/{TOTAL_CARDS}</strong><span>lost cards found</span></div>
        <div><strong>{save.sigils.length}/5</strong><span>sigils</span></div>
        <div><strong>{save.brews}</strong><span>spells brewed</span></div>
      </div>
      <h3>The old powers · {beat(BOSS_IDS)}/{BOSS_IDS.length}</h3>
      <div className="adv-rivals">{BOSS_IDS.map(rival)}</div>
      <h3>The Spire · {beat(ENDGAME_IDS)}/{ENDGAME_IDS.length}</h3>
      <div className="adv-rivals">{ENDGAME_IDS.map(rival)}</div>
      <h3>Duelists of the Vale · {beat(DUELIST_IDS)}/{DUELIST_IDS.length}</h3>
      <div className="adv-rivals">{DUELIST_IDS.map(rival)}</div>
    </div>
  );
}

// ---------------------------------------------------------------- journal

export function Journal({ save, onChange, onClose, onQuit }: { save: Save; onChange: () => void; onClose: () => void; onQuit: () => void }) {
  const [tab, setTab] = useState<"book" | "satchel" | "journey">("book");
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" || (e.key === "Tab" && !(e.target instanceof HTMLInputElement))) {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);
  return (
    <div className="adv-scrim">
      <div className="adv-journal">
        <div className="adv-journal-tabs">
          <span className="adv-journal-owner">{save.name}'s journal</span>
          <button className={tab === "book" ? "on" : ""} onClick={() => setTab("book")}>Spellbook</button>
          <button className={tab === "satchel" ? "on" : ""} onClick={() => setTab("satchel")}>Satchel</button>
          <button className={tab === "journey" ? "on" : ""} onClick={() => setTab("journey")}>Journey</button>
          <span className="adv-grow" />
          <button className="adv-quiet" onClick={onQuit} title="Your journey is saved">Rest for now</button>
          <button className="adv-close" onClick={onClose} aria-label="Close journal">✕</button>
        </div>
        <div className="adv-journal-body">
          {tab === "book" && <Spellbook save={save} onChange={onChange} />}
          {tab === "satchel" && <Satchel save={save} />}
          {tab === "journey" && <Journey save={save} />}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- cauldron

export function Cauldron({ save, onBrew, onClose }: { save: Save; onBrew: (reagent: string, card: string) => void; onClose: () => void }) {
  const owned = Object.entries(save.reagents).filter(([, n]) => n > 0).map(([id]) => id);
  const [reagent, setReagent] = useState<string | null>(owned[0] ?? null);
  const [text, setText] = useState("");
  const [results, setResults] = useState<CardSummary[]>([]);
  const [pick, setPick] = useState<CardSummary | null>(null);
  useEffect(() => {
    let live = true;
    const t = setTimeout(() => {
      searchCards({ text: text.trim(), limit: 60 })
        .then((r) => live && setResults(r.filter((c) => !c.type_line.includes("Land") && c.supported)))
        .catch(() => live && setResults([]));
    }, 200);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [text]);
  return (
    <div className="adv-scrim">
      <div className="adv-cauldron">
        <div className="adv-cauldron-head">
          <h2>The Cauldron</h2>
          <button className="adv-close" onClick={onClose} aria-label="Step away">✕</button>
        </div>
        <p className="adv-muted">Drop in a rare reagent and speak the name of a spell. The brew will remember it for you.</p>
        <div className="adv-cauldron-reagents">
          {owned.map((id) => (
            <button key={id} className={`adv-reagent-pick ${reagent === id ? "on" : ""}`} onClick={() => setReagent(id)}>
              <span className="adv-reagent-gem" style={{ background: REAGENTS[id]?.color }} />
              {REAGENTS[id]?.name ?? id}{save.reagents[id] > 1 ? ` ×${save.reagents[id]}` : ""}
            </button>
          ))}
        </div>
        <input className="adv-search wide" autoFocus placeholder="Speak the spell's name, or what it does: “flying”, “destroy”, “dragon”…" value={text} onChange={(e) => setText(e.target.value)} />
        <div className="adv-cauldron-body">
          <div className="adv-cauldron-list">
            {results.map((c) => (
              <button key={c.name} className={`adv-cauldron-row ${pick?.name === c.name ? "on" : ""}`} onClick={() => setPick(c)}>
                <span>{c.name}</span>
                <span className="adv-muted">{c.type_line}</span>
              </button>
            ))}
            {results.length === 0 && <p className="adv-muted">The brew stays murky. Try another word.</p>}
          </div>
          <div className="adv-cauldron-preview">
            {pick ? <CardPreview card={asView(pick)} /> : <p className="adv-muted">Choose a spell to see it rise in the steam.</p>}
          </div>
        </div>
        <div className="adv-actions">
          <button className="adv-btn" onClick={onClose}>Not yet</button>
          <button className="adv-btn primary" disabled={!reagent || !pick} onClick={() => reagent && pick && onBrew(reagent, pick.name)}>
            {pick && reagent ? `Brew ${pick.name}` : "Brew"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- spoils and finds

export function Spoils({ title, options, picks, onDone }: { title: string; options: string[]; picks: number; onDone: (chosen: string[]) => void }) {
  const sums = useSummaries(options);
  const [chosen, setChosen] = useState<string[]>([]);
  const left = picks - chosen.length;
  return (
    <div className="adv-scrim">
      <div className="adv-spoils">
        <h2>{title}</h2>
        <p className="adv-muted">{left > 1 ? `Choose ${left} cards to keep.` : left === 1 ? "Choose a card to keep." : "Into your satchel they go."}</p>
        <div className="adv-spoils-row">
          {options.map((n) => {
            const s = sums.get(n);
            const taken = chosen.includes(n);
            return (
              <div key={n} className={`adv-spoil ${taken ? "taken" : ""}`}>
                {s ? <CardPreview card={asView(s)} /> : <div className="adv-spoil-blank">{n}</div>}
                <button className="adv-btn primary" disabled={taken || left <= 0} onClick={() => setChosen([...chosen, n])}>{taken ? "Taken" : "Take this"}</button>
              </div>
            );
          })}
        </div>
        <div className="adv-actions">
          <button className="adv-btn primary" disabled={left > 0} onClick={() => onDone(chosen)}>Onward</button>
        </div>
      </div>
    </div>
  );
}

export function Found({ card, reagent, sigil, onClose }: { card?: string; reagent?: string; sigil?: string; onClose: () => void }) {
  const sums = useSummaries(card ? [card] : []);
  const s = card ? sums.get(card) : undefined;
  useKeys(["e", "E", " ", "Enter", "Escape"], onClose, 250);
  const r = reagent ? REAGENTS[reagent] : undefined;
  const sg = sigil ? SIGILS[sigil] : undefined;
  return (
    <div className="adv-scrim light" onClick={onClose}>
      <div className="adv-found">
        <div className="adv-found-rays" />
        {card && (
          <>
            <div className="adv-found-label">You found a card!</div>
            {s ? <div className="adv-found-card"><Card card={asView(s)} size="hand" /></div> : null}
            <div className="adv-found-name">{card}</div>
          </>
        )}
        {r && (
          <>
            <div className="adv-found-label">A rare reagent!</div>
            <div className="adv-found-gem" style={{ background: r.color }} />
            <div className="adv-found-name">{r.name}</div>
            <p className="adv-muted">{r.text}</p>
          </>
        )}
        {sg && (
          <>
            <div className="adv-found-label">You received a sigil!</div>
            <div className="adv-found-gem sigil" style={{ background: sg.color }} />
            <div className="adv-found-name">{sg.name}</div>
          </>
        )}
        <div className="adv-hint">Press E to continue</div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- duel intro

export function DuelIntro({ person, you, onBegin, onFlee }: { person: Person; you: Person["look"]; onBegin: () => void; onFlee?: () => void }) {
  const d = person.duel!;
  useKeys(["e", "E", " ", "Enter"], onBegin, 600);
  return (
    <div className={`adv-duel-intro rank-${d.rank}`}>
      <div className="adv-duel-band top" />
      <div className="adv-duel-band bottom" />
      <div className="adv-duel-side left"><Portrait person={{ look: you }} size={192} flip={false} /></div>
      <div className="adv-duel-vs">VS</div>
      <div className="adv-duel-side right"><Portrait person={person} size={192} /></div>
      <div className="adv-duel-info">
        <div className="adv-duel-rank">{d.rank === "boss" ? "One of the old powers" : d.rank === "endgame" ? "The Rift Spire" : "A duelist challenges you"}</div>
        <div className="adv-duel-name">{person.name}{person.title ? <span>, {person.title}</span> : null}</div>
        <div className="adv-duel-theme">{d.theme}</div>
        {d.life && <div className="adv-duel-life">Starts at {d.life} life</div>}
        <div className="adv-actions center">
          {onFlee && <button className="adv-btn" onClick={onFlee}>Not now</button>}
          <button className="adv-btn primary big" onClick={onBegin}>Duel!</button>
        </div>
      </div>
    </div>
  );
}
