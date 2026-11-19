"""Extra real cards for adventure mode, written in MTGJSON's AtomicCards format.

The desktop app uses MTGJSON's own text once cards are downloaded; this file makes the
adventure playable before that (and in the browser during development). Every card here uses
simple, current oracle wording the rules engine runs in full; the client-api tests check that.

Run:  python3 tools/adventure/cards.py   (writes crates/client-api/data/adventure-cards.json)
"""
import json, re, pathlib

# name | mana cost | type line | P/T | oracle text (\n between abilities)
CARDS = r"""
Wastes||Basic Land||{T}: Add {C}.
Field Creeper|{2}|Artifact Creature — Scarecrow|2/1|
Phyrexian Walker|{0}|Artifact Creature — Phyrexian Construct|0/3|
Steel Wall|{3}|Artifact Creature — Wall|0/4|Defender
Wall of Spears|{3}|Artifact Creature — Wall|2/3|Defender\nFirst strike
Razorfield Thresher|{7}|Artifact Creature — Phyrexian Construct|6/4|
Hexplate Golem|{7}|Artifact Creature — Golem|5/7|
Phyrexian Hulk|{6}|Artifact Creature — Phyrexian Golem|5/4|
Prizefighter Construct|{5}|Artifact Creature — Construct|6/2|
Gold-Forged Sentinel|{6}|Artifact Creature — Chimera|4/4|Flying
Guardian Automaton|{4}|Artifact Creature — Construct|3/3|When this creature dies, you gain 3 life.
Skyscanner|{3}|Artifact Creature — Thopter|1/1|Flying\nWhen this creature enters, draw a card.
Pierce Strider|{4}|Artifact Creature — Phyrexian Construct|3/3|When this creature enters, target opponent loses 3 life.
Perilous Myr|{2}|Artifact Creature — Phyrexian Myr|1/1|When this creature dies, it deals 2 damage to any target.
Bottle Gnomes|{3}|Artifact Creature — Gnome|1/3|Sacrifice this creature: You gain 3 life.
Alloy Myr|{3}|Artifact Creature — Myr|2/2|{T}: Add one mana of any color.
Darksteel Myr|{3}|Artifact Creature — Myr|0/1|Indestructible
Filigree Familiar|{3}|Artifact Creature — Fox|2/2|When this creature enters, you gain 2 life.\nWhen this creature dies, draw a card.
Ornithopter of Paradise|{2}|Artifact Creature — Thopter|1/1|Flying\n{T}: Add one mana of any color.
Moonglove Extract|{3}|Artifact||Sacrifice this artifact: It deals 2 damage to any target.
Rod of Ruin|{4}|Artifact||{3}, {T}: This artifact deals 1 damage to any target.
Millstone|{2}|Artifact||{2}, {T}: Target player mills two cards.
Meteorite|{5}|Artifact||When this artifact enters, it deals 2 damage to any target.\n{T}: Add one mana of any color.
Cloudkin Seer|{2}{U}|Creature — Elemental Wizard|2/1|Flying\nWhen this creature enters, draw a card.
Prodigal Sorcerer|{2}{U}|Creature — Human Wizard|1/1|{T}: This creature deals 1 damage to any target.
Scroll Thief|{2}{U}|Creature — Merfolk Rogue|1/3|Whenever this creature deals combat damage to a player, draw a card.
Thieving Magpie|{2}{U}{U}|Creature — Bird|1/3|Flying\nWhenever this creature deals combat damage to a player, draw a card.
Talrand's Invocation|{2}{U}{U}|Sorcery||Create two 2/2 blue Drake creature tokens with flying.
Exclude|{2}{U}|Instant||Counter target creature spell.\nDraw a card.
Remove Soul|{1}{U}|Instant||Counter target creature spell.
Boomerang|{U}{U}|Instant||Return target permanent to its owner's hand.
Giant Octopus|{3}{U}|Creature — Octopus|3/3|
Tolarian Scholar|{2}{U}|Creature — Human Wizard|2/3|
Benthic Giant|{5}{U}|Creature — Giant|4/5|Hexproof
Spectral Sailor|{U}|Creature — Spirit Pirate|1/1|Flash\nFlying\n{3}{U}: Draw a card.
Fortress Crab|{3}{U}|Creature — Crab|1/6|
Aven Fisher|{3}{U}|Creature — Bird Soldier|2/2|Flying\nWhen this creature dies, you may draw a card.
Tome Scour|{U}|Sorcery||Target player mills five cards.
Mind Sculpt|{1}{U}|Sorcery||Target opponent mills seven cards.
Mist Raven|{2}{U}{U}|Creature — Bird|2/2|Flying\nWhen this creature enters, return target creature to its owner's hand.
Jace's Ingenuity|{3}{U}{U}|Instant||Draw three cards.
Opportunity|{2}{U}{U}|Instant||Target player draws four cards.
Lumengrid Warden|{1}{U}|Creature — Human Wizard|1/3|
Kraken Hatchling|{U}|Creature — Kraken|0/4|
Zephyr Sprite|{U}|Creature — Faerie|1/1|Flying
Pelakka Wurm|{4}{G}{G}{G}|Creature — Wurm|7/7|Trample\nWhen this creature enters, you gain 7 life.\nWhen this creature dies, draw a card.
Ancient Brontodon|{6}{G}{G}|Creature — Dinosaur|9/9|
Vastwood Gorger|{5}{G}|Creature — Wurm|5/6|
Sentinel Spider|{3}{G}{G}|Creature — Spider|4/4|Vigilance\nReach
Mammoth Spider|{4}{G}|Creature — Spider|3/5|Reach
Monstrous Growth|{1}{G}|Sorcery||Target creature gets +4/+4 until end of turn.
Thornweald Archer|{1}{G}|Creature — Elf Archer|2/1|Reach\nDeathtouch
Gigantosaurus|{G}{G}{G}{G}{G}|Creature — Dinosaur|10/10|
Jade Mage|{1}{G}{G}|Creature — Human Shaman|2/1|{2}{G}: Create a 1/1 green Saproling creature token.
Sprout|{G}|Instant||Create a 1/1 green Saproling creature token.
Thrashing Brontodon|{1}{G}{G}|Creature — Dinosaur|3/4|{1}, Sacrifice this creature: Destroy target artifact or enchantment.
Kavu Climber|{3}{G}{G}|Creature — Kavu|3/3|When this creature enters, draw a card.
Vine Trellis|{1}{G}|Creature — Plant Wall|0/4|Defender\n{T}: Add {G}.
Mighty Leap|{1}{W}|Instant||Target creature gets +2/+2 and gains flying until end of turn.
Moment of Heroism|{1}{W}|Instant||Target creature gets +2/+2 and gains lifelink until end of turn.
Dawnstrike Paladin|{3}{W}{W}|Creature — Human Knight|2/4|Vigilance\nLifelink
Nyx-Fleece Ram|{1}{W}|Creature — Sheep|0/5|At the beginning of your upkeep, you gain 1 life.
Soul Warden|{W}|Creature — Human Cleric|1/1|Whenever another creature enters, you gain 1 life.
Zetalpa, Primal Dawn|{6}{W}{W}|Legendary Creature — Elder Dinosaur|4/8|Flying, double strike, vigilance, trample, indestructible
Zombify|{3}{B}|Sorcery||Return target creature card from your graveyard to the battlefield.
Painful Lesson|{2}{B}|Sorcery||Target player draws two cards and loses 2 life.
Blood Pact|{2}{B}|Instant||Target player draws two cards and loses 2 life.
Deathbloom Thallid|{2}{B}|Creature — Fungus|3/2|When this creature dies, create a 1/1 green Saproling creature token.
Phyrexian Gargantua|{4}{B}{B}|Creature — Phyrexian Horror|4/4|When this creature enters, you draw two cards and you lose 2 life.
Shambling Goblin|{B}|Creature — Zombie Goblin|1/1|When this creature dies, target creature an opponent controls gets -1/-1 until end of turn.
Ember Hauler|{R}{R}|Creature — Goblin|2/2|{1}, Sacrifice this creature: It deals 2 damage to any target.
Thunder Strike|{1}{R}|Instant||Target creature gets +2/+0 and gains first strike until end of turn.
Kiln Fiend|{3}{R}|Creature — Elemental Beast|0/6|Whenever you cast an instant or sorcery spell, this creature gets +3/+0 until end of turn.
Guttersnipe|{2}{R}|Creature — Goblin Shaman|2/2|Whenever you cast an instant or sorcery spell, this creature deals 2 damage to each opponent.
Goblin Fireslinger|{R}|Creature — Goblin Warrior|1/1|{T}: This creature deals 1 damage to target player.
Bogardan Firefiend|{2}{R}|Creature — Elemental Spirit|2/1|When this creature dies, it deals 2 damage to target creature.
Watchwolf|{G}{W}|Creature — Wolf|3/3|
Woolly Thoctar|{R}{G}{W}|Creature — Beast|5/4|
Boros Swiftblade|{R}{W}|Creature — Human Soldier|1/2|Double strike
Skyknight Legionnaire|{1}{R}{W}|Creature — Human Knight|2/2|Flying, haste
Lightning Helix|{R}{W}|Instant||Lightning Helix deals 3 damage to any target and you gain 3 life.
Terminate|{B}{R}|Instant||Destroy target creature. It can't be regenerated.
Putrefy|{1}{B}{G}|Instant||Destroy target artifact or creature. It can't be regenerated.
Vindicate|{1}{W}{B}|Sorcery||Destroy target permanent.
Absorb|{W}{U}{U}|Instant||Counter target spell. You gain 3 life.
Baleful Strix|{U}{B}|Artifact Creature — Bird|1/1|Flying, deathtouch\nWhen this creature enters, draw a card.
Rhox War Monk|{G}{W}{U}|Creature — Rhino Monk|3/4|Lifelink
Lightning Angel|{1}{R}{W}{U}|Creature — Angel|3/4|Flying, vigilance, haste
Fires of Yavimaya|{1}{R}{G}|Enchantment||Creatures you control have haste.\nSacrifice this enchantment: Target creature gets +2/+2 until end of turn.
Wee Dragonauts|{1}{U}{R}|Creature — Faerie Wizard|1/3|Flying\nWhenever you cast an instant or sorcery spell, this creature gets +2/+0 until end of turn.
Broodmate Dragon|{3}{B}{R}{G}|Creature — Dragon|4/4|Flying\nWhen this creature enters, create a 4/4 red Dragon creature token with flying.
Tower Gargoyle|{1}{W}{U}{B}|Artifact Creature — Gargoyle|4/4|Flying
Steward of Valeron|{G}{W}|Creature — Human Druid Knight|2/2|Vigilance\n{T}: Add {G}.
Rip-Clan Crasher|{R}{G}|Creature — Human Warrior|2/2|Haste
"""

