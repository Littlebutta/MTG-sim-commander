import { useCallback, useEffect, useRef, useState } from "react";
import type { GameSession, RewindPoint } from "../engine/session";
import type { Action, CardView, PlayerView, SeatView, Step, Target } from "../engine/types";
import { decisionKind, sameTarget } from "../engine/types";
import { themeForSeat, useSettings, type Settings } from "../settings";
import { Card, CardPreview, ManaCost, RulesPopup } from "./Card";
import { CrownIcon, ScrollIcon, ShieldIcon, SwordIcon } from "./icons";
import { CommanderModal, DamageModal, DiscardModal, GameOverModal, MulliganModal, TriggerModal } from "./Modals";
import { play, playChanges } from "../sound";
import { uiScale } from "../scale";
import { ArrowLayer, anchorEl, elementAtPointer, type ArrowSpec } from "./Arrows";

type Mode =
  | { kind: "idle" }
  | {
      kind: "target";
      title: string;
      /** A prompt per slot, when they differ (several triggers at once). */
      titles?: string[];
      slots: Target[][];
      chosen: Target[];
      build: (t: Target[]) => Action;
      /** Where the targeting arrows start. */
      source?: number;
      /** Triggered abilities must go on the stack; they can't be called off. */
      cancellable: boolean;
    }
  | { kind: "abilities"; card: CardView };

