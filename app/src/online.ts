// Online games over Steam. The lobby owner (host) runs the rules engine; everyone else sends
// their actions to the host and receives their own view of the table back. Steam only carries
// strings between players (see app/src-tauri/src/steam.rs); this file is the game protocol.

import { inTauri } from "./decks";
import { startRawGame, type GameSession, type Personality, type ResolvedDeck, type SeatSpec } from "./engine/session";
import type { Action, PlayerView } from "./engine/types";

export interface LobbyMember { id: string; name: string }
export interface LobbyState { lobby: string; me: string; host: string; is_host: boolean; members: LobbyMember[] }
export interface SteamStatus { available: boolean; name: string | null; error: string | null }

/** What the host tells everyone about the table being set up. */
export interface Roster {
  players: { id: string; name: string; deck: string | null }[];
  bots: number;
}

type Msg =
  | { t: "deck"; deck: ResolvedDeck }
  | { t: "action"; action: Action }
  | { t: "roster"; roster: Roster }
  | { t: "start"; seat: number }
  | { t: "view"; view: PlayerView }
  | { t: "error"; msg: string };

// Steam is reached through the desktop app. Automated tests can stand in for it by defining
// `window.__STEAM_TEST__` with the same two calls.
interface Transport {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  listen<T>(event: string, cb: (e: { payload: T }) => void): Promise<unknown>;
}
const testTransport = (globalThis as { __STEAM_TEST__?: Transport }).__STEAM_TEST__;
const hasSteam = () => !!testTransport || inTauri();

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (testTransport) return testTransport.invoke<T>(cmd, args);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}
async function listen<T>(event: string, cb: (e: { payload: T }) => void) {
  if (testTransport) return testTransport.listen<T>(event, cb);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, cb);
}

export const steamStatus = (): Promise<SteamStatus> =>
  hasSteam() ? invoke("steam_status") : Promise.resolve({ available: false, name: null, error: "Online play works in the desktop app with Steam running." });
export const hostLobby = () => invoke<void>("steam_create_lobby");
export const inviteFriends = () => invoke<void>("steam_invite");
export const leaveLobby = () => (hasSteam() ? invoke<void>("steam_leave") : Promise.resolve());
const send = (to: string | null, m: Msg) => invoke<void>("steam_send", { to, data: JSON.stringify(m) });

// ---------------------------------------------------------------- events

type Handler<T> = (v: T) => void;
const lobbyHandlers = new Set<Handler<LobbyState | null>>();
const messageHandlers = new Set<(from: string, m: Msg) => void>();
const errorHandlers = new Set<Handler<string>>();
let lobby: LobbyState | null = null;
let listening = false;

/** Start listening for Steam events (once, at startup). */
export async function listenSteam() {
  if (listening || !hasSteam()) return;
  listening = true;
  await listen<LobbyState | null>("steam-lobby", (e) => {
    lobby = e.payload;
    lobbyHandlers.forEach((h) => h(lobby));
  });
  await listen<{ from: string; data: string }>("steam-message", (e) => {
    let m: Msg;
    try {
      m = JSON.parse(e.payload.data);
    } catch {
      return;
    }
    messageHandlers.forEach((h) => h(e.payload.from, m));
  });
  await listen<string>("steam-error", (e) => errorHandlers.forEach((h) => h(e.payload)));
}

export const currentLobby = () => lobby;
export function onLobby(h: Handler<LobbyState | null>) {
  lobbyHandlers.add(h);
  return () => void lobbyHandlers.delete(h);
}
export function onSteamError(h: Handler<string>) {
  errorHandlers.add(h);
  return () => void errorHandlers.delete(h);
}
function onMessage(h: (from: string, m: Msg) => void) {
  messageHandlers.add(h);
  return () => void messageHandlers.delete(h);
}

// ---------------------------------------------------------------- lobby (before the game)

/** The host's side of setting up a table: collects decks, shares the roster, starts the game. */
export class HostLobby {
  decks = new Map<string, ResolvedDeck>();
  bots = 0;
  private off: () => void;
  constructor(private onChange: () => void) {
    this.off = onMessage((from, m) => {
      if (m.t === "deck") {
        this.decks.set(from, m.deck);
        this.share();
        this.onChange();
      }
    });
  }
  roster(state: LobbyState): Roster {
    return {
      players: state.members.map((p) => ({ id: p.id, name: p.name, deck: this.decks.get(p.id)?.name ?? null })),
      bots: this.bots,
    };
  }
  setMyDeck(state: LobbyState, d: ResolvedDeck) {
    this.decks.set(state.me, d);
    this.share();
    this.onChange();
  }
  setBots(n: number) {
    this.bots = n;
    this.share();
    this.onChange();
  }
  share() {
    if (lobby) void send(null, { t: "roster", roster: this.roster(lobby) });
  }
  ready(state: LobbyState) {
    return state.members.length + this.bots >= 2 && state.members.every((m) => this.decks.has(m.id));
  }
  async start(state: LobbyState, botDecks: ResolvedDeck[]): Promise<GameSession> {
    // Keep the lobby order (host first) so seats match what people saw.
    const people = [state.members.find((m) => m.id === state.me)!, ...state.members.filter((m) => m.id !== state.me)];
    const styles: Personality[] = ["Balanced", "Aggressive", "Controlling"];
    const seats: SeatSpec[] = [
      ...people.map((p) => ({ name: p.name, deck: this.decks.get(p.id)!, ai: null })),
      ...botDecks.slice(0, this.bots).map((deck, i) => ({ name: ["Mara", "Tobias", "Juniper"][i], deck, ai: styles[i % 3] })),
    ];
    const remotes = people.slice(1).map((p, i) => ({ id: p.id, seat: i + 1 }));
    this.off();
    const session = await hostSession(seats, remotes);
    remotes.forEach((r) => void send(r.id, { t: "start", seat: r.seat }));
    return session;
  }
  dispose() {
    this.off();
  }
}

