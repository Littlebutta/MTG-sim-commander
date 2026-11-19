// Who lives in Elderreach, what they say, what they duel with, and what can be found.
// Decks live in data/decks.json (built by tools/adventure/decks.py and checked by the
// client-api tests); the map in data/world.json (tools/adventure/maps.py).

import decksJson from "./data/decks.json";
import type { Personality } from "../engine/session";

export type Extra = "hat" | "hood" | "helm" | "beard" | "crown" | "cap" | "bandana" | "horns" | "none";

/** How a person looks: colors for the sprite, plus one feature. */
export interface Look {
  skin: string;
  hair: string;
  cloth: string;
  accent: string;
  extra?: Extra;
  /** Color of the hat, hood, helm, crown... */
  extraColor?: string;
  /** Bosses glow. */
  aura?: string;
}

export interface Duel {
  deck: string;
  style: Personality;
  /** Starting life, when not 20. */
  life?: number;
  rank: "duelist" | "boss" | "endgame";
  /** What their deck is about, in a line. */
  theme: string;
  /** Bosses hand over a sigil. */
  sigil?: string;
  /** And a rare reagent. */
  reagent?: string;
}

export interface Person {
  name: string;
  title?: string;
  look: Look;
  duel?: Duel;
  /** Before a duel, or everything a townsperson says. */
  intro: string[];
  /** What they say when you beat them. */
  beaten?: string[];
  /** What they say when they beat you. */
  gloat?: string[];
  /** Talking to them after you've won. */
  after?: string[];
}

const SKIN = ["#f6d3b3", "#e8b98f", "#c98e62", "#9a6440", "#6e4529", "#f0c8a0"];
const L = (skin: number, hair: string, cloth: string, accent: string, extra: Extra = "none", extraColor?: string, aura?: string): Look =>
  ({ skin: SKIN[skin], hair, cloth, accent, extra, extraColor, aura });

