import { useEffect, useState } from "react";
import { builtinDecks, checkDeck, listDecks, resolveDeck, type SavedDeck } from "../decks";
import { startGame, type GameSession, type Personality, type SeatSpec } from "../engine/session";

const NAMES = ["Mara", "Tobias", "Juniper", "Sol", "Priya", "Elias", "Wren", "Dario", "Hana", "Felix", "Odessa", "Kit"];
const STYLES: { value: Personality | "Random"; label: string; hint: string }[] = [
  { value: "Random", label: "Surprise me", hint: "" },
  { value: "Aggressive", label: "Aggressive", hint: "Attacks often and takes risks" },
  { value: "Balanced", label: "Balanced", hint: "Plays it sensibly" },
  { value: "Controlling", label: "Controlling", hint: "Holds back, answers threats, waits for the right moment" },
];
const RANDOM_DECK = "random";

interface Opp {
  name: string;
  deck: string;
  style: Personality | "Random";
}

function shuffle<T>(a: T[]): T[] {
  const b = [...a];
  for (let i = b.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [b[i], b[j]] = [b[j], b[i]];
  }
  return b;
}

export function SetupScreen({ onBack, onStart }: { onBack: () => void; onStart: (s: GameSession) => void }) {
  const [decks, setDecks] = useState<SavedDeck[]>([]);
  const [myDeck, setMyDeck] = useState<string>("");
  const [count, setCount] = useState(3);
  const [opps, setOpps] = useState<Opp[]>(() =>
    shuffle(NAMES).slice(0, 3).map((name) => ({ name, deck: RANDOM_DECK, style: "Random" as const })),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([listDecks(), builtinDecks()]).then(([m, b]) => {
      const all = [...m, ...b];
      setDecks(all);
      setMyDeck((d) => d || all[0]?.id || "");
    });
  }, []);

  const byId = (id: string) => decks.find((d) => d.id === id);
  const setOpp = (i: number, o: Partial<Opp>) => setOpps(opps.map((x, j) => (j === i ? { ...x, ...o } : x)));

  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const mine = byId(myDeck);
      if (!mine) throw new Error("Pick a deck to play.");
      const builtins = decks.filter((d) => d.builtin);
      const pool = shuffle(builtins.length ? builtins : decks);
      const seats: SeatSpec[] = [];
      const resolve = async (d: SavedDeck, who: string) => {
        const check = await checkDeck(d.text);
        if (!check.playable) throw new Error(`${who}: “${d.name}” can't be played yet. Open it in Decks to see why.`);
        return resolveDeck(d.name, d.text);
      };
      seats.push({ name: "You", deck: await resolve(mine, "Your deck"), ai: null });
      for (const [i, o] of opps.slice(0, count).entries()) {
        const d = o.deck === RANDOM_DECK ? pool[i % pool.length] : byId(o.deck);
        if (!d) throw new Error(`Pick a deck for ${o.name}.`);
        const style = o.style === "Random" ? (["Aggressive", "Balanced", "Controlling"] as const)[Math.floor(Math.random() * 3)] : o.style;
        seats.push({ name: o.name, deck: await resolve(d, o.name), ai: style });
      }
      onStart(await startGame(seats));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  const deckOptions = (withRandom: boolean) => (
    <>
      {withRandom && <option value={RANDOM_DECK}>Random built-in deck</option>}
      {decks.some((d) => !d.builtin) && (
        <optgroup label="Your decks">
          {decks.filter((d) => !d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
        </optgroup>
      )}
      <optgroup label="Built-in">
        {decks.filter((d) => d.builtin).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
      </optgroup>
    </>
  );

  return (
    <div className="screen setup-screen">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← Back</button>
        <h2>New game</h2>
        <span />
      </header>
      <div className="setup-card">
        <div className="menu-section first">
          <div className="menu-label">Your deck</div>
          <select className="field" value={myDeck} onChange={(e) => setMyDeck(e.target.value)}>{deckOptions(false)}</select>
        </div>
        <div className="menu-section">
          <div className="menu-label">Opponents</div>
          <div className="segmented">
            {[1, 2, 3].map((n) => (
              <button key={n} className={n === count ? "on" : ""} onClick={() => setCount(n)}>{n}</button>
            ))}
          </div>
          {opps.slice(0, count).map((o, i) => (
            <div key={i} className="opp-row">
              <input className="field opp-name" value={o.name} onChange={(e) => setOpp(i, { name: e.target.value })} aria-label="Opponent name" />
              <select className="field" value={o.deck} onChange={(e) => setOpp(i, { deck: e.target.value })} aria-label="Opponent deck">{deckOptions(true)}</select>
              <select className="field" value={o.style} onChange={(e) => setOpp(i, { style: e.target.value as Opp["style"] })} aria-label="Play style"
                title={STYLES.find((s) => s.value === o.style)?.hint}>
                {STYLES.map((s) => <option key={s.value} value={s.value}>{s.label}</option>)}
              </select>
            </div>
          ))}
          <p className="muted small">Computer players think through their plays with the full rules, size up who's winning, and remember who attacked them.</p>
        </div>
        {error && <div className="form-error">{error}</div>}
        <button className="btn btn-primary big" disabled={busy || !myDeck} onClick={start}>{busy ? "Shuffling…" : "Start game"}</button>
      </div>
    </div>
  );
}
