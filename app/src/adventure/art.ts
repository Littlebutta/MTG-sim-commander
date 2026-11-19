// Adventure mode's pixel art, all drawn in code: 16×16 tiles, people and objects. Nothing here
// is copied from any game; the palette and shapes are original, in a bright top-down style.
//
// Tile legend (data/world.json):
//   walkable  . grass   , flowers   " tall grass   : path   s sand   m marsh   a ash   h highland
//             g flagstones   b bridge   f wood floor   c dungeon floor   k rug   p spire floor
//             d door / exit mat   e cave mouth   E great door   x way out   u stairs up   v stairs down
//   blocking  T tree   P pine   D dead tree   R cliff   r boulder   ~ water   M bog   L lava
//             W wall   # fence   O U G J roofs   H house wall   N window   B bookshelf   K counter
//             C crate   Y crystal / pillar   Z spire stone   Q old machine

import type { Look } from "./content";

export const TILE = 16;
export const WALKABLE = new Set(".,\":smahgbfckdeEuvxpwi".split(""));
export const ANIMATED = new Set(["~", "M", "L", "Y", "x"]);

type Ctx = CanvasRenderingContext2D;

function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const px = (c: Ctx, x: number, y: number, col: string, w = 1, h = 1) => {
  c.fillStyle = col;
  c.fillRect(x, y, w, h);
};

interface Theme {
  floor: string; floorDark: string; floorLight: string;
  wall: string; wallDark: string; wallLight: string;
  accent: string; rug: string; rugEdge: string;
}

export const THEMES: Record<string, Theme> = {
  vale: { floor: "#9a9a8e", floorDark: "#7d7d72", floorLight: "#b5b5a8", wall: "#8d8478", wallDark: "#5f574d", wallLight: "#b3a999", accent: "#7ad0ff", rug: "#b03a3a", rugEdge: "#e0b84a" },
  shop: { floor: "#b9874f", floorDark: "#946638", floorLight: "#d0a46a", wall: "#e2cfa6", wallDark: "#a8875a", wallLight: "#f3e6c6", accent: "#c9a227", rug: "#9c2f3f", rugEdge: "#e3b94c" },
  tower: { floor: "#6f6577", floorDark: "#554c5e", floorLight: "#887e91", wall: "#3e3448", wallDark: "#2a2232", wallLight: "#5a4d68", accent: "#b388ff", rug: "#3f2a78", rugEdge: "#d4b04a" },
  barrow: { floor: "#4d5146", floorDark: "#3a3e35", floorLight: "#62675a", wall: "#2c2f2a", wallDark: "#1b1d1a", wallLight: "#454a40", accent: "#8fd18f", rug: "#3b2440", rugEdge: "#8a6a9a" },
  sanctum: { floor: "#e9e3d0", floorDark: "#cfc7ae", floorLight: "#fbf7ea", wall: "#c3b896", wallDark: "#9a8f6f", wallLight: "#ddd3b3", accent: "#e5c04a", rug: "#c9a227", rugEdge: "#fff1b0" },
  cinder: { floor: "#3d2c29", floorDark: "#2b1f1d", floorLight: "#55403b", wall: "#241817", wallDark: "#140c0b", wallLight: "#3a2a28", accent: "#ff6a3d", rug: "#7a1f12", rugEdge: "#ff9a3d" },
  grotto: { floor: "#3e5d6d", floorDark: "#2e4755", floorLight: "#527586", wall: "#213845", wallDark: "#14242d", wallLight: "#335262", accent: "#5fd3ff", rug: "#1f4f6a", rugEdge: "#7fe0ff" },
  hollow: { floor: "#5c4b33", floorDark: "#463926", floorLight: "#725f42", wall: "#2f3d22", wallDark: "#1e2816", wallLight: "#46592f", accent: "#9be05a", rug: "#3c5a22", rugEdge: "#b7e07a" },
  spire: { floor: "#3b3e4a", floorDark: "#2b2d37", floorLight: "#4e5261", wall: "#22242c", wallDark: "#14151a", wallLight: "#363a46", accent: "#4fc3ff", rug: "#1c3d5a", rugEdge: "#7fd8ff" },
  rift: { floor: "#261b3b", floorDark: "#1a122a", floorLight: "#35274f", wall: "#120a1e", wallDark: "#08040e", wallLight: "#22163a", accent: "#b388ff", rug: "#3a1f66", rugEdge: "#d0b0ff" },
};

const GROUND: Record<string, [string, string, string]> = {
  ".": ["#6cc04a", "#55a63a", "#8fd86a"],
  ",": ["#6cc04a", "#55a63a", "#8fd86a"],
  '"': ["#5bb040", "#3f8a2c", "#80cc5c"],
  h: ["#93c97e", "#76ad63", "#b6dea0"],
  m: ["#76853f", "#5c6930", "#919f52"],
  a: ["#6e625c", "#554b46", "#877c75"],
  s: ["#ecd8a0", "#d3bd82", "#f7e9bf"],
  ":": ["#cfa96c", "#ad8a52", "#e2c28a"],
};