export const PEOPLE: Record<string, Person> = {
  // ---------------------------------------------------------------- Hearthmoor
  bram: {
    name: "Old Bram", title: "Keeper of Bram's Curios", look: L(1, "#d9d9d9", "#7a5a3a", "#c9a227", "beard", "#e6e6e6"),
    intro: [],
  },
  innkeep: {
    name: "Marta", title: "Innkeeper", look: L(0, "#a0522d", "#6b8e23", "#f5deb3"),
    intro: ["Welcome to the Sleeping Fox! Sit, rest, get warm."],
  },
  bard: {
    name: "Lio", title: "Wandering bard", look: L(2, "#2f2f2f", "#8b3a62", "#e0b0ff", "cap", "#5b2a86"),
    intro: [
      "They say five old powers keep the Vale: the Barrow Queen, the High Seraph, the Molten King, the Voice of the Deep and the Thornfather.",
      "Best each of them and you'll carry their sigils. Carry all five and the Rift Spire opens. I wrote a song about it. It's very long.",
    ],
  },
  wenna: {
    name: "Granny Wenna", look: L(0, "#eeeeee", "#556b2f", "#deb887", "hood", "#8fbc8f"),
    intro: [],
  },
  ferrow: {
    name: "Little Ferrow", look: L(3, "#3b2a1a", "#4682b4", "#ffd700", "cap", "#cc3333"),
    intro: [
      "Did you know cards get lost out in the wild? Sometimes they're lying right on the path!",
      "And sometimes they hide in the tall grass. If you see something glint, go look!",
    ],
  },
  mira: {
    name: "Mira", look: L(0, "#e8a33d", "#9370db", "#ffffff"),
    intro: ["Old Bram's been waiting on you all morning. He says today's the day you set out.", "Duelists on the road will challenge you if they catch your eye. Keep your deck ready."],
  },
  old_tom: {
    name: "Old Tom", look: L(4, "#bbbbbb", "#8b4513", "#d2b48c", "beard", "#cccccc"),
    intro: ["The touchstones are older than the village. Lay a hand on one and it'll remember you.", "If you're ever bested out there, you'll wake beside the last stone you touched. Don't ask me how."],
  },
  kid_ash: {
    name: "Ash", look: L(1, "#222222", "#228b22", "#f0e68c"),
    intro: ["When I grow up I'm going to have four of every card. Four! That's the most you can carry, you know."],
  },
  guard_hal: {
    name: "Hal", title: "Village watch", look: L(2, "#4a3020", "#708090", "#b0c4de", "helm", "#a9a9a9"),
    intro: ["Road north splits at the crossroads. West is Mirefen, east the coast, and north the old Reach.", "Ysolde's tower stands just off the crossroads. Strange smells come out of that chimney."],
  },
  fisher_wen: {
    name: "Wen", title: "Lake fisher", look: L(3, "#1e1e1e", "#2e8b57", "#f4a460", "hat", "#d2b48c"),
    intro: ["The lake's calm today. Calm enough to hear the cards whisper, if you believe that sort of thing."],
  },
  pilgrim: {
    name: "Pilgrim Oda", look: L(1, "#5c4033", "#f5f5dc", "#daa520", "hood", "#f5f5dc"),
    intro: ["The Gilded Sanctum is west of here, past the pines. The High Seraph keeps it. She does not lose often."],
  },
  smith: {
    name: "Gorm", title: "Ashfoot smith", look: L(4, "#111111", "#4b4b4b", "#ff8c00", "bandana", "#b22222"),
    intro: ["The Molten King sleeps under the Cinder Depths. If you're going in, bring something that can stand the heat.", "Dragons hate flyers that block. Or so I hear. I mostly hit metal."],
  },
  hermit: {
    name: "The Hermit", look: L(5, "#777777", "#3e3e2e", "#8b8b6b", "hood", "#4e4e3e"),
    intro: ["Heh. Hollow Barrow's in the far north-west of the fen. The Queen in there drinks life like wine.", "Bring answers for flyers. Bring patience."],
  },
  scholar: {
    name: "Archivist Pell", look: L(0, "#6b4226", "#191970", "#c0c0c0", "hat", "#191970"),
    intro: [
      "These ruins were a city once. Then something in the Spire made everyone forget it.",
      "The door won't open for anyone without the five sigils. I've checked. Many times.",
    ],
  },
  sailor: {
    name: "Bosun Rake", look: L(2, "#8b0000", "#1c3f6e", "#ffffff", "bandana", "#1c3f6e"),
    intro: ["The Drowned Grotto's up the shore, in the cliffs. The Voice of the Deep says no to everything. Counterspells, mate. Bait them out."],
  },
  ysolde: {
    name: "Ysolde", title: "Hedge-wizard", look: L(0, "#c0c0ff", "#4b0082", "#ffd700", "hat", "#4b0082"),
    intro: [],
  },

  // ---------------------------------------------------------------- Thistle Meadow and Willowmere
  rowan: {
    name: "Rowan", title: "Squire of the Meadow", look: L(1, "#d2691e", "#c0c0c0", "#4169e1", "helm", "#c0c0c0"),
    duel: { deck: "rowan", style: "Aggressive", rank: "duelist", theme: "First strikers and brave little knights" },
    intro: ["Halt, traveler! A knight-in-training must test every blade on this road. Draw!"],
    beaten: ["Well struck. I'll train harder. Take a card, you've earned it."],
    gloat: ["Ha! First strike wins the day. Come back when you're quicker."],
    after: ["Strike first and they never strike back. Most of the time."],
  },
  tam: {
    name: "Tam", title: "Goblin-catcher", look: L(2, "#8b4513", "#a0522d", "#ff4500", "cap", "#556b2f"),
    duel: { deck: "tam", style: "Aggressive", rank: "duelist", theme: "Lots of little goblins, all at once" },
    intro: ["I catch goblins for a living. Kept a few. Want to meet them?"],
    beaten: ["They scattered! It'll take me all week to round them up."],
    gloat: ["Never underestimate a crowd of goblins."],
    after: ["Goblins are cheap, fast and loud. Like me."],
  },
  pip: {
    name: "Pip", title: "Stablehand", look: L(0, "#f4c430", "#8fbc8f", "#8b4513", "hat", "#deb887"),
    duel: { deck: "pip", style: "Balanced", rank: "duelist", theme: "Bears, lions and loyal farm hounds" },
    intro: ["Oh! You've got the look of a duelist. My animals could use the exercise."],
    beaten: ["Aw, they had fun anyway. Here, pick something from my satchel."],
    gloat: ["Good doggies! Good bears!"],
    after: ["Watchwolves are the best. Don't tell the bears."],
  },
  ida: {
    name: "Ida", title: "Herbalist", look: L(1, "#9acd32", "#2e8b57", "#ffc0cb", "hood", "#6b8e23"),
    duel: { deck: "ida", style: "Balanced", rank: "duelist", theme: "Growth draughts that make small things huge" },
    intro: ["These herbs make anything grow. Shall I show you on your creatures? No, on mine."],
    beaten: ["Overgrown, outgrown. Take a cutting from my garden."],
    gloat: ["Everything grows big if you water it right."],
    after: ["Keep a Giant Growth in hand. It wins more fights than it should."],
  },
  nell: {
    name: "Nell", title: "Old fisher", look: L(4, "#dcdcdc", "#4682b4", "#f0e68c", "hat", "#f0e68c"),
    duel: { deck: "nell", style: "Controlling", rank: "duelist", theme: "Merfolk who wriggle past your guard" },
    intro: ["Fish aren't biting. Maybe you will. Sit down, let's play."],
    beaten: ["Slipped the hook, you did. Fair catch."],
    gloat: ["Reeled in. They always come in eventually."],
    after: ["Patience is the whole trick. Fishing and dueling both."],
  },

  // ---------------------------------------------------------------- Mirefen and Hollow Barrow
  mosswick: {
    name: "Mosswick", title: "Bog hermit", look: L(5, "#556b2f", "#3b3b1f", "#9acd32", "hood", "#3b3b1f"),
    duel: { deck: "mosswick", style: "Controlling", rank: "duelist", theme: "Rats and creatures with deadly bites" },
    intro: ["Squeak. Squeak! My rats smell a stranger. One nibble is all it takes."],
    beaten: ["Back to the burrow, little ones. You there, take a card."],
    gloat: ["Deathtouch, dearie. It doesn't matter how big you are."],
    after: ["A 1/1 with deathtouch trades with anything. Remember that."],
  },
  vey: {
    name: "Sister Vey", title: "of the Crimson Veil", look: L(0, "#1a1a1a", "#8b0000", "#e6e6fa", "hood", "#4b0010"),
    duel: { deck: "vey", style: "Balanced", rank: "duelist", theme: "Vampires that drain life from afar" },
    intro: ["You look... warm. The Veil thanks you for your offering."],
    beaten: ["Such vigor. Take this, and keep your blood to yourself."],
    gloat: ["Mm. A little life from you, a little more for me."],
    after: ["Drain a little every turn and it adds up. Like interest."],
  },
  gil: {
    name: "Gil", title: "Corpse-lantern", look: L(5, "#9acd32", "#2f4f4f", "#adff2f", "hat", "#2f2f2f"),
    duel: { deck: "gil", style: "Balanced", rank: "duelist", theme: "Zombies that just keep getting back up" },
    intro: ["I light the way for the dead. They follow me everywhere. Everywhere."],
    beaten: ["They'll be back. They always are. Here, before they wake."],
    gloat: ["You can't keep a good corpse down."],
    after: ["Raise Dead is a fine card. Fine for me, anyway."],
  },
  morrow: {
    name: "Witch Morrow", title: "Rot-tender", look: L(5, "#6b8e23", "#2f4f2f", "#8b008b", "hat", "#2f4f2f"),
    duel: { deck: "morrow", style: "Controlling", rank: "duelist", theme: "Rot and spores: decay that feeds new growth" },
    intro: ["The Queen sleeps below. Nobody disturbs her who hasn't seen my garden first."],
    beaten: ["Rot to root, root to rot. Pass, then, and take a seedling."],
    gloat: ["Everything rots, child. Everything."],
    after: ["Putrefy kills creatures and artifacts both. Useful, isn't it?"],
  },
  hob: {
    name: "Hob", title: "Gravedigger of the Barrow", look: L(3, "#3b3b3b", "#5c4033", "#a9a9a9", "cap", "#3b3b3b"),
    duel: { deck: "hob", style: "Balanced", rank: "duelist", theme: "Card draw paid for in blood, and a full graveyard" },
    intro: ["I dig. I dig some more. Then I dig you a deck. Let's see if it fits."],
    beaten: ["Deep enough. Go on, the Queen's further in."],
    gloat: ["Another one for the Barrow."],
    after: ["Night's Whisper costs life. Life is cheap when you're winning."],
  },
  vesper: {
    name: "Vesper", title: "the Barrow Queen", look: L(0, "#0d0d0d", "#2b0a3d", "#c41e3a", "crown", "#c0c0c0", "#c41e3a"),
    duel: { deck: "vesper", style: "Controlling", life: 25, rank: "boss", theme: "Vampire nobility: flying, deathtouch, lifelink and cruel removal", sigil: "night", reagent: "nightshade" },
    intro: [
      "A living thing, in my Barrow. How long has it been? Centuries, perhaps.",
      "Kneel, and I will drink only a little. Stand, and I will drink it all.",
    ],
    beaten: ["...You stood. So few ever do.", "Take the Sigil of Night. When the Spire calls to you, remember who made you strong."],
    gloat: ["Delicious. Come back when you've grown more to give."],
    after: ["The Spire takes memories, child. Mine it took long ago. Do not let it take yours."],
  },

  // ---------------------------------------------------------------- Sunspire Highlands and the Gilded Sanctum
  edda: {
    name: "Captain Edda", title: "of the Highland Watch", look: L(2, "#ffd700", "#f5f5f5", "#1e90ff", "helm", "#dcdcdc"),
    duel: { deck: "edda", style: "Aggressive", rank: "duelist", theme: "Soldiers in ranks, made stronger together" },
    intro: ["Highland Watch! Present your deck for inspection. And by inspection I mean a duel."],
    beaten: ["Formation broken. Fair fight, recruit. Take your pay."],
    gloat: ["Ranks hold! Dismissed."],
    after: ["One soldier's nothing. Twenty soldiers with an anthem is an army."],
  },
  calix: {
    name: "Brother Calix", title: "Lifewarden", look: L(1, "#a0522d", "#fffaf0", "#ffd700", "hood", "#fffaf0"),
    duel: { deck: "calix", style: "Controlling", rank: "duelist", theme: "Healers who gain life faster than you can take it" },
    intro: ["Peace, traveler. Let us duel gently. I promise to heal you afterward."],
    beaten: ["Blessed are the bold. Take this, with my blessing."],
    gloat: ["There, there. You were simply outlasted."],
    after: ["Life is a resource. Spend it. Earn it back."],
  },
  lune: {
    name: "Lune", title: "Griffin rider", look: L(0, "#b0e0e6", "#87ceeb", "#ffffff", "cap", "#ffffff"),
    duel: { deck: "lune", style: "Aggressive", rank: "duelist", theme: "Hawks, griffins and angels over your head" },
    intro: ["The skies are mine! Can your deck block a griffin? Let's find out!"],
    beaten: ["Grounded! Ugh. Pick something, quickly, before I take off."],
    gloat: ["Look up, friend. Always look up."],
    after: ["Flying wins games. Reach and removal win them back."],
  },
  osric: {
    name: "Warden Osric", title: "Keeper of the Sanctum Gate", look: L(4, "#2f2f2f", "#e0e0e0", "#4682b4", "helm", "#f5f5f5"),
    duel: { deck: "osric", style: "Controlling", rank: "duelist", theme: "Walls, tappers and a wrath when it matters" },
    intro: ["None pass the Sanctum gate unproven. Prove yourself."],
    beaten: ["Proven. The way is open."],
    gloat: ["Unproven. Turn back and train."],
    after: ["A tapped creature can't hurt you. Remember that."],
  },
  maelis: {
    name: "Ser Maelis", title: "the Sunblade", look: L(1, "#ff8c00", "#b22222", "#ffd700", "helm", "#ffd700"),
    duel: { deck: "maelis", style: "Aggressive", rank: "duelist", theme: "Fire and faith: double strike, haste and Lightning Helix" },
    intro: ["The Seraph waits beyond me. My blade's sworn to her, and it's very sharp."],
    beaten: ["A worthy foe. Go to her, and may the sun find you."],
    gloat: ["The sun burns, friend. Remember the burn."],
    after: ["Red and white together: hit fast, heal faster."],
  },
  aurelia: {
    name: "Aurelia", title: "the High Seraph", look: L(0, "#fff8dc", "#fffff0", "#ffd700", "crown", "#ffd700", "#ffe680"),
    duel: { deck: "aurelia", style: "Balanced", life: 25, rank: "boss", theme: "A host of angels in the sky, and a wrath of god", sigil: "dawn", reagent: "halo_fragment" },
    intro: [
      "I have watched you climb, little one. Every step.",
      "Light reveals all things. Let us see what is revealed in you.",
    ],
    beaten: ["Revealed: courage, and more of it than I thought.", "Carry the Sigil of Dawn. Let it remind you of mornings, and of who you are."],
    gloat: ["The light has judged. Rise when you are ready."],
    after: ["When you face what waits in the Spire, hold onto your name. It will try to take that first."],
  },

  // ---------------------------------------------------------------- Emberpeak and the Cinder Depths
  brakka: {
    name: "Brakka", title: "Pit brawler", look: L(4, "#8b0000", "#8b4513", "#ff6347", "bandana", "#ff6347"),
    duel: { deck: "brakka", style: "Aggressive", rank: "duelist", theme: "Big, fast, and swinging before you blink" },
    intro: ["You! You look soft. I'll fix that. Fists up, cards out!"],
    beaten: ["Ha! You hit like a rockslide. Take a card. Take two, no, one."],
    gloat: ["Too slow! Haste, friend. Haste."],
    after: ["Kamahl taught me everything. Mostly how to hit things."],
  },
  snag: {
    name: "Chief Snag", title: "of the Ashfoot goblins", look: L(5, "#6b8e23", "#8b4513", "#ffd700", "horns", "#f5f5dc"),
    duel: { deck: "snag", style: "Aggressive", rank: "duelist", theme: "A goblin horde that blows itself up for fun" },
    intro: ["Snag is CHIEF. Chief says: duel! Goblins, assemble! All of you! Even Grub!"],
    beaten: ["Goblins... retreat! Bah. Snag gives tribute. Pick shiny."],
    gloat: ["SNAG WINS. Goblins, celebrate! Carefully! Not near the lava!"],
    after: ["Mogg Fanatic is Grub. Grub is best goblin. Grub explode."],
  },
  kessa: {
    name: "Kessa", title: "Flamecaller", look: L(2, "#ff4500", "#228b22", "#ff8c00"),
    duel: { deck: "kessa", style: "Aggressive", rank: "duelist", theme: "Wildfire: everything has haste" },
    intro: ["Fire and forest both want to spread. I just help them along."],
    beaten: ["Burned out. For now. Here, it's still warm."],
    gloat: ["Everything burns. Everything grows back. Lovely."],
    after: ["Fires of Yavimaya: every creature you cast attacks the turn it arrives."],
  },
  fizz: {
    name: "Fizz", title: "Spark-tinker", look: L(0, "#ffa500", "#696969", "#ffff00", "cap", "#8b4513"),
    duel: { deck: "fizz", style: "Balanced", rank: "duelist", theme: "Spells, spells, spells, and goblins who love them" },
    intro: ["Every spell I cast goes BANG. Every single one. Want to hear?"],
    beaten: ["Fizzled. Ugh. Okay, take something before it explodes."],
    gloat: ["BANG! I told you."],
    after: ["Guttersnipe deals 2 to you every time I cast an instant or sorcery. It adds up fast."],
  },
  sora: {
    name: "Sora", title: "Drake-tamer", look: L(1, "#1a1a1a", "#dc143c", "#ffd700", "bandana", "#ffd700"),
    duel: { deck: "sora", style: "Balanced", rank: "duelist", theme: "Dragons and whelps, with burn for anything that blocks" },
    intro: ["The Molten King's dragons nest past here. I raise the young ones. Want to meet them?"],
    beaten: ["My whelps need more training. And snacks. Go on through."],
    gloat: ["Dragons rule the sky. Always have."],
    after: ["A dragon with firebreathing gets bigger the more mountains you have."],
  },
  pyrrhus: {
    name: "Pyrrhus", title: "the Molten King", look: L(4, "#ff4500", "#2b0000", "#ff8c00", "crown", "#ffd700", "#ff5a1f"),
    duel: { deck: "pyrrhus", style: "Aggressive", life: 25, rank: "boss", theme: "Dragons, hellkites and lightning to the face", sigil: "flame", reagent: "dragons_heart" },
    intro: [
      "Who wakes the Molten King? Who walks in my fire and doesn't burn?",
      "Let's fix that. BURN.",
    ],
    beaten: ["Hah... HAH! You don't burn! You're stubborn as stone.", "Take the Sigil of Flame. It will keep you warm in the cold place at the top of the world."],
    gloat: ["Ash and ember. Come back when you're harder to melt."],
    after: ["The thing in the Spire is cold. So cold. Bring my fire to it."],
  },

  // ---------------------------------------------------------------- Tidewater Coast and the Drowned Grotto
  quill: {
    name: "Navigator Quill", look: L(2, "#4b0082", "#20b2aa", "#ffffff", "hat", "#20b2aa"),
    duel: { deck: "quill", style: "Balanced", rank: "duelist", theme: "Drakes and birds that strike from above" },
    intro: ["I chart the winds. They say a duel's blowing in. From you, I think."],
    beaten: ["Off course! Here, take this, and a heading: the Grotto's up the cliffs."],
    gloat: ["The wind was at my back."],
    after: ["Unsummon your opponent's blocker and fly right over."],
  },
  brine: {
    name: "Captain Brine", look: L(3, "#2f2f2f", "#000080", "#ffd700", "hat", "#1a1a1a"),
    duel: { deck: "brine", style: "Balanced", rank: "duelist", theme: "Pirates who steal cards and send your creatures home" },
    intro: ["Arr. That's a fine deck. Be a shame if it walked the plank. Duel me!"],
    beaten: ["Scuttled! Fine, fine. Pick your plunder."],
    gloat: ["Back to your hand, back to your hand! Ha!"],
    after: ["Man-o'-War: a body and a bounce in one card. Pirate's favorite."],
  },
  ivo: {
    name: "Stormcaller Ivo", look: L(0, "#e0ffff", "#191970", "#ff4500", "hood", "#191970"),
    duel: { deck: "ivo", style: "Controlling", rank: "duelist", theme: "Storms of spells and flying wizards who feed on them" },
    intro: ["Lightning and tide. I've bottled both. Let me open one."],
    beaten: ["The storm passes. Take a little of it with you."],
    gloat: ["You can't fight the weather."],
    after: ["Wee Dragonauts get +2/+0 for every instant or sorcery. Every one."],
  },
  ambrose: {
    name: "Professor Ambrose", look: L(1, "#f5f5f5", "#2f4f4f", "#4682b4", "hat", "#2f4f4f"),
    duel: { deck: "ambrose", style: "Controlling", rank: "duelist", theme: "Counterspells, card draw and big djinn" },
    intro: ["Ah, a student! No? A duelist? Same thing. Lesson one: no."],
    beaten: ["Excellent work. Full marks. Take this home and study it."],
    gloat: ["Countered. Do try to keep up."],
    after: ["Counterspell is only as good as your patience. And my patience is excellent."],
  },
  sela: {
    name: "Sela", title: "the Tide-Witch", look: L(0, "#008b8b", "#2e8b57", "#e0ffff", "hood", "#2e8b57"),
    duel: { deck: "sela", style: "Controlling", rank: "duelist", theme: "Walls, and the tide that drowns your library" },
    intro: ["The sea takes everything in the end. Your deck, for instance."],
    beaten: ["The tide turns. Go, the Voice is waiting."],
    gloat: ["Nothing left to draw. The sea has it all."],
    after: ["If your library runs out, you lose. The sea is patient."],
  },
  oshara: {
    name: "Oshara", title: "Voice of the Deep", look: L(2, "#00ced1", "#00008b", "#7fffd4", "crown", "#7fffd4", "#3fd0ff"),
    duel: { deck: "oshara", style: "Controlling", life: 25, rank: "boss", theme: "No. No. No. Then a djinn.", sigil: "tide", reagent: "abyssal_ink" },
    intro: [
      "The deep hears everything. It heard you coming three tides ago.",
      "Speak your spells. The deep will answer: no.",
    ],
    beaten: ["The deep has heard a new word. Yes.", "Take the Sigil of Tides. In the Spire, when they try to wash you away, be the rock."],
    gloat: ["No."],
    after: ["Patience. Even the deep cannot counter everything, forever."],
  },

  // ---------------------------------------------------------------- the Verdant Wilds and Thornheart Hollow
  thorne: {
    name: "Ranger Thorne", look: L(2, "#556b2f", "#2e4a1e", "#8fbc8f", "hood", "#2e4a1e"),
    duel: { deck: "thorne", style: "Controlling", rank: "duelist", theme: "Spiders and archers: nothing flies over them" },
    intro: ["Quiet. The spiders are listening. So am I. Let's see what you've got."],
    beaten: ["Good tracking. Here's something from the trail."],
    gloat: ["Caught in the web."],
    after: ["Reach blocks flyers. Deathtouch kills whatever it blocks."],
  },
  kip: {
    name: "Kip", title: "Dino-wrangler", look: L(1, "#ff6347", "#8b4513", "#ffd700", "hat", "#8b4513"),
    duel: { deck: "kip", style: "Aggressive", rank: "duelist", theme: "Dinosaurs, the bigger the better" },
    intro: ["YEEHAW! Saddle up, these dinos need a run!"],
    beaten: ["Whoa there! Easy, big fella. Fine, you win. Pick a card!"],
    gloat: ["STAMPEDE!"],
    after: ["There's a dino out there with ten power. TEN!"],
  },
  oakhart: {
    name: "Mother Oakhart", look: L(3, "#228b22", "#f5f5dc", "#228b22", "hood", "#6b8e23"),
    duel: { deck: "oakhart", style: "Balanced", rank: "duelist", theme: "Forest and field: wolves, knights and an anthem" },
    intro: ["The wilds raise their children strong. Let me show you."],
    beaten: ["Grown well, you have. Take a seed from my pocket."],
    gloat: ["Rest a while, little sprout."],
    after: ["Green and white together make a strong family."],
  },
  bramblebeard: {
    name: "Elder Bramblebeard", look: L(4, "#2e8b57", "#5c4033", "#9acd32", "beard", "#3cb371"),
    duel: { deck: "bramblebeard", style: "Balanced", rank: "duelist", theme: "Elves for mana, then very large creatures" },
    intro: ["Hrmm. A young one, deep in the Hollow. Let me see if your roots run deep."],
    beaten: ["Hrmm. Deeper than I thought. Go on to the Thornfather."],
    gloat: ["Hrmm. Too shallow. Grow, and return."],
    after: ["Ramp early, and you'll cast big things before the others."],
  },
  fenna: {
    name: "Druid Fenna", look: L(0, "#adff2f", "#556b2f", "#ff69b4", "hood", "#556b2f"),
    duel: { deck: "fenna", style: "Balanced", rank: "duelist", theme: "Saprolings, saprolings, and an Overrun" },
    intro: ["Mushrooms everywhere! Aren't they lovely? They're about to be everywhere on your side too."],
    beaten: ["Spores scattered. Take one, it'll grow on you."],
    gloat: ["One saproling is cute. Twelve is a strategy."],
    after: ["Jade Mage makes a saproling whenever you have spare mana."],
  },
  thornfather: {
    name: "The Thornfather", look: L(4, "#2e8b57", "#3b2a1a", "#7cfc00", "horns", "#5c4033", "#6aff5a"),
    duel: { deck: "thornfather", style: "Balanced", life: 25, rank: "boss", theme: "The oldest and largest things in the forest", sigil: "root", reagent: "elder_acorn" },
    intro: [
      "Hhhhhrrrmmm. I was old when the Spire was young.",
      "Little seed. Show me how you grow.",
    ],
    beaten: ["Hhhrrrm. You grow strangely. You grow well.", "Take the Sigil of Roots. In the Spire, remember the ground beneath you."],
    gloat: ["Hhhrm. Grow, little seed. Then return."],
    after: ["The Spire's master was a seed once too. It forgot. Do not forget."],
  },

  // ---------------------------------------------------------------- the Shattered Reach
  gearlock: {
    name: "Tinker Gearlock", look: L(1, "#a9a9a9", "#8b7355", "#cd7f32", "cap", "#cd7f32"),
    duel: { deck: "gearlock", style: "Balanced", rank: "duelist", theme: "Myr, metal and machines that work in any color" },
    intro: ["Oh! A visitor! Do you like gears? My myr do. They're made of them."],
    beaten: ["Clank. Bonk. Broken. Take a part, I've got spares."],
    gloat: ["Precision engineering!"],
    after: ["Artifacts fit in any deck. Any deck at all!"],
  },
  sentinel: {
    name: "The Brass Sentinel", title: "Last guard of the Reach", look: L(5, "#b8860b", "#b8860b", "#00ffff", "helm", "#b8860b"),
    duel: { deck: "sentinel", style: "Controlling", rank: "duelist", theme: "Walls of metal and slow, unstoppable constructs" },
    intro: ["INTRUDER. STATE PURPOSE.", "PURPOSE NOT RECOGNIZED. INITIATING DUEL."],
    beaten: ["PURPOSE... RECOGNIZED. PROCEED. TAKE REWARD."],
    gloat: ["INTRUDER REMOVED."],
    after: ["THE SPIRE REQUIRES FIVE SIGILS. THE SPIRE HAS ALWAYS REQUIRED FIVE SIGILS."],
  },

  // ---------------------------------------------------------------- the Rift Spire
  regent: {
    name: "The Iron Regent", title: "Warden of the Spire", look: L(5, "#4a4a4a", "#2f2f3a", "#00bfff", "crown", "#9aa0a6", "#4fc3ff"),
    duel: { deck: "regent", style: "Controlling", life: 30, rank: "endgame", theme: "Flying gargoyles, counterspells, wraths and machines that will not break", reagent: "clockwork_heart" },
    intro: [
      "FIVE SIGILS. IT HAS BEEN A LONG TIME SINCE ANYONE CARRIED FIVE.",
      "I WAS BUILT TO KEEP THE UPPER HALL CLOSED. I HAVE KEPT IT CLOSED FOR ONE THOUSAND YEARS.",
      "I WILL KEEP IT CLOSED FOR ONE THOUSAND MORE.",
    ],
    beaten: ["...THE HALL IS... OPEN.", "SHE IS ABOVE. SHE WILL ASK YOU TO FORGET. DO NOT."],
    gloat: ["THE HALL REMAINS CLOSED."],
    after: ["THE STAIRS ARE OPEN. I DO NOT RECOMMEND THEM."],
  },
  morvane: {
    name: "Morvane", title: "the Unremembering", look: L(0, "#e6e6fa", "#1b0f2e", "#b388ff", "crown", "#b388ff", "#b388ff"),
    duel: { deck: "morvane", style: "Controlling", life: 35, rank: "endgame", theme: "Every color at once: dragons, angels and every answer in the Vale" },
    intro: [
      "There you are. I wondered when you'd climb this high.",
      "Do you know what this place is? It's where the Vale keeps the things it doesn't want to remember.",
      "Lay down your cards. Forget the road, the village, the old shopkeeper. Forget your own name. It's so much quieter here.",
      "...No? Then I'll take it from you.",
    ],
    beaten: [
      "...You remember. All of it. How?",
      "The road. The village. The old man and his little shop. You held onto all of it.",
      "Then go home, traveler. Go home and tell them the Spire is quiet again.",
    ],
    gloat: ["Shh. Rest now. Forget."],
    after: ["Go home. They're waiting for you."],
  },
};

