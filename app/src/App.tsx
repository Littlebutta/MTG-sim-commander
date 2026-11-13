import { useEffect, useState } from "react";
import { startPractice, type GameSession } from "./engine/session";
import { appVersion, checkForUpdate, type UpdateState } from "./updater";
import { Table } from "./ui/Table";

export function App() {
  const [session, setSession] = useState<GameSession | null>(null);
  const [players, setPlayers] = useState(4);
  const [loading, setLoading] = useState(false);
  const [update, setUpdate] = useState<UpdateState>({ kind: "checking" });

  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    checkForUpdate(setUpdate).then(setUpdate);
    appVersion().then(setVersion).catch(() => {});
  }, []);

  if (session) return <Table session={session} onExit={() => setSession(null)} />;

  const start = async () => {
    setLoading(true);
    setSession(await startPractice(players));
    setLoading(false);
  };

  return (
    <div className="menu-screen">
      <div className="menu-card">
        <div className="brand">
          <div className="brand-mark" />
          <h1>Commander Table</h1>
          <p className="muted">Play Commander with your friends and your own decks.</p>
        </div>

        {update.kind === "available" && (
          <div className="update-banner">
            <span>Version {update.version} is available.</span>
            <button className="btn btn-primary" onClick={() => update.install()}>Update and restart</button>
          </div>
        )}
        {update.kind === "installing" && <div className="update-banner">Downloading update…</div>}
        {update.kind === "error" && <div className="update-banner muted">Couldn't check for updates: {update.message}</div>}

        <div className="menu-section">
          <div className="menu-label">Practice game</div>
          <div className="segmented">
            {[2, 3, 4].map((n) => (
              <button key={n} className={n === players ? "on" : ""} onClick={() => setPlayers(n)}>
                {n} players
              </button>
            ))}
          </div>
          <button className="btn btn-primary big" disabled={loading} onClick={start}>
            {loading ? "Shuffling…" : "Start practice game"}
          </button>
          <p className="muted small">You against practice opponents, using the built-in starter decks.</p>
        </div>

        <div className="menu-section soon">
          <div className="menu-row"><span>Host a game on Steam</span><span className="chip">Coming next</span></div>
          <div className="menu-row"><span>Import a deck</span><span className="chip">Coming next</span></div>
        </div>
        {version && <p className="muted small version">Version {version}</p>}
      </div>
    </div>
  );
}