function fillNoise(c: Ctx, base: [string, string, string], r: () => number, density = 0.12) {
  px(c, 0, 0, base[0], TILE, TILE);
  for (let i = 0; i < TILE * TILE * density; i++) {
    const x = Math.floor(r() * TILE);
    const y = Math.floor(r() * TILE);
    px(c, x, y, r() < 0.5 ? base[1] : base[2]);
  }
}

function grass(c: Ctx, ch: string, r: () => number) {
  const g = GROUND[ch] ?? GROUND["."];
  fillNoise(c, g, r, 0.07);
  // Little tufts.
  for (let i = 0; i < 3; i++) {
    const x = 1 + Math.floor(r() * 13);
    const y = 2 + Math.floor(r() * 12);
    px(c, x, y, g[1]);
    px(c, x + 2, y, g[1]);
    px(c, x + 1, y - 1, g[1]);
  }
}

function ground(c: Ctx, ch: string, r: () => number, theme: Theme) {
  switch (ch) {
    case ".": case "h": case "m": case "a": case ",": case '"':
      grass(c, ch === "," || ch === '"' ? "." : ch, r);
      if (ch === "a") for (let i = 0; i < 2; i++) if (r() < 0.5) px(c, Math.floor(r() * 16), Math.floor(r() * 16), "#ff7a3d");
      if (ch === ",") {
        const cols = ["#ff5a7a", "#fff36b", "#ffffff", "#9fd0ff", "#ff9f40"];
        for (let i = 0; i < 4; i++) {
          const x = 2 + Math.floor(r() * 12);
          const y = 2 + Math.floor(r() * 12);
          const col = cols[Math.floor(r() * cols.length)];
          px(c, x, y - 1, col); px(c, x - 1, y, col); px(c, x + 1, y, col); px(c, x, y + 1, col);
          px(c, x, y, "#ffd23f");
        }
      }
      if (ch === '"') {
        const g = GROUND['"'];
        for (let row = 0; row < 4; row++) {
          for (let col = 0; col < 4; col++) {
            const x = col * 4 + (row % 2) * 2;
            const y = row * 4 + 3;
            px(c, x, y - 3, g[2]); px(c, x + 1, y - 2, g[2]);
            px(c, x, y - 2, g[1], 1, 3); px(c, x + 2, y - 2, g[1], 1, 3);
            px(c, x + 1, y - 1, g[1], 1, 2);
            px(c, x + 3, y, "#2f6e22");
          }
        }
      }
      return;
    case "s": fillNoise(c, GROUND.s, r, 0.1); return;
    case ":": {
      fillNoise(c, GROUND[":"], r, 0.1);
      for (let i = 0; i < 3; i++) px(c, Math.floor(r() * 15), Math.floor(r() * 15), "#9a7a48", 2, 1);
      return;
    }
    case "g": {
      px(c, 0, 0, THEMES.vale.floor, 16, 16);
      px(c, 0, 0, THEMES.vale.floorLight, 16, 1); px(c, 0, 8, THEMES.vale.floorLight, 16, 1);
      px(c, 0, 7, THEMES.vale.floorDark, 16, 1); px(c, 0, 15, THEMES.vale.floorDark, 16, 1);
      px(c, 7, 0, THEMES.vale.floorDark, 1, 8); px(c, 15, 8, THEMES.vale.floorDark, 1, 8);
      for (let i = 0; i < 6; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), r() < 0.5 ? "#6aa24a" : "#86857a");
      return;
    }
    case "b": {
      px(c, 0, 0, "#9a6a3a", 16, 16);
      for (let y = 0; y < 16; y += 4) { px(c, 0, y, "#7a5028", 16, 1); px(c, 0, y + 1, "#b8844e", 16, 1); }
      px(c, 0, 0, "#5a3a1e", 1, 16); px(c, 15, 0, "#5a3a1e", 1, 16);
      return;
    }
    case "f": {
      px(c, 0, 0, theme.floor, 16, 16);
      for (let y = 0; y < 16; y += 4) px(c, 0, y + 3, theme.floorDark, 16, 1);
      const off = Math.floor(r() * 8);
      for (let y = 0; y < 16; y += 4) px(c, (off + y * 3) % 16, y, theme.floorDark, 1, 3);
      for (let i = 0; i < 6; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), theme.floorLight);
      return;
    }
    case "c": case "p": {
      fillNoise(c, [theme.floor, theme.floorDark, theme.floorLight], r, 0.12);
      if (ch === "p" || r() < 0.4) {
        px(c, 0, 0, theme.floorLight, 16, 1); px(c, 0, 0, theme.floorLight, 1, 16);
        px(c, 0, 15, theme.floorDark, 16, 1); px(c, 15, 0, theme.floorDark, 1, 16);
      }
      return;
    }
    case "k": {
      px(c, 0, 0, theme.rug, 16, 16);
      for (let i = 0; i < 10; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), theme.rugEdge + "55");
      return;
    }
  }
  grass(c, ".", r);
}

