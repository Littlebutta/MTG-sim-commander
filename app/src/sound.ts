// Sound effects, synthesized with Web Audio so there are no audio files to ship. Each effect is
// a few layered voices (tone, noise, envelope, filter) rather than a single beep.

import type { PlayerView } from "./engine/types";

export type Sfx =
  | "click" | "draw" | "shuffle" | "land" | "cast" | "castOther" | "resolve" | "enter" | "dies"
  | "tap" | "attack" | "block" | "hit" | "gain" | "yourTurn" | "phase" | "prompt" | "victory" | "defeat" | "error";

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let room: ConvolverNode | null = null;
let volume = 0.6;
let enabled = true;
let noiseBuf: AudioBuffer | null = null;

export function configureSound(on: boolean, vol: number) {
  enabled = on;
  volume = Math.max(0, Math.min(1, vol));
  if (master && ctx) master.gain.setTargetAtTime(enabled ? volume : 0, ctx.currentTime, 0.02);
}

function audio(): AudioContext | null {
  if (!enabled || volume <= 0) return null;
  try {
    if (!ctx) {
      const AC: typeof AudioContext | undefined = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!AC) return null;
      ctx = new AC();
      master = ctx.createGain();
      master.gain.value = volume;
      const limiter = ctx.createDynamicsCompressor();
      limiter.threshold.value = -14;
      limiter.ratio.value = 6;
      master.connect(limiter).connect(ctx.destination);
      // A short, soft room so sounds sit on a table instead of inside a speaker.
      room = ctx.createConvolver();
      const len = Math.floor(ctx.sampleRate * 0.9);
      const ir = ctx.createBuffer(2, len, ctx.sampleRate);
      for (let ch = 0; ch < 2; ch++) {
        const d = ir.getChannelData(ch);
        for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / len, 3.2);
      }
      room.buffer = ir;
      const wet = ctx.createGain();
      wet.gain.value = 0.16;
      room.connect(wet).connect(master);
      noiseBuf = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
      const n = noiseBuf.getChannelData(0);
      for (let i = 0; i < n.length; i++) n[i] = Math.random() * 2 - 1;
    }
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

/** Browsers only allow sound after the first click or key press. */
export function unlockSound() {
  audio();
}

interface Voice {
  type?: OscillatorType;
  /** Start and (optional) end frequency in Hz. */
  f: number;
  f2?: number;
  at?: number;
  dur: number;
  gain?: number;
  attack?: number;
  /** How much goes to the room reverb, 0 to 1. */
  wet?: number;
  detune?: number;
}

function out(c: AudioContext, node: AudioNode, wet = 0.3) {
  node.connect(master!);
  if (wet > 0 && room) {
    const send = c.createGain();
    send.gain.value = wet;
    node.connect(send).connect(room);
  }
}

function tone(c: AudioContext, v: Voice) {
  const t0 = c.currentTime + (v.at ?? 0);
  const o = c.createOscillator();
  o.type = v.type ?? "sine";
  o.frequency.setValueAtTime(v.f, t0);
  if (v.f2) o.frequency.exponentialRampToValueAtTime(v.f2, t0 + v.dur);
  if (v.detune) o.detune.value = v.detune;
  const g = c.createGain();
  const peak = v.gain ?? 0.2;
  g.gain.setValueAtTime(0.0001, t0);
  g.gain.exponentialRampToValueAtTime(peak, t0 + (v.attack ?? 0.008));
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + v.dur);
  o.connect(g);
  out(c, g, v.wet ?? 0.3);
  o.start(t0);
  o.stop(t0 + v.dur + 0.05);
}

interface Noise {
  at?: number;
  dur: number;
  gain?: number;
  filter?: BiquadFilterType;
  /** Filter frequency sweep. */
  f: number;
  f2?: number;
  q?: number;
  attack?: number;
  wet?: number;
}

function noise(c: AudioContext, v: Noise) {
  const t0 = c.currentTime + (v.at ?? 0);
  const s = c.createBufferSource();
  s.buffer = noiseBuf;
  s.loop = true;
  s.loopStart = Math.random() * 0.5;
  const f = c.createBiquadFilter();
  f.type = v.filter ?? "bandpass";
  f.frequency.setValueAtTime(v.f, t0);
  if (v.f2) f.frequency.exponentialRampToValueAtTime(v.f2, t0 + v.dur);
  f.Q.value = v.q ?? 0.9;
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, t0);
  g.gain.exponentialRampToValueAtTime(v.gain ?? 0.2, t0 + (v.attack ?? 0.01));
  g.gain.exponentialRampToValueAtTime(0.0001, t0 + v.dur);
  s.connect(f).connect(g);
  out(c, g, v.wet ?? 0.2);
  s.start(t0, Math.random() * 0.5);
  s.stop(t0 + v.dur + 0.05);
}

