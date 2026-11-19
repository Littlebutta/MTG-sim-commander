// Light and life on top of the pixel map: soft shadows, drifting cloud shade, dungeon darkness
// lit by the player's lantern and glowing tiles, and small ambient particles (butterflies,
// fireflies, embers, motes). Drawn every frame at screen resolution, so it adds finer detail
// than the 16-pixel tiles.

type Ctx = CanvasRenderingContext2D;

interface Mood {
  /** 0 (none) to 1 (pitch black) outside the lights. */
  dark: number;
  /** Color of the darkness. */
  shade: string;
  /** Color washed over everything. */
  tint?: string;
  tintAlpha?: number;
  particles?: "butterflies" | "fireflies" | "embers" | "motes" | "bubbles" | "leaves" | "dust" | "pollen";
  clouds?: boolean;
  lantern?: number;
}

const MOODS: Record<string, Mood> = {
  meadow: { dark: 0, shade: "#000", clouds: true, particles: "butterflies" },
  village: { dark: 0, shade: "#000", clouds: true, particles: "pollen" },
  lake: { dark: 0, shade: "#000", clouds: true, tint: "#9fd8ff", tintAlpha: 0.05, particles: "pollen" },
  coast: { dark: 0, shade: "#000", clouds: true, tint: "#bfe8ff", tintAlpha: 0.06, particles: "pollen" },
  mire: { dark: 0.22, shade: "#0d1a10", tint: "#5a7a3a", tintAlpha: 0.12, particles: "fireflies", lantern: 90 },
  high: { dark: 0, shade: "#000", clouds: true, tint: "#fff6d8", tintAlpha: 0.06, particles: "pollen" },
  ember: { dark: 0.12, shade: "#1a0806", tint: "#ff7a3d", tintAlpha: 0.1, particles: "embers", lantern: 110 },
  wilds: { dark: 0.08, shade: "#08140a", clouds: true, tint: "#3a6a2a", tintAlpha: 0.06, particles: "leaves" },
  spire: { dark: 0.1, shade: "#0a0a14", tint: "#8a9ac0", tintAlpha: 0.1, particles: "motes" },
  // Indoors and below ground (by map theme).
  shop: { dark: 0.12, shade: "#1a0e04", tint: "#ffcf8a", tintAlpha: 0.06, particles: "dust", lantern: 140 },
  tower: { dark: 0.25, shade: "#0e0818", tint: "#b388ff", tintAlpha: 0.06, particles: "motes", lantern: 120 },
  barrow: { dark: 0.74, shade: "#05070a", particles: "dust", lantern: 96 },
  sanctum: { dark: 0.2, shade: "#1a1608", tint: "#fff1b0", tintAlpha: 0.08, particles: "motes", lantern: 140 },
  cinder: { dark: 0.62, shade: "#120404", tint: "#ff6a3d", tintAlpha: 0.06, particles: "embers", lantern: 80 },
  grotto: { dark: 0.62, shade: "#020a12", tint: "#5fd3ff", tintAlpha: 0.05, particles: "bubbles", lantern: 84 },
  hollow: { dark: 0.6, shade: "#040a02", tint: "#9be05a", tintAlpha: 0.04, particles: "fireflies", lantern: 84 },
  rift: { dark: 0.7, shade: "#08020f", tint: "#b388ff", tintAlpha: 0.08, particles: "motes", lantern: 84 },
};

/** Tiles that glow in the dark, with their light's color and reach (in world pixels). */
const GLOW: Record<string, [string, number]> = {
  L: ["255,120,40", 34],
  Y: ["140,200,255", 26],
  x: ["255,240,200", 28],
  E: ["200,160,255", 30],
  N: ["255,220,150", 22],
};

export function moodFor(theme: string, region: string | null): Mood {
  if (theme === "vale") return MOODS[region ?? "meadow"] ?? MOODS.meadow;
  return MOODS[theme] ?? MOODS.shop;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  max: number;
  seed: number;
}

let particles: Particle[] = [];
let particleKind = "";
let light: HTMLCanvasElement | null = null;

