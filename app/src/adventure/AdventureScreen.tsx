// Adventure mode: walk the Vale of Elderreach, duel the people you meet, gather cards and
// build your deck however you like (up to four of a card).

import "@fontsource/pixelify-sans/400.css";
import "@fontsource/pixelify-sans/600.css";
import "./adventure.css";
import { useCallback, useEffect, useRef, useState } from "react";
import type { GameSession } from "../engine/session";
import type { PlayerView } from "../engine/types";
import { Table } from "../ui/Table";
import { checkAdventureDeck, startDuel } from "./api";
import { challengeFor } from "./difficulty";
import type { Dir } from "./art";
import { DECKS, PEOPLE, REAGENTS, SIGILS, STARTERS, chaosDeck, isBasic, listText, parseList, type Person } from "./content";
import { Cauldron, DuelIntro, Found, Journal, Portrait, Spellbook, Spoils } from "./panels";
import { startMusic, stopMusic, setMusicMood, toggleMusic, musicMuted } from "./music";
import { findCard } from "./pools";
import { addCard, deleteSave, formatTime, lastSlot, loadSlots, newSave, writeSave, type Save } from "./save";
import { uiScale } from "../scale";
import { PLAYER_LOOK, WORLD, World, type Actor, type WorldEvent, type WorldObject } from "./world";

type Overlay =
  | { kind: "none" }
  | { kind: "say"; speaker: Person | null; text: string; resolve: () => void }
  | { kind: "choose"; speaker: Person | null; text: string; options: { label: string; hint?: string; color?: string }[]; resolve: (i: number) => void }
  | { kind: "found"; card?: string; reagent?: string; sigil?: string; resolve: () => void }
  | { kind: "spoils"; title: string; options: string[]; picks: number; resolve: (c: string[]) => void }
  | { kind: "intro"; person: Person; resolve: (go: boolean) => void; canFlee: boolean }
  | { kind: "ask"; speaker: Person | null; text: string; placeholder: string; resolve: (v: string) => void }
  | { kind: "journal" }
  | { kind: "spellbook" }
  | { kind: "cauldron"; resolve: (r: { reagent: string; card: string } | null) => void }
  | { kind: "ending" };

const KEYS: Record<string, Dir> = {
  ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", w: "up", s: "down", a: "left", d: "right", W: "up", S: "down", A: "left", D: "right",
};

// ---------------------------------------------------------------- dialogue box

