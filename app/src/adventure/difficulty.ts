// The difficulty curve. Early duels are gentle: opponents start with less life and play
// loosely (the AI picks among "good enough" plays instead of its best). Each duelist you beat
// and each sigil you earn makes the next duels tougher, until the old powers and the Rift
// Spire play at full strength.

import type { Person } from "./content";
import type { Save } from "./save";

export interface Challenge {
  /** Opponent's starting life. */
  life: number;
  /** AI skill from 0 (loose) to 1 (its best). */
  skill: number;
  /** How hard it is, in a word. */
  label: "Gentle" | "Fair" | "Tough" | "Fierce" | "Deadly";
}

/** Distinct opponents beaten so far. */
export function beatenCount(save: Save): number {
  return Object.keys(save.flags).filter((k) => k.startsWith("beat:") && save.flags[k]).length;
}

export function challengeFor(person: Person, save: Save): Challenge {
  const d = person.duel!;
  const beaten = beatenCount(save);
  const sigils = save.sigils.length;
  let life: number;
  let skill: number;
  if (d.rank === "endgame") {
    life = d.life ?? 30;
    skill = 1;
  } else if (d.rank === "boss") {
    // The first old power you face is a step up, not a wall; later ones are at full strength.
    life = Math.min(d.life ?? 25, 20 + 2 * sigils + Math.floor(beaten / 4));
    skill = Math.min(1, 0.7 + 0.1 * sigils + 0.02 * beaten);
  } else {
    // Duelists: 14 life and loose play at first, 20 life and sharp play after about ten wins.
    const step = beaten + 2 * sigils;
    life = Math.min(20, 14 + step);
    skill = Math.min(1, 0.3 + 0.07 * step);
  }
  const label = d.rank === "endgame" || (d.rank === "boss" && skill >= 1) ? "Deadly" : skill < 0.5 ? "Gentle" : skill < 0.7 ? "Fair" : skill < 0.9 ? "Tough" : "Fierce";
  return { life, skill, label };
}