function edgeShade(c: Ctx, mask: number, col: string, w = 1) {
  // mask bits: 1 up, 2 right, 4 down, 8 left are "different neighbor".
  if (mask & 1) px(c, 0, 0, col, 16, w);
  if (mask & 2) px(c, 16 - w, 0, col, w, 16);
  if (mask & 4) px(c, 0, 16 - w, col, 16, w);
  if (mask & 8) px(c, 0, 0, col, w, 16);
}

/** Paint one tile. `under` is the ground beneath trees and rocks; `mask` marks neighbors that
 * aren't the same tile (for shores, cliffs and roof edges). */
export function paintTile(c: Ctx, ch: string, themeName: string, variant: number, frame: number, mask: number, under: string, inside: boolean) {
  const theme = THEMES[themeName] ?? THEMES.vale;
  const r = rng(variant * 977 + ch.charCodeAt(0) * 131 + 7);
  const base = () => ground(c, under, rng(variant * 31 + under.charCodeAt(0)), theme);
  switch (ch) {
    case "~": {
      px(c, 0, 0, "#3f8fdc", 16, 16);
      const off = frame * 2;
      for (let y = 2; y < 16; y += 5) {
        for (let x = 0; x < 16; x += 8) {
          const xx = (x + off + (y % 2) * 4) % 16;
          px(c, xx, y, "#7cc0f5", 3, 1);
          px(c, (xx + 3) % 16, y + 1, "#6ab0ea", 2, 1);
        }
      }
      if (mask & 1) { px(c, 0, 0, "#e9f7ff", 16, 1); px(c, 0, 1, "#a8dcff", 16, 1); }
      if (mask & 8) px(c, 0, 0, "#cdeeff", 1, 16);
      if (mask & 2) px(c, 15, 0, "#cdeeff", 1, 16);
      if (mask & 4) px(c, 0, 15, "#2c6fb5", 16, 1);
      return;
    }
    case "M": {
      px(c, 0, 0, "#4b5a3c", 16, 16);
      for (let i = 0; i < 18; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), r() < 0.5 ? "#3e4b31" : "#5f6e48");
      const bx = (3 + frame * 5) % 14, by = (5 + frame * 3) % 13;
      px(c, bx, by, "#8a9a6a"); px(c, bx + 1, by - 1, "#8a9a6a");
      edgeShade(c, mask, "#35402a");
      return;
    }
    case "L": {
      px(c, 0, 0, "#d9461f", 16, 16);
      for (let i = 0; i < 26; i++) {
        const x = Math.floor(r() * 16), y = Math.floor(r() * 16);
        px(c, (x + frame) % 16, y, r() < 0.5 ? "#ff9a3d" : "#ffd25a");
      }
      edgeShade(c, mask, "#3a1f18", 2);
      return;
    }
    case "T": {
      base();
      px(c, 6, 11, "#6d4c2f", 4, 5); px(c, 6, 11, "#563a22", 1, 5);
      const o = "#1d4a20";
      px(c, 2, 2, o, 12, 10); px(c, 1, 4, o, 14, 6); px(c, 4, 1, o, 8, 12);
      px(c, 3, 2, "#2f7d32", 10, 9); px(c, 2, 4, "#2f7d32", 12, 5); px(c, 5, 1, "#2f7d32", 6, 11);
      px(c, 4, 3, "#46a84b", 5, 3); px(c, 3, 5, "#46a84b", 3, 2); px(c, 5, 2, "#6cc96a", 3, 1);
      px(c, 9, 8, "#256628", 4, 2);
      return;
    }
    case "P": {
      base();
      px(c, 7, 12, "#5a3a22", 2, 4);
      const o = "#173a33";
      for (let i = 0; i < 6; i++) {
        const w = 2 + i * 2;
        px(c, 8 - w / 2 - 1, 1 + i * 2, o, w + 2, 2);
        px(c, 8 - w / 2, 1 + i * 2, "#2d6b5a", w, 2);
        px(c, 8 - w / 2, 1 + i * 2, "#3f8a73", Math.max(1, w / 2 - 1), 1);
      }
      return;
    }
    case "D": {
      base();
      const b = "#5e5048";
      px(c, 7, 4, b, 2, 12); px(c, 4, 6, b, 3, 1); px(c, 3, 4, b, 1, 2); px(c, 9, 7, b, 4, 1); px(c, 12, 5, b, 1, 2);
      px(c, 6, 2, b, 1, 2); px(c, 9, 1, b, 1, 3); px(c, 7, 4, "#7a6a60", 1, 10);
      return;
    }
    case "R": {
      px(c, 0, 0, "#8d8478", 16, 16);
      for (let i = 0; i < 14; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), r() < 0.5 ? "#766e63" : "#a39a8c");
      px(c, Math.floor(r() * 10) + 2, 4, "#5f574d", 1, 5); px(c, Math.floor(r() * 10) + 2, 9, "#5f574d", 4, 1);
      if (mask & 1) { px(c, 0, 0, "#c2b8a6", 16, 2); px(c, 0, 2, "#a39a8c", 16, 1); }
      if (mask & 4) { px(c, 0, 13, "#5f574d", 16, 3); px(c, 0, 12, "#6e665a", 16, 1); }
      if (mask & 8) px(c, 0, 0, "#6e665a", 1, 16);
      if (mask & 2) px(c, 15, 0, "#6e665a", 1, 16);
      return;
    }
    case "r": {
      base();
      px(c, 3, 5, "#4d4740", 10, 9); px(c, 2, 7, "#4d4740", 12, 6);
      px(c, 4, 5, "#8f877c", 8, 8); px(c, 3, 7, "#8f877c", 10, 5);
      px(c, 5, 6, "#b1a999", 4, 2); px(c, 4, 8, "#b1a999", 2, 2);
      px(c, 3, 13, "#00000033", 10, 2);
      return;
    }
    case "#": {
      base();
      px(c, 0, 6, "#8b5a2b", 16, 2); px(c, 0, 10, "#8b5a2b", 16, 2);
      px(c, 2, 3, "#6d4320", 3, 11); px(c, 11, 3, "#6d4320", 3, 11);
      px(c, 3, 3, "#a87442", 1, 10); px(c, 12, 3, "#a87442", 1, 10);
      return;
    }
    case "O": case "U": case "G": case "J": {
      const pal: Record<string, [string, string, string]> = {
        O: ["#c0392b", "#8e2a20", "#e85a46"], U: ["#2e6fb7", "#1f4f86", "#4c8fd6"],
        G: ["#2f8a55", "#1f6038", "#4fb072"], J: ["#6a3fa0", "#472a6e", "#8c5cc8"],
      };
      const [m, d, l] = pal[ch];
      px(c, 0, 0, m, 16, 16);
      for (let y = 0; y < 16; y += 4) {
        px(c, 0, y + 3, d, 16, 1);
        for (let x = (y / 4) % 2 ? 0 : 4; x < 16; x += 8) px(c, x, y, d, 1, 3);
        px(c, 0, y, l, 16, 1);
      }
      if (mask & 1) { px(c, 0, 0, d, 16, 2); px(c, 0, 2, l, 16, 1); }
      if (mask & 4) { px(c, 0, 13, "#3a2a20", 16, 3); }
      if (mask & 8) px(c, 0, 0, d, 2, 16);
      if (mask & 2) px(c, 14, 0, d, 2, 16);
      return;
    }
    case "H": case "N": {
      px(c, 0, 0, "#f1e3c6", 16, 16);
      for (let i = 0; i < 8; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), "#e2d0ab");
      px(c, 0, 0, "#8b5a2b", 16, 2); px(c, 0, 14, "#7a4a22", 16, 2);
      if (mask & 8) px(c, 0, 0, "#8b5a2b", 2, 16);
      if (mask & 2) px(c, 14, 0, "#8b5a2b", 2, 16);
      if (ch === "N") {
        px(c, 3, 4, "#5a3a1e", 10, 8); px(c, 4, 5, "#7fc8f0", 8, 6);
        px(c, 4, 5, "#c8ecff", 3, 2); px(c, 7, 5, "#5a3a1e", 1, 6); px(c, 4, 8, "#5a3a1e", 8, 1);
      }
      return;
    }
    case "d": {
      if (!inside) {
        px(c, 0, 0, "#f1e3c6", 16, 16); px(c, 0, 0, "#8b5a2b", 16, 2);
        px(c, 3, 2, "#5a3a1e", 10, 14); px(c, 4, 3, "#8b5a2b", 8, 13); px(c, 4, 3, "#a5703c", 8, 2);
        px(c, 10, 9, "#e3b94c", 1, 2);
        return;
      }
      base();
      px(c, 2, 4, "#7a2f2f", 12, 10); px(c, 3, 5, "#a84040", 10, 8); px(c, 3, 5, "#e3b94c55", 10, 1);
      return;
    }
    case "W": {
      if (inside && themeName === "shop") {
        px(c, 0, 0, theme.wall, 16, 16);
        for (let x = 0; x < 16; x += 4) px(c, x, 0, theme.wallLight, 2, 10);
        px(c, 0, 10, theme.wallDark, 16, 6); px(c, 0, 10, "#6d4c2f", 16, 1);
        return;
      }
      px(c, 0, 0, theme.wall, 16, 16);
      const off = (variant % 2) * 4;
      for (let y = 0; y < 16; y += 5) {
        px(c, 0, y, theme.wallDark, 16, 1);
        for (let x = (off + y) % 8; x < 16; x += 8) px(c, x, y, theme.wallDark, 1, 5);
        px(c, 1, y + 1, theme.wallLight, 14, 1);
      }
      if (!(mask & 4)) return;
      px(c, 0, 13, theme.wallDark, 16, 3);
      return;
    }
    case "B": {
      px(c, 0, 0, "#5a3a1e", 16, 16); px(c, 1, 1, "#3a2414", 14, 14);
      const cols = ["#b03a3a", "#2e6fb7", "#2f8a55", "#c9a227", "#6a3fa0", "#d0d0d0"];
      for (const y of [1, 8]) {
        let x = 1;
        while (x < 15) {
          const w = 1 + Math.floor(r() * 2);
          const h = 4 + Math.floor(r() * 3);
          px(c, x, y + 7 - h - 1, cols[Math.floor(r() * cols.length)], w, h);
          x += w + (r() < 0.2 ? 1 : 0);
        }
        px(c, 1, y + 6, "#6d4c2f", 14, 1);
      }
      return;
    }
    case "K": {
      base();
      px(c, 0, 2, "#6d4320", 16, 12); px(c, 0, 2, "#a87442", 16, 4); px(c, 0, 6, "#5a3a1e", 16, 1);
      px(c, 2, 3, "#c99a62", 4, 1);
      return;
    }
    case "C": {
      base();
      if (r() < 0.5) {
        px(c, 2, 3, "#6d4320", 12, 12); px(c, 3, 4, "#a87442", 10, 10);
        px(c, 3, 8, "#6d4320", 10, 1); px(c, 8, 4, "#6d4320", 1, 10);
      } else {
        px(c, 4, 2, "#5a3a1e", 8, 13); px(c, 3, 4, "#5a3a1e", 10, 9); px(c, 5, 3, "#8b5a2b", 6, 11); px(c, 4, 5, "#8b5a2b", 8, 7);
        px(c, 3, 6, "#9aa0a6", 10, 1); px(c, 3, 11, "#9aa0a6", 10, 1);
      }
      return;
    }
    case "Y": {
      base();
      const glow = frame % 2 ? theme.accent : theme.accent + "cc";
      if (["sanctum", "spire", "barrow", "vale"].includes(themeName)) {
        // A pillar.
        px(c, 4, 1, theme.wallDark, 8, 15); px(c, 5, 2, theme.wallLight, 6, 13); px(c, 5, 2, "#ffffff22", 2, 13);
        px(c, 3, 1, theme.wallDark, 10, 2); px(c, 3, 13, theme.wallDark, 10, 3);
        if (themeName !== "barrow") px(c, 7, 6, glow, 2, 2);
        return;
      }
      px(c, 6, 2, "#ffffff", 1, 1);
      px(c, 5, 3, glow, 4, 10); px(c, 6, 1, glow, 2, 3); px(c, 9, 7, glow, 3, 6); px(c, 3, 8, glow, 2, 5);
      px(c, 6, 4, "#ffffff88", 1, 6);
      return;
    }
    case "Z": {
      px(c, 0, 0, "#1d1430", 16, 16);
      for (let i = 0; i < 10; i++) px(c, Math.floor(r() * 16), Math.floor(r() * 16), "#2b1f45");
      if (r() < 0.5) { px(c, 5, 5, "#b388ff88", 6, 1); px(c, 7, 3, "#b388ff88", 1, 6); }
      if (mask & 1) px(c, 0, 0, "#4a3a70", 16, 2);
      if (mask & 4) px(c, 0, 14, "#0c0816", 16, 2);
      return;
    }
    case "Q": {
      base();
      px(c, 3, 3, "#7a5a2a", 10, 10); px(c, 2, 5, "#7a5a2a", 12, 6); px(c, 5, 2, "#7a5a2a", 6, 12);
      px(c, 4, 4, "#c9963e", 8, 8); px(c, 6, 6, "#7a5a2a", 4, 4); px(c, 7, 7, "#3a2a14", 2, 2);
      px(c, 4, 4, "#e8bb63", 3, 1);
      return;
    }
    case "e": case "E": {
      if (ch === "e") {
        px(c, 0, 0, "#8d8478", 16, 16);
        px(c, 2, 3, "#3a332c", 12, 13); px(c, 3, 2, "#3a332c", 10, 14);
        px(c, 4, 4, "#0d0b09", 8, 12); px(c, 3, 6, "#0d0b09", 10, 10);
        return;
      }
      px(c, 0, 0, "#c3b896", 16, 16);
      px(c, 3, 2, "#7a6a4a", 10, 14); px(c, 4, 3, "#2a2018", 8, 13);
      px(c, 5, 2, "#e5c04a", 6, 1); px(c, 7, 6, "#e5c04a88", 2, 2);
      return;
    }
    case "x": {
      ground(c, "c", r, theme);
      px(c, 3, 3, theme.accent + (frame % 2 ? "66" : "44"), 10, 10);
      px(c, 5, 5, theme.accent + "88", 6, 6);
      return;
    }
    case "u": case "v": {
      ground(c, "p", r, theme);
      for (let i = 0; i < 4; i++) {
        const y = ch === "u" ? 12 - i * 3 : 3 + i * 3;
        px(c, 2, y, theme.wallLight, 12, 2); px(c, 2, y + 2, theme.wallDark, 12, 1);
      }
      return;
    }
  }
  ground(c, ch, r, theme);
  if (ch === ":" || ch === "s") {
    // Soft edge where a path meets grass.
    const g = GROUND[under] ?? GROUND["."];
    if (mask & 1) for (let x = 0; x < 16; x++) if (r() < 0.5) px(c, x, 0, g[0]);
    if (mask & 4) for (let x = 0; x < 16; x++) if (r() < 0.5) px(c, x, 15, g[0]);
    if (mask & 8) for (let y = 0; y < 16; y++) if (r() < 0.5) px(c, 0, y, g[0]);
    if (mask & 2) for (let y = 0; y < 16; y++) if (r() < 0.5) px(c, 15, y, g[0]);
  }
}

