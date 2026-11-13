import { useState } from "react";
import type { Action, CardView, PlayerView, Target } from "../engine/types";
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

function targetName(view: PlayerView, t: Target): string {
  if ("Player" in t) return view.players[t.Player].name;
  return view.battlefield.find((c) => c.id === t.Object)?.name ?? `#${t.Object}`;
}

export function TriggerModal({ view, act }: { view: PlayerView; act: Act }) {
  const trigs = view.hints.triggers;
  const [order, setOrder] = useState<number[]>(trigs.map((t) => t.index));
  const [targets, setTargets] = useState<Record<number, Target[]>>(() =>
    Object.fromEntries(trigs.map((t) => [t.index, t.targets.map((opts) => opts[0]).filter(Boolean)])),
  );
  const move = (i: number, d: number) => {
    const o = [...order];
    const j = i + d;
    if (j < 0 || j >= o.length) return;
    [o[i], o[j]] = [o[j], o[i]];
    setOrder(o);
  };
  return (
    <div className="scrim">
      <div className="modal">
        <h2>Your triggered abilities</h2>
        <p className="muted">Order them on the stack. The bottom one resolves last.</p>
        <ol className="trigger-list">
          {[...order].reverse().map((idx, ri) => {
            const t = trigs.find((x) => x.index === idx)!;
            const i = order.length - 1 - ri;
            return (
              <li key={idx}>
                <div>
                  <strong>{t.source}</strong> <span className="muted">{t.text}</span>
                  {t.targets.map((opts, slot) => (
                    <select key={slot} value={JSON.stringify(targets[idx]?.[slot] ?? null)}
                      onChange={(e) => {
                        const next = [...(targets[idx] ?? [])];
                        next[slot] = JSON.parse(e.target.value);
                        setTargets({ ...targets, [idx]: next });
                      }}>
                      {opts.map((o) => <option key={JSON.stringify(o)} value={JSON.stringify(o)}>{targetName(view, o)}</option>)}
                    </select>
                  ))}
                </div>
                <span className="order-btns">
                  <button className="ghost" onClick={() => move(i, 1)}>▲</button>
                  <button className="ghost" onClick={() => move(i, -1)}>▼</button>
                </span>
              </li>
            );
          })}
        </ol>
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={() => act({ OrderTriggers: order.map((i) => [i, targets[i] ?? []]) })}>Put on stack</button>
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
