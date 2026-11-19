// Adventure music: chiptune pieces composed on the fly with Web Audio. Each part of the world
// has its own mood and an endless run of pieces in it (new chords, tunes, key, tempo, bass and
// drums each time), so it doesn't loop. No audio files to ship. M mutes it; the choice is remembered.

type Mood = {
  /** MIDI note of the key's root. */
  root: number;
  scale: number[];
  bpm: number;
  /** Chord roots as scale degrees (0-based), one per bar. */
  chords: number[];
  lead: OscillatorType;
  /** 0 to 1: how busy the melody is. */
  busy: number;
  /** Arpeggio pattern over the chord tones, in eighth notes. */
  arp: number[];
  drums: boolean;
  seed: number;
};

const MAJOR = [0, 2, 4, 5, 7, 9, 11];
const MINOR = [0, 2, 3, 5, 7, 8, 10];
const DORIAN = [0, 2, 3, 5, 7, 9, 10];
const MIXO = [0, 2, 4, 5, 7, 9, 10];
const PHRYG = [0, 1, 3, 5, 7, 8, 10];

const MOODS: Record<string, Mood> = {
  meadow: { root: 60, scale: MAJOR, bpm: 112, chords: [0, 4, 5, 3, 0, 3, 4, 4], lead: "square", busy: 0.6, arp: [0, 1, 2, 1, 0, 1, 2, 1], drums: true, seed: 3 },
  village: { root: 62, scale: MAJOR, bpm: 100, chords: [0, 3, 4, 0, 5, 3, 1, 4], lead: "square", busy: 0.5, arp: [0, 2, 1, 2, 0, 2, 1, 2], drums: true, seed: 11 },
  home: { root: 65, scale: MAJOR, bpm: 84, chords: [0, 5, 3, 4, 0, 5, 1, 4], lead: "triangle", busy: 0.35, arp: [0, 1, 2, 3, 2, 1, 0, 1], drums: false, seed: 5 },
  lake: { root: 64, scale: MAJOR, bpm: 88, chords: [0, 2, 3, 4, 0, 2, 3, 3], lead: "triangle", busy: 0.4, arp: [0, 1, 2, 3, 4, 3, 2, 1], drums: false, seed: 17 },
  coast: { root: 62, scale: MIXO, bpm: 104, chords: [0, 6, 3, 0, 0, 6, 4, 4], lead: "square", busy: 0.55, arp: [0, 2, 1, 2, 3, 2, 1, 2], drums: true, seed: 23 },
  mire: { root: 57, scale: MINOR, bpm: 76, chords: [0, 5, 3, 4, 0, 5, 6, 4], lead: "triangle", busy: 0.35, arp: [0, 2, 1, 2, 0, 2, 1, 2], drums: false, seed: 29 },
  high: { root: 60, scale: MAJOR, bpm: 96, chords: [0, 3, 0, 4, 5, 3, 4, 0], lead: "square", busy: 0.45, arp: [0, 1, 2, 1, 0, 1, 2, 1], drums: true, seed: 31 },
  ember: { root: 59, scale: PHRYG, bpm: 132, chords: [0, 0, 1, 0, 5, 5, 6, 4], lead: "sawtooth", busy: 0.7, arp: [0, 0, 2, 0, 1, 0, 2, 0], drums: true, seed: 37 },
  wilds: { root: 62, scale: DORIAN, bpm: 108, chords: [0, 3, 0, 3, 6, 3, 4, 0], lead: "square", busy: 0.6, arp: [0, 2, 1, 2, 0, 2, 1, 2], drums: true, seed: 41 },
  dungeon: { root: 55, scale: MINOR, bpm: 92, chords: [0, 5, 0, 4, 0, 5, 6, 4], lead: "triangle", busy: 0.4, arp: [0, 1, 2, 0, 1, 2, 0, 1], drums: true, seed: 43 },
  spire: { root: 56, scale: PHRYG, bpm: 80, chords: [0, 1, 0, 6, 0, 1, 5, 4], lead: "sawtooth", busy: 0.35, arp: [0, 2, 3, 2, 0, 2, 3, 2], drums: false, seed: 47 },
  duel: { root: 57, scale: MINOR, bpm: 140, chords: [0, 5, 6, 4, 0, 5, 3, 4], lead: "square", busy: 0.75, arp: [0, 1, 2, 1, 0, 1, 2, 1], drums: true, seed: 53 },
};