// ---------------------------------------------------------------- people

const BODY: Record<"down" | "up" | "right", string[][]> = {
  down: [[
    "................",
    ".....oooooo.....",
    "....ohhhhhho....",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohssssssho...",
    "...osesssseso...",
    "...osssssssso...",
    "....oossssoo....",
    "...occcaaccco...",
    "..osccccccccso..",
    "..osccccccccso..",
    "...occddddcco...",
    "...oddddddddo...",
    "....obbo.obbo...",
    ".....oo...oo....",
  ], [
    "................",
    ".....oooooo.....",
    "....ohhhhhho....",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohssssssho...",
    "...osesssseso...",
    "...osssssssso...",
    "....oossssoo....",
    "...occcaaccco...",
    "..osccccccccso..",
    "..osccccccccso..",
    "...occddddcco...",
    "...oddddddddo...",
    "....obbo..oo....",
    ".....oo.........",
  ]],
  up: [[
    "................",
    ".....oooooo.....",
    "....ohhhhhho....",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "....oohhhhoo....",
    "...occcccccco...",
    "..osccccccccso..",
    "..osccccccccso..",
    "...occddddcco...",
    "...oddddddddo...",
    "....obbo.obbo...",
    ".....oo...oo....",
  ], [
    "................",
    ".....oooooo.....",
    "....ohhhhhho....",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "....oohhhhoo....",
    "...occcccccco...",
    "..osccccccccso..",
    "..osccccccccso..",
    "...occddddcco...",
    "...oddddddddo...",
    "....oo..obbo....",
    ".........oo.....",
  ]],
  right: [[
    "................",
    ".....oooooo.....",
    "....ohhhhhho....",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhsssso...",
    "...ohhhssseso...",
    "...ohhsssssso...",
    "....oossssoo....",
    "....occaacco....",
    "....occcccso....",
    "....occcccso....",
    "....occddcco....",
    "....oddddddo....",
    ".....obbbbo.....",
    ".....oooooo.....",
  ], [
    "................",
    ".....oooooo.....",
    "....ohhhhhho....",
    "...ohhhhhhhho...",
    "...ohhhhhhhho...",
    "...ohhhhsssso...",
    "...ohhhssseso...",
    "...ohhsssssso...",
    "....oossssoo....",
    "....occaacco....",
    "....occcccso....",
    "....occcccso....",
    "....occddcco....",
    "....oddddddo....",
    "....obbo.obo....",
    "....oo....oo....",
  ]],
};

