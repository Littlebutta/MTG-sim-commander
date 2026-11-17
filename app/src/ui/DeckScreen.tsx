import { useEffect, useMemo, useState } from "react";
import { builtinDecks, checkDeck, deleteDeck, listDecks, newDeckId, saveDeck, type DeckCheck, type SavedDeck } from "../decks";

const PLACEHOLDER = `Commander
1 Atraxa, Praetors' Voice

Deck
1 Sol Ring
1 Arcane Signet
10 Forest
...`;

function Pips({ colors }: { colors: string }) {
  if (!colors) return <span className="pip pip-C" title="Colorless" />;
  return (
    <span className="pips">
      {colors.split("").map((c) => <span key={c} className={`pip pip-${c}`} />)}
    </span>
  );
}

function CheckPanel({ check, busy }: { check: DeckCheck | null; busy: boolean }) {
  const [showUnsupported, setShowUnsupported] = useState(false);
  if (!check) return <div className="check muted">{busy ? "Checking…" : "Paste a decklist to check it."}</div>;
  return (
    <div className="check">
      <div className="check-head">
        <Pips colors={check.color_identity} />
        <span className="check-cmdr">{check.commanders.join(" & ") || "No commander yet"}</span>
        <span className={`check-count ${check.card_count === 100 ? "ok" : ""}`}>{check.card_count}/100</span>
      </div>
      <div className={`check-verdict ${check.legal ? "ok" : check.playable ? "warn" : "bad"}`}>
        {check.legal ? "Commander-legal" : check.playable ? "Playable in casual games" : "Can't be played yet"}
      </div>
      {check.unknown.length > 0 && (
        <div className="check-group">
          <div className="check-label">Cards we couldn't find</div>
          <ul>{check.unknown.map((n) => <li key={n}>{n}</li>)}</ul>
          <p className="muted small">Check the spelling. New cards appear once the card data updates.</p>
        </div>
      )}
      {check.problems.length > 0 && (
        <div className="check-group">
          <div className="check-label">Deck-building issues</div>
          <ul>{check.problems.map((p) => <li key={p}>{p}</li>)}</ul>
        </div>
      )}
      {check.unsupported.length > 0 && (
        <div className="check-group">
          <button className="check-label link" onClick={() => setShowUnsupported(!showUnsupported)}>
            {check.unsupported.length} card{check.unsupported.length > 1 ? "s" : ""} with rules text the game can't run yet {showUnsupported ? "▾" : "▸"}
          </button>
          {showUnsupported && <ul className="columns">{check.unsupported.map((n) => <li key={n}>{n}</li>)}</ul>}
          <p className="muted small">They can still be cast; the parts the game doesn't understand do nothing.</p>
        </div>
      )}
    </div>
  );
}

