import { useCallback, useEffect, useRef, useState } from "react";
import type { GameSession, RewindPoint } from "../engine/session";
import type { Action, CardView, PlayerView, SeatView, Step, Target } from "../engine/types";
import { decisionKind, sameTarget } from "../engine/types";
import { themeForSeat, useSettings, type Settings } from "../settings";
import { Card, CardPreview, ManaCost, RulesPopup } from "./Card";
import { CrownIcon, ScrollIcon, ShieldIcon, SwordIcon } from "./icons";
import { DiscardModal, GameOverModal, MulliganModal, TriggerModal } from "./Modals";

type Mode =
  | { kind: "idle" }
  | { kind: "target"; title: string; slots: Target[][]; chosen: Target[]; build: (t: Target[]) => Action }
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
    <div className={`medallion medallion-${position} ${targetable ? "glow-target clickable" : ""} ${seat.has_priority ? "has-priority" : ""}`} onClick={onClick}>
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

// The engine writes "<name> casts X."; for the local player, whose seat is "You", say "You cast X."
const VERBS: Record<string, string> = {
  casts: "cast", plays: "play", mulligans: "mulligan", concedes: "concede", loses: "lose", wins: "win", goes: "go",
  draws: "draw", attacks: "attack", blocks: "block", activates: "activate", discards: "discard", keeps: "keep", gains: "gain",
};
function youGrammar(line: string): string {
  return line.replace(/^You (\w+)\b/, (m, v: string) => (VERBS[v] ? `You ${VERBS[v]}` : m));
}