function shade(hex: string, f: number): string {
  const n = parseInt(hex.slice(1, 7), 16);
  const ch = (s: number) => Math.max(0, Math.min(255, Math.round(((n >> s) & 255) * f)));
  return `rgb(${ch(16)},${ch(8)},${ch(0)})`;
}

export type Dir = "up" | "down" | "left" | "right";
const spriteCache = new Map<string, HTMLCanvasElement>();

/** A person, 16×16, facing `dir`, on walk frame 0 or 1. */
export function personSprite(look: Look, dir: Dir, frame: number): HTMLCanvasElement {
  const key = `${JSON.stringify(look)}|${dir}|${frame}`;
  const hit = spriteCache.get(key);
  if (hit) return hit;
  const cv = document.createElement("canvas");
  cv.width = TILE;
  cv.height = TILE;
  const c = cv.getContext("2d")!;
  const rows = BODY[dir === "left" ? "right" : dir][frame % 2];
  const pal: Record<string, string> = {
    o: "#1b1420", h: look.hair, s: look.skin, e: "#1b1420", c: look.cloth, d: shade(look.cloth, 0.7), a: look.accent, b: "#3a2a20",
  };
  rows.forEach((row, y) => {
    [...row].forEach((ch, x) => {
      const col = pal[ch];
      if (!col) return;
      const xx = dir === "left" ? 15 - x : x;
      px(c, xx, y, col);
    });
  });
  // Hair highlight.
  px(c, dir === "left" ? 9 : 6, 2, shade(look.hair, 1.25), 2, 1);
  const ex = look.extraColor ?? look.accent;
  const flip = (x: number, w = 1) => (dir === "left" ? 16 - x - w : x);
  switch (look.extra) {
    case "hat":
      px(c, 2, 2, "#1b1420", 12, 2); px(c, 3, 2, ex, 10, 1);
      px(c, 5, 0, "#1b1420", 6, 3); px(c, 6, 0, ex, 4, 2);
      if (look.cloth === look.extraColor || look.hair === "#c0c0ff") { px(c, 7, -1, ex, 2, 2); }
      break;
    case "cap":
      px(c, 4, 1, ex, 8, 2); px(c, 4, 1, shade(ex, 1.2), 8, 1);
      if (dir === "down") px(c, 5, 3, shade(ex, 0.7), 6, 1);
      if (dir === "right" || dir === "left") px(c, flip(11, 3), 3, shade(ex, 0.7), 3, 1);
      break;
    case "hood":
      for (const [y, x0, w] of [[1, 5, 6], [2, 4, 8], [3, 3, 10], [4, 3, 1], [4, 12, 1], [5, 3, 1], [5, 12, 1], [6, 3, 1], [6, 12, 1], [7, 3, 1], [7, 12, 1]] as const) {
        px(c, x0, y, ex, w, 1);
      }
      if (dir === "up") px(c, 4, 4, ex, 8, 4);
      break;
    case "helm":
      px(c, 4, 1, "#1b1420", 8, 1); px(c, 4, 2, ex, 8, 3); px(c, 3, 3, ex, 10, 2);
      px(c, 5, 2, shade(ex, 1.3), 3, 1);
      if (dir === "down") px(c, 7, 4, shade(ex, 0.6), 2, 2);
      break;
    case "crown":
      px(c, 4, 0, ex, 8, 2); px(c, 4, -1, ex, 1, 1); px(c, 7, -1, ex, 2, 1); px(c, 11, -1, ex, 1, 1);
      px(c, 7, 0, "#ff3355", 2, 1);
      break;
    case "bandana":
      px(c, 3, 3, ex, 10, 2);
      if (dir !== "down") px(c, flip(2, 2), 4, ex, 2, 3);
      break;
    case "horns":
      px(c, 2, 0, ex, 2, 3); px(c, 12, 0, ex, 2, 3); px(c, 1, -1, ex, 1, 2); px(c, 14, -1, ex, 1, 2);
      break;
    case "beard":
      if (dir === "down") { px(c, 4, 8, ex, 8, 2); px(c, 5, 10, ex, 6, 2); px(c, 6, 11, ex, 4, 2); px(c, 6, 8, shade(ex, 0.75), 4, 1); }
      if (dir === "right" || dir === "left") { px(c, flip(8, 4), 8, ex, 4, 3); px(c, flip(8, 3), 11, ex, 3, 1); }
      break;
  }
  spriteCache.set(key, cv);
  return cv;
}

