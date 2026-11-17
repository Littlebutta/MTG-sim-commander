// Arrows drawn over the table: attackers to the player they attack, blockers to what they block,
// a spell or ability to its targets, and the live arrow that follows the pointer while you drag
// or choose a target.

import { useEffect, useRef, useState, type ReactNode } from "react";

export type Anchor = { obj: number } | { player: number } | "cursor";
export type ArrowTone = "attack" | "block" | "target" | "drag" | "incoming";
export interface ArrowSpec {
  key: string;
  from: Anchor;
  to: Anchor;
  tone: ArrowTone;
}

// The pointer's last position, so an arrow that starts mid-gesture has somewhere to point.
const pointer = { x: typeof window !== "undefined" ? window.innerWidth / 2 : 0, y: typeof window !== "undefined" ? window.innerHeight / 2 : 0 };
if (typeof window !== "undefined") {
  window.addEventListener("pointermove", (e) => {
    pointer.x = e.clientX;
    pointer.y = e.clientY;
  }, { passive: true });
}
export const pointerPos = () => ({ ...pointer });

/** The element on the table that stands for an object or a player. */
export function anchorEl(a: Exclude<Anchor, "cursor">): Element | null {
  if ("obj" in a) return document.querySelector(`.table [data-obj="${a.obj}"]`);
  return (
    document.querySelector(`.table .opp-panel[data-player="${a.player}"] .sigil`) ??
    document.querySelector(`.table .medallion[data-player="${a.player}"]`) ??
    document.querySelector(`.table [data-player="${a.player}"]`)
  );
}

/** What's under the pointer, ignoring the arrow layer itself. */
export function elementAtPointer(): Element | null {
  return document.elementFromPoint(pointer.x, pointer.y);
}

const center = (el: Element) => {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
};

export function ArrowLayer({ arrows, snap, ghost }: { arrows: ArrowSpec[]; snap?: (el: Element | null) => Element | null; ghost?: ReactNode }) {
  const [, redraw] = useState(0);
  const raf = useRef(0);
  const active = arrows.length > 0 || !!ghost;

  useEffect(() => {
    if (!active) return;
    const kick = () => {
      if (raf.current) return;
      raf.current = requestAnimationFrame(() => {
        raf.current = 0;
        redraw((n) => n + 1);
      });
    };
    window.addEventListener("pointermove", kick, { passive: true });
    window.addEventListener("resize", kick);
    // Cards slide and tap; follow them.
    const iv = setInterval(kick, 200);
    return () => {
      window.removeEventListener("pointermove", kick);
      window.removeEventListener("resize", kick);
      clearInterval(iv);
      cancelAnimationFrame(raf.current);
      raf.current = 0;
    };
  }, [active]);

  if (!active) return null;

  const snapped = snap?.(elementAtPointer()) ?? null;
  const paths = arrows.flatMap((a) => {
    if (a.from === "cursor") return [];
    const fromEl = anchorEl(a.from);
    if (!fromEl) return [];
    const s = center(fromEl);
    let e: { x: number; y: number };
    let locked = false;
    if (a.to === "cursor") {
      if (snapped) {
        e = center(snapped);
        locked = true;
      } else e = { ...pointer };
    } else {
      const toEl = anchorEl(a.to);
      if (!toEl) return [];
      e = center(toEl);
      locked = true;
    }
    const dx = e.x - s.x;
    const dy = e.y - s.y;
    const len = Math.hypot(dx, dy);
    if (len < 24) return [];
    // Stop a little short so the head sits on the target, not under the cursor.
    const back = locked ? 14 : 4;
    const ex = e.x - (dx / len) * back;
    const ey = e.y - (dy / len) * back;
    // A gentle bow, always curving the same way so crossing arrows stay readable.
    const bow = Math.min(80, len * 0.18);
    const cx = (s.x + ex) / 2 - (dy / len) * bow;
    const cy = (s.y + ey) / 2 + (dx / len) * bow;
    return [{ key: a.key, d: `M ${s.x} ${s.y} Q ${cx} ${cy} ${ex} ${ey}`, tone: a.tone, locked, s }];
  });

  return (
    <>
      <svg className="arrow-layer" width="100%" height="100%" aria-hidden>
        <defs>
          {(["attack", "block", "target", "drag", "incoming"] as ArrowTone[]).map((t) => (
            <marker key={t} id={`head-${t}`} viewBox="0 0 10 10" refX="6" refY="5" markerWidth="4.5" markerHeight="4.5" orient="auto-start-reverse">
              <path d="M0,0 L10,5 L0,10 z" className={`arrow-head tone-${t}`} />
            </marker>
          ))}
        </defs>
        {paths.map((p) => (
          <g key={p.key} className={`arrow tone-${p.tone} ${p.locked ? "locked" : ""}`}>
            <circle cx={p.s.x} cy={p.s.y} r={5} className="arrow-dot" />
            <path d={p.d} className="arrow-glow" />
            <path d={p.d} className="arrow-line" markerEnd={`url(#head-${p.tone})`} />
          </g>
        ))}
      </svg>
      {ghost && (
        <div className="drag-ghost" style={{ left: pointer.x, top: pointer.y }}>
          {ghost}
        </div>
      )}
    </>
  );
}
