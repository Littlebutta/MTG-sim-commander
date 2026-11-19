// The overworld: tile maps, walking, people who notice you, and drawing it all.

import worldJson from "./data/world.json";
import { ANIMATED, DETAIL, TILE, WALKABLE, detailTile, paintObject, paintTile, personSprite, type Dir } from "./art";
import { drawClouds, drawLight, drawParticles, moodFor, softShadow, type View } from "./atmosphere";
import { PEOPLE, REAGENTS, SIGILS, hashString } from "./content";
import type { Place, Save } from "./save";

export interface WorldObject {
  kind: "npc" | "card" | "reagent" | "sign" | "warp" | "waystone" | "well" | "cauldron" | "gate";
  x: number;
  y: number;
  id?: string;
  dir?: Dir;
  sight?: number;
  text?: string;
  region?: string;
  hidden?: boolean;
  chest?: boolean;
  to?: string;
  tx?: number;
  ty?: number;
}

export interface MapData {
  name: string;
  theme: string;
  w: number;
  h: number;
  tiles: string[];
  objects: WorldObject[];
}

interface WorldFile {
  start: Place;
  regions: Record<string, { name: string; rect: [number, number, number, number] }>;
  maps: Record<string, MapData>;
}

export const WORLD = worldJson as unknown as WorldFile;

const DX: Record<Dir, [number, number]> = { up: [0, -1], down: [0, 1], left: [-1, 0], right: [1, 0] };
const OPPOSITE: Record<Dir, Dir> = { up: "down", down: "up", left: "right", right: "left" };
const INSIDE_THEMES = new Set(["shop", "tower"]);
const GROUND_CHARS = new Set(".,\"hmas:gfcpk".split(""));
const SOLID_OBJECTS = new Set(["npc", "sign", "waystone", "well", "cauldron", "gate"]);
/** Open ground a lost card or chest can be left on. */
const SPOT_CHARS = new Set('."hamgscp,f'.split(""));

/**
 * Where this journey's lost cards and chests lie. Each one moves to a random open spot it
 * could already be reached from: in the same region of the overworld, or the same part of a
 * dungeon (never past a gate, a person or a wall it wasn't already behind). Spots stay clear of
 * doors, people and signs, don't crowd each other, and don't close off narrow paths.
 */
function scatter(mapId: string, data: MapData, seed: number): WorldObject[] {
  const loot = (o: WorldObject) => o.kind === "card" || o.kind === "reagent";
  const fixed = data.objects.filter((o) => !loot(o));
  const key = (x: number, y: number) => y * data.w + x;
  const solid = new Set(fixed.filter((o) => SOLID_OBJECTS.has(o.kind)).map((o) => key(o.x, o.y)));
  const walk = (x: number, y: number) => x >= 0 && y >= 0 && x < data.w && y < data.h && WALKABLE.has(data.tiles[y][x]) && !solid.has(key(x, y));
  // Keep a ring of space around anything you walk into or talk to, and where you arrive.
  const near = new Set<number>();
  const ring = (x: number, y: number, r: number) => {
    for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) near.add(key(x + dx, y + dy));
  };
  for (const o of fixed) ring(o.x, o.y, o.kind === "warp" ? 2 : 1);
  for (const m of Object.values(WORLD.maps)) for (const o of m.objects) if (o.kind === "warp" && o.to === mapId && o.tx !== undefined && o.ty !== undefined) ring(o.tx, o.ty, 2);
  if (WORLD.start.map === mapId) ring(WORLD.start.x, WORLD.start.y, 2);
  const out = fixed.slice();
  for (const o of data.objects.filter(loot)) {
    const home = o.region && WORLD.regions[o.region] ? WORLD.regions[o.region].rect : Object.values(WORLD.regions).find(({ rect: r }) => o.x >= r[0] && o.x < r[2] && o.y >= r[1] && o.y < r[3])?.rect;
    const rect = mapId === "overworld" ? home : undefined;
    const inRect = (x: number, y: number) => !rect || (x >= rect[0] && x < rect[2] && y >= rect[1] && y < rect[3]);
    const seen = new Set([key(o.x, o.y)]);
    const queue: [number, number][] = [[o.x, o.y]];
    const spots: [number, number][] = [];
    for (let i = 0; i < queue.length; i++) {
      const [x, y] = queue[i];
      const open = [[1, 0], [-1, 0], [0, 1], [0, -1]].filter(([dx, dy]) => walk(x + dx, y + dy)).length;
      if (inRect(x, y) && SPOT_CHARS.has(data.tiles[y][x]) && !near.has(key(x, y)) && (o.hidden || open >= 3)) spots.push([x, y]);
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx;
        const ny = y + dy;
        if (seen.has(key(nx, ny)) || !walk(nx, ny) || !inRect(nx, ny)) continue;
        seen.add(key(nx, ny));
        queue.push([nx, ny]);
      }
    }
    const [x, y] = spots.length ? spots[hashString(`${seed}:${mapId}:${o.id}`) % spots.length] : [o.x, o.y];
    ring(x, y, 2);
    out.push({ ...o, x, y });
  }
  return out;
}

