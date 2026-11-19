// Card pools: which cards can turn up lying around the Vale. A journey can draw from one or
// more of them; the choice lives in the save and can be changed from the journal.

import { builtinDecks, listDecks, lookupCards, searchCards, type CardSummary } from "../decks";
import { LOST_POOLS, isBasic, lostCard, parseList } from "./content";

export type PoolId = "elderreach" | "library" | "decks" | "mine";

export const POOLS: { id: PoolId; name: string; text: string }[] = [
  { id: "elderreach", name: "Cards of the Vale", text: "Hand-picked cards that suit each region: knights in the highlands, merfolk by the lake, dragons in the cinder lands." },
  { id: "library", name: "The whole library", text: "Any card in Magic. On the desktop that's the full downloaded card library, so every find is a surprise." },
  { id: "decks", name: "Commander deck cards", text: "Cards from the ready-made Commander decks." },
  { id: "mine", name: "Cards from your decks", text: "Cards from the decks you've built in the deck builder." },
];

export const DEFAULT_POOLS: PoolId[] = ["library"];

/** The color a region's finds lean toward ("" for any color). */
const REGION_COLOR: Record<string, string> = {
  meadow: "", village: "", spire: "", lake: "U", coast: "U", grotto: "U", mire: "B", barrow: "B",
  high: "W", sanctum: "W", ember: "R", cinder: "R", wilds: "G", hollow: "G",
};

const cache = new Map<PoolId, Promise<CardSummary[]>>();

async function namesToCards(names: string[]): Promise<CardSummary[]> {
  const unique = [...new Set(names.map((n) => n.replace(/\*CMDR\*|\*F\*/g, "").replace(/\s*\(.*$/, "").trim()))].filter((n) => n && !isBasic(n));
  return (await lookupCards(unique)).filter((c) => !c.type_line.startsWith("Basic"));
}

function load(id: PoolId): Promise<CardSummary[]> {
  if (id === "mine") return listDecks().then((ds) => namesToCards(ds.flatMap((d) => Object.keys(parseList(d.text)))));
  let p = cache.get(id);
  if (!p) {
    p = (async () => {
      if (id === "library") {
        // Every card there is, even ones the duels can only partly play.
        const all = await searchCards({ text: "", limit: 1_000_000, include_banned: true });
        return all.filter((c) => !c.type_line.startsWith("Basic"));
      }
      if (id === "decks") return namesToCards((await builtinDecks()).flatMap((d) => Object.keys(parseList(d.text))));
      return namesToCards(Object.values(LOST_POOLS).flat());
    })().catch((e) => {
      cache.delete(id);
      throw e;
    });
    cache.set(id, p);
  }
  return p;
}

/** How many different cards a set of pools holds. */
export async function poolSize(ids: PoolId[], playableOnly = false): Promise<number> {
  const lists = await Promise.all(ids.map(load));
  return new Set(lists.flat().filter((c) => !playableOnly || c.supported).map((c) => c.name)).size;
}

/** A random card for a lost-card spot, rolled when it's picked up. Finds lean toward the
 *  region's color, and hidden spots lean toward pricier cards, but anything can turn up. */
export async function findCard(region: string, hidden: boolean, pools: PoolId[], playableOnly = false): Promise<string> {
  const ids = pools.length ? pools : DEFAULT_POOLS;
  const vale = () => {
    const list = hidden ? LOST_POOLS.rare : LOST_POOLS[region] ?? LOST_POOLS.meadow;
    return list[Math.floor(Math.random() * list.length)] ?? lostCard(String(Math.random()), region, hidden);
  };
  try {
    const others = (await Promise.all(ids.filter((i) => i !== "elderreach").map(load))).flat().filter((c) => !playableOnly || c.supported);
    // With the Vale's cards in the mix, about a third of finds come from them.
    if (others.length === 0 || (ids.includes("elderreach") && Math.random() < 0.35)) return vale();
    const color = REGION_COLOR[region] ?? "";
    const fits = (c: CardSummary) => !color || c.color_identity === "" || c.color_identity.includes(color);
    let pick = others;
    if (Math.random() < 0.6) {
      const leaning = others.filter((c) => fits(c) && (hidden ? c.mana_value >= 4 : c.mana_value <= 5));
      if (leaning.length >= 8) pick = leaning;
    }
    return pick[Math.floor(Math.random() * pick.length)].name;
  } catch {
    return vale();
  }
}