// ---------------------------------------------------------------- objects

/** Things standing in the world. `t` is time in seconds, for sparkles. */
export function paintObject(c: Ctx, kind: string, x: number, y: number, t: number, opts: { color?: string; chest?: boolean; lit?: number; theme?: string } = {}) {
  switch (kind) {
    case "card": {
      // A card lying in the grass, catching the light.
      const bob = Math.sin(t * 3) > 0.6 ? 1 : 0;
      px(c, x + 4, y + 13, "#00000033", 8, 2);
      px(c, x + 4, y + 4 - bob, "#1b1420", 8, 10);
      px(c, x + 5, y + 5 - bob, "#4a2f6b", 6, 8);
      px(c, x + 6, y + 6 - bob, "#c9a227", 4, 6);
      px(c, x + 7, y + 7 - bob, "#4a2f6b", 2, 4);
      if ((t * 2) % 3 < 0.5) { px(c, x + 11, y + 3, "#ffffff"); px(c, x + 10, y + 4, "#fff6b0"); px(c, x + 12, y + 4, "#fff6b0"); px(c, x + 11, y + 5, "#fff6b0"); }
      return;
    }
    case "glint": {
      // A hidden card: just a glint now and then.
      const p = (t * 0.8 + (x * 7 + y * 13) / 100) % 2.6;
      if (p < 0.35) {
        const s = p < 0.17 ? 1 : 2;
        px(c, x + 8, y + 8 - s, "#ffffff", 1, s * 2 + 1); px(c, x + 8 - s, y + 8, "#ffffff", s * 2 + 1, 1);
      }
      return;
    }
    case "reagent": {
      const col = opts.color ?? "#ffffff";
      if (opts.chest) {
        px(c, x + 2, y + 5, "#3a2414", 12, 10); px(c, x + 3, y + 6, "#8b5a2b", 10, 8);
        px(c, x + 3, y + 6, "#a87442", 10, 3); px(c, x + 2, y + 9, "#c9a227", 12, 1); px(c, x + 7, y + 9, "#ffe680", 2, 3);
        if ((t * 1.5) % 2 < 0.4) px(c, x + 12, y + 4, col);
        return;
      }
      const g = 0.5 + 0.5 * Math.sin(t * 4);
      c.globalAlpha = 0.35 + 0.25 * g;
      px(c, x + 3, y + 3, col, 10, 10);
      c.globalAlpha = 1;
      px(c, x + 6, y + 4, "#1b1420", 4, 8); px(c, x + 5, y + 6, "#1b1420", 6, 4);
      px(c, x + 7, y + 5, col, 2, 6); px(c, x + 6, y + 7, col, 4, 2); px(c, x + 7, y + 5, "#ffffff", 1, 2);
      return;
    }
    case "sign": {
      px(c, x + 7, y + 9, "#5a3a1e", 2, 6);
      px(c, x + 2, y + 3, "#5a3a1e", 12, 8); px(c, x + 3, y + 4, "#b8844e", 10, 6);
      px(c, x + 4, y + 5, "#7a5028", 8, 1); px(c, x + 4, y + 7, "#7a5028", 6, 1);
      return;
    }
    case "waystone": {
      const glow = 0.5 + 0.5 * Math.sin(t * 2);
      px(c, x + 3, y + 13, "#00000033", 10, 2);
      px(c, x + 4, y + 2, "#3d3a46", 8, 12); px(c, x + 5, y + 1, "#3d3a46", 6, 1);
      px(c, x + 5, y + 2, "#6d6a7a", 6, 11); px(c, x + 5, y + 2, "#8d8a9a", 2, 11);
      c.globalAlpha = 0.5 + 0.5 * glow;
      px(c, x + 7, y + 4, opts.color ?? "#7ad0ff", 2, 2); px(c, x + 6, y + 7, opts.color ?? "#7ad0ff", 4, 1); px(c, x + 7, y + 9, opts.color ?? "#7ad0ff", 2, 2);
      c.globalAlpha = 1;
      return;
    }
    case "well": {
      px(c, x + 1, y + 6, "#5f574d", 14, 9); px(c, x + 2, y + 7, "#8d8478", 12, 7); px(c, x + 4, y + 8, "#2c6fb5", 8, 4);
      px(c, x + 2, y + 1, "#6d4320", 2, 7); px(c, x + 12, y + 1, "#6d4320", 2, 7); px(c, x + 1, y, "#8e2a20", 14, 2);
      return;
    }
    case "cauldron": {
      px(c, x + 2, y + 6, "#111111", 12, 9); px(c, x + 1, y + 7, "#111111", 14, 6); px(c, x + 3, y + 5, "#2b2b2b", 10, 2);
      px(c, x + 3, y + 7, "#2b2b2b", 2, 5);
      const b = Math.floor(t * 4) % 3;
      px(c, x + 4, y + 5, "#5bdc6a", 8, 2); px(c, x + 5 + b * 2, y + 3 - (b % 2), "#9bff8a", 2, 2); px(c, x + 9 - b, y + 2, "#9bff8a88", 1, 1);
      px(c, x + 3, y + 15, "#ff7a2f", 3, 1); px(c, x + 10, y + 15, "#ff7a2f", 3, 1); px(c, x + 6, y + 15, "#ffd25a", 4, 1);
      return;
    }
    case "gate": {
      // Five sockets on the Spire door, lit for each sigil carried.
      const lit = opts.lit ?? 0;
      const cols = ["#9b59b6", "#f5e6a8", "#ff6a3d", "#52c7ff", "#59d98e"];
      px(c, x + 1, y + 1, "#120a1e", 14, 14);
      for (let i = 0; i < 5; i++) {
        const sx = x + 2 + i * 2.6;
        px(c, Math.round(sx), y + 7, i < lit ? cols[i] : "#3a2f4a", 2, 2);
      }
      if (lit >= 5 && (t * 2) % 2 < 1) px(c, x + 2, y + 4, "#b388ff55", 12, 8);
      return;
    }
  }
}