export interface View {
  /** Camera top-left and size, in world pixels. */
  camX: number;
  camY: number;
  vw: number;
  vh: number;
  scale: number;
  w: number;
  h: number;
  time: number;
  dt: number;
  /** Player center, world pixels. */
  px: number;
  py: number;
  /** Visible glowing tiles: [tile char, world x, world y]. */
  glows: [string, number, number][];
  /** Other lights: world x, y, rgb, radius. */
  lights: [number, number, string, number][];
}

/** World-space effects under the people (call inside the world transform, before sprites). */
export function drawClouds(c: Ctx, mood: Mood, v: View) {
  if (!mood.clouds) return;
  // Big soft cloud shadows drifting slowly across the land.
  c.save();
  const drift = v.time * 6;
  for (let i = 0; i < 4; i++) {
    const span = 900;
    const cx = ((i * 337 + drift) % span) - 200 + Math.floor((v.camX - 200) / span) * span;
    for (let k = 0; k < 3; k++) {
      const x = cx + k * span;
      const y = v.camY + ((i * 211) % Math.max(1, v.vh)) + Math.sin(v.time * 0.05 + i) * 20;
      if (x < v.camX - 220 || x > v.camX + v.vw + 220) continue;
      const g = c.createRadialGradient(x, y, 10, x, y, 150);
      g.addColorStop(0, "rgba(10,30,20,0.13)");
      g.addColorStop(1, "rgba(10,30,20,0)");
      c.fillStyle = g;
      c.beginPath();
      c.ellipse(x, y, 190, 110, 0, 0, Math.PI * 2);
      c.fill();
    }
  }
  c.restore();
}

/** A soft round shadow at a sprite's feet. */
export function softShadow(c: Ctx, x: number, y: number, w = 7, h = 2.6, alpha = 0.28) {
  const g = c.createRadialGradient(x, y, 0, x, y, w);
  g.addColorStop(0, `rgba(0,0,0,${alpha})`);
  g.addColorStop(1, "rgba(0,0,0,0)");
  c.fillStyle = g;
  c.beginPath();
  c.ellipse(x, y, w, h, 0, 0, Math.PI * 2);
  c.fill();
}

function spawn(kind: string, v: View, anywhere: boolean): Particle {
  const x = v.camX + Math.random() * v.vw;
  const y = anywhere ? v.camY + Math.random() * v.vh : kind === "embers" || kind === "bubbles" ? v.camY + v.vh + 4 : v.camY + Math.random() * v.vh;
  const max = kind === "butterflies" ? 14 + Math.random() * 10 : 5 + Math.random() * 6;
  const p: Particle = { x, y, vx: 0, vy: 0, life: anywhere ? Math.random() * max : 0, max, seed: Math.random() * 1000 };
  switch (kind) {
    case "embers": p.vx = (Math.random() - 0.5) * 6; p.vy = -10 - Math.random() * 14; break;
    case "bubbles": p.vx = 0; p.vy = -6 - Math.random() * 6; break;
    case "leaves": p.x = v.camX + Math.random() * v.vw * 1.2 - v.vw * 0.2; p.y = anywhere ? p.y : v.camY - 4; p.vx = 6 + Math.random() * 6; p.vy = 8 + Math.random() * 6; break;
    case "pollen": case "dust": case "motes": p.vx = (Math.random() - 0.3) * 3; p.vy = (Math.random() - 0.6) * 2; break;
    default: p.vx = (Math.random() - 0.5) * 10; p.vy = (Math.random() - 0.5) * 6;
  }
  return p;
}

const COUNT: Record<string, number> = { butterflies: 7, fireflies: 26, embers: 34, motes: 30, bubbles: 18, leaves: 14, dust: 26, pollen: 22 };