export type WorldEvent =
  | { type: "interact"; obj: WorldObject }
  | { type: "challenge"; npc: Actor }
  | { type: "warp"; obj: WorldObject }
  | { type: "found"; obj: WorldObject }
  | { type: "region"; id: string; name: string };

/** A person on the map, with where they're standing and facing right now. */
export interface Actor {
  obj: WorldObject;
  id: string;
  x: number;
  y: number;
  /** Pixel offset while walking toward you. */
  fx: number;
  fy: number;
  dir: Dir;
  homeDir: Dir;
  nextTurn: number;
  /** Strolls around near home instead of standing still. */
  wanders: boolean;
  homeX: number;
  homeY: number;
  /** Mid-stroll: the step's direction and 0..1 progress (x,y is already the tile it's entering). */
  stroll: Dir | null;
  strollT: number;
  alert: number;
}

interface Built {
  data: MapData;
  canvas: HTMLCanvasElement;
  under: string[][];
  masks: number[][];
  animated: [number, number][];
}

const tileCache = new Map<string, HTMLCanvasElement>();
function tileCanvas(ch: string, theme: string, variant: number, frame: number, mask: number, under: string, inside: boolean) {
  const key = `${theme}|${ch}|${variant}|${frame}|${mask}|${under}|${inside ? 1 : 0}`;
  let cv = tileCache.get(key);
  if (!cv) {
    cv = document.createElement("canvas");
    cv.width = TILE;
    cv.height = TILE;
    paintTile(cv.getContext("2d")!, ch, theme, variant, frame, mask, under, inside);
    tileCache.set(key, cv);
  }
  return cv;
}

const groupOf = (ch: string | undefined) => {
  if (ch === undefined) return "edge";
  if ("HNd".includes(ch)) return "house";
  if ("Re".includes(ch)) return "rock";
  if ("ZE".includes(ch)) return "spire";
  return ch;
};

