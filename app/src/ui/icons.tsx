// Original line icons used for card art glyphs and UI chrome.

type P = { size?: number; className?: string };

const svg = (size: number, className: string | undefined, children: React.ReactNode) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.6}
    strokeLinecap="round" strokeLinejoin="round" className={className} aria-hidden>
    {children}
  </svg>
);

export const CreatureGlyph = ({ size = 24, className }: P) =>
  svg(size, className, <>
    <path d="M5 19c1-5 3-8 7-9 4 1 6 4 7 9" />
    <path d="M8 9l-2-5 4 3M16 9l2-5-4 3" />
    <circle cx="10" cy="13" r=".6" fill="currentColor" />
    <circle cx="14" cy="13" r=".6" fill="currentColor" />
  </>);

export const SpellGlyph = ({ size = 24, className }: P) =>
  svg(size, className, <>
    <path d="M12 3c3 4 5 6 5 10a5 5 0 0 1-10 0c0-2 1-3 2-4 0 2 1 3 2 3 0-3-1-6 1-9z" />
  </>);

export const ArtifactGlyph = ({ size = 24, className }: P) =>
  svg(size, className, <>
    <path d="M12 3l7 4v10l-7 4-7-4V7z" />
    <path d="M12 3v18M5 7l14 10M19 7L5 17" opacity=".45" />
  </>);

export const EnchantmentGlyph = ({ size = 24, className }: P) =>
  svg(size, className, <>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M5 19l2-2M17 7l2-2" />
  </>);

export const LandGlyph = ({ size = 24, className }: P) =>
  svg(size, className, <>
    <path d="M2 19l6-9 4 5 3-4 7 8z" />
    <circle cx="17" cy="6" r="2" />
  </>);

export const SwordIcon = ({ size = 16, className }: P) =>
  svg(size, className, <>
    <path d="M14 4h6v6L9 21l-3-3z" />
    <path d="M5 14l5 5M3 21l3-3" />
  </>);

export const ShieldIcon = ({ size = 16, className }: P) =>
  svg(size, className, <path d="M12 3l8 3v6c0 5-4 8-8 9-4-1-8-4-8-9V6z" />);

export const ScrollIcon = ({ size = 18, className }: P) =>
  svg(size, className, <>
    <path d="M7 4h11v13a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3v-1h11v1" />
    <path d="M7 4a2 2 0 0 0-2 2v10M10 8h5M10 12h5" />
  </>);

export const SleepIcon = ({ size = 14, className }: P) =>
  svg(size, className, <path d="M4 6h6l-6 7h6M14 12h5l-5 6h5" />);

export const CrownIcon = ({ size = 14, className }: P) =>
  svg(size, className, <path d="M3 8l4 4 5-7 5 7 4-4-2 11H5z" />);

export function TypeGlyph({ types, size = 24, className }: { types: string[]; size?: number; className?: string }) {
  if (types.includes("Creature")) return <CreatureGlyph size={size} className={className} />;
  if (types.includes("Land")) return <LandGlyph size={size} className={className} />;
  if (types.includes("Instant") || types.includes("Sorcery")) return <SpellGlyph size={size} className={className} />;
  if (types.includes("Enchantment")) return <EnchantmentGlyph size={size} className={className} />;
  return <ArtifactGlyph size={size} className={className} />;
}