/** A guest's side: shows the host's roster, sends a deck, waits for the game to start. */
export class GuestLobby {
  roster: Roster | null = null;
  private off: () => void;
  constructor(onChange: () => void, onStart: (s: GameSession) => void) {
    let started: ClientSession | null = null;
    this.off = onMessage((from, m) => {
      if (!lobby || from !== lobby.host) return;
      if (m.t === "roster") {
        this.roster = m.roster;
        onChange();
      } else if (m.t === "start") {
        started = new ClientSession(lobby.host);
      } else if (m.t === "view" && started && !started.ready) {
        started.receive(m.view);
        this.off();
        onStart(started);
      }
    });
  }
  sendDeck(d: ResolvedDeck) {
    if (lobby) void send(lobby.host, { t: "deck", deck: d });
  }
  dispose() {
    this.off();
  }
}

// ---------------------------------------------------------------- in game

async function hostSession(seats: SeatSpec[], remotes: { id: string; seat: number }[]): Promise<GameSession> {
  const s = await startRawGame(seats);
  const subs = new Set<(e?: string) => void>();
  let queued = false;
  const broadcast = () => {
    if (queued) return;
    queued = true;
    setTimeout(() => {
      queued = false;
      for (const r of remotes) {
        const view = JSON.parse(s.view_for(r.seat)) as PlayerView;
        // The whole history gets long; recent lines are plenty for the ticker and log.
        view.log = view.log.slice(-300);
        void send(r.id, { t: "view", view });
      }
    }, 0);
  };
  const offMsg = onMessage((from, m) => {
    if (m.t !== "action") return;
    const r = remotes.find((x) => x.id === from);
    if (!r) return;
    try {
      s.submit_as(r.seat, JSON.stringify(m.action));
    } catch (e) {
      void send(from, { t: "error", msg: String(e).replace(/^illegal: /, "") });
    }
    broadcast();
    subs.forEach((cb) => cb());
  });
  // Someone left: the computer takes their seat so the game can go on.
  const offLobby = onLobby((st) => {
    if (!st) return;
    for (const r of remotes) {
      if (!st.members.some((m) => m.id === r.id)) s.make_ai(r.seat);
    }
    broadcast();
    subs.forEach((cb) => cb());
  });
  broadcast();
  return {
    view: () => JSON.parse(s.view()) as PlayerView,
    submit: (a) => {
      try {
        s.submit(JSON.stringify(a));
        broadcast();
        return null;
      } catch (e) {
        return String(e);
      }
    },
    nextBot: () => JSON.parse(s.next_bot()),
    applyBot: () => {
      s.apply_bot();
      broadcast();
    },
    rewindEnabled: false,
    rewindPoints: () => [],
    rewind: () => "Not in online games",
    subscribe: (cb) => {
      subs.add(cb);
      return () => void subs.delete(cb);
    },
    leave: () => {
      offMsg();
      offLobby();
      void leaveLobby();
    },
  };
}

class ClientSession implements GameSession {
  ready = false;
  rewindEnabled = false;
  private current: PlayerView | null = null;
  private waiting = false;
  private subs = new Set<(e?: string) => void>();
  private offMsg: () => void;
  private offLobby: () => void;
  constructor(private host: string) {
    this.offMsg = onMessage((from, m) => {
      if (from !== this.host) return;
      if (m.t === "view") this.receive(m.view);
      else if (m.t === "error") {
        this.waiting = false;
        this.subs.forEach((cb) => cb(m.msg));
      }
    });
    this.offLobby = onLobby((st) => {
      if (!st || !st.members.some((m) => m.id === this.host)) {
        this.subs.forEach((cb) => cb("The host left the game."));
      }
    });
  }
  receive(v: PlayerView) {
    this.current = v;
    this.ready = true;
    this.waiting = false;
    this.subs.forEach((cb) => cb());
  }
  view() {
    return this.current!;
  }
  submit(action: Action) {
    // One move at a time: wait for the host to answer before sending another.
    if (this.waiting) return null;
    this.waiting = true;
    void send(this.host, { t: "action", action });
    return null;
  }
  nextBot() {
    return null;
  }
  applyBot() {}
  rewindPoints() {
    return [];
  }
  rewind() {
    return "Not in online games";
  }
  subscribe(cb: (e?: string) => void) {
    this.subs.add(cb);
    return () => void this.subs.delete(cb);
  }
  leave() {
    this.offMsg();
    this.offLobby();
    void leaveLobby();
  }
}