const builtMaps = new Map<string, Built>();
function build(id: string): Built {
  const hit = builtMaps.get(id);
  if (hit) return hit;
  const data = WORLD.maps[id];
  const inside = INSIDE_THEMES.has(data.theme);
  const at = (x: number, y: number) => (x >= 0 && y >= 0 && x < data.w && y < data.h ? data.tiles[y][x] : undefined);
  const fallback = data.theme === "vale" ? "." : inside && data.theme === "shop" ? "f" : data.theme === "spire" || data.theme === "rift" ? "p" : "c";
  const under: string[][] = [];
  const masks: number[][] = [];
  const animated: [number, number][] = [];
  for (let y = 0; y < data.h; y++) {
    under.push([]);
    masks.push([]);
    for (let x = 0; x < data.w; x++) {
      const ch = data.tiles[y][x];
      let u = fallback;
      for (const [dx, dy] of [[0, 1], [-1, 0], [1, 0], [0, -1], [1, 1], [-1, 1]]) {
        const n = at(x + dx, y + dy);
        if (n && GROUND_CHARS.has(n) && n !== "k") {
          u = n === "," || n === '"' ? "." : n;
          break;
        }
      }
      under[y].push(u);
      const g = groupOf(ch);
      let m = 0;
      if (groupOf(at(x, y - 1)) !== g) m |= 1;
      if (groupOf(at(x + 1, y)) !== g) m |= 2;
      if (groupOf(at(x, y + 1)) !== g) m |= 4;
      if (groupOf(at(x - 1, y)) !== g) m |= 8;
      // Shores only show against land, not the map edge.
      if (ch === "~") {
        if (at(x, y - 1) === undefined) m &= ~1;
        if (at(x, y + 1) === undefined) m &= ~4;
        if (at(x - 1, y) === undefined) m &= ~8;
        if (at(x + 1, y) === undefined) m &= ~2;
      }
      masks[y].push(m);
      if (ANIMATED.has(ch)) animated.push([x, y]);
    }
  }
  // Baked at twice the tile resolution, with half-pixel detail on top (see detailTile).
  const S = TILE * DETAIL;
  const canvas = document.createElement("canvas");
  canvas.width = data.w * S;
  canvas.height = data.h * S;
  const c = canvas.getContext("2d")!;
  c.imageSmoothingEnabled = false;
  for (let y = 0; y < data.h; y++) {
    for (let x = 0; x < data.w; x++) {
      const variant = hashString(`${x},${y}`) % 4;
      const ch = data.tiles[y][x];
      c.drawImage(tileCanvas(ch, data.theme, variant, 0, masks[y][x], under[y][x], inside), x * S, y * S, S, S);
      if (!ANIMATED.has(ch)) detailTile(c, ch, x * S, y * S, hashString(`${id}:${x},${y}`), [at(x, y - 1), at(x + 1, y), at(x, y + 1), at(x - 1, y)], under[y][x]);
    }
  }
  const b = { data, canvas, under, masks, animated };
  builtMaps.set(id, b);
  return b;
}

export interface Input {
  held: Dir[];
  run: boolean;
  /** A direction pressed since the last frame, even if it was already let go (quick taps). */
  tap?: Dir | null;
}

/** Tiles per second. */
const WALK_SPEED = 6.5;
const RUN_SPEED = 11;

export class World {
  mapId!: string;
  map!: Built;
  actors: Actor[] = [];
  px = 0;
  py = 0;
  dir: Dir = "down";
  /** Walking from (px,py) toward the next tile: 0..1. */
  progress = 0;
  moving: Dir | null = null;
  stepCount = 0;
  turnDelay = 0;
  lastRegion = "";
  time = 0;
  /** A person walking up to challenge you. */
  approaching: { actor: Actor; path: number; done: () => void } | null = null;
  bump = 0;

  /** Each map's objects, with this journey's lost cards and chests scattered. */
  private placed = new Map<string, WorldObject[]>();

  constructor(public save: Save) {
    if (save.seed === undefined) save.seed = Math.floor(Math.random() * 2 ** 31);
    this.enter({ map: save.map, x: save.x, y: save.y, dir: save.dir });
  }

