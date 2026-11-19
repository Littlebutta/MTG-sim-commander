// Adventure mode's windows: the journal (spellbook, satchel, journey), the cauldron, spoils
// after a duel, finding things, and the duel introduction.

import { useEffect, useMemo, useRef, useState } from "react";
import { lookupCards, searchCards, type CardSummary } from "../decks";
import type { CardView } from "../engine/types";
import { BuilderScreen } from "../ui/BuilderScreen";
import { CardPreview, RulesText } from "../ui/Card";
import { explain } from "../ui/keywords";
import { checkAdventureDeck, type AdventureDeckCheck } from "./api";
import { personSprite } from "./art";
import { BASICS, BOSS_IDS, DUELIST_IDS, ENDGAME_IDS, PEOPLE, REAGENTS, SIGILS, listText, type Person } from "./content";
import { DEFAULT_POOLS, POOLS, poolSize, type PoolId } from "./pools";
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

// ---------------------------------------------------------------- spellbook (deck)

/** Your deck, built with the same tools as the main deck builder, from the cards you carry. */
export function Spellbook({ save, onChange, onClose }: { save: Save; onChange: () => void; onClose: () => void }) {
  const [check, setCheck] = useState<AdventureDeckCheck | null>(null);
  const [text, setText] = useState(listText(save.deck));
  useEffect(() => {
    let live = true;
    const t = setTimeout(() => checkAdventureDeck(text).then((c) => live && setCheck(c)).catch(() => {}), 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [text]);
  useKeys(["Escape"], onClose, 200);
  return (
    <div className="adv-builder">
      <BuilderScreen
        onBack={onClose}
        collection={{
          title: `${save.name}'s spellbook`,
          name: save.deckName,
          cards: save.deck,
          owned: save.collection,
          basics: BASICS,
          maxCopies: 4,
          minCards: 40,
          backLabel: "Back to the road",
          onChange: (name, cards) => {
            save.deckName = name;
            save.deck = { ...cards };
            setText(listText(save.deck));
            onChange();
          },
          status: check && (
            <div className={`adv-build-status ${check.playable ? "ok" : "bad"}`}>
              {check.playable
                ? "Ready for the road."
                : check.problems[0] ?? (check.unknown.length ? `Unknown: ${check.unknown.join(", ")}` : "Not ready yet.")}
              <span className="muted"> · Up to 4 of any card you own, basic lands as many as you like, at least 40 cards.</span>
            </div>
          ),
        }}
      />
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

// ---------------------------------------------------------------- card pools

function Pools({ save, onChange }: { save: Save; onChange: () => void }) {
  const on = save.pools?.length ? save.pools : DEFAULT_POOLS;
  const [size, setSize] = useState<number | null>(null);
  const key = on.join(",");
  useEffect(() => {
    let live = true;
    setSize(null);
    poolSize(on).then((n) => live && setSize(n)).catch(() => {});
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const toggle = (id: PoolId) => {
    const next = on.includes(id) ? on.filter((x) => x !== id) : [...on, id];
    if (next.length === 0) return;
    save.pools = POOLS.map((p) => p.id).filter((x) => next.includes(x));
    onChange();
  };
  return (
    <div className="adv-pools">
      <h3>Cards you can find</h3>
      <p className="adv-muted">Where the cards lying around the Vale come from. Change it whenever you like; cards you've already found stay yours.</p>
      <div className="adv-pool-list">
        {POOLS.map((p) => (
          <label key={p.id} className={`adv-pool ${on.includes(p.id) ? "on" : ""}`}>
            <input type="checkbox" checked={on.includes(p.id)} onChange={() => toggle(p.id)} />
            <span>
              <strong>{p.name}</strong>
              <span className="adv-muted">{p.text}</span>
            </span>
          </label>
        ))}
      </div>
      <p className="adv-muted small">{size === null ? "Counting cards…" : `${size.toLocaleString()} different cards to find.`}</p>
    </div>
  );
}

// ---------------------------------------------------------------- journal

export function Journal({ save, onChange, onClose, onQuit, onSpellbook, onNewJourney }: {
  save: Save; onChange: () => void; onClose: () => void; onQuit: () => void; onSpellbook: () => void; onNewJourney: () => void;
}) {
  const [tab, setTab] = useState<"satchel" | "journey" | "options">("journey");
  const [confirm, setConfirm] = useState(false);
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
          <button onClick={onSpellbook}>Spellbook</button>
          <button className={tab === "journey" ? "on" : ""} onClick={() => setTab("journey")}>Journey</button>
          <button className={tab === "satchel" ? "on" : ""} onClick={() => setTab("satchel")}>Satchel</button>
          <button className={tab === "options" ? "on" : ""} onClick={() => setTab("options")}>Options</button>
          <span className="adv-grow" />
          <button className="adv-quiet" onClick={onQuit} title="Your journey is saved">Leave the Vale</button>
          <button className="adv-close" onClick={onClose} aria-label="Close journal">✕</button>
        </div>
        <div className="adv-journal-body">
          {tab === "satchel" && <Satchel save={save} />}
          {tab === "journey" && <Journey save={save} />}
          {tab === "options" && (
            <div className="adv-options">
              <Pools save={save} onChange={onChange} />
              <h3>Music</h3>
              <p className="adv-muted">Press M, or the Music button in the corner, to turn the music on or off. Your choice is remembered.</p>
              <h3>A fresh start</h3>
              {!confirm ? (
                <button className="adv-btn danger" onClick={() => setConfirm(true)}>Begin a new journey</button>
              ) : (
                <div className="adv-confirm">
                  <p>Starting over forgets {save.name}'s journey: every card, sigil and rival. Are you sure?</p>
                  <div className="adv-actions">
                    <button className="adv-btn" onClick={() => setConfirm(false)}>Keep it</button>
                    <button className="adv-btn danger" onClick={onNewJourney}>Start over</button>
                  </div>
                </div>
              )}
            </div>
          )}
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

/** Keyword reminders for a found card, so you know what it does without opening anything. */
function FoundTerms({ card }: { card: CardSummary }) {
  const kws = card.oracle_text.split("\n").flatMap((l) => l.split(/,\s*/)).map((w) => w.trim()).filter((w) => /^[A-Z][a-z]+( [a-z]+)?$/.test(w));
  const terms = explain(kws, card.oracle_text);
  if (terms.length === 0) return null;
  return (
    <dl className="glossary adv-found-terms">
      {terms.map(([k, v]) => <div key={k}><dt>{k}</dt><dd><RulesText text={v} /></dd></div>)}
    </dl>
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
            {s ? <div className="adv-found-card"><CardPreview card={asView(s)} /></div> : <div className="adv-found-name">{card}</div>}
            {s && <FoundTerms card={s} />}
            {s && <div className="adv-found-note">{s.type_line.includes("Land") ? "A land for your deck." : `Added to your satchel. Open your spellbook to put it in your deck.`}</div>}
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