export const SIGILS: Record<string, { name: string; color: string }> = {
  night: { name: "Sigil of Night", color: "#9b59b6" },
  dawn: { name: "Sigil of Dawn", color: "#f5e6a8" },
  flame: { name: "Sigil of Flame", color: "#ff6a3d" },
  tide: { name: "Sigil of Tides", color: "#52c7ff" },
  root: { name: "Sigil of Roots", color: "#59d98e" },
};

/** Rare reagents. Each one, dropped in Ysolde's cauldron, becomes any spell you name. */
export const REAGENTS: Record<string, { name: string; text: string; color: string }> = {
  moonpetal: { name: "Moonpetal", text: "A white flower that only opens under the moon. It hums.", color: "#e8e8ff" },
  phoenix_ember: { name: "Phoenix Ember", text: "Still warm. It will always be warm.", color: "#ff7a2f" },
  leviathan_pearl: { name: "Leviathan Pearl", text: "A pearl the size of a fist, from something much bigger.", color: "#bfe8ff" },
  mandrake: { name: "Mandrake Root", text: "It screamed when you pulled it. It's quiet now. Mostly.", color: "#c8a46a" },
  starfall_shard: { name: "Starfall Shard", text: "A sliver of a fallen star, cold to the touch.", color: "#d0c4ff" },
  sunstone: { name: "Sunstone", text: "Glows faintly gold, even in the dark.", color: "#ffd84a" },
  grave_lily: { name: "Grave Lily", text: "Grows only where the dead sleep soundly.", color: "#9fd6a6" },
  seraph_feather: { name: "Seraph Feather", text: "Lighter than air. Literally: hold it loosely.", color: "#fff6d5" },
  wyrm_scale: { name: "Wyrm Scale", text: "Shed by something old and enormous.", color: "#d9573b" },
  tide_glass: { name: "Tide Glass", text: "Sea-worn glass that shows the tide inside it.", color: "#5fd3d3" },
  heartwood: { name: "Heartwood", text: "From the very center of an ancient tree.", color: "#8b5a2b" },
  voidglass: { name: "Voidglass", text: "Black glass with nothing inside. You can't stop looking at it.", color: "#6a4bb0" },
  nightshade: { name: "Nightshade Vial", text: "The Barrow Queen's own poison. Handle gently.", color: "#7a2a9c" },
  halo_fragment: { name: "Halo Fragment", text: "A piece of light, broken off and kept.", color: "#fff1a8" },
  dragons_heart: { name: "Dragon's Heart", text: "A coal that beats.", color: "#ff3b1f" },
  abyssal_ink: { name: "Abyssal Ink", text: "Ink from the deepest place in the sea. It writes by itself.", color: "#1f3fbf" },
  elder_acorn: { name: "Elder Acorn", text: "The Thornfather's seed. Plant it somewhere you love.", color: "#7a5a2a" },
  clockwork_heart: { name: "Clockwork Heart", text: "Ticks steadily. It never needs winding.", color: "#9fc3d9" },
};

