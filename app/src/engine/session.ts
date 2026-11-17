// The local game session. Games run the Rust engine compiled to WebAssembly, both in the
// browser during development and inside the desktop app.

import init, { WasmSession, starter_decks, check_deck, resolve_deck } from "./pkg/engine_wasm.js";
import type { Action, PlayerView } from "./types";

export interface PendingBot {
  player: number;
  think_ms: number;
}

export interface GameSession {
  view(): PlayerView;
  submit(action: Action): string | null;
  /** If a computer player is deciding, who and how long they'd think; otherwise null. */
  nextBot(): PendingBot | null;
  /** Play the move the computer player decided on. */
  applyBot(): void;
  /** Stack items you passed on that you can still go back and respond to, newest first. */
  /** Whether the "go back and respond" house rule is on for this game. */
  rewindEnabled: boolean;
  rewindPoints(): RewindPoint[];
  rewind(index: number): string | null;
}

export interface RewindPoint {
  index: number;
  name: string;
  controller: number;
  turn: number;
}

export type Personality = "Aggressive" | "Balanced" | "Controlling";

/** A deck turned into card definitions (opaque to the UI). */
export interface ResolvedDeck {
  name: string;
  commanders: unknown[];
  cards: unknown[];
}

export interface SeatSpec {
  name: string;
  deck: ResolvedDeck;
  ai: Personality | null;
}

let ready: Promise<unknown> | null = null;
export async function engineReady() {
  ready ??= init();
  await ready;
}

function wrap(s: WasmSession, rewind = true): GameSession {
  return {
    view: () => JSON.parse(s.view()) as PlayerView,
    submit: (action) => {
      try {
        s.submit(JSON.stringify(action));
        return null;
      } catch (e) {
        return String(e);
      }
    },
    nextBot: () => JSON.parse(s.next_bot()) as PendingBot | null,
    applyBot: () => s.apply_bot(),
    rewindEnabled: rewind,
    rewindPoints: () => JSON.parse(s.rewind_points()) as RewindPoint[],
    rewind: (i) => {
      try {
        s.rewind(i);
        return null;
      } catch (e) {
        return String(e);
      }
    },
  };
}

export async function startPractice(players: number, seed = Date.now()): Promise<GameSession> {
  await engineReady();
  return wrap(new WasmSession(BigInt(seed), players));
}

export async function startGame(seats: SeatSpec[], opts: { rewind?: boolean } = {}, seed = Date.now()): Promise<GameSession> {
  await engineReady();
  const s = WasmSession.from_seats(BigInt(seed), JSON.stringify(seats));
  s.set_rewind(opts.rewind ?? true);
  return wrap(s, opts.rewind ?? true);
}

export async function starterDecks(): Promise<{ name: string; text: string }[]> {
  await engineReady();
  return JSON.parse(starter_decks());
}

/** Built-in card pool only; the desktop app checks against the full downloaded pool. */
export async function checkDeckBuiltin(text: string) {
  await engineReady();
  return JSON.parse(check_deck(text));
}

export async function resolveDeckBuiltin(name: string, text: string): Promise<ResolvedDeck> {
  await engineReady();
  return JSON.parse(resolve_deck(name, text));
}
