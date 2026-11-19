// The journey's save: where you are, the cards you carry, what you've found and who you've
// beaten. Kept in the game folder (data/adventure.json) in the desktop app, or in localStorage
// in a browser.

import { inTauri } from "../decks";
import type { Dir } from "./art";

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

export async function loadSave(): Promise<Save | null> {
  try {
    let raw: unknown;
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      raw = await invoke("load_adventure");
    } else {
      raw = JSON.parse(localStorage.getItem(LS) ?? "null");
    }
    if (raw && typeof raw === "object" && (raw as Save).v === 1) return raw as Save;
  } catch {
    /* no save, or unreadable */
  }
  return null;
}

export async function writeSave(save: Save): Promise<void> {
  save.saved = Date.now();
  try {
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_adventure", { save });
    } else {
      localStorage.setItem(LS, JSON.stringify(save));
    }
  } catch (e) {
    console.warn("adventure save failed", e);
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
