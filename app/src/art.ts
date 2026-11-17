// Card art. The desktop app downloads it from Scryfall once and keeps it in data/art; in a
// browser (development) the image is loaded straight from Scryfall.

import { useEffect, useState } from "react";
import { inTauri } from "./decks";

export interface Art {
  url: string;
  artist: string | null;
}

const cache = new Map<string, Promise<Art | null>>();
let queue: Promise<unknown> = Promise.resolve();

async function fetchArt(name: string): Promise<Art | null> {
  if (inTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    // One at a time, so a full board doesn't flood Scryfall.
    const p = queue.then(() => invoke<Art | null>("card_art", { name }).catch(() => null));
    queue = p;
    return p;
  }
  const url = `https://api.scryfall.com/cards/named?exact=${encodeURIComponent(name)}&format=image&version=art_crop`;
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve({ url, artist: null });
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

export function artFor(name: string): Promise<Art | null> {
  let p = cache.get(name);
  if (!p) {
    p = fetchArt(name);
    cache.set(name, p);
  }
  return p;
}

/** The art for a card, once it has loaded (null until then, or if there is none). */
export function useArt(name: string): Art | null {
  const [art, setArt] = useState<Art | null>(null);
  useEffect(() => {
    let live = true;
    setArt(null);
    artFor(name).then((a) => live && setArt(a));
    return () => {
      live = false;
    };
  }, [name]);
  return art;
}