/** Ambient particles, in world space (inside the world transform), drawn finer than a tile pixel. */
export function drawParticles(c: Ctx, mood: Mood, v: View) {
  const kind = mood.particles ?? "";
  if (kind !== particleKind) {
    particleKind = kind;
    particles = kind ? Array.from({ length: COUNT[kind] ?? 20 }, () => spawn(kind, v, true)) : [];
  }
  if (!kind) return;
  const fine = 1 / Math.max(1, v.scale / 2);
  for (let i = 0; i < particles.length; i++) {
    const p = particles[i];
    p.life += v.dt;
    const t = v.time + p.seed;
    if (kind === "butterflies") {
      p.vx += Math.sin(t * 1.3) * 8 * v.dt;
      p.vy += Math.cos(t * 0.9) * 6 * v.dt;
      p.vx = Math.max(-12, Math.min(12, p.vx));
      p.vy = Math.max(-8, Math.min(8, p.vy));
    } else if (kind === "fireflies") {
      p.vx = Math.sin(t * 0.7) * 5;
      p.vy = Math.cos(t * 0.5) * 4;
    } else if (kind === "leaves") {
      p.vx = 8 + Math.sin(t * 2) * 8;
    }
    p.x += p.vx * v.dt;
    p.y += p.vy * v.dt;
    const off = p.x < v.camX - 30 || p.x > v.camX + v.vw + 30 || p.y < v.camY - 30 || p.y > v.camY + v.vh + 30;
    if (p.life > p.max || off) {
      particles[i] = spawn(kind, v, off);
      continue;
    }
    const fade = Math.min(1, p.life / 0.8, (p.max - p.life) / 0.8);
    c.globalAlpha = Math.max(0, fade);
    switch (kind) {
      case "butterflies": {
        const flap = Math.abs(Math.sin(t * 14));
        const col = ["#ffffff", "#ffe36b", "#ffb0d0", "#b0d8ff"][Math.floor(p.seed) % 4];
        c.fillStyle = "rgba(0,0,0,0.15)";
        c.fillRect(p.x, p.y + 6, 2, fine);
        c.fillStyle = col;
        c.fillRect(p.x - 1.5 * flap, p.y, 1.5 * flap, 1.5);
        c.fillRect(p.x + 0.5, p.y, 1.5 * flap, 1.5);
        c.fillStyle = "#3a2a20";
        c.fillRect(p.x, p.y, 0.5, 1.5);
        break;
      }
      case "fireflies": {
        const glow = 0.5 + 0.5 * Math.sin(t * 3);
        const g = c.createRadialGradient(p.x, p.y, 0, p.x, p.y, 5);
        g.addColorStop(0, `rgba(230,255,140,${0.55 * glow})`);
        g.addColorStop(1, "rgba(230,255,140,0)");
        c.fillStyle = g;
        c.fillRect(p.x - 5, p.y - 5, 10, 10);
        c.fillStyle = `rgba(250,255,200,${0.6 + 0.4 * glow})`;
        c.fillRect(p.x - fine / 2, p.y - fine / 2, fine, fine);
        break;
      }
      case "embers": {
        c.fillStyle = p.seed % 2 > 1 ? "#ffd25a" : "#ff7a3d";
        c.fillRect(p.x, p.y, fine, fine * 1.5);
        break;
      }
      case "bubbles": {
        c.strokeStyle = "rgba(190,240,255,0.7)";
        c.lineWidth = fine * 0.8;
        c.beginPath();
        c.arc(p.x + Math.sin(t * 2) * 1.5, p.y, 1 + (p.seed % 1.5), 0, Math.PI * 2);
        c.stroke();
        break;
      }
      case "leaves": {
        c.fillStyle = ["#d9a03a", "#b5562b", "#8fbf3a"][Math.floor(p.seed) % 3];
        const s = Math.sin(t * 3);
        c.fillRect(p.x, p.y, 2 * Math.abs(s) + fine, fine * 1.2);
        break;
      }
      case "motes": {
        c.fillStyle = "rgba(200,220,255,0.8)";
        c.fillRect(p.x, p.y, fine, fine);
        break;
      }
      default: {
        c.fillStyle = kind === "dust" ? "rgba(255,240,210,0.55)" : "rgba(255,255,230,0.7)";
        c.fillRect(p.x, p.y, fine, fine);
      }
    }
  }
  c.globalAlpha = 1;
}