def colors_of(cost):
    return [c for c in "WUBRG" if "{" + c + "}" in cost]

out = {}
for line in CARDS.strip().splitlines():
    name, cost, tl, pt, text = line.split("|")
    text = text.replace("\\n", "\n")
    left, _, subs = tl.partition(" — ")
    words = left.split()
    supertypes = [w for w in words if w in ("Basic", "Legendary", "Snow")]
    types = [w for w in words if w not in supertypes]
    cols = colors_of(cost)
    ident = sorted(set(cols + colors_of(text)), key="WUBRG".index)
    card = {
        "name": name, "layout": "normal", "colors": cols, "colorIdentity": ident,
        "types": types, "supertypes": supertypes, "subtypes": subs.split() if subs else [],
        "text": text, "keywords": [], "legalities": {"commander": "Legal"},
    }
    if cost: card["manaCost"] = cost
    if pt:
        p, t = pt.split("/"); card["power"] = p; card["toughness"] = t
    if "Legendary" in supertypes and "Creature" in types:
        card["leadershipSkills"] = {"brawl": False, "commander": True, "oathbreaker": False}
    out[name] = [card]

path = pathlib.Path(__file__).resolve().parents[2] / "crates/client-api/data/adventure-cards.json"
path.write_text(json.dumps({"meta": {"date": "2026-11-19", "version": "adventure"}, "data": out}, indent=1, ensure_ascii=False) + "\n")
print(f"{len(out)} cards -> {path}")
