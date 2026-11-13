// The local game session. Practice games run the Rust engine compiled to WebAssembly,
// both in the browser during development and inside the desktop app.

import init, { WasmSession } from "./pkg/engine_wasm.js";
import type { Action, PlayerView } from "./types";

export interface GameSession {
  view(): PlayerView;
  submit(action: Action): string | null;
}

let ready: Promise<unknown> | null = null;

export async function startPractice(players: number, seed = Date.now()): Promise<GameSession> {
  ready ??= init();
  await ready;
  const s = new WasmSession(BigInt(seed), players);
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
  };
}