function DialogueBox({ speaker, text, onDone, children }: { speaker: Person | null; text: string; onDone?: () => void; children?: React.ReactNode }) {
  const [shown, setShown] = useState(0);
  useEffect(() => {
    setShown(0);
    const t = setInterval(() => setShown((n) => (n >= text.length ? n : n + 2)), 16);
    return () => clearInterval(t);
  }, [text]);
  const done = shown >= text.length;
  useEffect(() => {
    if (!onDone) return;
    const key = (e: KeyboardEvent) => {
      if (["e", "E", " ", "Enter"].includes(e.key)) {
        e.preventDefault();
        if (!done) setShown(text.length);
        else onDone();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [done, onDone, text]);
  return (
    <div className="adv-dialogue" onClick={() => (done ? onDone?.() : setShown(text.length))}>
      {speaker && (
        <div className="adv-speaker">
          <Portrait person={speaker} size={64} />
          <span>{speaker.name}</span>
        </div>
      )}
      <div className="adv-dialogue-text">
        {text.slice(0, shown)}
        {done && onDone && <span className="adv-more">▼</span>}
      </div>
      {done && children}
    </div>
  );
}

function ChoiceList({ options, onPick }: { options: { label: string; hint?: string; color?: string }[]; onPick: (i: number) => void }) {
  const [sel, setSel] = useState(0);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown" || e.key === "s") setSel((s) => (s + 1) % options.length);
      else if (e.key === "ArrowUp" || e.key === "w") setSel((s) => (s + options.length - 1) % options.length);
      else if (["e", "E", " ", "Enter"].includes(e.key)) {
        e.preventDefault();
        onPick(sel);
      } else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [options, sel, onPick]);
  return (
    <div className="adv-choices" onClick={(e) => e.stopPropagation()}>
      {options.map((o, i) => (
        <button key={i} className={`adv-choice ${i === sel ? "on" : ""}`} onMouseEnter={() => setSel(i)} onClick={() => onPick(i)}>
          {o.color && <span className="adv-choice-swatch" style={{ background: o.color }} />}
          <span className="adv-choice-label">{o.label}</span>
          {o.hint && i === sel && <span className="adv-choice-hint">{o.hint}</span>}
        </button>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------- save slots

/** Pick a journey to continue, or a slot to begin a new one in. */
function SlotPicker({ onPick, onBack }: { onPick: (slot: number, save: Save | null) => void; onBack: () => void }) {
  const [slots, setSlots] = useState<(Save | null)[] | null>(null);
  const [confirm, setConfirm] = useState<number | null>(null);
  useEffect(() => {
    loadSlots().then(setSlots);
  }, []);
  const last = lastSlot();
  return (
    <div className="screen adv-slots">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← Back</button>
        <h2>Adventure</h2>
        <span />
      </header>
      <div className="adv-slot-list">
        {!slots && <p className="muted">Looking for your journeys…</p>}
        {slots?.map((s, i) => {
          const slot = i + 1;
          return (
            <div key={slot} className={`adv-slot ${s ? "" : "empty"} ${s && slot === last ? "last" : ""}`}>
              <div className="adv-slot-main">
                {s ? <Portrait person={{ look: PLAYER_LOOK }} size={56} /> : <span className="adv-slot-n">{slot}</span>}
                <div className="adv-slot-text">
                  <strong>{s ? s.name || "A new traveler" : `Journey ${slot}`}</strong>
                  {s ? (
                    <span className="muted">
                      {formatTime(s.playtime)} on the road · {s.sigils.length} of {Object.keys(SIGILS).length} sigils · {Object.values(s.collection).reduce((a, b) => a + b, 0)} cards · {s.wins} wins
                      <br />Saved {new Date(s.saved).toLocaleString()}
                    </span>
                  ) : (
                    <span className="muted">Empty</span>
                  )}
                </div>
              </div>
              <div className="adv-slot-actions">
                {s && confirm !== slot && <button className="btn" onClick={() => setConfirm(slot)}>Delete</button>}
                {s && confirm === slot && (
                  <>
                    <span className="muted small">Delete this journey?</span>
                    <button className="btn" onClick={() => setConfirm(null)}>Keep</button>
                    <button className="btn btn-primary danger" onClick={async () => { await deleteSave(slot); setConfirm(null); setSlots(await loadSlots()); }}>Delete</button>
                  </>
                )}
                <button className="btn btn-primary" onClick={() => onPick(slot, s)} autoFocus={slot === last}>{s ? "Continue" : "Begin a journey"}</button>
              </div>
            </div>
          );
        })}
      </div>
      <p className="muted small adv-slot-hint">Your journey saves itself as you go: at every touchstone, door and find, and every few seconds while you walk.</p>
    </div>
  );
}

// ---------------------------------------------------------------- a typed answer

function AskBox({ speaker, text, placeholder, onDone }: { speaker: Person | null; text: string; placeholder: string; onDone: (v: string) => void }) {
  const [v, setV] = useState("");
  return (
    <DialogueBox speaker={speaker} text={text}>
      <form className="adv-ask" onSubmit={(e) => { e.preventDefault(); onDone(v.trim() || placeholder); }} onClick={(e) => e.stopPropagation()}>
        <input className="adv-name" autoFocus maxLength={16} placeholder={placeholder} value={v} onChange={(e) => setV(e.target.value)} />
        <button className="adv-btn primary" type="submit">That's me</button>
      </form>
    </DialogueBox>
  );
}

// ---------------------------------------------------------------- the adventure

export function AdventureScreen({ onExit }: { onExit: () => void }) {
  const [playing, setPlaying] = useState(false);
  const [slot, setSlot] = useState<number | null>(null);
  const slotRef = useRef(1);
  const [muted, setMuted] = useState(musicMuted());
  const [overlay, setOverlay] = useState<Overlay>({ kind: "none" });
  const [banner, setBanner] = useState<{ key: number; text: string } | null>(null);
  /** The respawn transition: your vision blurs and darkens, then your eyes open somewhere safe. */
  const [fade, setFade] = useState<{ phase: "out" | "in"; text?: string } | null>(null);
  const [duel, setDuel] = useState<{ session: GameSession; done: (won: boolean) => void } | null>(null);
  const [, setTick] = useState(0);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const worldRef = useRef<World | null>(null);
  const overlayRef = useRef(overlay);
  overlayRef.current = overlay;
  const busyRef = useRef(false);
  const heldRef = useRef<Dir[]>([]);
  const runRef = useRef(false);
  /** Open the spellbook once the current conversation ends. */
  const thenSpellbookRef = useRef(false);

  const persist = useCallback(() => {
    if (worldRef.current) void writeSave(worldRef.current.save, slotRef.current);
  }, []);

  // Save when the window is closed or hidden, so quitting never loses the walk since the last save.
  useEffect(() => {
    if (!playing) return;
    const flush = () => persist();
    const vis = () => document.visibilityState === "hidden" && persist();
    window.addEventListener("pagehide", flush);
    window.addEventListener("beforeunload", flush);
    document.addEventListener("visibilitychange", vis);
    return () => {
      window.removeEventListener("pagehide", flush);
      window.removeEventListener("beforeunload", flush);
      document.removeEventListener("visibilitychange", vis);
      persist();
    };
  }, [playing, persist]);

  // ---- overlay helpers (scripts await these)
  const say = (speaker: Person | null, text: string) => new Promise<void>((resolve) => setOverlay({ kind: "say", speaker, text, resolve }));
  const sayAll = async (speaker: Person | null, lines: string[]) => {
    for (const l of lines) await say(speaker, l);
  };
  const choose = (speaker: Person | null, text: string, options: { label: string; hint?: string; color?: string }[]) =>
    new Promise<number>((resolve) => setOverlay({ kind: "choose", speaker, text, options, resolve }));
  const ask = (speaker: Person | null, text: string, placeholder: string) =>
    new Promise<string>((resolve) => setOverlay({ kind: "ask", speaker, text, placeholder, resolve }));
  const showFound = (f: { card?: string; reagent?: string; sigil?: string }) => new Promise<void>((resolve) => setOverlay({ kind: "found", ...f, resolve }));
  const close = () => setOverlay({ kind: "none" });
  const name = (s: string) => s.replace(/\{name\}/g, worldRef.current?.save.name ?? "traveler");

  // ---- scripts
  async function run(script: () => Promise<void>) {
    if (busyRef.current) return;
    busyRef.current = true;
    try {
      await script();
    } finally {
      busyRef.current = false;
      heldRef.current = [];
      if (thenSpellbookRef.current) {
        thenSpellbookRef.current = false;
        setOverlay({ kind: "spellbook" });
      } else close();
    }
  }

  async function bramIntro(w: World) {
    const bram = PEOPLE.bram;
    const s = w.save;
    if (!s.name) {
      await say(bram, "Ah! There you are. Come in, come in, mind the boxes.");
      s.name = (await ask(bram, "Now, my memory's not what it was. Remind me your name?", "Wren")).slice(0, 16);
      persist();
    }
    await sayAll(bram, [
      name(`{name}, of course! How could I forget.`),
      "Forty years I've kept this shop, and every one of those years someone your age walks in with that look. The road look.",
      "Well. I can't send you out there empty-handed, can I? The Vale's full of duelists, and every one of them will want a match.",
      "I've put together seven decks for travelers just starting out. Simple ones. Each knows one thing and does it well. Pick the one that speaks to you.",
    ]);
    for (;;) {
      const i = await choose(bram, "Which deck will you carry?", STARTERS.map((c) => ({ label: c.label, hint: c.pitch, color: c.color })));
      const choice = STARTERS[i];
      const ok = await choose(bram, `${choice.pitch} Is this the one?`, [{ label: `Yes, the ${choice.label} deck` }, { label: "Let me look again" }]);
      if (ok !== 0) continue;
      let list: Record<string, number>;
      let deckName: string;
      if (choice.id === "chaos") {
        const c = chaosDeck();
        list = c.list;
        deckName = c.name;
      } else {
        list = parseList(DECKS.starters[choice.id].text);
        deckName = DECKS.starters[choice.id].name;
      }
      s.starter = choice.id;
      s.deck = { ...list };
      s.deckName = deckName;
      for (const [n, k] of Object.entries(list)) if (!isBasic(n)) addCard(s, n, k);
      break;
    }
    await sayAll(bram, [
      starterLine(s.starter),
      `Here: your ${s.deckName} deck. Treat it well and it'll treat you well.`,
      "Out there you'll find cards lying about, lost by careless travelers. Some in plain sight, some hidden in tall grass. Keep your eyes open for a glint.",
      "Win a duel and your opponent will usually let you take a card from their deck. That's the custom.",
      "Build your deck any way you like. No rules on the road, except one: no more than four copies of any card. Lands from the earth are free, take as many as you need.",
      "Open your journal with Esc to see your cards, and your spellbook to change your deck.",
      "And if you ever find something truly rare, a strange root or a glowing stone, take it to Ysolde in her tower by the crossroads. Her cauldron can brew it into any card you can name.",
      "The touchstones out there will remember you. Touch one, and if you're ever beaten, you'll wake beside it. The one in the village square is closest.",
      "Go on, then. The road's waiting. And come back and tell me about it sometime.",
    ]);
    s.flags["met:bram"] = true;
    persist();
  }
  const starterLine = (id: string | null) =>
    id === "chaos" ? "Chaos, eh? Let me just... reach into the bottom drawer... don't look... there. Whatever that is, it's yours now." : id === "colorless" ? "Clockwork! My old favorite. They don't care what color the land is." : "A fine choice. It suits you.";

  async function talk(w: World, obj: WorldObject) {
    const id = obj.id!;
    const p = PEOPLE[id];
    const s = w.save;
    if (!p) return;
    if (id === "bram") {
      if (!s.starter) return bramIntro(w);
      return sayAll(p, [
        s.sigils.length === 0 ? "Back already? Have you been to the crossroads yet? Ysolde's tower is just past it." : `${s.sigils.length} sigil${s.sigils.length > 1 ? "s" : ""}! I knew you had it in you.`,
        "Remember: four copies of a card at most, lands as many as you like. Esc opens your journal.",
      ]);
    }
    if (id === "ysolde") {
      const has = Object.values(s.reagents).some((n) => n > 0);
      if (!s.flags["met:ysolde"]) {
        s.flags["met:ysolde"] = true;
        await sayAll(p, [
          "Oh! A visitor. Don't touch the cauldron. Well, do, but not yet.",
          "This cauldron is very old and very hungry. Feed it something rare, a reagent, and it will brew you any card you can name.",
          "Any card at all. Spells, creatures, artifacts, even rare lands. Every card there has ever been is in that brew somewhere.",
        ]);
      }
      if (!has) return say(p, "Come back when you've found something rare. Moonpetals, phoenix embers, that kind of thing. The old powers carry them too, I hear.");
      const go = await choose(p, "You've brought something rare! Shall we brew?", [{ label: "Brew a spell" }, { label: "Not now" }]);
      if (go === 0) return brew(w);
      return;
    }
    if (id === "innkeep") {
      const go = await choose(p, "Rest a while? I'll keep your place safe, and you'll wake here if anything goes wrong out there.", [{ label: "Rest" }, { label: "No thanks" }]);
      if (go === 0) {
        s.respawn = { map: w.mapId, x: w.px, y: w.py, dir: w.dir };
        s.stone = null;
        persist();
        await say(p, "Sleep well! Your journey is saved.");
      }
      return;
    }
    if (id === "wenna") {
      if (!s.flags["gift:wenna"]) {
        await sayAll(p, ["Oh, it's you, dear! Setting out today? Bram told me.", "Here, take this. It was my husband's favorite. It watched over him on every road he took."]);
        s.flags["gift:wenna"] = true;
        addCard(s, "Serra Angel");
        persist();
        await showFound({ card: "Serra Angel" });
        return say(p, "Put it in your deck now, mind. Open your journal and find it in your spellbook.");
      }
      return say(p, "Come home safe, dear. And eat something.");
    }
    if (!p.duel) return sayAll(p, p.intro);
    s.flags[`met:${id}`] = true;
    if (s.flags[`beat:${id}`]) {
      await sayAll(p, p.after ?? ["Good duel."]);
      const again = await choose(p, p.duel.rank === "duelist" ? "Fancy another round?" : "Do you wish to face me again?", [{ label: "Duel again" }, { label: "Farewell" }]);
      if (again === 0) return challenge(w, id, true);
      return;
    }
    await sayAll(p, p.intro);
    return challenge(w, id, true);
  }

  async function challenge(w: World, id: string, canFlee: boolean) {
    const p = PEOPLE[id];
    const s = w.save;
    s.flags[`met:${id}`] = true;
    const check = await checkAdventureDeck(listText(s.deck));
    if (!check.playable) {
      // Don't let them keep challenging you: they wait while you fix your deck.
      // (saved, so they stay patient after you quit and come back)
      s.flags[`wait:${id}`] = true;
      persist();
      await say(p, "Hold on, your deck's not ready for a proper duel. Go on, sort it out. I'll wait right here.");
      await say(null, `${check.problems[0] ?? "Your deck isn't ready"}. Your spellbook opens so you can fix it. Talk to ${p.name} when you're ready.`);
      thenSpellbookRef.current = true;
      return;
    }
    delete s.flags[`wait:${id}`];
    const go = await new Promise<boolean>((resolve) => setOverlay({ kind: "intro", person: p, resolve, canFlee }));
    if (!go) {
      await say(p, "Another time, then.");
      return;
    }
    close();
    const level = challengeFor(p, s);
    let won = false;
    try {
      const session = await startDuel(
        { name: s.deckName, text: listText(s.deck) },
        { name: p.name, deckName: `${p.name}'s deck`, text: DECKS.duelists[p.duel!.deck], style: p.duel!.style, life: level.life, skill: level.skill },
      );
      won = await new Promise<boolean>((resolve) => setDuel({ session, done: resolve }));
    } catch (e) {
      await say(null, `The duel couldn't begin: ${e instanceof Error ? e.message : String(e)}`);
      return;
    } finally {
      setDuel(null);
    }
    if (won) await victory(w, id);
    else await defeat(w, id);
  }

  async function victory(w: World, id: string) {
    const p = PEOPLE[id];
    const d = p.duel!;
    const s = w.save;
    const first = !s.flags[`beat:${id}`];
    s.wins++;
    s.flags[`beat:${id}`] = true;
    persist();
    await sayAll(p, p.beaten ?? ["Well played."]);
    // Spoils: pick from cards in their deck.
    const pool = Object.keys(parseList(DECKS.duelists[d.deck])).filter((n) => !isBasic(n));
    const picks = !first ? 1 : d.rank === "endgame" ? 3 : d.rank === "boss" ? 2 : 1;
    const options = pool.sort(() => Math.random() - 0.5).slice(0, picks + 2);
    const chosen = await new Promise<string[]>((resolve) => setOverlay({ kind: "spoils", title: `Spoils from ${p.name}`, options, picks, resolve }));
    for (const c of chosen) addCard(s, c);
    if (d.sigil && !s.sigils.includes(d.sigil)) {
      s.sigils.push(d.sigil);
      await showFound({ sigil: d.sigil });
    }
    if (d.reagent && first) {
      s.reagents[d.reagent] = (s.reagents[d.reagent] ?? 0) + 1;
      await showFound({ reagent: d.reagent });
    }
    persist();
    if (d.sigil && s.sigils.length === 5) {
      await say(null, "Five sigils. Far to the north, beyond the ruins of the Reach, something old stirs in the Rift Spire. The door is waiting.");
    }
    if (id === "morvane" && first) {
      setOverlay({ kind: "ending" });
      await new Promise<void>((resolve) => {
        const key = (e: KeyboardEvent) => {
          if (["e", "E", " ", "Enter", "Escape"].includes(e.key)) {
            window.removeEventListener("keydown", key);
            resolve();
          }
        };
        setTimeout(() => window.addEventListener("keydown", key), 1500);
      });
      s.flags["ending"] = true;
      persist();
    }
  }

  async function defeat(w: World, id: string) {
    const p = PEOPLE[id];
    const s = w.save;
    s.losses++;
    await sayAll(p, p.gloat ?? ["Better luck next time."]);
    close();
    const wait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
    setFade({ phase: "out", text: "Your vision blurs..." });
    await wait(2200);
    w.enter(s.respawn);
    persist();
    setFade({ phase: "in" });
    await wait(2000);
    setFade(null);
    setBanner({ key: Date.now(), text: w.region()?.name ?? "" });
    await say(null, s.stone ? "You wake beside the touchstone, its hum steady and familiar." : "You wake somewhere safe, a little wiser.");
  }

  async function brew(w: World) {
    const s = w.save;
    const ysolde = PEOPLE.ysolde;
    const r = await new Promise<{ reagent: string; card: string } | null>((resolve) => setOverlay({ kind: "cauldron", resolve }));
    if (!r) return;
    s.reagents[r.reagent] -= 1;
    if (s.reagents[r.reagent] <= 0) delete s.reagents[r.reagent];
    addCard(s, r.card);
    s.brews++;
    persist();
    await say(ysolde, `In goes the ${REAGENTS[r.reagent]?.name ?? "reagent"}... the brew turns, and turns, and... there!`);
    await showFound({ card: r.card });
    await say(ysolde, "Lovely work, if I say so myself. It's in your satchel now.");
  }

  async function handle(w: World, ev: WorldEvent) {
    const s = w.save;
    switch (ev.type) {
      case "region":
        setBanner({ key: Date.now(), text: ev.name });
        if (!s.seenRegions.includes(ev.id)) s.seenRegions.push(ev.id);
        return;
      case "warp": {
        const o = ev.obj;
        if (!s.starter && w.mapId === "shop") {
          return run(async () => {
            await say(PEOPLE.bram, "Hold on, hold on! You're not leaving without a deck. Come back here.");
            w.enter({ map: "shop", x: 4, y: 5, dir: "up" });
            await bramIntro(w);
          });
        }
        w.enter({ map: o.to!, x: o.tx!, y: o.ty!, dir: (o.dir as Dir) ?? "down" });
        const r = w.region();
        if (r) {
          w.lastRegion = r.id;
          setBanner({ key: Date.now(), text: r.name });
        }
        persist();
        return;
      }
      case "found": {
        const o = ev.obj;
        return run(async () => {
          s.flags[`got:${w.mapId}:${o.id}`] = true;
          if (o.kind === "card") {
            const card = await findCard(o.region ?? "meadow", !!o.hidden, s.pools ?? []);
            addCard(s, card);
            persist();
            await showFound({ card });
          } else if (o.kind === "reagent" && o.id) {
            s.reagents[o.id] = (s.reagents[o.id] ?? 0) + 1;
            persist();
            if (o.chest) await say(null, "You open the old chest...");
            await showFound({ reagent: o.id });
          }
        });
      }
      case "challenge":
        return run(async () => {
          await new Promise<void>((resolve) => w.startApproach(ev.npc as Actor, resolve));
          const p = PEOPLE[ev.npc.id];
          await sayAll(p, p.intro);
          await challenge(w, ev.npc.id, false);
        });
      case "interact": {
        const o = ev.obj;
        return run(async () => {
          switch (o.kind) {
            case "npc": return talk(w, o);
            case "sign": return sayAll(null, (o.text ?? "").split("\n"));
            case "well": return say(null, "A deep, cool well. Someone has dropped a coin in for luck. You hear it, far below, still falling.");
            case "waystone": {
              s.respawn = { map: w.mapId, x: w.px, y: w.py, dir: w.dir };
              s.stone = o.id ?? null;
              persist();
              return sayAll(null, ["You lay a hand on the touchstone. It hums, warm, and remembers you.", "Your journey is saved."]);
            }
            case "cauldron": {
              const has = Object.values(s.reagents).some((n) => n > 0);
              if (!has) return say(null, "The cauldron bubbles, green and patient. It wants something rare.");
              return brew(w);
            }
            case "gate": {
              const n = s.sigils.length;
              if (n < 5) return sayAll(null, ["A great black door, sealed. Five hollows are cut into it in a ring.", n === 0 ? "None of them glow." : `${n} of them glow, in answer to the sigils you carry.`]);
              await sayAll(null, ["All five hollows blaze with light. The sigils in your satchel grow warm.", "With a sound like a held breath let go, the door of the Rift Spire opens."]);
              s.flags[`open:${o.id}`] = true;
              persist();
              return;
            }
          }
        });
      }
    }
  }

  // Development helpers for testing scripts without walking everywhere.
  if ((import.meta as unknown as { env?: { DEV?: boolean } }).env?.DEV) {
    (window as unknown as Record<string, unknown>).__adv = {
      world: () => worldRef.current,
      victory: (id: string) => worldRef.current && run(() => victory(worldRef.current!, id)),
      go: (map: string, x: number, y: number) => worldRef.current?.enter({ map, x, y, dir: "down" }),
      found: (card: string) => run(() => showFound({ card })),
      brew: () => worldRef.current && run(() => brew(worldRef.current!)),
      defeat: (id: string) => worldRef.current && run(() => defeat(worldRef.current!, id)),
    };
  }

  // ---- start / continue
  const begin = (s: Save) => {
    const w = new World(s);
    worldRef.current = w;
    setPlaying(true);
    startMusic();
    const r = w.region();
    if (r) {
      w.lastRegion = r.id;
      setTimeout(() => setBanner({ key: Date.now(), text: r.name }), 300);
    }
    if (!s.starter) setTimeout(() => run(() => bramIntro(w)), 500);
  };

  // ---- the game loop
  useEffect(() => {
    if (!playing) return;
    let raf = 0;
    let last = performance.now();
    let saveTimer = 0;
    const loop = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const w = worldRef.current;
      const cv = canvasRef.current;
      if (w && cv && !duel) {
        const paused = busyRef.current || overlayRef.current.kind !== "none";
        if (!paused) {
          w.save.playtime += dt;
          saveTimer += dt;
          if (saveTimer > 15) {
            saveTimer = 0;
            persist();
          }
        }
        const ev = w.update(dt, { held: heldRef.current, run: runRef.current }, paused);
        if (ev) void handle(w, ev);
        const dpr = window.devicePixelRatio || 1;
        // The interface is zoomed as a whole (see scale.ts); draw at real screen pixels.
        const px = dpr * (uiScale() || 1);
        const W = Math.floor(cv.clientWidth * px);
        const H = Math.floor(cv.clientHeight * px);
        if (cv.width !== W || cv.height !== H) {
          cv.width = W;
          cv.height = H;
        }
        const scale = Math.max(2, Math.round(H / (13.5 * 16)));
        setMusicMood(w.mapId === "overworld" ? w.region()?.id ?? "meadow" : w.mapId);
        w.draw(cv.getContext("2d")!, W, H, scale);
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playing, duel]);

  // ---- input
  useEffect(() => {
    if (!playing) return;
    const down = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (duel) return;
      const d = KEYS[e.key];
      if (d) {
        e.preventDefault();
        if (!heldRef.current.includes(d)) heldRef.current = [...heldRef.current, d];
        return;
      }
      if (e.key === "Shift") runRef.current = true;
      if ((e.key === "m" || e.key === "M") && overlayRef.current.kind !== "spellbook" && overlayRef.current.kind !== "ask") {
        setMuted(toggleMusic());
        return;
      }
      const w = worldRef.current;
      if (!w || busyRef.current || overlayRef.current.kind !== "none") return;
      if (e.key === "e" || e.key === "E" || e.key === " " || e.key === "Enter") {
        e.preventDefault();
        const ev = w.interact();
        if (ev) void handle(w, ev);
      } else if (e.key === "Escape" || e.key === "Tab") {
        e.preventDefault();
        heldRef.current = [];
        setOverlay({ kind: "journal" });
      }
    };
    const up = (e: KeyboardEvent) => {
      const d = KEYS[e.key];
      if (d) heldRef.current = heldRef.current.filter((x) => x !== d);
      if (e.key === "Shift") runRef.current = false;
    };
    const blur = () => {
      heldRef.current = [];
      runRef.current = false;
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", blur);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playing, duel]);

  // ---------------------------------------------------------------- render

  useEffect(() => () => stopMusic(), []);
  useEffect(() => {
    if (duel) setMusicMood("duel");
  }, [duel]);
  if (slot === null) {
    return (
      <SlotPicker
        onBack={onExit}
        onPick={(n, s) => {
          slotRef.current = n;
          setSlot(n);
          const save = s ?? newSave("", WORLD.start);
          if (!s) void writeSave(save, n);
          begin(save);
        }}
      />
    );
  }
  if (!playing || !worldRef.current) return <div className="adv-root" />;
  if (duel) {
    // Adventure duels have no commanders, so the command zones are hidden (see adventure.css).
    return (
      <div className="adv-duel">
        <Table
          session={duel.session}
          onExit={() => {
            const v: PlayerView = duel.session.view();
            const d = v.decision as { GameOver?: { winner: number | null } };
            duel.done(!!d.GameOver && d.GameOver.winner === v.you);
          }}
        />
      </div>
    );
  }
  const w = worldRef.current!;
  const o = overlay;
  return (
    <div className="adv-root">
      <canvas ref={canvasRef} className={`adv-canvas ${fade ? `adv-canvas-${fade.phase}` : ""}`} />
      {fade && (
        <div key={fade.phase} className={`adv-fade ${fade.phase}`}>
          {fade.phase === "in" ? <><div className="adv-lid top" /><div className="adv-lid bottom" /></> : <div className="adv-fade-text">{fade.text}</div>}
        </div>
      )}
      {banner && (
        <div key={banner.key} className="adv-banner" onAnimationEnd={() => setBanner(null)}>{banner.text}</div>
      )}
      {o.kind === "none" && !busyRef.current && (
        <div className="adv-hud">
          <button className="adv-journal-btn" onClick={() => setMuted(toggleMusic())} title="Music (M)">{muted ? "Music off" : "Music on"}</button>
          <button className="adv-journal-btn" onClick={() => setOverlay({ kind: "spellbook" })} title="Your deck">Spellbook</button>
          <button className="adv-journal-btn" onClick={() => setOverlay({ kind: "journal" })} title="Journal (Esc)">Journal</button>
        </div>
      )}
      {o.kind === "ask" && <AskBox speaker={o.speaker} text={o.text} placeholder={o.placeholder} onDone={(v) => { close(); o.resolve(v); }} />}
      {o.kind === "spellbook" && <Spellbook save={w.save} onChange={() => { setTick((t) => t + 1); persist(); }} onClose={() => { persist(); close(); }} />}
      {o.kind === "say" && <DialogueBox speaker={o.speaker} text={o.text} onDone={() => { close(); o.resolve(); }} />}
      {o.kind === "choose" && (
        <DialogueBox speaker={o.speaker} text={o.text}>
          <ChoiceList options={o.options} onPick={(i) => { close(); o.resolve(i); }} />
        </DialogueBox>
      )}
      {o.kind === "found" && <Found card={o.card} reagent={o.reagent} sigil={o.sigil} onClose={() => { close(); o.resolve(); }} />}
      {o.kind === "spoils" && <Spoils title={o.title} options={o.options} picks={o.picks} onDone={(c) => { close(); o.resolve(c); }} />}
      {o.kind === "intro" && <DuelIntro person={o.person} you={w.playerLook()} challenge={challengeFor(o.person, w.save)} onBegin={() => o.resolve(true)} onFlee={o.canFlee ? () => o.resolve(false) : undefined} />}
      {o.kind === "cauldron" && <Cauldron save={w.save} onBrew={(reagent, card) => { close(); o.resolve({ reagent, card }); }} onClose={() => { close(); o.resolve(null); }} />}
      {o.kind === "journal" && (
        <Journal
          save={w.save}
          onChange={() => setTick((t) => t + 1)}
          onClose={() => { persist(); close(); }}
          onQuit={async () => { if (worldRef.current) await writeSave(worldRef.current.save, slotRef.current); stopMusic(); onExit(); }}
          onSpellbook={() => setOverlay({ kind: "spellbook" })}
          onNewJourney={() => {
            setOverlay({ kind: "none" });
            const s = newSave("", WORLD.start);
            void writeSave(s, slotRef.current);
            begin(s);
          }}
        />
      )}
      {o.kind === "ending" && (
        <div className="adv-ending">
          <div className="adv-ending-text">
            <p>The Hall of Forgetting falls quiet.</p>
            <p>Far below, in the Vale, people look up from their work, and for a moment remember something they had forgotten they'd lost.</p>
            <p>In Hearthmoor, an old shopkeeper smiles, and puts the kettle on.</p>
            <h2>{w.save.name}, keeper of the Vale</h2>
            <p className="adv-muted">{formatTime(w.save.playtime)} on the road · {w.save.wins} duels won · {w.save.sigils.length} sigils · {w.save.brews} spells brewed</p>
            <p className="adv-muted">The road goes on. Every duelist in the Vale will still take a rematch, and there are always more lost cards to find.</p>
            <p className="adv-hint">Press E to continue</p>
          </div>
        </div>
      )}
    </div>
  );
}