/** Chord progressions (scale degrees, one per bar) that new pieces are built from. */
const BRIGHT = [
  [0, 4, 5, 3, 0, 3, 4, 4], [0, 3, 4, 0, 5, 3, 1, 4], [0, 5, 3, 4, 0, 5, 1, 4], [0, 2, 3, 4, 0, 2, 3, 3],
  [0, 3, 0, 4, 5, 3, 4, 0], [0, 0, 3, 3, 5, 5, 4, 4], [3, 4, 2, 5, 1, 4, 0, 0], [0, 6, 3, 0, 0, 6, 4, 4],
  [5, 3, 0, 4, 5, 3, 1, 4], [0, 1, 3, 4, 0, 1, 4, 4], [0, 4, 0, 3, 1, 4, 0, 0], [3, 0, 4, 5, 3, 0, 1, 4],
];
const DARK = [
  [0, 5, 3, 4, 0, 5, 6, 4], [0, 5, 0, 4, 0, 5, 6, 4], [0, 0, 1, 0, 5, 5, 6, 4], [0, 3, 0, 4, 5, 3, 6, 6],
  [0, 6, 5, 6, 0, 6, 5, 4], [0, 2, 3, 4, 5, 3, 1, 4], [5, 5, 3, 3, 0, 0, 4, 4], [0, 1, 0, 6, 0, 1, 5, 4],
  [0, 3, 5, 4, 0, 3, 6, 4], [3, 2, 0, 0, 3, 2, 4, 4], [0, 6, 3, 4, 0, 6, 5, 5], [0, 4, 5, 2, 3, 0, 1, 4],
];
const ARPS = [
  [0, 1, 2, 1, 0, 1, 2, 1], [0, 2, 1, 2, 0, 2, 1, 2], [0, 1, 2, 3, 2, 1, 0, 1], [0, 1, 2, 3, 4, 3, 2, 1],
  [0, 0, 2, 0, 1, 0, 2, 0], [0, 2, 3, 2, 0, 2, 3, 2], [2, 1, 0, 1, 2, 1, 0, 3], [0, 3, 1, 3, 2, 3, 1, 3],
];
const LEADS: OscillatorType[] = ["square", "triangle", "square", "sawtooth"];

/** Map and region names to a mood. */
function moodFor(where: string): string {
  if (MOODS[where]) return where;
  if (["shop", "inn", "house_a", "house_b", "tower"].includes(where)) return "home";
  if (["barrow", "sanctum", "cinder", "grotto", "hollow"].includes(where)) return "dungeon";
  if (where.startsWith("spire")) return "spire";
  return "meadow";
}

const LS = "commander-table.music";
let muted = (() => {
  try {
    return localStorage.getItem(LS) === "off";
  } catch {
    return false;
  }
})();

let ctx: AudioContext | null = null;
let bus: GainNode | null = null;
let timer = 0;
let current = "";
let nextTime = 0;
let step = 0;
let playing = false;

/** One piece: two sections with their own chords and tunes, played A A B A. */
interface Song {
  root: number;
  scale: number[];
  bpm: number;
  lead: OscillatorType;
  arp: number[];
  chordsA: number[];
  chordsB: number[];
  melA: number[];
  melB: number[];
  /** 0 root-fifth, 1 walking, 2 pulsing octaves. */
  bass: number;
  /** 0 none, 1 steady, 2 half-time, 3 busy. */
  drums: number;
  /** A soft second voice in the B section. */
  harmony: boolean;
}
let song: Song | null = null;
/** Which piece of the current mood is playing; each area cycles through its own pieces. */
const pieceOf: Record<string, number> = {};

const VOLUME = 0.22;
const STRUCTURE = ["A", "A", "B", "A"];
const BARS = STRUCTURE.length * 8;

function rng(seed: number) {
  let s = (seed * 9301 + 49297) % 233280;
  return () => {
    s = (s * 9301 + 49297) % 233280;
    return s / 233280;
  };
}

/** Eight bars of eighth-note melody (scale steps, or -1 for a rest). */
function compose(chords: number[], busy: number, r: () => number, start: number): number[] {
  const out: number[] = [];
  let note = start;
  // A two-bar rhythm motif, reused so the tune sounds like a tune and not a random walk.
  const motif = Array.from({ length: 16 }, (_, i) => i % 4 === 0 || r() < busy);
  for (let bar = 0; bar < 8; bar++) {
    const chord = chords[bar];
    for (let e = 0; e < 8; e++) {
      const i = (bar % 2) * 8 + e;
      const strong = e % 4 === 0;
      const last = bar === 7 && e >= 4;
      if (last) {
        out.push(e === 4 ? chord : -1);
        continue;
      }
      if (!motif[i]) {
        out.push(-1);
        continue;
      }
      if (strong) {
        const tones = [chord, chord + 2, chord + 4, chord + 7];
        note = tones.reduce((best, t) => (Math.abs(t - note) < Math.abs(best - note) ? t : best), tones[0]);
      } else {
        note += [-2, -1, -1, 1, 1, 2, 3, -3][Math.floor(r() * 8)];
      }
      note = Math.max(-1, Math.min(11, note));
      out.push(Math.max(0, note));
    }
  }
  return out;
}