export function Table({ session, onExit }: { session: GameSession; onExit: () => void }) {
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
  const settings = useSettings();

  const me = view.players[view.you];
  const opponents = view.players.filter((p) => !p.is_you);
  const focused = opponents.find((p) => p.id === focus && !p.lost) ?? opponents.find((p) => !p.lost) ?? opponents[0];
  const kind = decisionKind(view.decision);
  const myPriority = view.your_decision && kind === "Priority";

  const act = useCallback(
    (a: Action) => {
      const err = session.submit(a);
      if (err) setToast(err.replace(/^illegal: /, ""));
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
  const canAct =
    view.hints.castable.length > 0 ||
    view.hints.activatable.some(([id, i]) => {
      const c = view.battlefield.find((b) => b.id === id);
      return c?.abilities.find((a) => a.index === i && !a.is_mana);
    });
  const meaningfulStop = (() => {
    if (view.stack.length > 0) return view.stack[0].controller !== view.you && canAct;
    const mine = view.active === view.you;
    if (mine && (view.step === "Main1" || view.step === "Main2")) return canAct || view.hints.playable_lands.length > 0;
    if (!canAct) return false;
    const inCombat = view.battlefield.some((c) => c.attacking !== null && (c.attacking === view.you || c.controller === view.you));
    if ((view.step === "DeclareAttackers" || view.step === "DeclareBlockers") && inCombat) return true;
    return !mine && view.step === "End";
  })();
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
  const currentSlot: Target[] | null = mode.kind === "target" ? mode.slots[mode.chosen.length] ?? null : null;
  const isTargetable = (t: Target) => !!currentSlot?.some((x) => sameTarget(x, t));
  const chooseTarget = (t: Target) => {
    if (mode.kind !== "target") return;
    const chosen = [...mode.chosen, t];
    if (chosen.length === mode.slots.length) act(mode.build(chosen));
    else setMode({ ...mode, chosen });
  };

  const beginCast = (c: CardView) => {
    const slots = view.hints.cast_targets[c.id] ?? [];
    let x = 0;
    if (c.has_x) {
      const pool = Object.values(me.mana_pool).reduce((a, b) => a + b, 0);
      const answer = window.prompt(`Choose X for ${c.name}`, String(Math.max(0, pool - c.mana_value)));
      if (answer === null) return;
      x = Math.max(0, parseInt(answer, 10) || 0);
    }
    if (slots.length === 0) act({ Cast: { card: c.id, targets: [], x } });
    else setMode({ kind: "target", title: `${c.name}: choose ${c.spell_targets[0] ?? "a target"}`, slots, chosen: [], build: (t) => ({ Cast: { card: c.id, targets: t, x } }) });
  };

  const activate = (c: CardView, index: number) => {
    const ab = c.abilities.find((a) => a.index === index)!;
    if (ab.is_mana) {
      let color: string | null = null;
      if (ab.label.includes("any color")) {
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
    else setMode({ kind: "target", title: `${c.name}: choose ${ab.targets[0]}`, slots, chosen: [], build: (t) => ({ Activate: { object: c.id, ability: index, targets: t, x: 0 } }) });
  };

  // ---------------------------------------------------------------- clicks
  const clickHand = (c: CardView) => {
    if (mode.kind === "target") return;
    if (view.hints.playable_lands.includes(c.id)) act({ PlayLand: c.id });
    else if (view.hints.castable.includes(c.id)) beginCast(c);
  };

  const clickPermanent = (c: CardView) => {
    if (mode.kind === "target") {
      if (isTargetable({ Object: c.id })) chooseTarget({ Object: c.id });
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
    if (mode.kind === "target") return isTargetable({ Object: c.id }) ? "target" : null;
    if (attacks.has(c.id)) return "attack";
    if (blocks.has(c.id)) return "block";
    if (pickedBlocker === c.id) return "selected";
    if (view.your_decision && kind === "DeclareAttackers" && view.hints.attackers.includes(c.id)) return "play";
    if (view.your_decision && kind === "DeclareBlockers" && view.hints.blockers.includes(c.id)) return "play";
    if (view.your_decision && kind === "DeclareBlockers" && pickedBlocker !== null && c.attacking === view.you) return "target";
    if (c.abilities.some((a) => a.activatable && !a.is_mana)) return "play";
    return null;
  };

  // ---------------------------------------------------------------- layout helpers
  const rowsFor = (pid: number) => {
    const mine = view.battlefield.filter((c) => c.controller === pid);
    const lands = mine.filter((c) => c.types.includes("Land") && !c.types.includes("Creature"));
    const creatures = mine.filter((c) => c.types.includes("Creature"));
    const other = mine.filter((c) => !lands.includes(c) && !creatures.includes(c));
    return { lands, front: creatures, support: other };
  };
  const tagFor = (c: CardView) => {
    if (attacks.has(c.id)) return `→ ${view.players[attacks.get(c.id)!].name}`;
    if (blocks.has(c.id)) return `blocks ${view.battlefield.find((b) => b.id === blocks.get(c.id))?.name ?? ""}`;
    if (c.attacking !== null && c.controller !== view.you) return `→ ${view.players[c.attacking].name}`;
    return null;
  };
  const renderRow = (cards: CardView[], cls: string) => (
    <div className={`row ${cls}`}>
      {cards.map((c) => (
        <Card key={c.id} card={c} size="field" glow={glowFor(c)} onClick={() => clickPermanent(c)} onHover={setHover} onInspect={setInspect} badge={tagFor(c)} />
      ))}
    </div>
  );

  const opp = rowsFor(focused.id);
  const mine = rowsFor(view.you);
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
    if (mode.kind === "target") return mode.title;
    if (!view.your_decision) {
      const p = Object.values(view.decision)[0] as { player?: number };
      return p.player !== undefined ? `Waiting for ${view.players[p.player].name}…` : "";
    }
    if (kind === "DeclareAttackers") return `Choose attackers. They'll attack ${focused.name} (pick another opponent at the top to switch).`;
    if (kind === "DeclareBlockers") return pickedBlocker !== null ? "Now pick the attacker to block." : "Choose a blocker, then the attacker it blocks.";
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

  const topIsTarget = (t: Target) => mode.kind === "target" && isTargetable(t);

  return (
    <div className={`table ${view.active === view.you ? "my-turn" : "their-turn"}`}>
      {/* Opponent seats */}
      <header className="seats">
        {opponents.map((p) => (
          <button key={p.id} className={`seat ${p.id === focused.id ? "focused" : ""} ${p.id !== focused.id && view.battlefield.some((c) => c.controller === p.id && isTargetable({ Object: c.id })) ? "has-targets" : ""} ${p.is_active ? "active" : ""} ${p.lost ? "lost" : ""} ${thinking === p.id ? "thinking" : ""} ${topIsTarget({ Player: p.id }) ? "glow-target" : ""}`}
            onClick={() => (topIsTarget({ Player: p.id }) ? chooseTarget({ Player: p.id }) : setFocus(p.id))}>
            <Sigil seat={p} size={28} />
            <span className="seat-name">{p.name}</span>
            <span className="seat-life">{p.life}</span>
            <span className="seat-meta">{thinking === p.id ? <span className="thinking-dots">thinking</span> : `${p.hand_count} in hand · ${p.library_count} in library`}</span>
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
              <div key={p.id} className={`opp-panel theme-surface ${p.is_active ? "is-active" : ""} ${topIsTarget({ Player: p.id }) ? "glow-target" : ""} ${p.lost ? "lost" : ""}`}
                data-theme={themeForSeat(p.id)}
                onClick={() => (topIsTarget({ Player: p.id }) ? chooseTarget({ Player: p.id }) : setFocus(p.id))}>
                <div className="opp-panel-head">
                  <Sigil seat={p} size={30} />
                  <div>
                    <div className="seat-name">{p.name}{p.id === focused.id && kind === "DeclareAttackers" && view.your_decision ? " · attack target" : ""}</div>
                    <div className="seat-meta">{thinking === p.id ? <span className="thinking-dots">thinking</span> : <><span className="bump" key={p.hand_count}>{p.hand_count}</span> in hand · {p.library_count} library · {p.graveyard.length} graveyard</>}</div>
                  </div>
                  <div className="opp-cmd">{p.command.map((c) => <Card key={c.id} card={c} size="mini" onHover={setHover} onInspect={setInspect} />)}</div>
                  <span className="seat-life">{p.life}</span>
                </div>
                {renderRow([...r.lands, ...r.support], "row-back")}
                {renderRow(r.front, "row-front")}
              </div>
            );
          })}
        </section>
      ) : (
      <section className="board board-opp theme-surface" data-theme={themeForSeat(focused.id)}>
        <Medallion seat={focused} position="top" targetable={topIsTarget({ Player: focused.id })} onClick={() => topIsTarget({ Player: focused.id }) && chooseTarget({ Player: focused.id })} />
        <div className="opp-side">
          <div className={`pile ${backClass(settings)}`} style={backStyle(settings)} title="Library">{focused.library_count}</div>
          <div className="pile pile-gy" title="Graveyard">{focused.graveyard.length}</div>
          {focused.command.map((c) => <Card key={c.id} card={c} size="mini" onHover={setHover} onInspect={setInspect} />)}
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
      <section className="board board-me theme-surface" data-theme={settings.theme}>
        {renderRow(mine.front, "row-front")}
        {renderRow([...mine.lands, ...mine.support], "row-back")}
      </section>

      {/* Bottom: command zone, hand, medallion, piles, action */}
      <footer className="dock">
        <div className="dock-left">
          {myCommand.map((c) => (
            <div key={c.id} className="command-slot">
              <Card card={c} size="mini" glow={view.hints.castable.includes(c.id) ? "play" : null} onClick={() => view.hints.castable.includes(c.id) && beginCast(c)} onHover={setHover} onInspect={setInspect} />
              <span className="slot-label">Command</span>
            </div>
          ))}
        </div>
        <div className="hand">
          {view.hand.map((c, i) => {
            const n = view.hand.length;
            const offset = i - (n - 1) / 2;
            const playable = view.hints.playable_lands.includes(c.id) || view.hints.castable.includes(c.id);
            return (
              <Card key={c.id} card={c} size="hand" glow={playable && mode.kind === "idle" ? "play" : null} dim={mode.kind === "target"}
                onClick={() => clickHand(c)} onHover={setHover} onInspect={setInspect} className={drawn.has(c.id) ? "drawn" : undefined}
                style={{ transform: `translateX(${offset * -18}px) rotate(${offset * 2.2}deg) translateY(${Math.abs(offset) * 4}px)`, zIndex: i }} />
            );
          })}
        </div>
        <Medallion seat={me} position="bottom" targetable={topIsTarget({ Player: me.id })} onClick={() => topIsTarget({ Player: me.id }) && chooseTarget({ Player: me.id })} />
        <div className="dock-right">
          <div className="piles">
            <div className={`pile ${backClass(settings)}`} style={backStyle(settings)} title="Library">{me.library_count}<small>Library</small></div>
            <div className="pile pile-gy" title="Graveyard">{me.graveyard.length}<small>Graveyard</small></div>
          </div>
          <div className="actions">
            <div className="status">{status}</div>
            {mode.kind === "target" ? (
              <button className="btn" onClick={() => setMode({ kind: "idle" })}>Cancel</button>
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
            <div key={s.id} className={`stack-item ${i === 0 ? "top" : ""} ${topIsTarget({ Object: s.id }) ? "glow-target clickable" : ""}`}
              onClick={() => topIsTarget({ Object: s.id }) && chooseTarget({ Object: s.id })}
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
            <div key={s.id} className={`stack-item ${i === 0 ? "top" : ""} ${topIsTarget({ Object: s.id }) ? "glow-target clickable" : ""}`}
              onClick={() => topIsTarget({ Object: s.id }) && chooseTarget({ Object: s.id })}
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
      {inspect && <RulesPopup card={inspect} onClose={() => setInspect(null)} />}

      {/* Hover preview */}
      {hover && <div className="preview-wrap"><CardPreview card={hover} /></div>}

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
      {kind === "PutTriggersOnStack" && view.your_decision && <TriggerModal view={view} act={act} />}
      {kind === "Discard" && view.your_decision && <DiscardModal view={view} act={act} onHover={setHover} />}
      {kind === "GameOver" && <GameOverModal view={view} onExit={onExit} />}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
