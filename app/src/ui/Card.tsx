import { useArt } from "../art";
import type { CardView } from "../engine/types";
import { CrownIcon, ShieldIcon, SleepIcon, SwordIcon, TapIcon, TypeGlyph } from "./icons";
import { explain } from "./keywords";

const PALETTE: Record<string, [string, string, string]> = {
  W: ["#f6efd9", "#d9c9a0", "#8f7f5a"],
  U: ["#9fd0f5", "#3f7fbf", "#1d3a63"],
  B: ["#9b8aa8", "#4e3d5c", "#1c1424"],
  R: ["#ffc08a", "#d9573a", "#5c1a12"],
  G: ["#c9e6a8", "#4f9a55", "#1d3d22"],
  C: ["#e2e5ea", "#9aa0aa", "#4a4f58"],
  M: ["#ffe7a3", "#c79a3a", "#4f3a12"],
};

function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
}

/** An original painted-landscape placeholder, unique per card, used until real art loads. */
function ProceduralArt({ card }: { card: CardView }) {
  const basic = ["Plains", "Island", "Swamp", "Mountain", "Forest"].findIndex((t) => card.subtypes.includes(t));
  const key = card.colors.length > 1 ? "M" : card.colors[0] ?? (basic >= 0 ? "WUBRG"[basic] : "C");
  const [sky, mid, deep] = PALETTE[key] ?? PALETTE.C;
  let h = hash(card.name);
  const rnd = () => ((h = Math.imul(h ^ (h >>> 15), 2246822507) >>> 0), (h % 1000) / 1000);
  const ridge = (base: number, amp: number) => {
    const pts = ["0,70"];
    for (let x = 0; x <= 100; x += 10) pts.push(`${x},${(base - rnd() * amp).toFixed(1)}`);
    pts.push("100,70");
    return pts.join(" ");
  };
  const id = `g${hash(card.name + card.id)}`;
  return (
    <svg className="art-proc" viewBox="0 0 100 70" preserveAspectRatio="xMidYMid slice" aria-hidden>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={sky} />
          <stop offset="1" stopColor={mid} />
        </linearGradient>
      </defs>
      <rect width="100" height="70" fill={`url(#${id})`} />
      <circle cx={15 + rnd() * 70} cy={12 + rnd() * 14} r={5 + rnd() * 6} fill="#fff" opacity=".35" />
      <polygon points={ridge(42, 16)} fill={mid} opacity=".75" />
      <polygon points={ridge(54, 12)} fill={deep} opacity=".7" />
      <polygon points={ridge(64, 8)} fill={deep} />
    </svg>
  );
}

/** Card art: the real art once downloaded, otherwise the painted placeholder. */
export function CardArt({ card, glyph = 40 }: { card: CardView; glyph?: number }) {
  const art = useArt(card.name);
  if (art) return <img className="art-img" src={art.url} alt="" draggable={false} />;
  return (
    <>
      <ProceduralArt card={card} />
      <TypeGlyph types={card.types} size={glyph} className="art-glyph" />
    </>
  );
}

/** CSS class picking the frame palette from a card's colors. */
export function frameClass(c: CardView): string {
  if (c.types.includes("Land") && c.colors.length === 0) {
    const basic = ["Plains", "Island", "Swamp", "Mountain", "Forest"].findIndex((t) => c.subtypes.includes(t));
    if (basic >= 0) return `f-land f-${"WUBRG"[basic]}`;
    return "f-land f-C";
  }
  if (c.colors.length === 0) return "f-C";
  if (c.colors.length > 1) return "f-M";
  return `f-${c.colors[0]}`;
}

/** Mana cost string like "{2}{G}{G}" as pips. */
export function ManaCost({ cost, size = "sm" }: { cost: string; size?: "sm" | "md" }) {
  const syms = cost.match(/\{[^}]+\}/g) ?? [];
  return (
    <span className={`mana-cost ${size}`}>
      {syms.map((s, i) => {
        const v = s.slice(1, -1);
        const cls = /^[WUBRGC]$/.test(v) ? `pip-${v}` : v.includes("/") ? "pip-hy" : "pip-N";
        return (
          <span key={i} className={`pip ${cls}`}>
            {/^[WUBRG]$/.test(v) ? "" : v.replace("/", "")}
          </span>
        );
      })}
    </span>
  );
}

/** Rules text with {X} symbols rendered as pips. */
export function RulesText({ text }: { text: string }) {
  return (
    <>
      {text.split("\n").map((line, i) => (
        <p key={i}>
          {line.split(/(\{[^}]+\})/g).map((part, j) =>
            /^\{[^}]+\}$/.test(part) ? (
              part === "{T}" ? <span key={j} className="pip pip-T" title="Tap"><TapIcon size={11} /></span> : <ManaCost key={j} cost={part} />
            ) : (
              <span key={j}>{part}</span>
            ),
          )}
        </p>
      ))}
    </>
  );
}

function ptClass(c: CardView): string {
  if (c.power === null || c.toughness === null) return "";
  if (c.damage > 0) return "pt-hurt";
  const bp = c.base_power ?? c.power;
  const bt = c.base_toughness ?? c.toughness;
  if (c.power > bp || c.toughness > bt) return "pt-up";
  if (c.power < bp || c.toughness < bt) return "pt-down";
  return "";
}

