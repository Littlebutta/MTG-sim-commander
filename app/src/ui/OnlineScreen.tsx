import { useEffect, useRef, useState } from "react";
import { builtinDecks, checkDeck, listDecks, resolveDeck, type SavedDeck } from "../decks";
import type { GameSession, ResolvedDeck } from "../engine/session";
import {
  currentLobby, GuestLobby, HostLobby, hostLobby, inviteFriends, leaveLobby, onLobby, onSteamError, steamStatus,
  type LobbyState, type SteamStatus,
} from "../online";

// Play with friends over Steam: host a table, invite friends, everyone picks a deck, and the
// host starts. Empty seats can go to computer players.

export function OnlineScreen({ onBack, onStart }: { onBack: () => void; onStart: (s: GameSession) => void }) {
  const [status, setStatus] = useState<SteamStatus | null>(null);
  const [lobby, setLobby] = useState<LobbyState | null>(currentLobby());
  const [decks, setDecks] = useState<SavedDeck[]>([]);
  const [myDeck, setMyDeck] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [, bump] = useState(0);
  const host = useRef<HostLobby | null>(null);
  const guest = useRef<GuestLobby | null>(null);

  useEffect(() => {
    steamStatus().then(setStatus);
    Promise.all([listDecks(), builtinDecks()]).then(([m, b]) => setDecks([...m, ...b]));
    const offL = onLobby(setLobby);
    const offE = onSteamError((e) => {
      setError(e);
      setBusy(false);
    });
    return () => {
      offL();
      offE();
    };
  }, []);

  // Become host or guest as the lobby says.
  useEffect(() => {
    if (!lobby) {
      host.current?.dispose();
      guest.current?.dispose();
      host.current = guest.current = null;
      return;
    }
    setBusy(false);
    if (lobby.is_host && !host.current) {
      guest.current?.dispose();
      guest.current = null;
      host.current = new HostLobby(() => bump((n) => n + 1));
    } else if (!lobby.is_host && !guest.current) {
      host.current?.dispose();
      host.current = null;
      guest.current = new GuestLobby(() => bump((n) => n + 1), onStart);
    }
    host.current?.share();
  }, [lobby, onStart]);
  useEffect(() => () => {
    host.current?.dispose();
    guest.current?.dispose();
  }, []);

  const resolve = async (d: SavedDeck): Promise<ResolvedDeck> => {
    const check = await checkDeck(d.text);
    if (!check.playable) throw new Error(`“${d.name}” can't be played yet. Open it in Decks to see why.`);
    return resolveDeck(d.name, d.text);
  };
  const pickDeck = async (id: string) => {
    setMyDeck(id);
    setError(null);
    const d = decks.find((x) => x.id === id);
    if (!d || !lobby) return;
    try {
      const r = await resolve(d);
      if (host.current) host.current.setMyDeck(lobby, r);
      else guest.current?.sendDeck(r);
      bump((n) => n + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const start = async () => {
    if (!lobby || !host.current) return;
    setBusy(true);
    try {
      const builtins = decks.filter((d) => d.builtin).sort(() => Math.random() - 0.5);
      const botDecks = await Promise.all(builtins.slice(0, host.current.bots).map(resolve));
      const s = await host.current.start(lobby, botDecks);
      host.current = null;
      onStart(s);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };
  const leave = async () => {
    await leaveLobby();
    setLobby(null);
  };

  const deckSelect = (
    <select className="field" value={myDeck} onChange={(e) => pickDeck(e.target.value)} aria-label="Your deck">
      <option value="" disabled>Pick your deck…</option>
      {decks.some((d) => !d.builtin) && <optgroup label="Your decks">{decks.filter((d) => !d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</optgroup>}
      <optgroup label="Built-in">{decks.filter((d) => d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}</optgroup>
    </select>
  );

  const roster = lobby && host.current ? host.current.roster(lobby) : guest.current?.roster ?? null;
  const seatsLeft = lobby ? 4 - lobby.members.length : 0;

  return (
    <div className="screen setup-screen">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← Back</button>
        <h2>Play with friends</h2>
        <span />
      </header>
      <div className="setup-card">
        {!status ? (
          <p className="muted">Checking Steam…</p>
        ) : !status.available ? (
          <div className="menu-section first">
            <div className="menu-label">Steam isn't available</div>
            <p>{status.error}</p>
          </div>
        ) : !lobby ? (
          <div className="menu-section first">
            <div className="menu-label">Signed in to Steam as {status.name}</div>
            <p className="muted">Host a table and invite friends from Steam. Friends who get an invite can accept it from Steam, even with the game closed.</p>
            <button className="btn btn-primary big" disabled={busy} onClick={() => { setBusy(true); setError(null); void hostLobby(); }}>
              {busy ? "Opening a table…" : "Host a game"}
            </button>
          </div>
        ) : (
          <>
            <div className="menu-section first">
              <div className="menu-label">At the table</div>
              <ul className="lobby-list">
                {lobby.members.map((m) => {
                  const deck = roster?.players.find((p) => p.id === m.id)?.deck;
                  return (
                    <li key={m.id}>
                      <span className="lobby-name">{m.name}{m.id === lobby.host ? <span className="chip">Host</span> : null}{m.id === lobby.me ? <span className="muted"> (you)</span> : null}</span>
                      <span className={deck ? "lobby-ready" : "muted"}>{deck ?? "choosing a deck…"}</span>
                    </li>
                  );
                })}
                {roster && roster.bots > 0 && Array.from({ length: roster.bots }, (_, i) => (
                  <li key={`bot${i}`}><span className="lobby-name">Computer player</span><span className="muted">random deck</span></li>
                ))}
              </ul>
              <button className="btn" onClick={() => void inviteFriends()} disabled={seatsLeft <= 0}>Invite friends</button>
            </div>
            <div className="menu-section">
              <div className="menu-label">Your deck</div>
              {deckSelect}
            </div>
            {lobby.is_host && host.current && (
              <div className="menu-section">
                <div className="menu-label">Computer players</div>
                <div className="segmented" style={{ gridTemplateColumns: `repeat(${seatsLeft + 1}, 1fr)` }}>
                  {Array.from({ length: seatsLeft + 1 }, (_, n) => (
                    <button key={n} className={n === host.current!.bots ? "on" : ""} onClick={() => host.current!.setBots(n)}>{n}</button>
                  ))}
                </div>
              </div>
            )}
            {error && <div className="form-error">{error}</div>}
            {lobby.is_host ? (
              <button className="btn btn-primary big" disabled={busy || !host.current?.ready(lobby)} onClick={start}>
                {busy ? "Shuffling…" : host.current?.ready(lobby) ? "Start game" : "Waiting for everyone to pick a deck"}
              </button>
            ) : (
              <p className="muted">Waiting for {lobby.members.find((m) => m.id === lobby.host)?.name ?? "the host"} to start.</p>
            )}
            <button className="btn btn-quiet" onClick={leave}>Leave table</button>
          </>
        )}
        {!lobby && error && <div className="form-error">{error}</div>}
      </div>
    </div>
  );
}
