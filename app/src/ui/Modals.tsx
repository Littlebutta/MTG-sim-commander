import { useState } from "react";
import type { Action, AssignDamageDecision, CardView, PlayerView, Target } from "../engine/types";
import { Card, CardPreview } from "./Card";

type Act = (a: Action) => void;
type Hover = (c: CardView | null) => void;

type Group = "Lands" | "Ramp" | "Spells";
const isLand = (c: CardView) => c.types.includes("Land");
/** Mana creatures, mana rocks and land-fetching spells. */
export const isRamp = (c: CardView) =>
  !isLand(c) && (/\badd (\{|one mana|two mana|three mana|mana)/i.test(c.text) || /search your library for (a|up to \w+) basic land/i.test(c.text) || /search your library for a land/i.test(c.text));
const groupOf = (c: CardView): Group => (isLand(c) ? "Lands" : isRamp(c) ? "Ramp" : "Spells");

/** A plain-English read on an opening hand, for new players. */
export function handAdvice(hand: CardView[], identity: string[], mulligans: number): { verdict: "keep" | "mull" | "maybe"; text: string } {
  const lands = hand.filter(isLand);
  const ramp = hand.filter(isRamp).length;
  const cheap = hand.filter((c) => !isLand(c) && c.mana_value <= 3).length;
  const n = lands.length;
  const colorsIn = new Set(lands.flatMap((c) => {
    const basic = ["Plains", "Island", "Swamp", "Mountain", "Forest"].findIndex((t) => c.subtypes.includes(t));
    if (basic >= 0) return ["WUBRG"[basic]];
    return (c.text.match(/\{([WUBRG])\}/g) ?? []).map((m) => m[1]);
  }));
  const missing = identity.filter((x) => !colorsIn.has(x));
  const after = mulligans > 0 ? " Each further mulligan costs you a card, so lean toward keeping." : " Your first mulligan is free in a multiplayer game.";
  if (n <= 1) return { verdict: "mull", text: `Only ${n === 0 ? "no lands" : "1 land"}. You'd likely miss land drops for turns. Mulligan.${mulligans > 1 ? " (With so few cards left, keeping can be right if you have cheap plays.)" : ""}` };
  if (n >= 6) return { verdict: "mull", text: `${n} lands and almost nothing to cast. This hand floods. Mulligan.` };
  if (n === 2 && ramp === 0 && cheap < 2) return { verdict: "mull", text: `2 lands, no ramp and little to do early. Risky: you need to draw lands fast.${after}` };
  if (n === 2) return { verdict: "maybe", text: `2 lands${ramp ? ` plus ${ramp} ramp` : ""} and ${cheap} cheap play${cheap === 1 ? "" : "s"}. Keepable, but you need to hit your next land drop.` };
  if (missing.length > 1 && identity.length > 1) return { verdict: "maybe", text: `${n} lands, but none make ${missing.join(" or ")}. Fine if your early cards don't need those colors.` };
  if (n === 5 && cheap === 0) return { verdict: "maybe", text: `5 lands and only expensive spells. Keepable, but slow.` };
  return { verdict: "keep", text: `${n} lands${ramp ? `, ${ramp} ramp` : ""} and ${cheap} cheap play${cheap === 1 ? "" : "s"}. A solid start. Keep.` };
}

export function MulliganModal({ view, act, onHover }: { view: PlayerView; act: Act; onHover: Hover }) {
  const need = view.hints.mulligan_bottom;
  const [bottom, setBottom] = useState<number[]>([]);
  const toggle = (id: number) =>
    setBottom((b) => (b.includes(id) ? b.filter((x) => x !== id) : b.length < need ? [...b, id] : b));
  const me = view.players[view.you];
  const advice = handAdvice(view.hand, me.color_identity, me.mulligans ?? 0);
  const groups: Group[] = ["Lands", "Ramp", "Spells"];
  return (
    <div className="scrim">
      <div className="modal wide mull-modal">
        <h2>Opening hand</h2>
        <p className="muted">
          {need > 0 ? `Keep, then choose ${need} card${need > 1 ? "s" : ""} to put on the bottom.` : "Hover a card to read it. Your first mulligan in a multiplayer game is free."}
        </p>
        <div className="mull-groups">
          {groups.map((g) => {
            const cards = view.hand.filter((c) => groupOf(c) === g);
            if (cards.length === 0) return null;
            return (
              <div key={g} className={`mull-group mull-${g.toLowerCase()}`}>
                <div className="mull-group-label">{g} <span>{cards.length}</span></div>
                <div className="modal-cards">
                  {cards.map((c) => (
                    <Card key={c.id} card={c} size="hand" glow={bottom.includes(c.id) ? "selected" : null} onClick={need ? () => toggle(c.id) : undefined} onHover={onHover} />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
        <div className={`mull-advice ${advice.verdict}`}>
          <span className="mull-advice-tag">{advice.verdict === "keep" ? "Suggestion: keep" : advice.verdict === "mull" ? "Suggestion: mulligan" : "Your call"}</span>
          <span>{advice.text}</span>
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

const ZONE_WORDS: Record<string, { where: string; stay: string; how: string }> = {
  Graveyard: { where: "your graveyard", stay: "Leave it in the graveyard", how: "was put into your graveyard" },
  Exile: { where: "exile", stay: "Leave it in exile", how: "was exiled" },
  Hand: { where: "your hand", stay: "Keep it in your hand", how: "was returned to your hand" },
  Library: { where: "your library", stay: "Leave it in your library", how: "was put into your library" },
};

/** Your commander changed zones: keep it where it went, or move it to the command zone. */
export function CommanderModal({ view, act }: { view: PlayerView; act: Act }) {
  const d = (view.decision as { CommanderZone: { zone: string } }).CommanderZone;
  const card = view.commander_question;
  const w = ZONE_WORDS[d.zone] ?? { where: d.zone.toLowerCase(), stay: "Leave it there", how: "changed zones" };
  const name = card?.name ?? "Your commander";
  return (
    <div className="scrim">
      <div className="modal cmdr-modal">
        {card && <div className="cmdr-modal-card"><CardPreview card={card} /></div>}
        <div className="cmdr-modal-body">
          <h2>{name} {w.how}</h2>
          <p className="muted">It's your commander, so you choose where it ends up.</p>
          <button className="choice" onClick={() => act({ CommanderToCommandZone: true })} autoFocus>
            <strong>Move it to the command zone</strong>
            <span className="muted small">You can cast it again from there. It costs 2 more mana for each time you've already cast it from the command zone.</span>
          </button>
          <button className="choice" onClick={() => act({ CommanderToCommandZone: false })}>
            <strong>{w.stay}</strong>
            <span className="muted small">{d.zone === "Hand" ? "You can cast it from your hand for its normal cost." : d.zone === "Library" ? "You may draw it again later." : `It stays in ${w.where}, where cards that bring things back can reach it.`}</span>
          </button>
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