/** Lost cards in each part of the world. Hidden ones come from the rare pool. */
export const LOST_POOLS: Record<string, string[]> = {
  meadow: ["Grizzly Bears", "Savannah Lions", "Wild Griffin", "Storm Crow", "Giant Growth", "Shock", "Raise the Alarm", "Divination", "Elvish Visionary", "Watchwolf", "Centaur Courser", "Lightning Strike", "Mind Stone", "Skyscanner", "Sprout"],
  village: ["Grizzly Bears", "Glory Seeker", "Coral Merfolk", "Field Creeper", "Raise the Alarm", "Shock"],
  lake: ["Coral Merfolk", "Kraken Hatchling", "Steward of Valeron", "Watercourser", "Giant Octopus", "Scroll Thief", "Fortress Crab", "Unsummon", "Rip-Clan Crasher"],
  mire: ["Vampire of the Dire Moon", "Typhoid Rats", "Night's Whisper", "Gravedigger", "Last Gasp", "Vampire Spawn", "Deathbloom Thallid", "Douse in Gloom", "Sovereign's Bite", "Shambling Goblin", "Blood Pact"],
  barrow: ["Cadaver Imp", "Gifted Aetherborn", "Phyrexian Gargantua", "Murder", "Grasp of Darkness", "Zombify"],
  high: ["Healer's Hawk", "Knight of Meadowgrain", "Attended Knight", "Seraph of Dawn", "Benalish Marshal", "Moment of Heroism", "Revitalize", "Inspiring Overseer", "Soul Warden"],
  sanctum: ["Serra Angel", "Shepherd of the Lost", "Dawnstrike Paladin", "Glorious Anthem", "Day of Judgment", "Boros Swiftblade"],
  ember: ["Lightning Bolt", "Searing Spear", "Furnace Whelp", "Beetleback Chief", "Fire Imp", "Ember Hauler", "Thunder Strike", "Hordeling Outburst", "Mogg Fanatic"],
  cinder: ["Guttersnipe", "Kiln Fiend", "Volcanic Dragon", "Flame Slash", "Charging Monstrosaur", "Skyknight Legionnaire"],
  coast: ["Cloudkin Seer", "Man-o'-War", "Counterspell", "Repulse", "Spectral Sailor", "Mist Raven", "Exclude", "Thieving Magpie", "Aven Fisher"],
  grotto: ["Wee Dragonauts", "Jace's Ingenuity", "Benthic Giant", "Opportunity", "Mahamoti Djinn", "Prodigal Sorcerer"],
  wilds: ["Llanowar Elves", "Kalonian Tusker", "Thornweald Archer", "Jade Mage", "Kavu Climber", "Sentinel Spider", "Monstrous Growth", "Garruk's Companion", "Thrashing Brontodon", "Harmonize"],
  hollow: ["Pelakka Wurm", "Leatherback Baloth", "Birds of Paradise", "Overrun", "Rhox War Monk", "Putrefy"],
  spire: ["Alloy Myr", "Perilous Myr", "Filigree Familiar", "Ornithopter of Paradise", "Meteorite", "Darksteel Myr", "Gold-Forged Sentinel", "Guardian Automaton"],
  rare: ["Vampire Nighthawk", "Serra Angel", "Shivan Dragon", "Mahamoti Djinn", "Pelakka Wurm", "Lightning Helix", "Terminate", "Absorb", "Broodmate Dragon", "Sol Ring", "Archangel", "Flametongue Kavu", "Ravenous Chupacabra", "Birds of Paradise", "Lightning Angel", "Wrath of God", "Baleful Strix", "Shivan Hellkite"],
};

