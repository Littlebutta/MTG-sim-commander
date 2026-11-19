import { useState } from "react";
import { BACKS, THEMES, imageToBack, saveSettings, useSettings } from "../settings";
import { play } from "../sound";

export function SettingsScreen({ onBack }: { onBack: () => void }) {
  const s = useSettings();
  const [error, setError] = useState<string | null>(null);
  const upload = async (f: File | undefined) => {
    if (!f) return;
    try {
      setError(null);
      await saveSettings({ customBack: await imageToBack(f), back: "custom" });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <div className="screen settings-screen">
      <header className="screen-head">
        <button className="ghost" onClick={onBack}>← Back</button>
        <h2>Table &amp; cards</h2>
        <span />
      </header>
      <div className="settings-body">
        <div className="menu-label">Sound</div>
        <div className="sound-row">
          <label className="toggle"><input type="checkbox" checked={s.sound} onChange={(e) => saveSettings({ sound: e.target.checked }).then(() => play("resolve"))} /> Sound effects</label>
          <label className="volume">
            <span className="muted small">Volume</span>
            <input type="range" min={0} max={100} step={5} value={Math.round(s.volume * 100)} disabled={!s.sound}
              onChange={(e) => saveSettings({ volume: Number(e.target.value) / 100 })}
              onPointerUp={() => play("cast")} onKeyUp={() => play("cast")} />
            <span className="muted small volume-num">{Math.round(s.volume * 100)}%</span>
          </label>
          <span className="sound-tests">
            {([["Draw", "draw"], ["Spell", "cast"], ["Attack", "attack"], ["Damage", "hit"], ["Your turn", "yourTurn"]] as const).map(([label, id]) => (
              <button key={id} className="ghost small" disabled={!s.sound} onClick={() => play(id)}>{label}</button>
            ))}
          </span>
        </div>
        <div className="menu-label">Your side of the table</div>
        <div className="theme-grid">
          {THEMES.map((t) => (
            <button key={t.id} className={`theme-tile ${s.theme === t.id ? "on" : ""}`} onClick={() => saveSettings({ theme: t.id })}>
              <span className="theme-swatch theme-surface" data-theme={t.id} />
              <span className="theme-name">{t.name}</span>
              <span className="muted small">{t.hint}</span>
            </button>
          ))}
        </div>
        <div className="menu-label">Card backs</div>
        <div className="back-grid">
          {BACKS.map((b) => (
            <button key={b.id} className={`back-tile ${s.back === b.id ? "on" : ""}`}
              onClick={() => (b.id === "custom" && !s.customBack ? document.getElementById("back-file")?.click() : saveSettings({ back: b.id }))}>
              <span className={`back-swatch card-back back-${b.id === "custom" ? "sigil" : b.id}`}
                style={b.id === "custom" && s.customBack ? { backgroundImage: `url(${s.customBack})` } : undefined} />
              <span className="theme-name">{b.name}</span>
            </button>
          ))}
        </div>
        <div className="upload-row">
          <label className="btn">
            Choose an image for your card backs…
            <input id="back-file" type="file" accept="image/*" hidden onChange={(e) => upload(e.target.files?.[0])} />
          </label>
          {s.customBack && <button className="btn btn-quiet" onClick={() => saveSettings({ customBack: null, back: "sigil" })}>Remove</button>}
          {error && <span className="form-error inline">{error}</span>}
        </div>
        <p className="muted small">Pick any picture you like. It's cropped to card shape and saved in the game folder.</p>
      </div>
    </div>
  );
}