  enter(p: Place) {
    this.mapId = p.map;
    this.map = build(p.map);
    this.px = p.x;
    this.py = p.y;
    this.dir = p.dir;
    this.moving = null;
    this.progress = 0;
    this.actors = this.map.data.objects
      .filter((o) => o.kind === "npc" && o.id && PEOPLE[o.id])
      .map((o) => {
        const p = PEOPLE[o.id!];
        const boss = !!p.duel && p.duel.rank !== "duelist";
        // About half the folk outdoors (and every creature) wander; bosses and shopkeepers keep still.
        const wanders = !boss && this.mapId === "overworld" && (!!p.look.creature || hashString(`wander:${o.id}`) % 2 === 0);
        return { obj: o, id: o.id!, x: o.x, y: o.y, fx: 0, fy: 0, dir: o.dir ?? "down", homeDir: o.dir ?? "down", nextTurn: 1 + Math.random() * 4, alert: 0, wanders, homeX: o.x, homeY: o.y, stroll: null, strollT: 0 };
      });
    // Never arrive inside a wall or a person (say, a position saved before the map changed).
    if (this.blocked(this.px, this.py)) {
      const seen = new Set([`${this.px},${this.py}`]);
      const queue: [number, number][] = [[this.px, this.py]];
      for (let i = 0; i < queue.length && i < 4000; i++) {
        const [x, y] = queue[i];
        if (!this.blocked(x, y)) {
          this.px = x;
          this.py = y;
          break;
        }
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const k = `${x + dx},${y + dy}`;
          if (!seen.has(k) && this.tile(x + dx, y + dy) !== undefined) {
            seen.add(k);
            queue.push([x + dx, y + dy]);
          }
        }
      }
    }
    this.save.map = p.map;
    this.save.x = this.px;
    this.save.y = this.py;
    this.save.dir = p.dir;
  }

  /** Objects that are still in the world (picked-up cards and reagents are gone). */
  objects(): WorldObject[] {
    let all = this.placed.get(this.mapId);
    if (!all) {
      all = scatter(this.mapId, this.map.data, this.save.seed ?? 0);
      this.placed.set(this.mapId, all);
    }
    return all.filter((o) => {
      if ((o.kind === "card" || o.kind === "reagent") && this.save.flags[`got:${this.mapId}:${o.id}`]) return false;
      if (o.kind === "gate" && this.save.flags[`open:${o.id}`]) return false;
      return o.kind !== "npc";
    });
  }

  tile(x: number, y: number): string | undefined {
    const d = this.map.data;
    return x >= 0 && y >= 0 && x < d.w && y < d.h ? d.tiles[y][x] : undefined;
  }

  blocked(x: number, y: number): boolean {
    const t = this.tile(x, y);
    if (t === undefined || !WALKABLE.has(t)) return true;
    if (this.actors.some((a) => a.x === x && a.y === y)) return true;
    return this.objects().some((o) => o.x === x && o.y === y && (SOLID_OBJECTS.has(o.kind) || ((o.kind === "card" || o.kind === "reagent") && !o.hidden)));
  }

  facing(): [number, number] {
    const [dx, dy] = DX[this.dir];
    return [this.px + dx, this.py + dy];
  }

  region(): { id: string; name: string } | null {
    if (this.mapId !== "overworld") return { id: this.mapId, name: this.map.data.name };
    for (const [id, r] of Object.entries(WORLD.regions)) {
      const [x0, y0, x1, y1] = r.rect;
      if (this.px >= x0 && this.px < x1 && this.py >= y0 && this.py < y1) return { id, name: r.name };
    }
    return null;
  }

  /** Press the action button: whatever is in front of you. */
  interact(): WorldEvent | null {
    if (this.moving || this.approaching) return null;
    const [fx, fy] = this.facing();
    const actor = this.actors.find((a) => a.x === fx && a.y === fy);
    if (actor) {
      actor.dir = OPPOSITE[this.dir];
      return { type: "interact", obj: actor.obj };
    }
    // Reach over a shop counter.
    if (this.tile(fx, fy) === "K") {
      const [dx, dy] = DX[this.dir];
      const over = this.actors.find((a) => a.x === fx + dx && a.y === fy + dy);
      if (over) {
        over.dir = OPPOSITE[this.dir];
        return { type: "interact", obj: over.obj };
      }
    }
    const obj = this.objects().find((o) => o.x === fx && o.y === fy && o.kind !== "warp");
    if (obj) return { type: obj.kind === "card" || obj.kind === "reagent" ? "found" : "interact", obj };
    return null;
  }

  private sightCheck(): Actor | null {
    for (const a of this.actors) {
      const p = PEOPLE[a.id];
      const sight = a.obj.sight ?? 0;
      if (!p.duel || sight <= 0 || this.save.flags[`beat:${a.id}`] || this.save.flags[`wait:${a.id}`]) continue;
      const [dx, dy] = DX[a.dir];
      for (let i = 1; i <= sight; i++) {
        const x = a.x + dx * i;
        const y = a.y + dy * i;
        if (x === this.px && y === this.py) return a;
        const t = this.tile(x, y);
        if (t === undefined || !WALKABLE.has(t) || this.actors.some((o) => o.x === x && o.y === y)) break;
        if (this.objects().some((o) => o.x === x && o.y === y && SOLID_OBJECTS.has(o.kind))) break;
      }
    }
    return null;
  }

  /** Advance the world. Returns something that happened, if anything did. */
  update(dt: number, input: Input, paused: boolean): WorldEvent | null {
    this.time += dt;
    this.bump = Math.max(0, this.bump - dt);
    if (this.approaching) {
      const ap = this.approaching;
      const a = ap.actor;
      if (a.stroll) {
        a.stroll = null;
        a.fx = a.fy = 0;
      }
      const [dx, dy] = DX[a.dir];
      if (a.alert > 0) {
        a.alert -= dt;
        return null;
      }
      const dist = Math.abs(a.x - this.px) + Math.abs(a.y - this.py);
      if (dist <= 1) {
        a.fx = a.fy = 0;
        this.dir = OPPOSITE[a.dir];
        this.approaching = null;
        ap.done();
        return null;
      }
      ap.path += dt * 5;
      if (ap.path >= 1) {
        ap.path = 0;
        a.x += dx;
        a.y += dy;
        a.fx = a.fy = 0;
      } else {
        a.fx = dx * ap.path * TILE;
        a.fy = dy * ap.path * TILE;
      }
      return null;
    }
    if (paused) return null;
    // People glance around now and then.
    for (const a of this.actors) {
      if (a.stroll) {
        // A leisurely step, easing in and out.
        a.strollT = Math.min(1, a.strollT + dt * 2.6);
        const [sx, sy] = DX[a.stroll];
        const ease = a.strollT * a.strollT * (3 - 2 * a.strollT);
        a.fx = -sx * (1 - ease) * TILE;
        a.fy = -sy * (1 - ease) * TILE;
        if (a.strollT >= 1) {
          a.stroll = null;
          a.fx = a.fy = 0;
        }
        continue;
      }
      a.nextTurn -= dt;
      if (a.nextTurn <= 0 && a.wanders && Math.random() < 0.6) {
        a.nextTurn = 1.2 + Math.random() * 3;
        const dirs: Dir[] = ["up", "down", "left", "right"];
        const d = dirs[Math.floor(Math.random() * 4)];
        const [sx, sy] = DX[d];
        const nx = a.x + sx;
        const ny = a.y + sy;
        const [mx, my] = this.moving ? DX[this.moving] : [0, 0];
        const home = Math.abs(nx - a.homeX) + Math.abs(ny - a.homeY) <= 3;
        const free = !this.blocked(nx, ny) && !(nx === this.px && ny === this.py) && !(nx === this.px + mx && ny === this.py + my);
        const warp = this.objects().some((o) => o.kind === "warp" && Math.abs(o.x - nx) + Math.abs(o.y - ny) <= 1);
        a.dir = d;
        if (home && free && !warp) {
          a.x = nx;
          a.y = ny;
          a.stroll = d;
          a.strollT = 0;
          a.fx = -sx * TILE;
          a.fy = -sy * TILE;
        }
        continue;
      }
      if (a.nextTurn <= 0) {
        const p = PEOPLE[a.id];
        const boss = p.duel && p.duel.rank !== "duelist";
        a.nextTurn = 2.5 + Math.random() * 4;
        if (!boss) {
          const dirs: Dir[] = ["up", "down", "left", "right"];
          a.dir = a.dir !== a.homeDir || Math.random() < 0.5 ? a.homeDir : dirs[Math.floor(Math.random() * 4)];
        }
      }
    }
    if (this.moving) {
      // Ease into the first step from a standstill; after that, keep a steady pace.
      const speed = (input.run ? RUN_SPEED : WALK_SPEED) * (this.chain ? 1 : 0.8 + 0.2 * this.progress);
      this.progress += dt * speed;
      if (this.progress >= 1) {
        const extra = this.progress - 1;
        const [dx, dy] = DX[this.moving];
        this.px += dx;
        this.py += dy;
        this.progress = 0;
        this.moving = null;
        this.stepCount++;
        this.save.x = this.px;
        this.save.y = this.py;
        this.save.dir = this.dir;
        const ev = this.arrived();
        if (ev) {
          this.chain = false;
          return ev;
        }
        // Still holding a direction: carry straight on into the next tile with no pause, turning
        // corners without stopping.
        const want = input.held[input.held.length - 1];
        if (want) {
          this.dir = want;
          const [nx, ny] = DX[want];
          if (!this.blocked(this.px + nx, this.py + ny)) {
            this.moving = want;
            this.progress = Math.min(extra, 0.5);
            this.chain = true;
            return null;
          }
          this.bump = 0.2;
        }
        this.chain = false;
      }
      return null;
    }
    // Someone may have turned to face you while you stood still.
    const seen = this.sightCheck();
    if (seen) return { type: "challenge", npc: seen };
    const held = input.held[input.held.length - 1];
    const want = held ?? input.tap ?? undefined;
    if (!want) {
      this.turnDelay = 0;
      this.wasWalking = false;
      return null;
    }
    // A quick tap toward a new direction just turns you to face it; holding it walks.
    if (want !== this.dir && !this.wasWalking) {
      this.dir = want;
      this.turnDelay = held ? 0.07 : 0;
      return null;
    }
    this.turnDelay -= dt;
    if (this.turnDelay > 0) return null;
    this.dir = want;
    const [dx, dy] = DX[want];
    if (this.blocked(this.px + dx, this.py + dy)) {
      this.bump = 0.2;
      this.wasWalking = false;
      return null;
    }
    this.moving = want;
    this.chain = false;
    this.wasWalking = !!held;
    return null;
  }
  /** Walking on from the last tile without stopping. */
  private chain = false;
  private wasWalking = false;

  private arrived(): WorldEvent | null {
    const here = this.objects();
    const warp = here.find((o) => o.kind === "warp" && o.x === this.px && o.y === this.py);
    if (warp) return { type: "warp", obj: warp };
    const hidden = here.find((o) => (o.kind === "card" || o.kind === "reagent") && o.hidden && o.x === this.px && o.y === this.py);
    if (hidden) return { type: "found", obj: hidden };
    const r = this.region();
    if (r && r.id !== this.lastRegion) {
      this.lastRegion = r.id;
      return { type: "region", ...r };
    }
    const seen = this.sightCheck();
    if (seen) return { type: "challenge", npc: seen };
    return null;
  }

  /** The challenger shouts, then walks up to you. */
  startApproach(actor: Actor, done: () => void) {
    actor.alert = 0.7;
    // Face the player along the line they were seen on.
    if (actor.x === this.px) actor.dir = this.py > actor.y ? "down" : "up";
    else actor.dir = this.px > actor.x ? "right" : "left";
    this.approaching = { actor, path: 0, done };
    this.moving = null;
  }

  // ---------------------------------------------------------------- drawing

  private lastDraw = 0;
  private cam: { map: string; x: number; y: number } | null = null;

  draw(c: CanvasRenderingContext2D, w: number, h: number, scale: number) {
    const d = this.map.data;
    const dt = Math.min(0.1, Math.max(0, this.time - this.lastDraw));
    this.lastDraw = this.time;
    const inside = INSIDE_THEMES.has(d.theme);
    const [mdx, mdy] = this.moving ? DX[this.moving] : [0, 0];
    const ppx = (this.px + mdx * this.progress) * TILE;
    const ppy = (this.py + mdy * this.progress) * TILE;
    const vw = w / scale;
    const vh = h / scale;
    let camX = ppx + TILE / 2 - vw / 2;
    let camY = ppy + TILE / 2 - vh / 2;
    const mw = d.w * TILE;
    const mh = d.h * TILE;
    camX = mw <= vw ? (mw - vw) / 2 : Math.max(0, Math.min(mw - vw, camX));
    camY = mh <= vh ? (mh - vh) / 2 : Math.max(0, Math.min(mh - vh, camY));
    // The camera glides after you instead of being bolted on; it snaps on a new map.
    if (this.cam && this.cam.map === this.mapId && Math.abs(this.cam.x - camX) < vw && Math.abs(this.cam.y - camY) < vh) {
      const k = 1 - Math.exp(-dt * 10);
      this.cam.x += (camX - this.cam.x) * k;
      this.cam.y += (camY - this.cam.y) * k;
    } else this.cam = { map: this.mapId, x: camX, y: camY };
    camX = this.cam.x;
    camY = this.cam.y;
    camX = Math.round(camX * scale) / scale;
    camY = Math.round(camY * scale) / scale;

    c.save();
    c.imageSmoothingEnabled = false;
    c.fillStyle = d.theme === "vale" ? "#2d5a27" : "#08060b";
    c.fillRect(0, 0, w, h);
    c.scale(scale, scale);
    c.translate(-camX, -camY);
    c.drawImage(this.map.canvas, 0, 0, mw, mh);

    const x0 = Math.floor(camX / TILE) - 1;
    const y0 = Math.floor(camY / TILE) - 1;
    const x1 = Math.ceil((camX + vw) / TILE) + 1;
    const y1 = Math.ceil((camY + vh) / TILE) + 1;
    const frame = Math.floor(this.time * 3) % 4;
    for (const [x, y] of this.map.animated) {
      if (x < x0 || x > x1 || y < y0 || y > y1) continue;
      const variant = hashString(`${x},${y}`) % 4;
      c.drawImage(tileCanvas(d.tiles[y][x], d.theme, variant, frame, this.map.masks[y][x], this.map.under[y][x], inside), x * TILE, y * TILE);
    }

    // Light and shade: what glows, and the mood of where you are.
    const mood = moodFor(d.theme, this.mapId === "overworld" ? this.region()?.id ?? null : null);
    const glows: [string, number, number][] = [];
    const shadowTiles: [number, number][] = [];
    for (let y = Math.max(0, y0); y <= Math.min(d.h - 1, y1); y++) {
      const row = d.tiles[y];
      for (let x = Math.max(0, x0); x <= Math.min(d.w - 1, x1); x++) {
        const ch = row[x];
        if (ch === "L" || ch === "Y" || ch === "x" || ch === "E" || (ch === "N" && mood.dark > 0.3)) glows.push([ch, x * TILE + 8, y * TILE + 8]);
        else if (ch === "T" || ch === "P" || ch === "r") shadowTiles.push([x, y]);
      }
    }
    const lights: [number, number, string, number][] = [];
    const view: View = { camX, camY, vw, vh, scale, w, h, time: this.time, dt, px: ppx + 8, py: ppy + 8, glows, lights };
    for (const [x, y] of shadowTiles) softShadow(c, x * TILE + 8, y * TILE + 15.5, 7.5, 2.4, 0.22);
    drawClouds(c, mood, view);

    // Things and people, back to front.
    type Drawable = { y: number; draw: () => void };
    const list: Drawable[] = [];
    const lit = this.save.sigils.length;
    for (const o of this.objects()) {
      if (o.kind === "warp") continue;
      if (o.x < x0 || o.x > x1 || o.y < y0 || o.y > y1) continue;
      const ox = o.x * TILE;
      const oy = o.y * TILE;
      if ((o.kind === "card" || o.kind === "reagent") && o.hidden) {
        list.push({ y: oy, draw: () => paintObject(c, "glint", ox, oy, this.time) });
        continue;
      }
      const color = o.kind === "reagent" && o.id ? REAGENTS[o.id]?.color : o.kind === "waystone" && o.id === this.save.stone ? "#ffd25a" : undefined;
      if (o.kind === "cauldron") lights.push([ox + 8, oy + 8, "120,255,140", 40]);
      else if (o.kind === "waystone") lights.push([ox + 8, oy + 6, o.id === this.save.stone ? "255,210,90" : "140,200,255", 30]);
      else if (o.kind === "reagent") lights.push([ox + 8, oy + 8, "255,230,160", 22]);
      else if (o.kind === "gate") lights.push([ox + 8, oy + 8, "190,140,255", 18 + lit * 8]);
      if (o.kind !== "sign" && o.kind !== "gate") list.push({ y: oy - 0.1, draw: () => softShadow(c, ox + 8, oy + 14.5, 6, 2, 0.22) });
      list.push({ y: oy, draw: () => paintObject(c, o.kind, ox, oy, this.time, { color, chest: o.chest, lit, theme: d.theme }) });
    }
    for (const a of this.actors) {
      if (a.x < x0 || a.x > x1 || a.y < y0 || a.y > y1) continue;
      const p = PEOPLE[a.id];
      const ax = a.x * TILE + a.fx;
      const ay = a.y * TILE + a.fy;
      const walking = (this.approaching?.actor === a && a.alert <= 0) || !!a.stroll;
      list.push({
        y: ay,
        draw: () => {
          if (p.look.aura) {
            c.globalAlpha = 0.25 + 0.15 * Math.sin(this.time * 3);
            c.fillStyle = p.look.aura;
            c.beginPath();
            c.ellipse(ax + 8, ay + 9, 10, 9, 0, 0, Math.PI * 2);
            c.fill();
            c.globalAlpha = 1;
          }
          softShadow(c, ax + 8, ay + 14.8, 6, 2.2, 0.3);
          c.drawImage(personSprite(p.look, a.dir, walking ? Math.floor(this.time * 8) % 2 : 0), ax, ay - 1, TILE, TILE);
          if (a.alert > 0) {
            // "!" over their head.
            c.fillStyle = "#ffffff";
            c.fillRect(ax + 5, ay - 12, 6, 10);
            c.fillStyle = "#e2553a";
            c.fillRect(ax + 7, ay - 11, 2, 5);
            c.fillRect(ax + 7, ay - 5, 2, 2);
          }
          if (p.duel && this.save.flags[`beat:${a.id}`] && p.duel.rank === "duelist") return;
        },
      });
    }
    const look = this.playerLook();
    const pf = this.moving ? (this.progress < 0.5 ? 1 : 0) * ((this.stepCount % 2) + 1) : 0;
    list.push({
      y: ppy,
      draw: () => {
        softShadow(c, ppx + 8, ppy + 14.8, 6, 2.2, 0.3);
        c.drawImage(personSprite(look, this.dir, pf ? 1 : 0), ppx, ppy - 1 - (pf ? 1 : 0), TILE, TILE);
      },
    });
    list.sort((a, b) => a.y - b.y);
    for (const it of list) it.draw();
    drawParticles(c, mood, view);
    c.restore();
    drawLight(c, mood, view);
  }

  playerLook() {
    return PLAYER_LOOK;
  }
}

export const PLAYER_LOOK = { skin: "#f0c8a0", hair: "#5a3a22", cloth: "#d9573b", accent: "#f5e6a8", extra: "cap" as const, extraColor: "#2e6fb7" };

export function sigilColor(id: string) {
  return SIGILS[id]?.color ?? "#ffffff";
}