export function hashString(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
}

/** The card waiting at a lost-card spot. Fixed per spot, so every journey finds the same ones. */
export function lostCard(id: string, region: string, hidden: boolean): string {
  const pool = hidden ? LOST_POOLS.rare : LOST_POOLS[region] ?? LOST_POOLS.meadow;
  return pool[hashString(id) % pool.length];
}

// ---------------------------------------------------------------- decks

export interface DeckData {
  starters: Record<string, { name: string; text: string }>;
  duelists: Record<string, string>;
}
export const DECKS = decksJson as DeckData;

export const BASICS = ["Plains", "Island", "Swamp", "Mountain", "Forest", "Wastes"];
export const isBasic = (name: string) => BASICS.includes(name);

/** Deck text to {name: count}. */
export function parseList(text: string): Record<string, number> {
  const out: Record<string, number> = {};
  for (const line of text.split("\n")) {
    const m = line.trim().match(/^(\d+)x?\s+(.+)$/);
    if (m) out[m[2]] = (out[m[2]] ?? 0) + Number(m[1]);
  }
  return out;
}

export function listText(list: Record<string, number>): string {
  return Object.entries(list).filter(([, n]) => n > 0).map(([name, n]) => `${n} ${name}`).join("\n");
}

export interface StarterChoice {
  id: string;
  label: string;
  color: string;
  pitch: string;
}

