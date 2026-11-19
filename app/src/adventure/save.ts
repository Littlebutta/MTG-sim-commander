// The journey's save: where you are, the cards you carry, what you've found and who you've
// beaten. Kept in the game folder (data/adventure.json) in the desktop app, or in localStorage
// in a browser.

import { inTauri } from "../decks";
import type { Dir } from "./art";
import type { PoolId } from "./pools";

export interface Place {
  map: string;
  x: number;
  y: number;
  dir: Dir;
}

export interface Save extends Place {
  v: 1;
  name: string;
  /** Where you wake after losing a duel: the last touchstone you laid a hand on. */
  respawn: Place;
  /** The touchstone that remembers you. */
  stone: string | null;
  /** Cards you own (basic lands are free and unlimited, so they aren't counted). */
  collection: Record<string, number>;
  /** The deck you duel with, basic lands included. */
  deck: Record<string, number>;
  deckName: string;
  reagents: Record<string, number>;
  sigils: string[];
  /** beat:<id>, got:<object id>, met:<id>... */
  flags: Record<string, boolean>;
  starter: string | null;
  seenRegions: string[];
  playtime: number;
  wins: number;
  losses: number;
  brews: number;
  /** Which card pools lost cards come from. Missing in older saves: the Vale's own cards. */
  pools?: PoolId[];
  created: number;
  saved: number;
}

const LS = "commander-table.adventure";

export function newSave(name: string, start: Place): Save {
  return {
    v: 1, name, ...start, respawn: start, stone: null, collection: {}, deck: {}, deckName: "My deck", reagents: {}, sigils: [], flags: {},
    starter: null, seenRegions: [], playtime: 0, wins: 0, losses: 0, brews: 0, created: Date.now(), saved: Date.now(),
  };
}

export const SLOTS = [1, 2, 3];

/** Slot 1 uses the key the single save always had, so older journeys show up there. */
const lsKey = (slot: number) => (slot === 1 ? LS : `${LS}-${slot}`);

function valid(raw: unknown): Save | null {
  if (!raw || typeof raw !== "object") return null;
  const s = raw as Save;
  if (s.v !== 1 || typeof s.map !== "string" || !s.deck || !s.collection) return null;
  // Fill anything an older save is missing.
  return { ...newSave(s.name ?? "", { map: s.map, x: s.x, y: s.y, dir: s.dir }), ...s };
}

export async function loadSave(slot: number): Promise<Save | null> {
  try {
    let raw: unknown;
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      raw = await invoke("load_adventure", { slot });
    } else {
      raw = JSON.parse(localStorage.getItem(lsKey(slot)) ?? "null");
    }
    return valid(raw);
  } catch {
    return null;
  }
}

export async function loadSlots(): Promise<(Save | null)[]> {
  return Promise.all(SLOTS.map(loadSave));
}

/** Which slot was played last, to put it first. */
export function lastSlot(): number {
  try {
    const n = Number(localStorage.getItem(`${LS}-last`));
    return SLOTS.includes(n) ? n : 1;
  } catch {
    return 1;
  }
}

export async function writeSave(save: Save, slot: number): Promise<boolean> {
  save.saved = Date.now();
  try {
    localStorage.setItem(`${LS}-last`, String(slot));
  } catch {
    /* fine */
  }
  try {
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_adventure", { slot, save });
    } else {
      localStorage.setItem(lsKey(slot), JSON.stringify(save));
    }
    return true;
  } catch (e) {
    console.warn("adventure save failed", e);
    return false;
  }
}

export async function deleteSave(slot: number): Promise<void> {
  try {
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_adventure", { slot });
    } else {
      localStorage.removeItem(lsKey(slot));
    }
  } catch (e) {
    console.warn("adventure delete failed", e);
  }
}

export function addCard(save: Save, name: string, n = 1) {
  save.collection[name] = (save.collection[name] ?? 0) + n;
}

export function formatTime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return h > 0 ? `${h}h ${m.toString().padStart(2, "0")}m` : `${m}m`;
}