/** Bell-like note: a fundamental with two quieter, slightly inharmonic partials. */
function bell(c: AudioContext, f: number, at: number, dur: number, gain: number, wet = 0.5) {
  tone(c, { f, at, dur, gain, wet });
  tone(c, { f: f * 2.01, at, dur: dur * 0.6, gain: gain * 0.35, wet });
  tone(c, { f: f * 3.02, at, dur: dur * 0.35, gain: gain * 0.15, wet });
}

/** A card sliding off a pile: paper hiss with a quick flick at the end. */
function cardSlide(c: AudioContext, at = 0, gain = 0.16) {
  noise(c, { at, dur: 0.16, gain, f: 2600, f2: 5200, q: 0.7, attack: 0.03 });
  noise(c, { at: at + 0.12, dur: 0.05, gain: gain * 0.9, filter: "highpass", f: 3800, attack: 0.004 });
}

/** A card set down on the table. */
function cardDown(c: AudioContext, at = 0, gain = 0.22) {
  noise(c, { at, dur: 0.07, gain, filter: "lowpass", f: 1400, f2: 500, attack: 0.003 });
  tone(c, { f: 150, f2: 85, at, dur: 0.11, gain: gain * 0.8, wet: 0.15 });
}

const NOTE = (semis: number, base = 440) => base * Math.pow(2, semis / 12);

const last: Partial<Record<Sfx, number>> = {};

