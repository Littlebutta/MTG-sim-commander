// Player settings: board theme and card back. Saved in the game folder (data/settings.json)
// in the desktop app, or in localStorage in a browser.

import { useEffect, useState } from "react";
import { inTauri } from "./decks";

export const THEMES = [
  { id: "classic", name: "Night Table", hint: "Dark violet felt" },
  { id: "moss", name: "Mossy Keep", hint: "Weathered stone and moss" },
  { id: "tavern", name: "Old Tavern", hint: "Warm oak planks by the fire" },
  { id: "arcane", name: "Star Library", hint: "Deep blue, drifting runes" },
  { id: "forge", name: "Ember Forge", hint: "Black iron and glowing coals" },
] as const;
export type ThemeId = (typeof THEMES)[number]["id"];

export const BACKS = [
  { id: "sigil", name: "Five Sigils" },
  { id: "tide", name: "Tidal Knot" },
  { id: "grove", name: "Grove Rings" },
  { id: "night", name: "Night Sky" },
  { id: "custom", name: "Your image" },
] as const;
export type BackId = (typeof BACKS)[number]["id"];

export interface Settings {
  theme: ThemeId;
  back: BackId;
  /** Data URL of the player's own card-back image. */
  customBack: string | null;
}

const DEFAULTS: Settings = { theme: "classic", back: "sigil", customBack: null };
const LS = "commander-table.settings";

let current: Settings = DEFAULTS;
const listeners = new Set<(s: Settings) => void>();

export async function loadSettings(): Promise<Settings> {
  try {
    let raw: unknown = null;
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      raw = await invoke("load_settings");
    } else {
      raw = JSON.parse(localStorage.getItem(LS) ?? "null");
    }
    current = { ...DEFAULTS, ...(raw && typeof raw === "object" ? raw : {}) } as Settings;
  } catch {
    current = DEFAULTS;
  }
  listeners.forEach((l) => l(current));
  return current;
}

export async function saveSettings(patch: Partial<Settings>) {
  current = { ...current, ...patch };
  listeners.forEach((l) => l(current));
  try {
    if (inTauri()) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_settings", { settings: current });
    } else {
      localStorage.setItem(LS, JSON.stringify(current));
    }
  } catch {
    /* settings stay for this session */
  }
}

export function useSettings(): Settings {
  const [s, setS] = useState(current);
  useEffect(() => {
    listeners.add(setS);
    return () => {
      listeners.delete(setS);
    };
  }, []);
  return s;
}

/** Shrink an uploaded image to card-back size so settings stay small. */
export function imageToBack(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const w = 300;
      const h = 420;
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      const ctx = c.getContext("2d")!;
      const scale = Math.max(w / img.width, h / img.height);
      const dw = img.width * scale;
      const dh = img.height * scale;
      ctx.drawImage(img, (w - dw) / 2, (h - dh) / 2, dw, dh);
      resolve(c.toDataURL("image/jpeg", 0.85));
      URL.revokeObjectURL(img.src);
    };
    img.onerror = () => reject(new Error("That file isn't an image we can read."));
    img.src = URL.createObjectURL(file);
  });
}

/** Pick a theme for a computer player's side of the table. */
export function themeForSeat(seat: number): ThemeId {
  return THEMES[(seat * 2 + 1) % THEMES.length].id;
}