function makeSong(moodId: string, piece: number): Song {
  const m = MOODS[moodId] ?? MOODS.meadow;
  const r = rng(m.seed * 1000 + piece * 37 + 1);
  const minorish = m.scale === MINOR || m.scale === PHRYG || m.scale === DORIAN;
  const pool = minorish ? DARK : BRIGHT;
  // The first piece of each area is the one it always had; later ones wander further.
  const chordsA = piece === 0 ? m.chords : pool[Math.floor(r() * pool.length)];
  let chordsB = pool[Math.floor(r() * pool.length)];
  if (chordsB === chordsA) chordsB = pool[(pool.indexOf(chordsA) + 3) % pool.length];
  const shift = piece === 0 ? 0 : [0, 2, -2, 5, -3, 3, -5][Math.floor(r() * 7)];
  const busy = Math.max(0.2, Math.min(0.85, m.busy + (r() - 0.5) * 0.25));
  return {
    root: m.root + shift,
    scale: m.scale,
    bpm: Math.round(m.bpm * (piece === 0 ? 1 : 0.9 + r() * 0.2)),
    lead: piece === 0 ? m.lead : m.lead === "sawtooth" ? "sawtooth" : LEADS[Math.floor(r() * LEADS.length)],
    arp: piece === 0 ? m.arp : ARPS[Math.floor(r() * ARPS.length)],
    chordsA,
    chordsB,
    melA: compose(chordsA, busy, r, 4),
    melB: compose(chordsB, Math.min(0.9, busy + 0.1), r, 6),
    bass: Math.floor(r() * 3),
    drums: m.drums ? 1 + Math.floor(r() * 3) : r() < 0.25 ? 2 : 0,
    harmony: r() < 0.7,
  };
}

const midi = (n: number) => 440 * Math.pow(2, (n - 69) / 12);
const degree = (s: Song, d: number, octave = 0) => s.root + octave * 12 + s.scale[((d % 7) + 7) % 7] + Math.floor(d / 7) * 12;

function tone(type: OscillatorType, freq: number, at: number, dur: number, gain: number, vibrato = false) {
  if (!ctx || !bus) return;
  const o = ctx.createOscillator();
  const g = ctx.createGain();
  o.type = type;
  o.frequency.value = freq;
  if (vibrato) {
    const lfo = ctx.createOscillator();
    const lg = ctx.createGain();
    lfo.frequency.value = 5.5;
    lg.gain.value = freq * 0.006;
    lfo.connect(lg).connect(o.frequency);
    lfo.start(at + 0.08);
    lfo.stop(at + dur + 0.05);
  }
  g.gain.setValueAtTime(0, at);
  g.gain.linearRampToValueAtTime(gain, at + 0.01);
  g.gain.setTargetAtTime(gain * 0.6, at + 0.03, 0.08);
  g.gain.setTargetAtTime(0, at + dur * 0.85, 0.03);
  o.connect(g).connect(bus);
  o.start(at);
  o.stop(at + dur + 0.2);
}

function noise(at: number, dur: number, gain: number, hp: number) {
  if (!ctx || !bus) return;
  const len = Math.floor(ctx.sampleRate * dur);
  const b = ctx.createBuffer(1, len, ctx.sampleRate);
  const d = b.getChannelData(0);
  for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / len);
  const src = ctx.createBufferSource();
  src.buffer = b;
  const f = ctx.createBiquadFilter();
  f.type = "highpass";
  f.frequency.value = hp;
  const g = ctx.createGain();
  g.gain.value = gain;
  src.connect(f).connect(g).connect(bus);
  src.start(at);
}

function nextSong() {
  const n = (pieceOf[current] ?? -1) + 1;
  pieceOf[current] = n;
  song = makeSong(current, n);
  step = 0;
}