export const STARTERS: StarterChoice[] = [
  { id: "white", label: "White", color: "#f3ead0", pitch: "Brave soldiers and griffins who fight together and win by numbers." },
  { id: "blue", label: "Blue", color: "#4f9be0", pitch: "Drakes in the sky, counterspells in hand. Clever, patient, a little sneaky." },
  { id: "black", label: "Black", color: "#9b7bb0", pitch: "Zombies, rats and removal. Nothing stays dead, except your opponent's creatures." },
  { id: "red", label: "Red", color: "#e2553a", pitch: "Goblins with haste and spells that burn. Fast, loud and reckless." },
  { id: "green", label: "Green", color: "#4fae64", pitch: "Elves for mana and big beasts to spend it on. Growth spells make them bigger." },
  { id: "colorless", label: "Colorless", color: "#b7bcc6", pitch: "Clockwork constructs that need no color at all. Sturdy, steady and strange." },
  { id: "chaos", label: "Chaos", color: "linear-gradient(90deg,#f3ead0,#4f9be0,#9b7bb0,#e2553a,#4fae64)", pitch: "Bram reaches into the bottom drawer without looking. Two or three colors, all mixed up. Nobody knows what you'll get. Not even Bram." },
];

const COLOR_OF: Record<string, string> = { white: "W", blue: "U", black: "B", red: "R", green: "G" };
const BASIC_OF: Record<string, string> = { W: "Plains", U: "Island", B: "Swamp", R: "Mountain", G: "Forest" };
/** Gold cards Chaos can throw in, by the colors they need. */
const GOLD: [string, string][] = [
  ["Watchwolf", "GW"], ["Steward of Valeron", "GW"], ["Boros Swiftblade", "RW"], ["Lightning Helix", "RW"], ["Terminate", "BR"],
  ["Putrefy", "BG"], ["Vindicate", "WB"], ["Baleful Strix", "UB"], ["Wee Dragonauts", "UR"], ["Rip-Clan Crasher", "RG"],
  ["Skyknight Legionnaire", "RW"], ["Rhox War Monk", "GWU"], ["Woolly Thoctar", "RGW"], ["Absorb", "WU"], ["Fires of Yavimaya", "RG"],
];