/** Screen-space light: darkness with lights cut out of it, a color wash, and a vignette. */
export function drawLight(c: Ctx, mood: Mood, v: View) {
  const { w, h, scale } = v;
  const toX = (x: number) => (x - v.camX) * scale;
  const toY = (y: number) => (y - v.camY) * scale;
  if (mood.dark > 0) {
    if (!light) light = document.createElement("canvas");
    if (light.width !== w || light.height !== h) {
      light.width = w;
      light.height = h;
    }
    const l = light.getContext("2d")!;
    l.globalCompositeOperation = "source-over";
    l.clearRect(0, 0, w, h);
    l.fillStyle = mood.shade;
    l.globalAlpha = mood.dark;
    l.fillRect(0, 0, w, h);
    l.globalAlpha = 1;
    l.globalCompositeOperation = "destination-out";
    const hole = (x: number, y: number, r: number, strength = 1) => {
      const g = l.createRadialGradient(x, y, 0, x, y, r);
      g.addColorStop(0, `rgba(0,0,0,${strength})`);
      g.addColorStop(0.55, `rgba(0,0,0,${strength * 0.6})`);
      g.addColorStop(1, "rgba(0,0,0,0)");
      l.fillStyle = g;
      l.fillRect(x - r, y - r, r * 2, r * 2);
    };
    const flicker = 1 + Math.sin(v.time * 9) * 0.015 + Math.sin(v.time * 23) * 0.01;
    hole(toX(v.px), toY(v.py), (mood.lantern ?? 80) * scale * flicker);
    for (const [ch, x, y] of v.glows) {
      const gl = GLOW[ch];
      if (gl) hole(toX(x), toY(y), gl[1] * scale, 0.85);
    }
    for (const [x, y, , r] of v.lights) hole(toX(x), toY(y), r * scale, 0.9);
    c.drawImage(light, 0, 0);
    // Colored glow from lights.
    c.save();
    c.globalCompositeOperation = "lighter";
    const tint = (x: number, y: number, rgb: string, r: number, a: number) => {
      const g = c.createRadialGradient(x, y, 0, x, y, r);
      g.addColorStop(0, `rgba(${rgb},${a})`);
      g.addColorStop(1, `rgba(${rgb},0)`);
      c.fillStyle = g;
      c.fillRect(x - r, y - r, r * 2, r * 2);
    };
    tint(toX(v.px), toY(v.py), "255,190,110", (mood.lantern ?? 80) * 0.7 * scale, 0.07 * mood.dark);
    for (const [ch, x, y] of v.glows) {
      const gl = GLOW[ch];
      if (gl) tint(toX(x), toY(y), gl[0], gl[1] * scale, 0.12);
    }
    for (const [x, y, rgb, r] of v.lights) tint(toX(x), toY(y), rgb, r * scale, 0.14);
    c.restore();
  }
  if (mood.tint && mood.tintAlpha) {
    c.save();
    c.globalCompositeOperation = "soft-light";
    c.globalAlpha = mood.tintAlpha * 3;
    c.fillStyle = mood.tint;
    c.fillRect(0, 0, w, h);
    c.restore();
  }
  // Vignette, and a little warmth from the top-left like late-morning sun.
  const vg = c.createRadialGradient(w / 2, h / 2, Math.min(w, h) * 0.45, w / 2, h / 2, Math.max(w, h) * 0.75);
  vg.addColorStop(0, "rgba(0,0,0,0)");
  vg.addColorStop(1, "rgba(0,0,0,0.28)");
  c.fillStyle = vg;
  c.fillRect(0, 0, w, h);
  if (mood.clouds) {
    const sun = c.createLinearGradient(0, 0, w * 0.6, h * 0.8);
    sun.addColorStop(0, "rgba(255,240,190,0.10)");
    sun.addColorStop(1, "rgba(255,240,190,0)");
    c.fillStyle = sun;
    c.fillRect(0, 0, w, h);
  }
}