export function play(name: Sfx, opts: { at?: number; pitch?: number } = {}) {
  const c = audio();
  if (!c) return;
  // The same effect twice in a few milliseconds just gets louder, so drop the echo.
  const now = performance.now() + (opts.at ?? 0) * 1000;
  if (last[name] !== undefined && Math.abs(now - last[name]!) < 45) return;
  last[name] = now;
  const at = opts.at ?? 0;
  const p = opts.pitch ?? 1;
  switch (name) {
    case "click":
      tone(c, { type: "triangle", f: 1250 * p, f2: 820, at, dur: 0.045, gain: 0.07, wet: 0.05 });
      noise(c, { at, dur: 0.025, gain: 0.04, filter: "highpass", f: 4200, attack: 0.002, wet: 0 });
      break;
    case "draw":
      cardSlide(c, at);
      break;
    case "shuffle":
      for (let i = 0; i < 7; i++) {
        noise(c, { at: at + i * 0.055, dur: 0.06, gain: 0.1 + (i % 2) * 0.04, f: 3000 + i * 250, q: 1.2, attack: 0.004 });
      }
      cardDown(c, at + 0.44, 0.18);
      break;
    case "land":
      cardDown(c, at, 0.24);
      // Earthy hum underneath.
      tone(c, { f: NOTE(-21), at: at + 0.02, dur: 0.5, gain: 0.12, attack: 0.03, wet: 0.5 });
      tone(c, { f: NOTE(-14), at: at + 0.05, dur: 0.42, gain: 0.06, attack: 0.04, wet: 0.5 });
      break;
    case "cast":
    case "castOther": {
      const base = name === "cast" ? 0 : -5;
      cardSlide(c, at, 0.1);
      // A rising shimmer: three stacked notes with a filtered-air swell behind them.
      [0, 7, 12].forEach((s, i) => {
        tone(c, { type: "triangle", f: NOTE(base + s - 12) * p, at: at + 0.05 + i * 0.05, dur: 0.55, gain: 0.085, attack: 0.05, wet: 0.7 });
        tone(c, { f: NOTE(base + s) * p, at: at + 0.05 + i * 0.05, dur: 0.4, gain: 0.03, attack: 0.05, wet: 0.7, detune: 7 });
      });
      noise(c, { at: at + 0.02, dur: 0.5, gain: 0.05, f: 900, f2: 5200, q: 2.5, attack: 0.2, wet: 0.6 });
      break;
    }
    case "resolve":
      bell(c, NOTE(12) * p, at, 0.5, 0.075);
      bell(c, NOTE(19) * p, at + 0.07, 0.6, 0.055);
      noise(c, { at, dur: 0.25, gain: 0.03, filter: "highpass", f: 5200, attack: 0.02, wet: 0.5 });
      break;
    case "enter":
      cardDown(c, at, 0.26);
      tone(c, { type: "triangle", f: NOTE(-17), f2: NOTE(-12), at: at + 0.02, dur: 0.3, gain: 0.09, attack: 0.02, wet: 0.4 });
      break;
    case "dies":
      tone(c, { type: "triangle", f: NOTE(-9), f2: NOTE(-26), at, dur: 0.5, gain: 0.11, attack: 0.01, wet: 0.5 });
      tone(c, { f: NOTE(-2), f2: NOTE(-19), at: at + 0.03, dur: 0.4, gain: 0.05, wet: 0.5 });
      noise(c, { at, dur: 0.4, gain: 0.07, filter: "lowpass", f: 1800, f2: 220, attack: 0.01, wet: 0.4 });
      break;
    case "tap":
      noise(c, { at, dur: 0.05, gain: 0.09, f: 1900, f2: 3400, q: 1.4, attack: 0.004, wet: 0.05 });
      tone(c, { type: "triangle", f: 520 * p, f2: 390, at, dur: 0.06, gain: 0.05, wet: 0.05 });
      break;
    case "attack":
      noise(c, { at, dur: 0.26, gain: 0.17, f: 500, f2: 3600, q: 1.6, attack: 0.09, wet: 0.3 });
      tone(c, { type: "sawtooth", f: NOTE(-22), f2: NOTE(-17), at: at + 0.02, dur: 0.24, gain: 0.05, attack: 0.05, wet: 0.3 });
      noise(c, { at: at + 0.2, dur: 0.07, gain: 0.1, filter: "highpass", f: 3200, attack: 0.004, wet: 0.3 });
      break;
    case "block":
      noise(c, { at, dur: 0.09, gain: 0.2, filter: "lowpass", f: 2400, f2: 600, attack: 0.003, wet: 0.3 });
      tone(c, { type: "square", f: 310, f2: 240, at, dur: 0.1, gain: 0.04, wet: 0.3 });
      bell(c, 1180, at + 0.005, 0.22, 0.03, 0.4);
      break;
    case "hit":
      tone(c, { f: 130, f2: 46, at, dur: 0.24, gain: 0.3, attack: 0.004, wet: 0.25 });
      noise(c, { at, dur: 0.12, gain: 0.2, filter: "lowpass", f: 2600, f2: 300, attack: 0.003, wet: 0.25 });
      tone(c, { type: "triangle", f: NOTE(-10), f2: NOTE(-15), at: at + 0.05, dur: 0.3, gain: 0.05, wet: 0.5 });
      break;
    case "gain":
      [0, 4, 7, 12].forEach((s, i) => bell(c, NOTE(s + 3) * p, at + i * 0.065, 0.55, 0.055, 0.6));
      break;
    case "yourTurn":
      bell(c, NOTE(-2), at, 0.9, 0.09, 0.7);
      bell(c, NOTE(5), at + 0.13, 1.1, 0.085, 0.7);
      tone(c, { f: NOTE(-14), at, dur: 0.9, gain: 0.05, attack: 0.08, wet: 0.6 });
      break;
    case "phase":
      tone(c, { type: "triangle", f: 700 * p, f2: 880 * p, at, dur: 0.07, gain: 0.03, attack: 0.01, wet: 0.3 });
      break;
    case "prompt":
      bell(c, NOTE(7), at, 0.5, 0.07, 0.6);
      bell(c, NOTE(4), at + 0.14, 0.7, 0.07, 0.6);
      break;
    case "victory":
      [0, 4, 7, 12, 16].forEach((s, i) => {
        bell(c, NOTE(s - 2), at + i * 0.13, 1.3, 0.08, 0.8);
        tone(c, { type: "triangle", f: NOTE(s - 14), at: at + i * 0.13, dur: 0.9, gain: 0.05, attack: 0.03, wet: 0.7 });
      });
      [0, 4, 7].forEach((s) => tone(c, { type: "triangle", f: NOTE(s + 10), at: at + 0.75, dur: 1.6, gain: 0.05, attack: 0.1, wet: 0.8 }));
      break;
    case "defeat":
      [0, -2, -5, -9].forEach((s, i) => {
        tone(c, { type: "triangle", f: NOTE(s - 9), at: at + i * 0.22, dur: 1.0, gain: 0.09, attack: 0.04, wet: 0.8 });
        tone(c, { f: NOTE(s - 21), at: at + i * 0.22, dur: 1.1, gain: 0.07, attack: 0.05, wet: 0.7 });
      });
      break;
    case "error":
      tone(c, { type: "square", f: 196, at, dur: 0.09, gain: 0.04, wet: 0.1 });
      tone(c, { type: "square", f: 165, at: at + 0.1, dur: 0.13, gain: 0.04, wet: 0.1 });
      break;
  }
}