function tick() {
  if (!ctx || !playing || !song) return;
  while (nextTime < ctx.currentTime + 0.25) {
    if (step >= BARS * 8) {
      nextSong();
      // A breath between pieces.
      nextTime += 1.2;
    }
    const s = song!;
    const eighth = 60 / s.bpm / 2;
    const barAll = Math.floor(step / 8);
    const section = STRUCTURE[Math.floor(barAll / 8)];
    const bar = barAll % 8;
    const e = step % 8;
    const chords = section === "B" ? s.chordsB : s.chordsA;
    const chord = chords[bar];
    const mel = section === "B" ? s.melB : s.melA;
    const at = nextTime;
    // Bass.
    if (s.bass === 0 && e % 2 === 0) tone("triangle", midi(degree(s, chord, -2) + (e === 2 || e === 6 ? 7 : 0)), at, eighth * 1.8, 0.32);
    else if (s.bass === 1 && e % 2 === 0) tone("triangle", midi(degree(s, chord + [0, 2, 4, 2][e / 2], -2)), at, eighth * 1.8, 0.3);
    else if (s.bass === 2) tone("triangle", midi(degree(s, chord, -2) + (e % 2 ? 12 : 0)), at, eighth * 0.9, 0.26);
    // Soft arpeggio.
    const tones = [0, 2, 4, 7];
    tone("square", midi(degree(s, chord + tones[s.arp[e] % 4], -1)), at, eighth * 0.9, 0.035);
    // Melody, and a second voice a third below in the B section.
    const n = mel[bar * 8 + e];
    if (n >= 0) {
      let len = 1;
      while (len < 4 && mel[bar * 8 + e + len] === -1 && (e + len) % 8 !== 0) len++;
      // The last A plays the tune an octave up for its first half, for lift.
      const lift = section === "A" && barAll >= 24 && bar < 4 ? 12 : 0;
      tone(s.lead, midi(degree(s, n, 0) + lift), at, eighth * len * 0.95, s.lead === "sawtooth" ? 0.05 : 0.08, len > 1);
      if (section === "B" && s.harmony) tone("triangle", midi(degree(s, n - 2, 0)), at, eighth * len * 0.95, 0.05);
    }
    if (s.drums) {
      const kick = s.drums === 2 ? e === 0 : s.drums === 3 ? e === 0 || e === 3 || e === 4 : e === 0 || e === 4;
      const snare = s.drums === 2 ? e === 4 : e === 2 || e === 6;
      if (kick) tone("sine", 70, at, 0.12, 0.35);
      if (snare) noise(at, 0.1, 0.12, 1800);
      if (s.drums !== 2 && e % 2 === 1) noise(at, 0.03, 0.05, 6000);
    }
    nextTime += eighth;
    step++;
  }
}

function ensure(): boolean {
  if (ctx) {
    if (ctx.state === "suspended") void ctx.resume();
    return true;
  }
  try {
    const AC: typeof AudioContext | undefined = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return false;
    ctx = new AC();
    bus = ctx.createGain();
    bus.gain.value = muted ? 0 : VOLUME;
    const lp = ctx.createBiquadFilter();
    lp.type = "lowpass";
    lp.frequency.value = 5200;
    bus.connect(lp).connect(ctx.destination);
    // Browsers only start audio after a key press or click.
    const resume = () => {
      if (ctx?.state === "suspended") void ctx.resume();
    };
    window.addEventListener("keydown", resume);
    window.addEventListener("pointerdown", resume);
    return true;
  } catch {
    return false;
  }
}

export function startMusic() {
  if (!ensure() || playing) return;
  playing = true;
  nextTime = ctx!.currentTime + 0.1;
  timer = window.setInterval(tick, 60);
}

export function stopMusic() {
  playing = false;
  window.clearInterval(timer);
  current = "";
}

/** Change the music when you walk somewhere new (it fades across). Coming back to an area
 * moves on to its next piece rather than starting the same one again. */
export function setMusicMood(where: string) {
  const id = moodFor(where);
  if (id === current) return;
  const first = current === "";
  current = id;
  nextSong();
  if (!ctx || !bus || first || muted) return;
  const t = ctx.currentTime;
  bus.gain.cancelScheduledValues(t);
  bus.gain.setValueAtTime(0, t);
  bus.gain.linearRampToValueAtTime(VOLUME, t + 1.2);
  nextTime = t + 0.15;
}

export function musicMuted(): boolean {
  return muted;
}

/** Mute or unmute; returns whether it's now muted. */
export function toggleMusic(): boolean {
  muted = !muted;
  try {
    localStorage.setItem(LS, muted ? "off" : "on");
  } catch {
    /* remembered for this session only */
  }
  if (ctx && bus) bus.gain.setTargetAtTime(muted ? 0 : VOLUME, ctx.currentTime, 0.1);
  return muted;
}
