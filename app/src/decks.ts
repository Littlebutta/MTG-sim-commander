// Deck library. In the desktop app decks are files in the game folder's data/decks and are
// checked against the full MTGJSON card pool; in a plain browser (development) they live in
// localStorage and only the built-in starter cards are known.

import { checkDeckBuiltin, resolveDeckBuiltin, starterDecks, type ResolvedDeck } from "./engine/session";

export interface DeckCheck {
  commanders: string[];
  card_count: number;
  color_identity: string;
  legal: boolean;
  playable: boolean;
  problems: string[];
  unknown: string[];
  unsupported: string[];
}

export interface SavedDeck {
  id: string;
  name: string;
  text: string;
  updated: number;
  /** Built-in decks can be copied but not edited or deleted. */
  builtin?: boolean;
}

export interface CardStatus {
  date: string | null;
  version: string | null;
  count: number;
  downloading: boolean;
  error: string | null;
}

export const inTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

const LS_KEY = "commander-table.decks";
function lsRead(): SavedDeck[] {
  try {
    return JSON.parse(localStorage.getItem(LS_KEY) ?? "[]");
  } catch {
    return [];
  }
}
function lsWrite(d: SavedDeck[]) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(d));
  } catch {
    /* storage unavailable */
  }
}

export async function builtinDecks(): Promise<SavedDeck[]> {
  return (await starterDecks()).map((d, i) => ({ id: `builtin-${i}`, name: d.name, text: d.text, updated: 0, builtin: true }));
}

export async function listDecks(): Promise<SavedDeck[]> {
  const mine = inTauri() ? await invoke<SavedDeck[]>("list_decks") : lsRead().sort((a, b) => b.updated - a.updated);
  return mine;
}

export async function saveDeck(deck: SavedDeck): Promise<void> {
  const d = { ...deck, updated: Date.now(), builtin: undefined };
  if (inTauri()) return invoke("save_deck", { deck: d });
  lsWrite([...lsRead().filter((x) => x.id !== d.id), d]);
}

export async function deleteDeck(id: string): Promise<void> {
  if (inTauri()) return invoke("delete_deck", { id });
  lsWrite(lsRead().filter((x) => x.id !== id));
}

export async function checkDeck(text: string): Promise<DeckCheck> {
  return inTauri() ? invoke<DeckCheck>("check_deck", { text }) : checkDeckBuiltin(text);
}

export async function resolveDeck(name: string, text: string): Promise<ResolvedDeck> {
  return inTauri() ? invoke<ResolvedDeck>("resolve_deck", { name, text }) : resolveDeckBuiltin(name, text);
}

export async function cardStatus(): Promise<CardStatus | null> {
  return inTauri() ? invoke<CardStatus>("card_status") : null;
}

export async function syncCards(): Promise<CardStatus | null> {
  return inTauri() ? invoke<CardStatus>("sync_cards") : null;
}

export function newDeckId(): string {
  return `d${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}