export function DeckScreen({ onBack }: { onBack: () => void }) {
  const [mine, setMine] = useState<SavedDeck[]>([]);
  const [builtin, setBuiltin] = useState<SavedDeck[]>([]);
  const [current, setCurrent] = useState<SavedDeck | null>(null);
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [check, setCheck] = useState<DeckCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const reload = async () => {
    const [m, b] = await Promise.all([listDecks(), builtinDecks()]);
    setMine(m);
    setBuiltin(b);
    return m;
  };
  useEffect(() => {
    reload().then((m) => (m.length ? open(m[0]) : startNew()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const open = (d: SavedDeck) => {
    setCurrent(d);
    setName(d.name);
    setText(d.text);
    setConfirmDelete(false);
    setMessage(null);
  };
  const startNew = () => open({ id: newDeckId(), name: "New deck", text: "", updated: 0 });

  // Check the list as it's typed (debounced).
  useEffect(() => {
    if (!text.trim()) {
      setCheck(null);
      return;
    }
    setChecking(true);
    const t = setTimeout(() => {
      checkDeck(text)
        .then(setCheck)
        .catch((e) => setMessage(String(e)))
        .finally(() => setChecking(false));
    }, 350);
    return () => clearTimeout(t);
  }, [text]);

  const readOnly = !!current?.builtin;
  const dirty = !!current && !readOnly && (name !== current.name || text !== current.text);
  const isSaved = useMemo(() => mine.some((d) => d.id === current?.id), [mine, current]);

  const save = async () => {
    if (!current) return;
    const d = { ...current, name: name.trim() || "Untitled deck", text };
    await saveDeck(d);
    const m = await reload();
    open(m.find((x) => x.id === d.id) ?? d);
    setMessage("Saved");
  };
  const copy = async () => {
    if (!current) return;
    const d: SavedDeck = { id: newDeckId(), name: `${name} (copy)`, text, updated: 0 };
    await saveDeck(d);
    const m = await reload();
    open(m.find((x) => x.id === d.id) ?? d);
    setMessage("Copied to your decks");
  };
  const remove = async () => {
    if (!current) return;
    await deleteDeck(current.id);
    const m = await reload();
    if (m.length) open(m[0]);
    else startNew();
  };

  useEffect(() => {
    if (!message) return;
    const t = setTimeout(() => setMessage(null), 2200);
    return () => clearTimeout(t);
  }, [message]);

  const item = (d: SavedDeck) => (
    <button key={d.id} className={`deck-item ${current?.id === d.id ? "on" : ""}`} onClick={() => open(d)}>
      <span className="deck-item-name">{d.name}</span>
      {d.builtin && <span className="chip">Built-in</span>}
    </button>
  );

  return (
    <div className="screen decks-screen">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← Back</button>
        <h2>Decks</h2>
        <span />
      </header>
      <div className="decks-layout">
        <nav className="deck-list">
          <button className="btn btn-primary" onClick={startNew}>New deck</button>
          <div className="menu-label">Your decks</div>
          {mine.length === 0 && <p className="muted small">None yet. Paste a list from Moxfield, Archidekt or MTGO.</p>}
          {mine.map(item)}
          {current && !readOnly && !isSaved && (
            <button className="deck-item on"><span className="deck-item-name">{name || "New deck"}</span><span className="chip">Unsaved</span></button>
          )}
          <div className="menu-label">Built-in</div>
          {builtin.map(item)}
        </nav>

        <section className="deck-editor">
          <input className="deck-name" value={name} disabled={readOnly} onChange={(e) => setName(e.target.value)} placeholder="Deck name" />
          <textarea className="deck-text" value={text} readOnly={readOnly} spellCheck={false} placeholder={PLACEHOLDER}
            onChange={(e) => setText(e.target.value)} />
          <div className="deck-actions">
            {readOnly ? (
              <button className="btn btn-primary" onClick={copy}>Make an editable copy</button>
            ) : (
              <>
                <button className="btn btn-primary" disabled={!dirty && isSaved} onClick={save}>{isSaved ? "Save changes" : "Save deck"}</button>
                {isSaved && <button className="btn" onClick={copy}>Duplicate</button>}
                {isSaved && (confirmDelete ? (
                  <span className="confirm">
                    Delete “{current?.name}”?
                    <button className="btn btn-primary danger" onClick={remove}>Delete</button>
                    <button className="btn" onClick={() => setConfirmDelete(false)}>Keep</button>
                  </span>
                ) : (
                  <button className="btn btn-quiet" onClick={() => setConfirmDelete(true)}>Delete</button>
                ))}
              </>
            )}
            {message && <span className="saved-note">{message}</span>}
          </div>
        </section>

        <aside className="deck-check">
          <CheckPanel check={check} busy={checking} />
          <p className="muted small format-help">
            One card per line, like <code>1 Sol Ring</code>. Put the commander under a <code>Commander</code> heading, or add <code>*CMDR*</code> after its name. Set codes and collector numbers are ignored.
          </p>
        </aside>
      </div>
    </div>
  );
}