const SunIcon = ({ size = 14 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" aria-hidden>
    <circle cx="12" cy="12" r="4" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M5 19l2-2M17 7l2-2" />
  </svg>
);
const DrawIcon = ({ size = 14 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinejoin="round" aria-hidden>
    <rect x="4" y="5" width="10" height="14" rx="2" /><rect x="10" y="3" width="10" height="14" rx="2" />
  </svg>
);
const MoonIcon = ({ size = 14 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinejoin="round" aria-hidden>
    <path d="M20 14A8 8 0 1 1 10 4a6 6 0 0 0 10 10z" />
  </svg>
);

const PHASES: { label: string; steps: Step[]; icon: (p: { size?: number }) => JSX.Element; sub: string }[] = [
  { label: "Upkeep", steps: ["Untap", "Upkeep"], icon: SunIcon, sub: "Untap, then upkeep" },
  { label: "Draw", steps: ["Draw"], icon: DrawIcon, sub: "Draw a card" },
  { label: "Main", steps: ["Main1"], icon: CrownIcon, sub: "Play a land, cast spells" },
  { label: "Combat", steps: ["BeginCombat", "DeclareAttackers", "DeclareBlockers", "FirstStrikeDamage", "CombatDamage", "EndCombat"], icon: SwordIcon, sub: "Attack and block" },
  { label: "Main 2", steps: ["Main2"], icon: ShieldIcon, sub: "Second main phase" },
  { label: "End", steps: ["End", "Cleanup"], icon: MoonIcon, sub: "End of turn" },
];
const phaseOf = (s: Step) => PHASES.findIndex((p) => p.steps.includes(s));

const backClass = (s: Settings) => `card-back back-${s.back === "custom" && !s.customBack ? "sigil" : s.back}`;
const backStyle = (s: Settings): React.CSSProperties | undefined =>
  s.back === "custom" && s.customBack ? { backgroundImage: `url(${s.customBack})` } : undefined;

const STEP_NAMES: Record<Step, string> = {
  Untap: "Untap", Upkeep: "Upkeep", Draw: "Draw", Main1: "Main phase", BeginCombat: "Beginning of combat",
  DeclareAttackers: "Declare attackers", DeclareBlockers: "Declare blockers", FirstStrikeDamage: "First-strike damage",
  CombatDamage: "Combat damage", EndCombat: "End of combat", Main2: "Second main phase", End: "End step", Cleanup: "Cleanup",
};

function Sigil({ seat, size }: { seat: SeatView; size: number }) {
  const cols = seat.color_identity.length ? seat.color_identity : ["C"];
  const stops = cols.map((c, i) => `var(--mana-${c}) ${(i / cols.length) * 100}% ${((i + 1) / cols.length) * 100}%`).join(", ");
  return (
    <div className="sigil" style={{ width: size, height: size, background: `conic-gradient(${stops})` }}>
      <span>{seat.name.slice(0, 1)}</span>
    </div>
  );
}

function Medallion({ seat, targetable, onClick, position }: { seat: SeatView; targetable: boolean; onClick?: () => void; position: "top" | "bottom" }) {
  return (
    <div className={`medallion medallion-${position} ${targetable ? "glow-target clickable" : ""} ${seat.has_priority ? "has-priority" : ""}`} onClick={onClick} data-player={seat.id}>
      <Sigil seat={seat} size={64} />
      <div className="life">{seat.life}</div>
      {Object.keys(seat.mana_pool).length > 0 && (
        <div className="pool">
          {Object.entries(seat.mana_pool).map(([k, v]) => (
            <span key={k} className={`pip pip-${k}`}>{v}</span>
          ))}
        </div>
      )}
    </div>
  );
}

const HeartIcon = ({ size = 12 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden>
    <path d="M12 21s-7.5-4.6-9.6-9.2C.9 8.4 3 4.5 6.8 4.5c2.2 0 3.6 1.2 5.2 3 1.6-1.8 3-3 5.2-3 3.8 0 5.9 3.9 4.4 7.3C19.5 16.4 12 21 12 21z" />
  </svg>
);

/** Life total, sitting right by the player's name so it reads as theirs. */
function LifePill({ life, big }: { life: number; big?: boolean }) {
  return (
    <span className={`life-pill ${big ? "big" : ""} ${life <= 10 ? "low" : ""}`} title="Life total">
      <HeartIcon size={big ? 13 : 11} />
      <span className="bump" key={life}>{life}</span>
    </span>
  );
}

/** How many cards a player is holding: a little fan of card backs with the number on it. */
function HandPill({ count, big }: { count: number; big?: boolean }) {
  return (
    <span className={`hand-pill ${big ? "big" : ""} ${count === 0 ? "empty" : ""}`} title={`${count} card${count === 1 ? "" : "s"} in hand`}>
      <span className="hand-fan" aria-hidden>
        {Array.from({ length: Math.min(Math.max(count, 1), 5) }, (_, i) => <i key={i} />)}
      </span>
      <span className="bump" key={count}>{count}</span>
      <span className="hand-pill-word">in hand</span>
    </span>
  );
}

/** A player's command zone, framed and labeled so it can't be mistaken for the battlefield. */
function CommandZone({ cards, onHover, onInspect, mine, castable, onCast }: {
  cards: CardView[]; onHover: (c: CardView | null) => void; onInspect: (c: CardView) => void;
  mine?: boolean; castable?: number[]; onCast?: (c: CardView) => void;
}) {
  return (
    <div className={`cmd-zone ${mine ? "mine" : ""} ${cards.length === 0 ? "empty" : ""}`}
      title={mine ? "Your command zone: cast your commander from here" : "Command zone: their commander waits here until it's cast"}>
      <span className="cmd-zone-label"><CrownIcon size={10} /> Command zone</span>
      <div className="cmd-zone-cards">
        {cards.length === 0 ? <span className="cmd-zone-out">Commander is out</span> : cards.map((c) => (
          <Card key={c.id} card={c} size="mini" glow={castable?.includes(c.id) ? "play" : null}
            onClick={onCast && castable?.includes(c.id) ? () => onCast(c) : undefined} onHover={onHover} onInspect={onInspect} />
        ))}
      </div>
    </div>
  );
}

// Your side of the table has nine places to put cards: three rows (front, middle, back), each
// with a left, center and right area. Cards land in a default place and can be dragged anywhere.
const ROWS = ["front", "mid", "back"] as const;
const COLS = ["l", "c", "r"] as const;
type Slot = `${(typeof ROWS)[number]}-${(typeof COLS)[number]}`;
const SLOT_NAMES: Record<Slot, string> = {
  "front-l": "Front left", "front-c": "Front line", "front-r": "Front right",
  "mid-l": "Middle left", "mid-c": "Middle", "mid-r": "Middle right",
  "back-l": "Back left", "back-c": "Lands", "back-r": "Back right",
};
const defaultSlot = (c: CardView): Slot =>
  c.types.includes("Creature") ? "front-c" : c.types.includes("Land") ? "back-c" : "back-l";

// Instants and flash cards you've asked not to pause the game for (right-click a card in hand).
const QUIET_KEY = "ct-quiet-cards";
function loadQuiet(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(QUIET_KEY) ?? "[]") as string[]);
  } catch {
    return new Set();
  }
}
function saveQuiet(s: Set<string>) {
  try {
    localStorage.setItem(QUIET_KEY, JSON.stringify([...s]));
  } catch {
    /* private window: the choice lasts for this session only */
  }
}
const isInstantSpeed = (c: CardView) => c.types.includes("Instant") || c.keywords.includes("Flash");

// The engine writes "<name> casts X."; for the local player, whose seat is "You", say "You cast X."
const VERBS: Record<string, string> = {
  casts: "cast", plays: "play", mulligans: "mulligan", concedes: "concede", loses: "lose", wins: "win", goes: "go",
  draws: "draw", attacks: "attack", blocks: "block", activates: "activate", discards: "discard", keeps: "keep", gains: "gain",
};
function youGrammar(line: string): string {
  return line.replace(/^You (\w+)\b/, (m, v: string) => (VERBS[v] ? `You ${VERBS[v]}` : m));
}

export function Table({ session, onExit: exitTable }: { session: GameSession; onExit: () => void }) {
  const [view, setView] = useState<PlayerView>(() => session.view());
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const [hover, setHover] = useState<CardView | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [logOpen, setLogOpen] = useState(false);
  const [focus, setFocus] = useState<number | null>(null);
  const [attacks, setAttacks] = useState<Map<number, number>>(new Map());
  const [blocks, setBlocks] = useState<Map<number, number>>(new Map());
  const [pickedBlocker, setPickedBlocker] = useState<number | null>(null);
  const [endTurn, setEndTurn] = useState<number | null>(null);
  const [fullControl, setFullControl] = useState(false);
  const [thinking, setThinking] = useState<number | null>(null);
  const [fast, setFast] = useState(false);
  const [inspect, setInspect] = useState<CardView | null>(null);
  const [layout, setLayout] = useState<"table" | "focus">(() => (view.players.length > 2 ? "table" : "focus"));
  const [banner, setBanner] = useState<{ key: number; phase: number; who: string } | null>(null);
  const [stackOpen, setStackOpen] = useState(false);
  const [points, setPoints] = useState<RewindPoint[]>([]);
  const [quiet, setQuiet] = useState<Set<string>>(loadQuiet);
  const [castPop, setCastPop] = useState<{ id: number; closed: boolean } | null>(null);
  // Your own battlefield, arranged the way you like: card ids per row, in order.
  const [arranged, setArranged] = useState<Partial<Record<Slot, number[]>>>({});
  // Your hand, in the order you've dragged it into. New cards join on the right.
  const [handOrder, setHandOrder] = useState<number[]>([]);
  // Graveyards: hover a pile to peek, click to keep it open. Opens by itself when a spell
  // you're casting can target a card in one.
  const [gyOpen, setGyOpen] = useState<number | null>(null);
  const [gyPeek, setGyPeek] = useState<number | null>(null);
  const settings = useSettings();
  const toggleQuiet = (name: string) => {
    const next = new Set(quiet);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    saveQuiet(next);
    setQuiet(next);
  };
  const onExit = () => {
    session.leave?.();
    exitTable();
  };
  // Online games change when other players move.
  useEffect(() => session.subscribe?.((err) => {
    setView(session.view());
    if (err) setToast(err);
  }), [session]);

  // Sound: play whatever changed since the last view.
  const heard = useRef<PlayerView>(view);
  useEffect(() => {
    play("shuffle");
  }, []);
  useEffect(() => {
    if (heard.current !== view) playChanges(heard.current, view);
    heard.current = view;
  }, [view]);

  const me = view.players[view.you];
  const opponents = view.players.filter((p) => !p.is_you);
  const focused = opponents.find((p) => p.id === focus && !p.lost) ?? opponents.find((p) => !p.lost) ?? opponents[0];
  const kind = decisionKind(view.decision);
  const myPriority = view.your_decision && kind === "Priority";

  const act = useCallback(
    (a: Action) => {
      const err = session.submit(a);
      if (err) {
        setToast(err.replace(/^illegal: /, ""));
        play("error");
      }
      setMode({ kind: "idle" });
      setView(session.view());
    },
    [session],
  );

  // Computer players: decide, "think" for a human-like moment, then play.
  useEffect(() => {
    if (view.your_decision || kind === "GameOver") {
      setThinking(null);
      return;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const step = () => {
      if (cancelled) return;
      // Instant passes happen in a batch so the table doesn't crawl through empty steps.
      let applied = false;
      for (let i = 0; i < 200; i++) {
        const next = session.nextBot();
        if (!next || next.think_ms > 0) {
          if (applied) {
            // Show what changed first; this effect runs again for the next move.
            setView(session.view());
            return;
          }
          if (!next) {
            setThinking(null);
            return;
          }
          setThinking(next.player);
          timer = setTimeout(() => {
            if (cancelled) return;
            session.applyBot();
            setThinking(null);
            setView(session.view());
          }, fast ? Math.min(next.think_ms, 350) : next.think_ms);
          return;
        }
        session.applyBot();
        applied = true;
      }
      setView(session.view());
    };
    timer = setTimeout(step, 30);
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [view, kind, session, fast]);

  // Space opens the stack: see everything waiting to resolve, hold priority, and (with the house
  // rule on) step back to something you let go and respond after all.
  useEffect(() => {
    setPoints(session.rewindEnabled ? session.rewindPoints() : []);
  }, [view, session]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable)) return;
      if (e.code === "Space") {
        e.preventDefault();
        if (e.repeat) return;
        (document.activeElement as HTMLElement | null)?.blur?.();
        setStackOpen((o) => !o);
      } else if (e.code === "Escape") setStackOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const goBack = (p: RewindPoint) => {
    const err = session.rewind(p.index);
    if (err) {
      setToast(err);
      return;
    }
    setMode({ kind: "idle" });
    setStackOpen(true);
    const v = session.view();
    lastPhase.current = `${v.turn}:${phaseOf(v.step)}`;
    setView(v);
    setToast(`Back to ${p.name}. Respond now, or let it resolve.`);
  };

  // Watch whoever is taking their turn.
  useEffect(() => {
    if (view.active !== view.you) setFocus(view.active);
  }, [view.active, view.you]);

  // Announce each new phase with a short banner.
  const phase = phaseOf(view.step);
  const lastPhase = useRef<string>("");
  useEffect(() => {
    const k = `${view.turn}:${phase}`;
    if (view.turn === 0 || k === lastPhase.current || phase < 0) return;
    lastPhase.current = k;
    const key = Date.now();
    setBanner({ key, phase, who: view.players[view.active].name });
    const t = setTimeout(() => setBanner((b) => (b?.key === key ? null : b)), 1100);
    return () => clearTimeout(t);
  }, [view.turn, phase, view.active, view.players]);

  // Cards that just arrived in hand fly in from the library.
  const seenHand = useRef<Set<number> | null>(null);
  const drawn = new Set<number>();
  if (seenHand.current) for (const c of view.hand) if (!seenHand.current.has(c.id)) drawn.add(c.id);
  useEffect(() => {
    seenHand.current = new Set(view.hand.map((c) => c.id));
  }, [view.hand]);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 3200);
    return () => clearTimeout(t);
  }, [toast]);

  // Reset combat selections when the decision changes.
  useEffect(() => {
    setAttacks(new Map());
    setBlocks(new Map());
    setPickedBlocker(null);
  }, [kind, view.turn]);

  // Auto-pass. Unless "Full control" is on, only stop when there's a real decision:
  // your main phases when you can do something, a spell of someone else's you could answer,
  // combat you're part of, and an opponent's end step, if you hold something you could cast.
  const usableAbility = view.hints.activatable.some(([id, i]) => {
    const c = view.battlefield.find((b) => b.id === id);
    return c?.abilities.find((a) => a.index === i && !a.is_mana);
  });
  const canAct = view.hints.castable.length > 0 || usableAbility;
  // Responding outside your main phase: cards you've silenced (right-click) don't count.
  const canRespond =
    usableAbility ||
    view.hints.castable.some((id) => {
      const c = view.hand.find((h) => h.id === id) ?? me.command.find((h) => h.id === id);
      return !c || !quiet.has(c.name);
    });
  const meaningfulStop = (() => {
    if (view.stack.length > 0) return view.stack[0].controller !== view.you && canRespond;
    const mine = view.active === view.you;
    if (mine && (view.step === "Main1" || view.step === "Main2")) return canAct || view.hints.playable_lands.length > 0;
    if (!canRespond) return false;
    const inCombat = view.battlefield.some((c) => c.attacking !== null && (c.attacking === view.you || c.controller === view.you));
    if ((view.step === "DeclareAttackers" || view.step === "DeclareBlockers") && inCombat) return true;
    return !mine && view.step === "End";
  })();
  // Someone else casts a spell you could answer: show it big for a moment.
  const top = view.stack[0];
  useEffect(() => {
    if (!top || top.controller === view.you || !top.card || top.is_ability || !canRespond) return;
    setCastPop((p) => (p?.id === top.id ? p : { id: top.id, closed: false }));
    const id = top.id;
    const t = setTimeout(() => setCastPop((p) => (p?.id === id ? { id, closed: true } : p)), 4500);
    return () => clearTimeout(t);
  }, [top?.id, top?.controller, view.you, canRespond]); // eslint-disable-line react-hooks/exhaustive-deps
  const popItem = castPop && !castPop.closed ? view.stack.find((x) => x.id === castPop.id && x.card) : undefined;

  useEffect(() => {
    if (mode.kind !== "idle" || stackOpen) return;
    const ending = endTurn !== null && endTurn === view.turn;
    if (endTurn !== null && endTurn !== view.turn) setEndTurn(null);
    let auto: Action | null = null;
    if (myPriority) {
      if (ending && view.stack.length === 0) auto = "Pass";
      else if (!fullControl && !meaningfulStop) auto = "Pass";
    } else if (view.your_decision && kind === "DeclareAttackers" && (ending || (!fullControl && view.hints.attackers.length === 0))) {
      auto = { Attack: [] };
    } else if (view.your_decision && kind === "DeclareBlockers" && !fullControl && view.hints.blockers.length === 0) {
      auto = { Block: [] };
    }
    if (!auto) return;
    const t = setTimeout(() => act(auto!), 120);
    return () => clearTimeout(t);
  }, [view, mode, endTurn, fullControl, meaningfulStop, myPriority, kind, act, stackOpen]);

  // ---------------------------------------------------------------- targeting
  // Choosing a target never commits on its own: pick (click, or drag onto it), then confirm.
  // Clicking a chosen target again takes it back.
  const currentSlot: Target[] | null = mode.kind === "target" ? mode.slots[mode.chosen.length] ?? null : null;
  const isTargetable = (t: Target) => !!currentSlot?.some((x) => sameTarget(x, t));
  const isChosen = (t: Target) => mode.kind === "target" && mode.chosen.some((x) => sameTarget(x, t));
  const pickable = (t: Target) => isTargetable(t) || isChosen(t);
  const targetsReady = mode.kind === "target" && mode.chosen.length === mode.slots.length;
  // Players whose graveyards hold something the current choice can target.
  const gyTargetOwners = currentSlot
    ? view.players.filter((p) => p.graveyard.some((c) => isTargetable({ Object: c.id }))).map((p) => p.id)
    : [];
  const [gyDismissed, setGyDismissed] = useState(false);
  const gyKey = gyTargetOwners.join(",");
  useEffect(() => {
    setGyDismissed(false);
    if (gyKey) setGyOpen((cur) => (cur !== null && gyKey.split(",").map(Number).includes(cur) ? cur : Number(gyKey.split(",").find((id) => Number(id) === view.you) ?? gyKey.split(",")[0])));
    else setGyOpen(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gyKey]);
  const gyShown = gyOpen !== null && !(gyDismissed && gyKey) ? gyOpen : gyPeek;
  const chooseTarget = (t: Target) => {
    if (mode.kind !== "target") return;
    const i = mode.chosen.findIndex((x) => sameTarget(x, t));
    if (i >= 0) setMode({ ...mode, chosen: mode.chosen.slice(0, i) });
    else if (isTargetable(t)) setMode({ ...mode, chosen: [...mode.chosen, t] });
  };
  const confirmTargets = () => {
    if (mode.kind === "target" && targetsReady) act(mode.build(mode.chosen));
  };
  const resetTargets = () => {
    if (mode.kind !== "target") return;
    if (mode.cancellable) setMode({ kind: "idle" });
    else setMode({ ...mode, chosen: [] });
  };

  const beginCast = (c: CardView, first?: Target) => {
    const slots = view.hints.cast_targets[c.id] ?? [];
    let x = 0;
    if (c.has_x) {
      const pool = Object.values(me.mana_pool).reduce((a, b) => a + b, 0);
      const answer = window.prompt(`Choose X for ${c.name}`, String(Math.max(0, pool - c.mana_value)));
      if (answer === null) return;
      x = Math.max(0, parseInt(answer, 10) || 0);
    }
    if (slots.length === 0) act({ Cast: { card: c.id, targets: [], x } });
    else {
      const chosen = first && slots[0]?.some((o) => sameTarget(o, first)) ? [first] : [];
      setMode({ kind: "target", title: `${c.name}: choose ${c.spell_targets[0] ?? "a target"}`, slots, chosen, source: c.id, cancellable: true, build: (t) => ({ Cast: { card: c.id, targets: t, x } }) });
    }
  };

  const activate = (c: CardView, index: number) => {
    const ab = c.abilities.find((a) => a.index === index)!;
    if (ab.is_mana) {
      let color: string | null = null;
      if (ab.label.includes("commander's color identity")) {
        const opts = [...me.color_identity];
        if (opts.length === 0) return;
        color = opts.length === 1 ? opts[0] : window.prompt(`Which color? ${opts.join(", ")}`, opts[0])?.toUpperCase().slice(0, 1) ?? null;
        if (!color) return;
      } else if (ab.label.includes("any color")) {
        color = window.prompt("Which color? W, U, B, R or G", me.color_identity[0] ?? "G")?.toUpperCase().slice(0, 1) ?? null;
        if (!color) return;
      } else if (ab.label.includes(" or ")) {
        const opts = ab.label.match(/\{([WUBRG])\}/g)?.map((s) => s[1]) ?? [];
        color = window.prompt(`Which color? ${opts.join(" or ")}`, opts[0])?.toUpperCase().slice(0, 1) ?? null;
        if (!color) return;
      }
      act({ ActivateMana: { object: c.id, ability: index, color } });
      return;
    }
    const slots = view.hints.ability_targets[`${c.id}:${index}`] ?? [];
    if (slots.length === 0) act({ Activate: { object: c.id, ability: index, targets: [], x: 0 } });
    else setMode({ kind: "target", title: `${c.name}: choose ${ab.targets[0]}`, slots, chosen: [], source: c.id, cancellable: true, build: (t) => ({ Activate: { object: c.id, ability: index, targets: t, x: 0 } }) });
  };

  // Triggered abilities: one with targets goes straight to choosing on the table; several are
  // ordered first. One without targets just goes on the stack.
  const trigs = view.your_decision && kind === "PutTriggersOnStack" ? view.hints.triggers : [];
  const startTriggers = useCallback(
    (order: number[]) => {
      const list = order.map((i) => view.hints.triggers.find((t) => t.index === i)!);
      const slots = list.flatMap((t) => t.targets);
      if (slots.length === 0) {
        act({ OrderTriggers: order.map((i) => [i, []]) });
        return;
      }
      const titles = list.flatMap((t) => t.targets.map((_, k) => `${t.source}: ${t.targets.length > 1 ? `choose target ${k + 1} of ${t.targets.length}` : "choose a target"}`));
      const withTargets = list.find((t) => t.targets.length > 0)!;
      const src = view.battlefield.find((c) => c.controller === view.you && c.name === withTargets.source)
        ?? view.battlefield.find((c) => c.name === withTargets.source);
      setMode({
        kind: "target",
        title: titles[0],
        titles,
        slots,
        chosen: [],
        source: src?.id,
        cancellable: false,
        build: (chosen) => {
          let k = 0;
          return { OrderTriggers: list.map((t) => [t.index, t.targets.map(() => chosen[k++])] as [number, Target[]]) };
        },
      });
    },
    [view, act],
  );
  const startedTrigger = useRef<string>("");
  const triggerKey = trigs.length === 1 ? `${view.turn}:${view.log.length}:${trigs[0].index}:${trigs[0].source}` : "";
  useEffect(() => {
    if (trigs.length !== 1 || mode.kind !== "idle") return;
    // Once per trigger, so a refused choice can't loop (the list below takes over).
    const k = triggerKey;
    if (startedTrigger.current === k) return;
    startedTrigger.current = k;
    startTriggers([trigs[0].index]);
  }, [trigs, mode.kind, startTriggers, triggerKey]);

  // ---------------------------------------------------------------- clicks
  const clickHand = (c: CardView) => {
    if (mode.kind === "target") return;
    if (view.hints.playable_lands.includes(c.id)) act({ PlayLand: c.id });
    else if (view.hints.castable.includes(c.id)) beginCast(c);
  };

  // Which attackers a blocker may block (a creature without flying or reach can't block a flyer).
  const canBlock = (blocker: number, attacker: number) => {
    const opts = view.hints.block_options?.find(([b]) => b === blocker);
    return !opts || opts[1].includes(attacker);
  };
  const whyNoBlock = (blocker: number, attacker: number) => {
    const b = view.battlefield.find((c) => c.id === blocker);
    const a = view.battlefield.find((c) => c.id === attacker);
    return `${b?.name ?? "That creature"} can't block ${a?.name ?? "it"}: ${a?.keywords.includes("Flying") ? "it has flying, so only creatures with flying or reach can block it" : "that block isn't allowed"}.`;
  };

  const clickPermanent = (c: CardView) => {
    if (mode.kind === "target") {
      chooseTarget({ Object: c.id });
      return;
    }
    if (view.hints.undo_mana.includes(c.id)) {
      act({ UndoMana: c.id });
      return;
    }
    if (kind === "DeclareAttackers" && view.your_decision && view.hints.attackers.includes(c.id)) {
      const next = new Map(attacks);
      if (next.has(c.id)) next.delete(c.id);
      else next.set(c.id, focused.id);
      setAttacks(next);
      return;
    }
    if (kind === "DeclareBlockers" && view.your_decision) {
      if (view.hints.blockers.includes(c.id)) {
        if (blocks.has(c.id)) {
          const next = new Map(blocks);
          next.delete(c.id);
          setBlocks(next);
        } else setPickedBlocker(c.id === pickedBlocker ? null : c.id);
        return;
      }
      if (pickedBlocker !== null && c.attacking === view.you) {
        if (!canBlock(pickedBlocker, c.id)) {
          setToast(whyNoBlock(pickedBlocker, c.id));
          play("error");
          return;
        }
        setBlocks(new Map(blocks).set(pickedBlocker, c.id));
        setPickedBlocker(null);
      }
      return;
    }
    const usable = c.abilities.filter((a) => a.activatable);
    if (usable.length === 1) activate(c, usable[0].index);
    else if (usable.length > 1) setMode({ kind: "abilities", card: c });
  };

  const glowFor = (c: CardView): "play" | "target" | "attack" | "block" | "selected" | null => {
    if (mode.kind === "target") return isChosen({ Object: c.id }) ? "selected" : isTargetable({ Object: c.id }) ? "target" : null;
    if (attacks.has(c.id)) return "attack";
    if (blocks.has(c.id)) return "block";
    if (pickedBlocker === c.id) return "selected";
    if (view.your_decision && kind === "DeclareAttackers" && view.hints.attackers.includes(c.id)) return "play";
    if (view.your_decision && kind === "DeclareBlockers" && view.hints.blockers.includes(c.id)) return "play";
    if (view.your_decision && kind === "DeclareBlockers" && pickedBlocker !== null && c.attacking === view.you) return canBlock(pickedBlocker, c.id) ? "target" : null;
    if (c.abilities.some((a) => a.activatable && !a.is_mana)) return "play";
    return null;
  };

  // ---------------------------------------------------------------- dragging
  // Press on something you can use and drag: a card from hand onto the table (or straight onto
  // its target), an attacker onto a player, a blocker onto an attacker. A short press is a click.
  type DragKind = "hand" | "attack" | "block" | "arrange";
  const [drag, setDrag] = useState<{ kind: DragKind; id: number } | null>(null);
  const press = useRef<{ kind: DragKind; id: number; x: number; y: number } | null>(null);
  const swallowClick = useRef(false);

  const dragKindFor = (id: number): DragKind | null => {
    if (view.your_decision) {
      if (kind === "DeclareAttackers" && view.hints.attackers.includes(id)) return "attack";
      if (kind === "DeclareBlockers" && view.hints.blockers.includes(id)) return "block";
    }
    // Any card in hand can be dragged: to play it, or to a new spot in your hand.
    if (mode.kind === "idle" && view.hand.some((c) => c.id === id)) return "hand";
    // Your own permanents can always be moved around your side of the table.
    if (mode.kind === "idle" && view.battlefield.some((c) => c.id === id && c.controller === view.you)) return "arrange";
    return null;
  };
  // Drop a permanent somewhere on your side: into the row under the pointer, before the card
  // under it (or at the end).
  const arrangeDrop = (id: number, el: Element | null) => {
    const slot = el?.closest(".board-me [data-slot]")?.getAttribute("data-slot") as Slot | null | undefined;
    if (!slot) return;
    const next: Partial<Record<Slot, number[]>> = {};
    for (const k of Object.keys(mySlots) as Slot[]) next[k] = mySlots[k].map((c) => c.id).filter((x) => x !== id);
    const list = next[slot]!;
    const over = objAt(el);
    let at = list.length;
    if (over && list.includes(over.id)) {
      const r = over.el.getBoundingClientRect();
      at = list.indexOf(over.id) + (pointerX.current > r.left + r.width / 2 ? 1 : 0);
    }
    list.splice(at, 0, id);
    setArranged(next);
  };
  const pointerX = useRef(0);
  const objAt = (el: Element | null) => {
    const hit = el?.closest("[data-obj]");
    return hit ? { el: hit, id: Number(hit.getAttribute("data-obj")) } : null;
  };
  const playerAt = (el: Element | null) => {
    const hit = el?.closest("[data-player]");
    return hit ? { el: hit, id: Number(hit.getAttribute("data-player")) } : null;
  };
  // Where a drag would land, given what's under the pointer.
  const dropTarget = (k: DragKind, id: number, el: Element | null): { el: Element; target: Target } | null => {
    if (k === "attack") {
      const p = playerAt(el);
      return p && p.id !== view.you && !view.players[p.id]?.lost ? { el: anchorEl({ player: p.id }) ?? p.el, target: { Player: p.id } } : null;
    }
    if (k === "arrange") return null;
    if (k === "block") {
      const o = objAt(el);
      const att = o && view.battlefield.find((c) => c.id === o.id && c.attacking === view.you);
      return o && att && canBlock(id, o.id) ? { el: o.el, target: { Object: o.id } } : null;
    }
    const first = view.hints.cast_targets[id]?.[0];
    if (!first) return null;
    const o = objAt(el);
    if (o && o.id !== id && first.some((t) => sameTarget(t, { Object: o.id }))) return { el: o.el, target: { Object: o.id } };
    const p = playerAt(el);
    if (p && first.some((t) => sameTarget(t, { Player: p.id }))) return { el: anchorEl({ player: p.id }) ?? p.el, target: { Player: p.id } };
    return null;
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const o = objAt(e.target as Element);
    const k = o ? dragKindFor(o.id) : null;
    press.current = o && k ? { kind: k, id: o.id, x: e.clientX, y: e.clientY } : null;
  };
  useEffect(() => {
    const move = (e: PointerEvent) => {
      pointerX.current = e.clientX;
      const p = press.current;
      if (!p || drag) return;
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) > 8) {
        setDrag({ kind: p.kind, id: p.id });
        setHover(null);
      }
    };
    const up = () => {
      const p = press.current;
      press.current = null;
      if (!drag || !p) return;
      setDrag(null);
      swallowClick.current = true;
      setTimeout(() => (swallowClick.current = false), 0);
      const el = elementAtPointer();
      const hit = dropTarget(drag.kind, drag.id, el);
      if (drag.kind === "arrange") {
        arrangeDrop(drag.id, el);
      } else if (drag.kind === "attack") {
        if (hit && "Player" in hit.target) setAttacks(new Map(attacks).set(drag.id, hit.target.Player));
      } else if (drag.kind === "block") {
        if (hit && "Object" in hit.target) {
          setBlocks(new Map(blocks).set(drag.id, hit.target.Object));
          setPickedBlocker(null);
        } else {
          const over = objAt(el);
          if (over && view.battlefield.some((c) => c.id === over.id && c.attacking === view.you) && !canBlock(drag.id, over.id)) {
            setToast(whyNoBlock(drag.id, over.id));
            play("error");
          }
        }
      } else {
        // Dropped back on the hand: move it there.
        if (!hit && el?.closest(".dock")) {
          const others = hand.filter((h) => h.id !== drag.id);
          const els = others.map((h) => document.querySelector(`.dock .hand [data-obj="${h.id}"]`));
          let at = others.length;
          for (let i = 0; i < others.length; i++) {
            const r = els[i]?.getBoundingClientRect();
            if (r && pointerX.current < r.left + r.width / 2) {
              at = i;
              break;
            }
          }
          const ids = others.map((h) => h.id);
          ids.splice(at, 0, drag.id);
          setHandOrder(ids);
          return;
        }
        const c = view.hand.find((h) => h.id === drag.id);
        if (!c) return;
        if (!view.your_decision || !(view.hints.playable_lands.includes(c.id) || view.hints.castable.includes(c.id))) return;
        if (view.hints.playable_lands.includes(c.id)) act({ PlayLand: c.id });
        else beginCast(c, hit?.target);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  });

  // Arrows: what's attacking whom, blocks, chosen targets, and the one following the pointer.
  const arrows: ArrowSpec[] = [];
  for (const c of view.battlefield) {
    if (c.attacking !== null && !attacks.has(c.id)) arrows.push({ key: `in${c.id}`, from: { obj: c.id }, to: { player: c.attacking }, tone: "incoming" });
  }
  // Blocks that have been declared, by anyone: a shield-colored arrow from blocker to attacker.
  for (const c of view.battlefield) {
    if (c.blocking !== null && !blocks.has(c.id)) arrows.push({ key: `bk${c.id}`, from: { obj: c.id }, to: { obj: c.blocking }, tone: "block" });
  }
  for (const [a, p] of attacks) arrows.push({ key: `at${a}`, from: { obj: a }, to: { player: p }, tone: "attack" });
  for (const [b, a] of blocks) arrows.push({ key: `bl${b}`, from: { obj: b }, to: { obj: a }, tone: "block" });
  if (mode.kind === "target" && mode.source !== undefined) {
    mode.chosen.forEach((t, i) => arrows.push({ key: `t${i}`, from: { obj: mode.source! }, to: "Player" in t ? { player: t.Player } : { obj: t.Object }, tone: "target" }));
    if (!targetsReady) arrows.push({ key: "tc", from: { obj: mode.source }, to: "cursor", tone: "drag" });
  }
  if (pickedBlocker !== null && !drag) arrows.push({ key: "pb", from: { obj: pickedBlocker }, to: "cursor", tone: "drag" });
  if (drag && drag.kind !== "arrange") arrows.push({ key: "drag", from: { obj: drag.id }, to: "cursor", tone: "drag" });
  const snap = (el: Element | null): Element | null => {
    if (drag) return dropTarget(drag.kind, drag.id, el)?.el ?? null;
    if (pickedBlocker !== null) return dropTarget("block", pickedBlocker, el)?.el ?? null;
    if (mode.kind === "target" && !targetsReady) {
      const o = objAt(el);
      if (o && isTargetable({ Object: o.id })) return anchorEl({ obj: o.id });
      const p = playerAt(el);
      if (p && isTargetable({ Player: p.id })) return anchorEl({ player: p.id });
    }
    return null;
  };
  const dragCard = drag?.kind === "hand" ? view.hand.find((c) => c.id === drag.id)
    : drag?.kind === "arrange" ? view.battlefield.find((c) => c.id === drag.id) : undefined;

  // ---------------------------------------------------------------- layout helpers
  const rowsFor = (pid: number) => {
    const mine = view.battlefield.filter((c) => c.controller === pid);
    const lands = mine.filter((c) => c.types.includes("Land") && !c.types.includes("Creature"));
    const creatures = mine.filter((c) => c.types.includes("Creature"));
    const other = mine.filter((c) => !lands.includes(c) && !creatures.includes(c));
    return { lands, front: creatures, support: other };
  };
  // Your own side: cards you've placed keep their spot; new ones join their usual place at the end.
  const mySlots = (() => {
    const out = {} as Record<Slot, CardView[]>;
    for (const r of ROWS) for (const c of COLS) out[`${r}-${c}`] = [];
    const cards = view.battlefield.filter((c) => c.controller === view.you);
    const byId = new Map(cards.map((c) => [c.id, c]));
    const placed = new Set<number>();
    for (const k of Object.keys(out) as Slot[]) {
      for (const id of arranged[k] ?? []) {
        const c = byId.get(id);
        if (c && !placed.has(id)) {
          out[k].push(c);
          placed.add(id);
        }
      }
    }
    for (const c of cards) if (!placed.has(c.id)) out[defaultSlot(c)].push(c);
    return out;
  })();
  const midUsed = COLS.some((c) => mySlots[`mid-${c}`].length > 0);
  const tagFor = (c: CardView) => {
    if (view.hints.undo_mana.includes(c.id) && mode.kind === "idle") return "↺ click to untap";
    if (attacks.has(c.id)) return `→ ${view.players[attacks.get(c.id)!].name}`;
    if (blocks.has(c.id)) return `blocks ${view.battlefield.find((b) => b.id === blocks.get(c.id))?.name ?? ""}`;
    if (c.blocking !== null) return `⛨ blocks ${view.battlefield.find((b) => b.id === c.blocking)?.name ?? ""}`;
    if (c.attacking !== null) {
      const by = view.battlefield.filter((b) => b.blocking === c.id);
      if (by.length) return `⛨ blocked by ${by.map((b) => b.name).join(", ")}`;
      if (c.controller === view.you && (view.step === "DeclareBlockers" || view.step === "FirstStrikeDamage" || view.step === "CombatDamage")) return "unblocked";
    }
    if (c.attacking !== null && c.controller !== view.you) return `→ ${view.players[c.attacking].name}`;
    return null;
  };
  const blockedIds = new Set(view.battlefield.filter((c) => c.blocking !== null).map((c) => c.blocking!));
  const combatCls = (c: CardView) => (c.blocking !== null ? "is-blocking" : blockedIds.has(c.id) ? "is-blocked" : "");
  const renderRow = (cards: CardView[], cls: string) => (
    <div className={`row ${cls}`}>
      {cards.map((c) => (
        <Card key={c.id} card={c} size="field" className={combatCls(c)} glow={glowFor(c)} onClick={() => clickPermanent(c)} onHover={setHover} onInspect={setInspect} badge={tagFor(c)} />
      ))}
    </div>
  );

  const hand = (() => {
    const byId = new Map(view.hand.map((c) => [c.id, c]));
    const kept = handOrder.map((id) => byId.get(id)).filter((c): c is CardView => !!c);
    const known = new Set(handOrder);
    return [...kept, ...view.hand.filter((c) => !known.has(c.id))];
  })();
  const opp = rowsFor(focused.id);
  const myCommand = me.command;

  // ---------------------------------------------------------------- primary button
  let primary: { label: string; action: () => void; tone?: string } | null = null;
  if (view.your_decision) {
    if (kind === "Priority") {
      primary = view.stack.length > 0 ? { label: "Resolve", action: () => act("Pass") } : { label: view.active === view.you && view.step === "Main2" ? "End turn" : "Next", action: () => act("Pass") };
    } else if (kind === "DeclareAttackers") {
      primary = { label: attacks.size ? `Attack with ${attacks.size}` : "No attacks", tone: attacks.size ? "danger" : undefined, action: () => act({ Attack: [...attacks.entries()] }) };
    } else if (kind === "DeclareBlockers") {
      primary = { label: blocks.size ? `Confirm ${blocks.size} block${blocks.size > 1 ? "s" : ""}` : "No blocks", action: () => act({ Block: [...blocks.entries()] }) };
    }
  }

  const status = (() => {
    if (mode.kind === "target") {
      if (targetsReady) return `${mode.title.split(":")[0]}: confirm, or click a target to change it.`;
      return (mode.titles?.[mode.chosen.length] ?? mode.title) + " (click it, or drag onto it)";
    }
    if (!view.your_decision) {
      const p = Object.values(view.decision)[0] as { player?: number };
      return p.player !== undefined ? `Waiting for ${view.players[p.player].name}…` : "";
    }
    if (kind === "DeclareAttackers") return "Drag a creature onto the player to attack, or click it to attack " + focused.name + ".";
    if (kind === "DeclareBlockers") return pickedBlocker !== null ? "Now pick the attacker to block." : "Drag a blocker onto the attacker it blocks.";
    if (kind === "AssignCombatDamage") return "Divide your attacker's damage among its blockers.";
    if (kind === "CommanderZone") return "Choose where your commander goes.";
    if (kind === "Priority" && view.stack.length > 0) return "You can respond, or let the top of the stack resolve.";
    return view.active === view.you ? "Your turn" : `${view.players[view.active].name}'s turn`;
  })();

  // When choosing targets, bring an opponent with legal targets into view.
  useEffect(() => {
    if (mode.kind !== "target" || !currentSlot) return;
    const onFocused = view.battlefield.some((c) => c.controller === focused.id && isTargetable({ Object: c.id }));
    if (onFocused) return;
    const other = opponents.find((p) => view.battlefield.some((c) => c.controller === p.id && isTargetable({ Object: c.id })));
    if (other) setFocus(other.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  // The most recent thing that happened, in words (zone bookkeeping lines are skipped).
  const latest = (() => {
    for (let i = view.log.length - 1; i >= 0 && i >= view.log.length - 12; i--) {
      if (!view.log[i].includes(" -> ")) return { i, line: view.log[i] };
    }
    return null;
  })();

  // The hover preview sits just left of the middle of the table (not at the far edge of a wide
  // screen) and hops to the right when the pointer is where it would appear.
  const previewSide = () => {
    const x = pointerX.current / uiScale();
    const mid = window.innerWidth / uiScale() / 2;
    return x > mid - 830 && x < mid - 420 ? "right" : "";
  };

  const topIsTarget = (t: Target) => mode.kind === "target" && isTargetable(t);
  const playerCls = (id: number) => (topIsTarget({ Player: id }) ? "glow-target" : isChosen({ Player: id }) ? "glow-chosen" : "");

  return (
    <div className={`table ${view.active === view.you ? "my-turn" : "their-turn"} ${drag ? `dragging drag-${drag.kind}` : ""}`}
      onPointerDown={onPointerDown}
      onPointerMove={(e) => (pointerX.current = e.clientX)}
      onClickCapture={(e) => {
        if (swallowClick.current) {
          e.stopPropagation();
          e.preventDefault();
          swallowClick.current = false;
        }
      }}>
      {/* Opponent seats */}
      <header className="seats">
        {opponents.map((p) => (
          <button key={p.id} className={`seat ${p.id === focused.id ? "focused" : ""} ${p.id !== focused.id && view.battlefield.some((c) => c.controller === p.id && isTargetable({ Object: c.id })) ? "has-targets" : ""} ${p.is_active ? "active" : ""} ${p.lost ? "lost" : ""} ${thinking === p.id ? "thinking" : ""} ${playerCls(p.id)}`} data-player={p.id}
            onClick={() => (pickable({ Player: p.id }) ? chooseTarget({ Player: p.id }) : setFocus(p.id))}>
            <Sigil seat={p} size={28} />
            <span className="seat-name">{p.name}</span>
            <LifePill life={p.life} />
            <span className="seat-meta">{thinking === p.id ? <span className="thinking-dots">thinking</span> : <><HandPill count={p.hand_count} /> · {p.library_count} in library</>}</span>
            {me.commander_damage[p.command[0]?.name ?? ""] !== undefined && (
              <span className="seat-cmd" title="Commander damage you've taken from them">⚔ {me.commander_damage[p.command[0]?.name ?? ""]}</span>
            )}
          </button>
        ))}
        <div className="seat-tools">
          <label className="toggle"><input type="checkbox" checked={fullControl} onChange={(e) => setFullControl(e.target.checked)} /> Full control</label>
          <label className="toggle" title="Computer players take less time to think"><input type="checkbox" checked={fast} onChange={(e) => setFast(e.target.checked)} /> Fast opponents</label>
          {opponents.length > 1 && (
            <button className="ghost view-toggle" onClick={() => setLayout(layout === "table" ? "focus" : "table")} title="Switch between seeing every opponent and one at a time">
              {layout === "table" ? "One opponent" : "All opponents"}
            </button>
          )}
          <button className={`ghost stack-key ${stackOpen ? "on" : ""}`} onClick={() => setStackOpen(!stackOpen)} title="See the stack and hold priority (Space)">
            Stack{view.stack.length > 0 ? ` ${view.stack.length}` : ""} <kbd>Space</kbd>
          </button>
          <button className="ghost" onClick={() => setLogOpen(!logOpen)} title="Game log"><ScrollIcon /></button>
          <button className="ghost" onClick={onExit}>Leave</button>
        </div>
      </header>

      {/* Opponents: everyone at once, or one at a time */}
      {layout === "table" && opponents.length > 1 ? (
        <section className={`board-opps n${opponents.length}`}>
          {opponents.map((p) => {
            const r = rowsFor(p.id);
            return (
              <div key={p.id} className={`opp-panel theme-surface ${p.is_active ? "is-active" : ""} ${playerCls(p.id)} ${p.lost ? "lost" : ""}`}
                data-theme={themeForSeat(p.id)} data-player={p.id}
                onClick={() => (pickable({ Player: p.id }) ? chooseTarget({ Player: p.id }) : setFocus(p.id))}>
                <div className="opp-panel-head">
                  <Sigil seat={p} size={30} />
                  <div className="opp-id">
                    <div className="opp-name-row">
                      <span className="seat-name">{p.name}</span>
                      <LifePill life={p.life} big />
                      <HandPill count={p.hand_count} big />
                      {p.is_active && <span className="turn-chip">Their turn</span>}
                      {p.id === focused.id && kind === "DeclareAttackers" && view.your_decision && <span className="muted small">attack target</span>}
                    </div>
                    <div className="seat-meta">{thinking === p.id ? <span className="thinking-dots">thinking</span> : <>{p.library_count} library · <button className="gy-link" onClick={(e) => { e.stopPropagation(); setGyOpen(gyOpen === p.id ? null : p.id); }} onMouseEnter={() => p.graveyard.length > 0 && setGyPeek(p.id)} onMouseLeave={() => setGyPeek(null)}>{p.graveyard.length} graveyard</button></>}</div>
                  </div>
                  <CommandZone cards={p.command} onHover={setHover} onInspect={setInspect} />
                </div>
                {renderRow([...r.lands, ...r.support], "row-back")}
                {renderRow(r.front, "row-front")}
              </div>
            );
          })}
        </section>
      ) : (
      <section className={`board board-opp theme-surface ${focused.is_active ? "is-active" : ""}`} data-theme={themeForSeat(focused.id)}>
        <div className="focus-id">
          <span className="seat-name">{focused.name}</span>
          <LifePill life={focused.life} big />
          <HandPill count={focused.hand_count} big />
          {focused.is_active && <span className="turn-chip">Their turn</span>}
        </div>
        <Medallion seat={focused} position="top" targetable={pickable({ Player: focused.id })} onClick={() => pickable({ Player: focused.id }) && chooseTarget({ Player: focused.id })} />
        <div className="opp-side">
          <div className={`pile ${backClass(settings)}`} style={backStyle(settings)} title="Library">{focused.library_count}</div>
          <div className="pile pile-gy clickable" title="Graveyard: hover to look, click to keep it open" data-player-gy={focused.id}
            onClick={() => setGyOpen(gyOpen === focused.id ? null : focused.id)} onMouseEnter={() => focused.graveyard.length > 0 && setGyPeek(focused.id)} onMouseLeave={() => setGyPeek(null)}>{focused.graveyard.length}</div>
          <CommandZone cards={focused.command} onHover={setHover} onInspect={setInspect} />
        </div>
        {renderRow([...opp.lands, ...opp.support], "row-back")}
        {renderRow(opp.front, "row-front")}
      </section>
      )}

      {/* Divider with phase track */}
      <div className="divider">
        <div className="turn-label">Turn {view.turn} · {view.players[view.active].name}</div>
        <div className="phase-track">
          {PHASES.map((ph) => (
            <span key={ph.label + (ph.steps.includes(view.step) ? view.turn : "")} className={`phase ${ph.steps.includes(view.step) ? "on" : ""}`}><ph.icon size={12} />{ph.label}</span>
          ))}
        </div>
        <div className="step-name">{STEP_NAMES[view.step]}</div>
        {latest && <div className="ticker" key={latest.i}>{youGrammar(latest.line)}</div>}
      </div>

      {/* My board */}
      <section className={`board board-me theme-surface ${midUsed || drag?.kind === "arrange" ? "three-rows" : ""}`} data-theme={settings.theme}>
        {ROWS.map((r) => (
          <div key={r} className={`row row-${r} row-slots`}>
            {COLS.map((c) => {
              const slot: Slot = `${r}-${c}`;
              return (
                <div key={c} className={`cell cell-${c}`} data-slot={slot}>
                  <span className="cell-label">{SLOT_NAMES[slot]}</span>
                  {mySlots[slot].map((card) => (
                    <Card key={card.id} card={card} size="field" className={combatCls(card)} glow={glowFor(card)} onClick={() => clickPermanent(card)} onHover={setHover} onInspect={setInspect} badge={tagFor(card)} />
                  ))}
                </div>
              );
            })}
          </div>
        ))}
      </section>

      {/* Bottom: command zone, hand, medallion, piles, action */}
      <footer className="dock">
        <div className="dock-left">
          <CommandZone cards={myCommand} mine castable={view.hints.castable} onCast={(c) => beginCast(c)} onHover={setHover} onInspect={setInspect} />
        </div>
        <div className="hand">
          {hand.map((c, i) => {
            const n = hand.length;
            const offset = i - (n - 1) / 2;
            const playable = view.hints.playable_lands.includes(c.id) || view.hints.castable.includes(c.id);
            return (
              <Card key={c.id} card={c} size="hand" glow={playable && mode.kind === "idle" ? "play" : mode.kind === "target" && mode.source === c.id ? "selected" : null}
                dim={mode.kind === "target" && mode.source !== c.id}
                onClick={() => clickHand(c)} onHover={drag ? undefined : setHover} onInspect={setInspect}
                className={[drawn.has(c.id) ? "drawn" : "", drag?.id === c.id ? "lifted" : "", isInstantSpeed(c) && quiet.has(c.name) ? "is-quiet" : ""].join(" ")}
                style={{ transform: `translateX(${offset * -18}px) rotate(${offset * 2.2}deg) translateY(${Math.abs(offset) * 4}px)`, zIndex: i }} />
            );
          })}
        </div>
        <Medallion seat={me} position="bottom" targetable={pickable({ Player: me.id })} onClick={() => pickable({ Player: me.id }) && chooseTarget({ Player: me.id })} />
        <div className="dock-right">
          <div className="piles">
            <div className={`pile ${backClass(settings)}`} style={backStyle(settings)} title="Library">{me.library_count}<small>Library</small></div>
            <div className="pile pile-gy clickable" title="Graveyard: hover to look, click to keep it open"
              onClick={() => setGyOpen(gyOpen === me.id ? null : me.id)} onMouseEnter={() => me.graveyard.length > 0 && setGyPeek(me.id)} onMouseLeave={() => setGyPeek(null)}>{me.graveyard.length}<small>Graveyard</small></div>
          </div>
          <div className="actions">
            <div className="status">{status}</div>
            {mode.kind === "target" ? (
              <>
                <button className={`btn btn-primary ${targetsReady ? "" : "is-waiting"}`} disabled={!targetsReady} onClick={confirmTargets}>Confirm</button>
                {(mode.cancellable || mode.chosen.length > 0) && (
                  <button className="btn btn-quiet" onClick={resetTargets}>{mode.cancellable ? "Cancel" : "Clear"}</button>
                )}
              </>
            ) : (
              primary && <button className={`btn btn-primary ${primary.tone ?? ""}`} onClick={primary.action}>{primary.label}</button>
            )}
            {myPriority && view.active === view.you && mode.kind === "idle" && primary?.label !== "End turn" && (
              <button className="btn btn-quiet" onClick={() => { setEndTurn(view.turn); act("Pass"); }}>End turn</button>
            )}
          </div>
        </div>
      </footer>

      {/* Stack */}
      {stackOpen ? (
        <aside className="stack stack-open" onClick={(e) => e.stopPropagation()}>
          <div className="stack-head">
            <div className="stack-title">Stack</div>
            <span className="muted small">{myPriority ? "You have priority" : "Holding: you'll get a chance to act"}</span>
            <button className="ghost" onClick={() => setStackOpen(false)} title="Close (Space)">✕</button>
          </div>
          {view.stack.length === 0 && <div className="stack-empty">Nothing is waiting to resolve.</div>}
          {view.stack.map((s, i) => (
            <div key={s.id} data-obj={s.id} className={`stack-item ${i === 0 ? "top" : ""} ${topIsTarget({ Object: s.id }) ? "glow-target clickable" : isChosen({ Object: s.id }) ? "glow-chosen clickable" : ""}`}
              onClick={() => pickable({ Object: s.id }) && chooseTarget({ Object: s.id })}
              onMouseEnter={() => s.card && setHover(s.card)} onMouseLeave={() => setHover(null)}>
              <div className="stack-name">
                <span>{i === 0 && <span className="stack-next">Next</span>}{s.name}{s.is_ability ? " — ability" : ""}</span>
                {s.card && <ManaCost cost={s.card.mana_cost} />}
              </div>
              <div className="stack-meta">{view.players[s.controller].name}{s.targets.length ? ` → ${s.targets.join(", ")}` : ""}</div>
              {(s.is_ability || !s.card) && <div className="stack-text">{s.text}</div>}
            </div>
          ))}
          {session.rewindEnabled && (
            <div className="rewind">
              <div className="rewind-title">Go back and respond</div>
              {points.length === 0 ? (
                <div className="muted small">When you let something on the stack go, it shows up here. You can step back to the last {2} and respond after all.</div>
              ) : (
                points.map((p, i) => (
                  <div key={`${p.turn}:${p.name}:${i}`} className="rewind-item">
                    <div>
                      <div className="stack-name"><span>{p.name}</span></div>
                      <div className="stack-meta">{view.players[p.controller].name} · turn {p.turn}{i === 0 ? " · most recent" : ""}</div>
                    </div>
                    <button className="btn" onClick={() => goBack(p)}>Go back</button>
                  </div>
                ))
              )}
            </div>
          )}
          <div className="stack-foot">
            {mode.kind === "idle" && primary && kind === "Priority" && (
              <button className="btn btn-primary" onClick={primary.action}>{primary.label}</button>
            )}
            <span className="muted small"><kbd>Space</kbd> to close</span>
          </div>
        </aside>
      ) : view.stack.length > 0 && (
        <aside className="stack">
          <div className="stack-title">Stack</div>
          {view.stack.map((s, i) => (
            <div key={s.id} data-obj={s.id} className={`stack-item ${i === 0 ? "top" : ""} ${topIsTarget({ Object: s.id }) ? "glow-target clickable" : isChosen({ Object: s.id }) ? "glow-chosen clickable" : ""}`}
              onClick={() => pickable({ Object: s.id }) && chooseTarget({ Object: s.id })}
              onMouseEnter={() => s.card && setHover(s.card)} onMouseLeave={() => setHover(null)}>
              <div className="stack-name">
                <span>{s.name}{s.is_ability ? " — ability" : ""}</span>
                {s.card && <ManaCost cost={s.card.mana_cost} />}
              </div>
              <div className="stack-meta">{view.players[s.controller].name}{s.targets.length ? ` → ${s.targets.join(", ")}` : ""}</div>
              {s.is_ability && <div className="stack-text">{s.text}</div>}
            </div>
          ))}
          <div className="stack-hint"><kbd>Space</kbd> for more</div>
        </aside>
      )}

      {banner && (() => {
        const ph = PHASES[banner.phase];
        return (
          <div className="phase-banner" key={banner.key}>
            <span className="ph-icon"><ph.icon size={20} /></span>
            <span className="ph-text">
              <span className="ph-title">{ph.label === "Upkeep" ? `${banner.who === "You" ? "Your" : `${banner.who}'s`} turn` : ph.label}</span>
              <span className="ph-sub">{ph.label === "Upkeep" ? `Turn ${view.turn}` : ph.sub}</span>
            </span>
          </div>
        );
      })()}
      <ArrowLayer arrows={arrows} snap={snap} ghost={dragCard && <Card card={dragCard} size="field" />} />
      {inspect && (
        <RulesPopup card={inspect} onClose={() => setInspect(null)}
          extra={isInstantSpeed(inspect) && inspect.controller === view.you ? (
            <label className="quiet-toggle">
              <input type="checkbox" checked={!quiet.has(inspect.name)} onChange={() => toggleQuiet(inspect.name)} />
              <span>
                <strong>Pause the game for {inspect.name}</strong>
                <span className="muted small">When off, holding this card won't stop the game at every step to let you respond. You can still cast it whenever the game stops.</span>
              </span>
            </label>
          ) : null} />
      )}
      {gyShown !== null && (() => {
        const owner = view.players[gyShown];
        const cards = [...owner.graveyard].reverse();
        const targeting = gyTargetOwners.includes(gyShown);
        return (
          <aside className={`gy-panel ${targeting ? "targeting" : ""}`} onMouseEnter={() => setGyPeek(gyShown)} onMouseLeave={() => setGyPeek(null)}>
            <div className="gy-head">
              <span className="gy-title">{owner.is_you ? "Your graveyard" : `${owner.name}'s graveyard`}</span>
              <span className="muted small">{cards.length} card{cards.length === 1 ? "" : "s"}, newest first</span>
              {gyTargetOwners.length > 1 && gyTargetOwners.map((id) => (
                <button key={id} className={`ghost small ${id === gyShown ? "on" : ""}`} onClick={() => setGyOpen(id)}>{view.players[id].name}</button>
              ))}
              <button className="ghost" onClick={() => { setGyOpen(null); setGyPeek(null); setGyDismissed(true); }} title="Close">✕</button>
            </div>
            {targeting && <div className="gy-hint">Click a glowing card to choose it.</div>}
            <div className="gy-cards">
              {cards.length === 0 && <span className="muted small">Empty.</span>}
              {cards.map((c) => (
                <Card key={c.id} card={c} size="field"
                  glow={isChosen({ Object: c.id }) ? "selected" : isTargetable({ Object: c.id }) ? "target" : null}
                  dim={mode.kind === "target" && !pickable({ Object: c.id })}
                  onClick={pickable({ Object: c.id }) ? () => chooseTarget({ Object: c.id }) : undefined}
                  onHover={setHover} onInspect={setInspect} />
              ))}
            </div>
          </aside>
        );
      })()}
      {popItem && popItem.card && (
        <div className="cast-pop" key={popItem.id} onClick={() => setCastPop({ id: popItem.id, closed: true })} title="Click to dismiss">
          <div className="cast-pop-head">
            <span className="cast-pop-who">{view.players[popItem.controller].name}</span> {popItem.controller === view.you ? "cast" : "casts"}
            {popItem.targets.length > 0 && <span className="muted"> targeting {popItem.targets.join(", ")}</span>}
          </div>
          <CardPreview card={popItem.card} />
          <div className="cast-pop-foot muted small">You can respond before it resolves.</div>
        </div>
      )}

      {/* Hover preview */}
      {hover && !drag && <div className={`preview-wrap ${previewSide()}`}><CardPreview card={hover} /></div>}

      {/* Ability menu */}
      {mode.kind === "abilities" && (
        <div className="scrim" onClick={() => setMode({ kind: "idle" })}>
          <div className="menu" onClick={(e) => e.stopPropagation()}>
            <div className="menu-title">{mode.card.name}</div>
            {mode.card.abilities.filter((a) => a.activatable).map((a) => (
              <button key={a.index} className="menu-item" onClick={() => activate(mode.card, a.index)}>
                <ManaCost cost={a.label.split(":")[0]} /> <span>{a.label.split(": ").slice(1).join(": ")}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Log */}
      {logOpen && (
        <aside className="log">
          <div className="log-title">Game log</div>
          <div className="log-body">{[...view.log].reverse().map((l, i) => <div key={i}>{youGrammar(l)}</div>)}</div>
        </aside>
      )}

      {kind === "Mulligan" && view.your_decision && <MulliganModal view={view} act={act} onHover={setHover} />}
      {(trigs.length > 1 || (trigs.length === 1 && startedTrigger.current === triggerKey)) && mode.kind === "idle" && <TriggerModal view={view} onDone={startTriggers} />}
      {kind === "AssignCombatDamage" && view.your_decision && <DamageModal key={JSON.stringify(view.decision)} view={view} act={act} onHover={setHover} />}
      {kind === "CommanderZone" && view.your_decision && <CommanderModal view={view} act={act} />}
      {kind === "Discard" && view.your_decision && <DiscardModal view={view} act={act} onHover={setHover} />}
      {kind === "GameOver" && <GameOverModal view={view} onExit={onExit} label={session.exitLabel} />}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
