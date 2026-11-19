// Adventure deck checks and duels. The desktop app checks decks against the full card pool;
// a browser (development) uses the built-in cards.

import { inTauri } from "../decks";
import { engineReady, startAdventureGame, type GameSession, type Personality, type ResolvedDeck } from "../engine/session";
import { check_adventure_deck, resolve_adventure_deck } from "../engine/pkg/engine_wasm.js";

export interface AdventureDeckCheck {
  card_count: number;
  playable: boolean;
  problems: string[];
  unknown: string[];
  unsupported: string[];
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function checkAdventureDeck(text: string): Promise<AdventureDeckCheck> {
  if (inTauri()) return invoke("check_adventure_deck", { text });
  await engineReady();
  return JSON.parse(check_adventure_deck(text));
}

export async function resolveAdventureDeck(name: string, text: string): Promise<ResolvedDeck> {
  if (inTauri()) return invoke("resolve_adventure_deck", { name, text });
  await engineReady();
  return JSON.parse(resolve_adventure_deck(name, text));
}

export async function startDuel(you: { name: string; text: string }, them: { name: string; deckName: string; text: string; style: Personality; life?: number; skill?: number }): Promise<GameSession> {
  const [mine, theirs] = await Promise.all([resolveAdventureDeck(you.name, you.text), resolveAdventureDeck(them.deckName, them.text)]);
  return startAdventureGame([
    { name: "You", deck: mine, ai: null },
    { name: them.name, deck: theirs, ai: them.style, life: them.life ?? null, skill: them.skill ?? null },
  ]);
}
