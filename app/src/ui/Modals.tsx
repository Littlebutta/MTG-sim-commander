import { useState } from "react";
import type { Action, AssignDamageDecision, CardView, PlayerView, Target } from "../engine/types";
import { Card } from "./Card";

type Act = (a: Action) => void;
type Hover = (c: CardView | null) => void;

export function MulliganModal({ view, act, onHover }: { view: PlayerView; act: Act; onHover: Hover }) {
  const need = view.hints.mulligan_bottom;
  const [bottom, setBottom] = useState<number[]>([]);
  const toggle = (id: number) =>
    setBottom((b) => (b.includes(id) ? b.filter((x) => x !== id) : b.length < need ? [...b, id] : b));
  return (
    <div className="scrim">
      <div className="modal wide">
        <h2>Opening hand</h2>
        <p className="muted">
          {need > 0 ? `Keep, then choose ${need} card${need > 1 ? "s" : ""} to put on the bottom.` : "Your first mulligan in a multiplayer game is free."}
        </p>
        <div className="modal-cards">
          {view.hand.map((c) => (
            <Card key={c.id} card={c} size="hand" glow={bottom.includes(c.id) ? "selected" : null} onClick={need ? () => toggle(c.id) : undefined} onHover={onHover} />
          ))}
        </div>
        <div className="modal-actions">
          <button className="btn" onClick={() => act("Mulligan")}>Mulligan</button>
          <button className="btn btn-primary" disabled={bottom.length !== need} onClick={() => act({ Keep: { bottom } })}>Keep</button>
        </div>
      </div>
    </div>
  );
}


/** Order several triggers at once. Their targets are chosen on the table afterwards. */
export function TriggerModal({ view, onDone }: { view: PlayerView; onDone: (order: number[]) => void }) {
  const trigs = view.hints.triggers;
  const [order, setOrder] = useState<number[]>(trigs.map((t) => t.index));
  const move = (i: number, d: number) => {
    const o = [...order];
    const j = i + d;
    if (j < 0 || j >= o.length) return;
    [o[i], o[j]] = [o[j], o[i]];
    setOrder(o);
  };
  const needTargets = trigs.some((t) => t.targets.length > 0);
  return (
    <div className="scrim">
      <div className="modal">
        <h2>Your triggered abilities</h2>
        <p className="muted">Order them on the stack. The top one resolves first.</p>
        <ol className="trigger-list">
          {[...order].reverse().map((idx, ri) => {
            const t = trigs.find((x) => x.index === idx)!;
            const i = order.length - 1 - ri;
            return (
              <li key={idx}>
                <div>
                  <strong>{t.source}</strong> <span className="muted">{t.text}</span>
                </div>
                <span className="order-btns">
                  <button className="ghost" onClick={() => move(i, 1)} title="Resolve sooner">▲</button>
                  <button className="ghost" onClick={() => move(i, -1)} title="Resolve later">▼</button>
                </span>
              </li>
            );
          })}
        </ol>
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={() => onDone(order)}>{needTargets ? "Choose targets" : "Put on stack"}</button>
        </div>
      </div>
    </div>
  );
}

