// Plain-English explanations of keywords and common game terms, for the right-click popup.
// Written for this game in our own words.

export const GLOSSARY: Record<string, string> = {
  "Flying": "Can only be blocked by creatures with flying or reach.",
  "Reach": "Can block creatures with flying.",
  "Trample": "If it's blocked, damage beyond what's needed to destroy the blockers carries over to the player or planeswalker it's attacking.",
  "Deathtouch": "Any amount of damage it deals to a creature is enough to destroy that creature.",
  "Lifelink": "Damage it deals also makes its controller gain that much life.",
  "Vigilance": "Attacking doesn't cause it to tap.",
  "Haste": "Can attack and use {T} abilities the turn it comes under your control.",
  "First strike": "Deals combat damage before creatures without first strike.",
  "Double strike": "Deals combat damage twice: once in the first-strike step and again in the regular step.",
  "Menace": "Can't be blocked except by two or more creatures.",
  "Defender": "Can't attack.",
  "Hexproof": "Can't be the target of spells or abilities your opponents control.",
  "Shroud": "Can't be the target of any spells or abilities.",
  "Indestructible": "Isn't destroyed by damage or by effects that say \"destroy\".",
  "Flash": "You may cast it any time you could cast an instant.",
  "Ward": "When it becomes the target of a spell or ability an opponent controls, counter that unless they pay the ward cost.",
  "Protection": "Can't be blocked, targeted, dealt damage, enchanted or equipped by anything with the named quality.",
  "Backup": "When it enters, put +1/+1 counters on target creature. If that's another creature, it also gains the listed abilities until end of turn.",
  "Myriad": "When it attacks, for each other opponent you may create a token copy attacking that player (or their planeswalker). Exile the tokens at end of combat.",
  "Cascade": "When you cast it, exile cards from the top of your library until you hit a nonland card that costs less. You may cast that card free; put the rest on the bottom in a random order.",
  "Convoke": "Each creature you tap while casting it pays for {1} or one mana of that creature's color.",
  "Cycling": "Pay the cycling cost and discard this card to draw a card.",
  "Flashback": "You may cast it from your graveyard for its flashback cost. Then exile it.",
  "Kicker": "You may pay an extra cost as you cast it for an added effect.",
  "Partner": "You can have two commanders if both have partner.",
  "Prowess": "Whenever you cast a noncreature spell, it gets +1/+1 until end of turn.",
  "Undying": "When it dies, if it had no +1/+1 counters, return it to the battlefield with a +1/+1 counter.",
  "Persist": "When it dies, if it had no -1/-1 counters, return it to the battlefield with a -1/-1 counter.",
  "Annihilator": "Whenever it attacks, the defending player sacrifices that many permanents.",
  "Infect": "Deals damage to creatures as -1/-1 counters and to players as poison counters.",
  "Toxic": "Players dealt combat damage by it also get that many poison counters.",
  "Equip": "Pay the equip cost to attach it to a creature you control. Only as a sorcery.",
  "Enchant": "Says what this Aura can be attached to.",
  "Scry": "Look at that many cards from the top of your library, then put any number on the bottom and the rest back on top in any order.",
  "Surveil": "Look at that many cards from the top of your library, then put any number into your graveyard and the rest back on top in any order.",
  "Mill": "Put that many cards from the top of a library into its owner's graveyard.",
  "Proliferate": "Choose any number of permanents and players, then give each another counter of each kind already there.",
  "Treasure": "A Treasure is an artifact token with \"{T}, Sacrifice this: Add one mana of any color.\"",
  "Food": "A Food is an artifact token with \"{2}, {T}, Sacrifice this: You gain 3 life.\"",
  "Clue": "A Clue is an artifact token with \"{2}, Sacrifice this: Draw a card.\"",
  "Investigate": "Create a Clue token.",
  "Connive": "Draw a card, then discard a card. If you discarded a nonland card, put a +1/+1 counter on this creature.",
  "Explore": "Reveal the top card of your library. Land: put it in your hand. Otherwise put a +1/+1 counter on this creature and you may put that card into your graveyard.",
  "Landfall": "Triggers whenever a land enters the battlefield under your control.",
  "Affinity": "Costs {1} less for each permanent of the named kind you control.",
  "Delve": "Each card you exile from your graveyard while casting it pays for {1}.",
  "Storm": "When you cast it, copy it for each spell cast before it this turn.",
  "Exalted": "Whenever a creature you control attacks alone, it gets +1/+1 until end of turn for each instance of exalted.",
  "Evoke": "You may cast it for its evoke cost. If you do, it's sacrificed when it enters.",
  "Bestow": "You may cast it as an Aura for its bestow cost. It becomes a creature again if the creature it's on leaves.",
  "Dash": "You may cast it for its dash cost. It gains haste and returns to your hand at the next end step.",
  "Unearth": "Return it from your graveyard to the battlefield. It gains haste, and is exiled at the next end step or if it would leave. Only as a sorcery.",
  "Madness": "If you discard it, you may cast it for its madness cost instead of putting it into your graveyard.",
  "Ninjutsu": "Return an unblocked attacker you control to hand and put this onto the battlefield tapped and attacking.",
  "Crew": "Tap creatures with total power equal to the number to make this Vehicle an artifact creature until end of turn.",
  "Fight": "Each deals damage equal to its power to the other.",
  "Goad": "Until your next turn, the goaded creature attacks each combat if able, and attacks a player other than you if able.",
  "Monarch": "The monarch draws an extra card at their end step. Deal combat damage to the monarch to take the title.",
  "Initiative": "Take the initiative to venture into the Undercity. Deal combat damage to its holder to take it.",
  "Commander tax": "Each time you cast your commander from the command zone, it costs {2} more for each previous time.",
  "Commander damage": "A player who takes 21 or more combat damage from a single commander loses the game.",
  "Summoning sickness": "A creature can't attack or use {T} abilities until you've controlled it continuously since your most recent turn began.",
};

/** Keywords and terms explained for a card, from its keyword list and rules text. */
export function explain(keywords: string[], text: string): [string, string][] {
  const found = new Map<string, string>();
  const lower = text.toLowerCase();
  for (const k of keywords) {
    const key = Object.keys(GLOSSARY).find((g) => g.toLowerCase() === k.toLowerCase());
    if (key) found.set(key, GLOSSARY[key]);
  }
  for (const [k, v] of Object.entries(GLOSSARY)) {
    if (found.has(k)) continue;
    const re = new RegExp(`\\b${k.toLowerCase().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`);
    if (re.test(lower)) found.set(k, v);
  }
  return [...found.entries()];
}