/** Compare two views of the game and play what happened in between. */
export function playChanges(prev: PlayerView, next: PlayerView) {
  if (!enabled) return;
  let at = 0;
  const cue = (name: Sfx, gap = 0.09, pitch?: number) => {
    play(name, { at, pitch });
    at += gap;
  };
  const kind = (v: PlayerView) => Object.keys(v.decision)[0];

  if (kind(next) === "GameOver" && kind(prev) !== "GameOver") {
    const w = (next.decision as { GameOver: { winner: number | null } }).GameOver.winner;
    play(w === next.you ? "victory" : "defeat");
    return;
  }
  if (next.turn !== prev.turn && next.active === next.you && next.turn > 0) cue("yourTurn", 0.2);
  if ((next.players[next.you]?.mulligans ?? 0) > (prev.players[prev.you]?.mulligans ?? 0)) cue("shuffle", 0.5);

  // The stack: new spells and abilities, and things resolving.
  const prevStack = new Set(prev.stack.map((s) => s.id));
  const nextStack = new Set(next.stack.map((s) => s.id));
  const added = next.stack.filter((s) => !prevStack.has(s.id));
  for (const s of added.slice(0, 2)) cue(s.controller === next.you ? "cast" : "castOther", 0.16, s.is_ability ? 1.26 : 1);
  const resolved = prev.stack.filter((s) => !nextStack.has(s.id)).length;

  // The battlefield: arrivals, departures, tapping, attackers.
  const before = new Map(prev.battlefield.map((c) => [c.id, c]));
  const after = new Map(next.battlefield.map((c) => [c.id, c]));
  const arrived = next.battlefield.filter((c) => !before.has(c.id));
  const left = prev.battlefield.filter((c) => !after.has(c.id));
  const lands = arrived.filter((c) => c.types.includes("Land"));
  const others = arrived.filter((c) => !c.types.includes("Land"));
  if (lands.length) cue("land", 0.14);
  if (others.length) cue("enter", 0.14);
  else if (resolved > 0 && !lands.length) cue("resolve", 0.12);
  if (left.length) cue("dies", 0.14);

  const newAttackers = next.battlefield.filter((c) => c.attacking !== null && before.get(c.id)?.attacking === null).length;
  if (newAttackers) cue("attack", 0.16);
  const newBlocks = next.battlefield.filter((c) => (c as { blocking?: number | null }).blocking != null && (before.get(c.id) as { blocking?: number | null } | undefined)?.blocking == null).length;
  if (newBlocks) cue("block", 0.12);
  if (!newAttackers) {
    const tapped = next.battlefield.filter((c) => c.tapped && before.get(c.id)?.tapped === false).length;
    if (tapped) cue("tap", 0.05, 1 + Math.min(tapped - 1, 4) * 0.06);
  }

  // Cards drawn (yours are heard one by one, up to three).
  const drew = next.hand.length - prev.hand.length;
  if (drew > 0 && next.turn > 0) for (let i = 0; i < Math.min(drew, 3); i++) cue("draw", 0.11);

  // Life totals.
  let lost = false;
  let gained = false;
  next.players.forEach((p, i) => {
    const was = prev.players[i]?.life;
    if (was === undefined) return;
    if (p.life < was) lost = true;
    if (p.life > was) gained = true;
  });
  if (lost) cue("hit", 0.12);
  if (gained) cue("gain", 0.12);

  // A question for you.
  const k = kind(next);
  if (next.your_decision && (!prev.your_decision || kind(prev) !== k) && (k === "CommanderZone" || k === "AssignCombatDamage" || k === "Discard" || k === "PutTriggersOnStack")) cue("prompt");
  else if (at === 0 && next.step !== prev.step && next.active === next.you && (next.step === "Main1" || next.step === "BeginCombat" || next.step === "Main2" || next.step === "End")) cue("phase");
}