/** Divide a blocked attacker's damage among its blockers (and past them, with trample). */
export function DamageModal({ view, act, onHover }: { view: PlayerView; act: Act; onHover: Hover }) {
  const d = (view.decision as { AssignCombatDamage: AssignDamageDecision }).AssignCombatDamage;
  const start = () => {
    const m: Record<string, number> = {};
    for (const [b] of d.blockers) m[`o${b}`] = 0;
    if (d.trample) m.p = 0;
    for (const [t, n] of d.suggested) m["Object" in t ? `o${t.Object}` : "p"] = n;
    return m;
  };
  const [amt, setAmt] = useState<Record<string, number>>(start);
  const card = (id: number) => view.battlefield.find((c) => c.id === id);
  const attacker = card(d.attacker);
  const used = Object.values(amt).reduce((a, b) => a + b, 0);
  const left = d.power - used;
  const allLethal = d.blockers.every(([b, l]) => amt[`o${b}`] >= l);
  const problem =
    left > 0 ? `${left} damage still to assign.` :
    d.trample && amt.p > 0 && !allLethal ? "Trample: give every blocker lethal damage before any goes to the player." :
    null;
  const bump = (k: string, by: number) =>
    setAmt((m) => {
      const v = m[k] + by;
      if (v < 0 || (by > 0 && left <= 0)) return m;
      return { ...m, [k]: v };
    });
  const submit = () => {
    const split: [Target, number][] = [];
    for (const [b] of d.blockers) if (amt[`o${b}`] > 0) split.push([{ Object: b }, amt[`o${b}`]]);
    if (d.trample && amt.p > 0) split.push([{ Player: d.defender }, amt.p]);
    act({ AssignDamage: split });
  };
  const stepper = (k: string) => (
    <div className="dmg-stepper">
      <button className="ghost" onClick={() => bump(k, -1)} disabled={amt[k] <= 0} aria-label="Less">−</button>
      <span className="dmg-amount">{amt[k]}</span>
      <button className="ghost" onClick={() => bump(k, 1)} disabled={left <= 0} aria-label="More">+</button>
    </div>
  );
  return (
    <div className="scrim">
      <div className="modal wide">
        <h2>Assign combat damage</h2>
        <p className="muted">
          {attacker?.name ?? "Your attacker"} deals {d.power} damage. Divide it among the creatures blocking it
          {d.trample ? "; with trample, anything past lethal for every blocker can go to the player" : ""}.
          {d.deathtouch ? " Deathtouch makes 1 damage lethal." : ""}
        </p>
        <div className="modal-cards dmg-cards">
          {d.blockers.map(([b, lethal]) => {
            const c = card(b);
            const k = `o${b}`;
            return (
              <div key={b} className={`dmg-slot${amt[k] >= lethal && lethal > 0 ? " lethal" : ""}`}>
                {c && <Card card={c} size="hand" onHover={onHover} />}
                <div className="dmg-note">{lethal > 0 ? `${lethal} is lethal` : "already doomed"}</div>
                {stepper(k)}
              </div>
            );
          })}
          {d.trample && (
            <div className={`dmg-slot player${!allLethal ? " locked" : ""}`}>
              <div className="dmg-player">{view.players[d.defender]?.name ?? "Player"}</div>
              <div className="dmg-note">Tramples over</div>
              {stepper("p")}
            </div>
          )}
        </div>
        <div className={problem ? "form-error" : "muted dmg-ok"}>{problem ?? "Ready."}</div>
        <div className="modal-actions">
          <button className="btn" onClick={() => setAmt(start())}>Suggested split</button>
          <button className="btn btn-primary" disabled={!!problem} onClick={submit}>Deal damage</button>
        </div>
      </div>
    </div>
  );
}

export function DiscardModal({ view, act, onHover }: { view: PlayerView; act: Act; onHover: Hover }) {
  const need = view.hints.discard_count;
  const [picked, setPicked] = useState<number[]>([]);
  return (
    <div className="scrim">
      <div className="modal wide">
        <h2>Discard to hand size</h2>
        <p className="muted">Choose {need} card{need > 1 ? "s" : ""} to discard.</p>
        <div className="modal-cards">
          {view.hand.map((c) => (
            <Card key={c.id} card={c} size="hand" glow={picked.includes(c.id) ? "selected" : null} onHover={onHover}
              onClick={() => setPicked((p) => (p.includes(c.id) ? p.filter((x) => x !== c.id) : p.length < need ? [...p, c.id] : p))} />
          ))}
        </div>
        <div className="modal-actions">
          <button className="btn btn-primary" disabled={picked.length !== need} onClick={() => act({ Discard: picked })}>Discard</button>
        </div>
      </div>
    </div>
  );
}

export function GameOverModal({ view, onExit }: { view: PlayerView; onExit: () => void }) {
  const d = view.decision as { GameOver: { winner: number | null } };
  const w = d.GameOver.winner;
  const won = w === view.you;
  return (
    <div className="scrim">
      <div className="modal">
        <h2 className={won ? "win" : ""}>{won ? "Victory" : w === null ? "Draw" : `${view.players[w].name} wins`}</h2>
        <p className="muted">Turn {view.turn}</p>
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={onExit}>Back to menu</button>
        </div>
      </div>
    </div>
  );
}
