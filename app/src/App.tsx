import { useEffect, useState } from "react";
import { cardStatus, syncCards, type CardStatus } from "./decks";
import { startPractice, type GameSession } from "./engine/session";
import { appVersion, checkForUpdate, type UpdateState } from "./updater";
import { DeckScreen } from "./ui/DeckScreen";
import { BuilderScreen } from "./ui/BuilderScreen";
import { loadSettings } from "./settings";
import { play, unlockSound } from "./sound";
import { SettingsScreen } from "./ui/SettingsScreen";
import { SetupScreen } from "./ui/SetupScreen";
import { Table } from "./ui/Table";
import { OnlineScreen } from "./ui/OnlineScreen";
import { listenSteam, onLobby } from "./online";

type Screen = "menu" | "decks" | "setup" | "settings" | "builder" | "online";

function CardData({ status }: { status: CardStatus | null }) {
  if (!status) return null;
  if (status.downloading) return <p className="muted small card-status">Downloading the latest cards…</p>;
  if (status.count > 0)
    return (
      <p className="muted small card-status">
        {status.count.toLocaleString()} cards{status.date ? ` · updated ${status.date}` : ""}
        {status.error ? ` · couldn't check for new cards` : ""}
      </p>
    );
  if (status.error) return <p className="muted small card-status">Card data: {status.error}</p>;
  return null;
}

export function App() {
  const [session, setSession] = useState<GameSession | null>(null);
  const [screen, setScreen] = useState<Screen>("menu");
  const [loading, setLoading] = useState(false);
  const [update, setUpdate] = useState<UpdateState>({ kind: "checking" });
  const [version, setVersion] = useState<string | null>(null);
  const [cards, setCards] = useState<CardStatus | null>(null);

  // Sound starts with the first click (browsers require it), and buttons get a soft click.
  useEffect(() => {
    const onDown = () => unlockSound();
    const onClick = (e: MouseEvent) => {
      const el = (e.target as Element | null)?.closest?.("button, label.toggle, .theme-tile, .back-tile");
      if (el && !(el as HTMLButtonElement).disabled) play("click");
    };
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("click", onClick, true);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("click", onClick, true);
    };
  }, []);

  useEffect(() => {
    checkForUpdate(setUpdate).then(setUpdate);
    appVersion().then(setVersion).catch(() => {});
    loadSettings();
    void listenSteam();
    // Check MTGJSON for new cards; show progress while it downloads.
    const poll = setInterval(() => cardStatus().then(setCards).catch(() => {}), 1500);
    syncCards()
      .then(setCards)
      .catch(() => cardStatus().then(setCards))
      .finally(() => clearInterval(poll));
    return () => clearInterval(poll);
  }, []);

  // Accepting a Steam invite puts you in a lobby: go to it.
  useEffect(() => onLobby((l) => {
    if (l) setScreen((s) => (s === "online" ? s : "online"));
  }), []);

  if (session) return <Table session={session} onExit={() => setSession(null)} />;
  if (screen === "online") return <OnlineScreen onBack={() => setScreen("menu")} onStart={(s) => { setScreen("menu"); setSession(s); }} />;
  if (screen === "builder") return <BuilderScreen onBack={() => setScreen("menu")} />;
  if (screen === "decks") return <DeckScreen onBack={() => setScreen("menu")} />;
  if (screen === "settings") return <SettingsScreen onBack={() => setScreen("menu")} />;
  if (screen === "setup") return <SetupScreen onBack={() => setScreen("menu")} onStart={(s) => { setScreen("menu"); setSession(s); }} />;

  const quick = async () => {
    setLoading(true);
    setSession(await startPractice(4));
    setLoading(false);
  };

  return (
    <div className="menu-screen">
      <div className="menu-card">
        <div className="brand">
          <div className="brand-mark" />
          <h1>Commander Table</h1>
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
          <button className="btn btn-primary big" onClick={() => setScreen("setup")}>New game</button>
          <button className="btn big" onClick={() => setScreen("online")}>Play with friends</button>
          <button className="btn big" onClick={() => setScreen("builder")}>Deck builder</button>
          <button className="btn big" onClick={() => setScreen("decks")}>Decks</button>
          <button className="btn big" onClick={() => setScreen("settings")}>Table &amp; cards</button>
          <button className="btn btn-quiet" disabled={loading} onClick={quick}>{loading ? "Shuffling…" : "Quick practice with built-in decks"}</button>
        </div>

        <CardData status={cards} />
        {version && <p className="muted small version">Version {version}</p>}
      </div>
    </div>
  );
}