/** Chaos: a random two- or three-color deck, mixed from the single-color starters. */
export function chaosDeck(rand: () => number = Math.random): { name: string; list: Record<string, number> } {
  const colors = ["white", "blue", "black", "red", "green"];
  for (let i = colors.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [colors[i], colors[j]] = [colors[j], colors[i]];
  }
  const picked = colors.slice(0, rand() < 0.6 ? 2 : 3);
  const letters = picked.map((c) => COLOR_OF[c]);
  const list: Record<string, number> = {};
  const per = picked.length === 2 ? 10 : 6;
  for (const c of picked) {
    const spells = Object.entries(parseList(DECKS.starters[c].text)).filter(([n]) => !isBasic(n));
    let taken = 0;
    for (const [name, n] of spells.sort(() => rand() - 0.5)) {
      if (taken >= per) break;
      const k = Math.min(n, 2, per - taken);
      list[name] = k;
      taken += k;
    }
  }
  const gold = GOLD.filter(([, need]) => need.split("").every((l) => letters.includes(l))).sort(() => rand() - 0.5);
  let spells = Object.values(list).reduce((a, b) => a + b, 0);
  for (const [g] of gold) {
    if (spells >= 23) break;
    const k = Math.min(2, 23 - spells);
    list[g] = k;
    spells += k;
  }
  if (spells < 23) {
    list["Alloy Myr"] = Math.min(2, 23 - spells);
    spells += list["Alloy Myr"];
  }
  if (spells < 23) list["Field Creeper"] = 23 - spells;
  // Seventeen lands, split by color.
  const base = Math.floor(17 / letters.length);
  letters.forEach((l, i) => (list[BASIC_OF[l]] = base + (i < 17 - base * letters.length ? 1 : 0)));
  const names: Record<string, string> = { W: "Dawn", U: "Tide", B: "Grave", R: "Ember", G: "Wild" };
  return { name: `Chaos: ${letters.map((l) => names[l]).join("-")}`, list };
}

export const DUELIST_IDS = Object.keys(PEOPLE).filter((k) => PEOPLE[k].duel?.rank === "duelist");
export const BOSS_IDS = Object.keys(PEOPLE).filter((k) => PEOPLE[k].duel?.rank === "boss");
export const ENDGAME_IDS = Object.keys(PEOPLE).filter((k) => PEOPLE[k].duel?.rank === "endgame");
