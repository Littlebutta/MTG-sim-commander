import { useCallback, useEffect, useState } from "react";
import type { GameSession } from "../engine/session";
import type { Action, CardView, PlayerView, SeatView, Step, Target } from "../engine/types";
import { decisionKind, sameTarget } from "../engine/types";
import { Card, CardPreview, ManaCost } from "./Card";
import { ScrollIcon } from "./icons";
import { DiscardModal, GameOverModal, MulliganModal, TriggerModal } from "./Modals";

type Mode =
  | { kind: "idle" }
  | { kind: "target"; title: string; slots: Target[][]; chosen: Target[]; build: (t: Target[]) => Action }
  | { kind: "abilities"; card: CardView };

const PHASES: { label: string; steps: Step[] }[] = [
  { label: "Upkeep", steps: ["Untap", "Upkeep"] },
  { label: "Draw", steps: ["Draw"] },
  { label: "Main", steps: ["Main1"] },
  { label: "Combat", steps: ["BeginCombat", "DeclareAttackers", "DeclareBlockers", "FirstStrikeDamage", "CombatDamage", "EndCombat"] },
  { label: "Main 2", steps: ["Main2"] },
  { label: "End", steps: ["End", "Cleanup"] },
];

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

  // Auto-pass: when "End turn" is on, or when there's nothing meaningful to do and full control is off.
  const nothingToDo =
    view.hints.castable.length === 0 &&
    view.hints.playable_lands.length === 0 &&
    !view.hints.activatable.some(([id, i]) => {
      const c = view.battlefield.find((b) => b.id === id);
      return c?.abilities.find((a) => a.index === i && !a.is_mana);
    });
  useEffect(() => {
    if (mode.kind !== "idle") return;
    const ending = endTurn !== null && endTurn === view.turn;
    if (endTurn !== null && endTurn !== view.turn) setEndTurn(null);
    let auto: Action | null = null;
    if (myPriority) {
      if (ending && view.stack.length === 0) auto = "Pass";
      else if (!fullControl && nothingToDo && !(view.active === view.you && (view.step === "Main1" || view.step === "Main2") && view.stack.length === 0)) auto = "Pass";
    } else if (view.your_decision && kind === "DeclareAttackers" && ending) {
      auto = { Attack: [] };
    }
    if (!auto) return;
    const t = setTimeout(() => act(auto!), 260);
    return () => clearTimeout(t);
  }, [view, mode, endTurn, fullControl, nothingToDo, myPriority, kind, act]);

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
        <Card key={c.id} card={c} size="field" glow={glowFor(c)} onClick={() => clickPermanent(c)} onHover={setHover} badge={tagFor(c)} />
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

  const topIsTarget = (t: Target) => mode.kind === "target" && isTargetable(t);

  return (
    <div className={`table ${view.active === view.you ? "my-turn" : "their-turn"}`}>
      {/* Opponent seats */}
      <header className="seats">
        {opponents.map((p) => (
          <button key={p.id} className={`seat ${p.id === focused.id ? "focused" : ""} ${p.id !== focused.id && view.battlefield.some((c) => c.controller === p.id && isTargetable({ Object: c.id })) ? "has-targets" : ""} ${p.is_active ? "active" : ""} ${p.lost ? "lost" : ""} ${topIsTarget({ Player: p.id }) ? "glow-target" : ""}`}
            onClick={() => (topIsTarget({ Player: p.id }) ? chooseTarget({ Player: p.id }) : setFocus(p.id))}>
            <Sigil seat={p} size={28} />
            <span className="seat-name">{p.name}</span>
            <span className="seat-life">{p.life}</span>
            <span className="seat-meta">{p.hand_count} in hand · {p.library_count} in library</span>
            {me.commander_damage[p.command[0]?.name ?? ""] !== undefined && (
              <span className="seat-cmd" title="Commander damage you've taken from them">⚔ {me.commander_damage[p.command[0]?.name ?? ""]}</span>
            )}
          </button>
        ))}
        <div className="seat-tools">
          <label className="toggle"><input type="checkbox" checked={fullControl} onChange={(e) => setFullControl(e.target.checked)} /> Full control</label>
          <button className="ghost" onClick={() => setLogOpen(!logOpen)} title="Game log"><ScrollIcon /></button>
          <button className="ghost" onClick={onExit}>Leave</button>
        </div>
      </header>

      {/* Opponent board */}
      <section className="board board-opp">
        <Medallion seat={focused} position="top" targetable={topIsTarget({ Player: focused.id })} onClick={() => topIsTarget({ Player: focused.id }) && chooseTarget({ Player: focused.id })} />
        <div className="opp-side">
          <div className="pile" title="Library">{focused.library_count}</div>
          <div className="pile pile-gy" title="Graveyard">{focused.graveyard.length}</div>
          {focused.command.map((c) => <Card key={c.id} card={c} size="mini" onHover={setHover} />)}
        </div>
        {renderRow([...opp.lands, ...opp.support], "row-back")}
        {renderRow(opp.front, "row-front")}
      </section>

      {/* Divider with phase track */}
      <div className="divider">
        <div className="turn-label">Turn {view.turn} · {view.players[view.active].name}</div>
        <div className="phase-track">
          {PHASES.map((ph) => (
            <span key={ph.label} className={`phase ${ph.steps.includes(view.step) ? "on" : ""}`}>{ph.label}</span>
          ))}
        </div>
        <div className="step-name">{STEP_NAMES[view.step]}</div>
      </div>

      {/* My board */}
      <section className="board board-me">
        {renderRow(mine.front, "row-front")}
        {renderRow([...mine.lands, ...mine.support], "row-back")}
      </section>

      {/* Bottom: command zone, hand, medallion, piles, action */}
      <footer className="dock">
        <div className="dock-left">
          {myCommand.map((c) => (
            <div key={c.id} className="command-slot">
              <Card card={c} size="mini" glow={view.hints.castable.includes(c.id) ? "play" : null} onClick={() => view.hints.castable.includes(c.id) && beginCast(c)} onHover={setHover} />
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
                onClick={() => clickHand(c)} onHover={setHover}
                style={{ transform: `translateX(${offset * -18}px) rotate(${offset * 2.2}deg) translateY(${Math.abs(offset) * 4}px)`, zIndex: i }} />
            );
          })}
        </div>
        <Medallion seat={me} position="bottom" targetable={topIsTarget({ Player: me.id })} onClick={() => topIsTarget({ Player: me.id }) && chooseTarget({ Player: me.id })} />
        <div className="dock-right">
          <div className="piles">
            <div className="pile" title="Library">{me.library_count}<small>Library</small></div>
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
      {view.stack.length > 0 && (
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
        </aside>
      )}

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
          <div className="log-body">{[...view.log].reverse().map((l, i) => <div key={i}>{l}</div>)}</div>
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