export interface CardProps {
  card: CardView;
  size: "field" | "hand" | "mini";
  glow?: "play" | "target" | "attack" | "block" | "selected" | null;
  dim?: boolean;
  onClick?: () => void;
  onHover?: (c: CardView | null) => void;
  /** Right-click: show the card's rules in detail. */
  onInspect?: (c: CardView) => void;
  badge?: string | null;
  style?: React.CSSProperties;
  className?: string;
}

export function Card({ card, size, glow, dim, onClick, onHover, onInspect, badge, style, className }: CardProps) {
  const toughness = card.toughness !== null ? card.toughness - card.damage : null;
  const isCreature = card.types.includes("Creature");
  return (
    <div
      className={[
        "card",
        `card-${size}`,
        frameClass(card),
        card.tapped ? "tapped" : "",
        glow ? `glow-${glow}` : "",
        dim ? "dim" : "",
        onClick ? "clickable" : "",
        card.attacking !== null ? "is-attacking" : "",
        className ?? "",
      ].join(" ")}
      style={style}
      data-obj={card.id}
      onClick={onClick}
      onMouseEnter={() => onHover?.(card)}
      onMouseLeave={() => onHover?.(null)}
      onContextMenu={(e) => {
        e.preventDefault();
        onInspect?.(card);
      }}
    >
      <div className="card-art">
        <CardArt card={card} glyph={size === "hand" ? 52 : 40} />
      </div>
      <div className="card-name">
        <span>{card.name}</span>
      </div>
      {size === "hand" && (
        <div className="card-cost">
          <ManaCost cost={card.mana_cost} />
        </div>
      )}
      {size === "hand" && <div className="card-type">{card.type_line}</div>}
      {isCreature && card.power !== null && (
        <div className={`pt ${ptClass(card)}`}>
          {card.power}/{toughness}
        </div>
      )}
      <div className="card-badges">
        {card.is_commander && <span className="badge badge-cmdr" title="Commander"><CrownIcon /></span>}
        {card.summoning_sick && isCreature && card.zone === "Battlefield" && !card.keywords.includes("Haste") && (
          <span className="badge" title="Summoning sick"><SleepIcon /></span>
        )}
        {card.attacking !== null && <span className="badge badge-atk" title="Attacking"><SwordIcon size={12} /></span>}
        {card.blocking !== null && <span className="badge badge-blk" title="Blocking"><ShieldIcon size={12} /></span>}
        {Object.entries(card.counters).map(([k, v]) => (
          <span key={k} className="badge badge-ctr">{k === "+1/+1" ? `+${v}` : `${v} ${k}`}</span>
        ))}
        {card.unsupported && <span className="badge badge-warn" title="Some text needs manual play">!</span>}
      </div>
      {badge && <div className="card-tag">{badge}</div>}
    </div>
  );
}

/** The large hover preview with full rules text. */
export function CardPreview({ card }: { card: CardView }) {
  const art = useArt(card.name);
  return (
    <div className={`preview ${frameClass(card)}`}>
      <div className="preview-head">
        <span className="preview-name">{card.name}</span>
        <ManaCost cost={card.mana_cost} size="md" />
      </div>
      <div className="preview-art">
        <CardArt card={card} glyph={88} />
        {art?.artist && <span className="artist">Illus. {art.artist}</span>}
      </div>
      <div className="preview-type">{card.type_line}</div>
      <div className="preview-text">
        {card.text ? <RulesText text={card.text} /> : <p className="muted">No rules text.</p>}
        {card.unsupported && <p className="warn">Parts of this card aren't automated yet. Play them by hand.</p>}
        {card.keywords.length > 0 && <p className="muted small">Has: {card.keywords.join(", ")}</p>}
        <p className="muted small hint-right">Right-click for keyword rules</p>
      </div>
      {card.power !== null && (
        <div className="preview-pt">
          {card.power}/{card.toughness}
          {card.damage > 0 && <span className="dmg"> ({card.damage} dmg)</span>}
        </div>
      )}
    </div>
  );
}

/** Right-click popup: the card's text plus what each keyword and term on it means. */
export function RulesPopup({ card, onClose }: { card: CardView; onClose: () => void }) {
  const terms = explain(card.keywords, card.text);
  if (card.is_commander) terms.push(["Commander tax", "Each time you cast your commander from the command zone, it costs {2} more for each previous time."]);
  if (card.summoning_sick && card.types.includes("Creature") && card.zone === "Battlefield")
    terms.push(["Summoning sickness", "It can't attack or use {T} abilities until you've controlled it since the start of your turn."]);
  return (
    <div className="scrim" onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }}>
      <div className={`rules-popup ${frameClass(card)}`} onClick={(e) => e.stopPropagation()}>
        <div className="rules-art"><CardArt card={card} glyph={64} /></div>
        <div className="rules-body">
          <div className="preview-head">
            <span className="preview-name">{card.name}</span>
            <ManaCost cost={card.mana_cost} size="md" />
          </div>
          <div className="preview-type">{card.type_line}</div>
          <div className="preview-text">{card.text ? <RulesText text={card.text} /> : <p className="muted">No rules text.</p>}</div>
          {terms.length > 0 && (
            <dl className="glossary">
              {terms.map(([k, v]) => (
                <div key={k}>
                  <dt>{k}</dt>
                  <dd><RulesText text={v} /></dd>
                </div>
              ))}
            </dl>
          )}
          {card.unsupported && <p className="warn small">Parts of this card aren't automated yet; those parts do nothing.</p>}
          <button className="btn rules-close" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
}
