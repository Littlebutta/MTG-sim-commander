// Card pools: which cards can turn up lying around the Vale. A journey can draw from one or
// more of them; the choice lives in the save and can be changed from the journal.

import { builtinDecks, listDecks, lookupCards, searchCards, type CardSummary } from "../decks";
import { LOST_POOLS, hashString, isBasic, lostCard, parseList } from "./content";

export type PoolId = "elderreach" | "library" | "decks" | "mine";

export const POOLS: { id: PoolId; name: string; text: string }[] = [
  { id: "elderreach", name: "Cards of the Vale", text: "Hand-picked cards that suit each region: knights in the highlands, merfolk by the lake, dragons in the cinder lands." },
  { id: "library", name: "The whole library", text: "Any card the game fully supports. On the desktop that's the full downloaded card library, so every find is a surprise." },
  { id: "decks", name: "Commander deck cards", text: "Cards from the ready-made Commander decks." },
  { id: "mine", name: "Cards from your decks", text: "Cards from the decks you've built in the deck builder." },
];

export const DEFAULT_POOLS: PoolId[] = ["elderreach"];

/** The color a region's finds lean toward ("" for any color). */
const REGION_COLOR: Record<string, string> = {
  meadow: "", village: "", spire: "", lake: "U", coast: "U", grotto: "U", mire: "B", barrow: "B",
  high: "W", sanctum: "W", ember: "R", cinder: "R", wilds: "G", hollow: "G",
};

const cache = new Map<PoolId, Promise<CardSummary[]>>();

async function namesToCards(names: string[]): Promise<CardSummary[]> {
  const unique = [...new Set(names.map((n) => n.replace(/\*CMDR\*|\*F\*/g, "").replace(/\s*\(.*$/, "").trim()))].filter((n) => n && !isBasic(n));
  return (await lookupCards(unique)).filter((c) => c.supported && !c.type_line.startsWith("Basic"));
}

function load(id: PoolId): Promise<CardSummary[]> {
  if (id === "mine") return listDecks().then((ds) => namesToCards(ds.flatMap((d) => Object.keys(parseList(d.text)))));
  let p = cache.get(id);
  if (!p) {
    p = (async () => {
      if (id === "library") {
        const all = await searchCards({ text: "", limit: 1_000_000 });
        return all.filter((c) => c.supported && !c.type_line.startsWith("Basic"));
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
export async function poolSize(ids: PoolId[]): Promise<number> {
  const lists = await Promise.all(ids.map(load));
  return new Set(lists.flat().map((c) => c.name)).size;
}

/** The card waiting at a lost-card spot. Fixed per spot and pool choice. */
export async function findCard(spot: string, region: string, hidden: boolean, pools: PoolId[]): Promise<string> {
  const ids = pools.length ? pools : DEFAULT_POOLS;
  if (ids.length === 1 && ids[0] === "elderreach") return lostCard(spot, region, hidden);
  try {
    const others = (await Promise.all(ids.filter((i) => i !== "elderreach").map(load))).flat();
    const color = REGION_COLOR[region] ?? "";
    const fits = (c: CardSummary) => !color || c.color_identity === "" || c.color_identity.includes(color);
    // Hidden spots hold the better finds: pricier cards.
    let pick = others.filter((c) => fits(c) && (hidden ? c.mana_value >= 4 : c.mana_value <= 4));
    if (pick.length < 8) pick = others.filter(fits);
    if (pick.length < 8) pick = others;
    const names = [...new Set(pick.map((c) => c.name))].sort();
    if (ids.includes("elderreach")) names.push(...(hidden ? LOST_POOLS.rare : LOST_POOLS[region] ?? LOST_POOLS.meadow));
    if (names.length === 0) return lostCard(spot, region, hidden);
    return names[hashString(`${spot}|${ids.join(",")}`) % names.length];
  } catch {
    return lostCard(spot, region, hidden);
  }
}
