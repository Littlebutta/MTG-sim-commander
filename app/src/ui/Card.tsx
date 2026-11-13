import type { CardView } from "../engine/types";
import { CrownIcon, ShieldIcon, SleepIcon, SwordIcon, TypeGlyph } from "./icons";

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
              part === "{T}" ? <span key={j} className="pip pip-T">⟳</span> : <ManaCost key={j} cost={part} />
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
  badge?: string | null;
  style?: React.CSSProperties;
}

export function Card({ card, size, glow, dim, onClick, onHover, badge, style }: CardProps) {
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
      ].join(" ")}
      style={style}
      onClick={onClick}
      onMouseEnter={() => onHover?.(card)}
      onMouseLeave={() => onHover?.(null)}
    >
      <div className="card-art">
        <TypeGlyph types={card.types} size={size === "hand" ? 52 : 40} className="art-glyph" />
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
  return (
    <div className={`preview ${frameClass(card)}`}>
      <div className="preview-head">
        <span className="preview-name">{card.name}</span>
        <ManaCost cost={card.mana_cost} size="md" />
      </div>
      <div className="preview-art">
        <TypeGlyph types={card.types} size={88} className="art-glyph" />
      </div>
      <div className="preview-type">{card.type_line}</div>
      <div className="preview-text">
        {card.text ? <RulesText text={card.text} /> : <p className="muted">No rules text.</p>}
        {card.unsupported && <p className="warn">Parts of this card aren't automated yet. Play them by hand.</p>}
        {card.keywords.length > 0 && <p className="muted small">Has: {card.keywords.join(", ")}</p>}
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
