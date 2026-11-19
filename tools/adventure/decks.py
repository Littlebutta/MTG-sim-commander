"""Every deck in adventure mode: the seven starter decks and each duelist's deck.

Each deck lists its spells; basic lands fill it to its size, split by the colors its spells
need. Run:  python3 tools/adventure/decks.py   (writes app/src/adventure/data/decks.json)
The client-api tests check every deck resolves and is a legal adventure deck.
"""
import json, pathlib, re

BASIC = {"W": "Plains", "U": "Island", "B": "Swamp", "R": "Mountain", "G": "Forest"}

def parse(spells):
    out = []
    for part in spells.split(";"):
        part = part.strip()
        if not part:
            continue
        n, name = part.split(" ", 1)
        out.append((int(n), name.strip()))
    return out

def build(spells, size=40, lands=None, extra_lands=""):
    cards = parse(spells) + parse(extra_lands)
    n_spells = sum(n for n, _ in cards)
    n_lands = (size - n_spells) if lands is None else lands
    assert n_lands > 0, spells
    text = "\n".join(f"{n} {name}" for n, name in cards)
    return text, n_lands

# Pips per color come from card costs, read from the card files.
ROOT = pathlib.Path(__file__).resolve().parents[2]
COST = {}
for f in ["crates/client-api/data/starter-cards.json", "crates/client-api/data/adventure-cards.json"]:
    for name, faces in json.loads((ROOT / f).read_text())["data"].items():
        COST[name] = faces[0].get("manaCost", "")

def deck(spells, size=40, colorless_lands="Wastes"):
    cards = parse(spells)
    n_lands = size - sum(n for n, _ in cards)
    pips = {c: 0 for c in "WUBRG"}
    for n, name in cards:
        assert name in COST, f"unknown card {name}"
        for c in "WUBRG":
            pips[c] += n * COST[name].count("{" + c + "}")
    total = sum(pips.values())
    lines = [f"{n} {name}" for n, name in cards]
    if total == 0:
        lines.append(f"{n_lands} {colorless_lands}")
    else:
        colors = [c for c in "WUBRG" if pips[c]]
        alloc = {c: max(1, round(n_lands * pips[c] / total)) for c in colors}
        # Fix rounding so lands add up exactly.
        while sum(alloc.values()) > n_lands:
            c = max(alloc, key=lambda k: alloc[k]); alloc[c] -= 1
        while sum(alloc.values()) < n_lands:
            c = max(colors, key=lambda k: pips[k] / alloc[k]); alloc[c] += 1
        for c in colors:
            lines.append(f"{alloc[c]} {BASIC[c]}")
    return "\n".join(lines)

STARTERS = {
    "white": ("Dawnguard", "4 Savannah Lions; 3 Glory Seeker; 3 Youthful Knight; 2 Venerable Monk; 3 Wild Griffin; 2 Pillarfield Ox; 1 Serra Angel; 2 Raise the Alarm; 2 Charge; 1 Sunlance"),
    "blue": ("Tidecaller", "3 Merfolk of the Pearl Trident; 3 Coral Merfolk; 3 Wind Drake; 2 Horned Turtle; 2 Snapping Drake; 2 Azure Drake; 1 Air Elemental; 2 Unsummon; 2 Essence Scatter; 2 Divination; 1 Counterspell"),
    "black": ("Gravecaller", "3 Typhoid Rats; 3 Walking Corpse; 3 Scathe Zombies; 2 Dusk Imp; 2 Giant Cockroach; 2 Gravedigger; 1 Zombie Goliath; 2 Disfigure; 2 Vicious Hunger; 2 Raise Dead; 1 Hand of Death"),
    "red": ("Emberheart", "4 Raging Goblin; 3 Goblin Bully; 3 Gray Ogre; 2 Raging Cougar; 2 Hill Giant; 2 Canyon Minotaur; 1 Fire Elemental; 3 Shock; 2 Volcanic Hammer; 1 Brute Force"),
    "green": ("Wildheart", "3 Llanowar Elves; 4 Grizzly Bears; 3 Centaur Courser; 2 Giant Spider; 2 Ironroot Treefolk; 2 Spined Wurm; 1 Craw Wurm; 3 Giant Growth; 2 Titanic Growth; 1 Harmonize"),
    "colorless": ("Clockwork", "4 Field Creeper; 3 Bronze Sable; 3 Yotian Soldier; 2 Steel Wall; 2 Guardian Automaton; 2 Palladium Myr; 2 Skyscanner; 1 Phyrexian Hulk; 1 Razorfield Thresher; 2 Mind Stone; 1 Rod of Ruin"),
}

# Duelists: id -> spells. Lands fill each deck to 40.
DUELISTS = {
    # Thistle Meadow, near Hearthmoor: gentle first duels.
    "pip": "4 Grizzly Bears; 3 Savannah Lions; 3 Watchwolf; 3 Glory Seeker; 2 Runeclaw Bear; 2 Silvercoat Lion; 2 Steward of Valeron; 2 Pillarfield Ox; 2 Giant Growth",
    "rowan": "4 Tundra Wolves; 4 Youthful Knight; 3 Attended Knight; 2 Benalish Knight; 2 Knight of Meadowgrain; 2 Standing Troops; 2 Ekundu Griffin; 2 Sure Strike; 2 Mighty Leap",
    "nell": "4 Merfolk of the Pearl Trident; 4 Coral Merfolk; 3 Vodalian Soldiers; 3 Watercourser; 2 Scroll Thief; 2 Giant Octopus; 2 Kraken Hatchling; 2 Unsummon; 1 Divination",
    "tam": "4 Mons's Goblin Raiders; 4 Goblin Bully; 3 Goblin Instigator; 3 Raging Goblin; 2 Goblin Roughrider; 2 Krenko's Command; 2 Dragon Fodder; 2 Shock",
    "ida": "4 Llanowar Elves; 3 Elvish Warrior; 3 Nessian Courser; 2 Trained Armodon; 2 Rumbling Baloth; 3 Giant Growth; 2 Titanic Growth; 2 Monstrous Growth; 1 Overrun",
    # Mirefen.
    "mosswick": "4 Typhoid Rats; 4 Muck Rats; 3 Vampire of the Dire Moon; 2 Vampire Champion; 2 Feral Abomination; 2 Gifted Aetherborn; 3 Disfigure; 2 Last Gasp; 1 Murder",
    "vey": "4 Child of Night; 3 Vampire Spawn; 2 Bloodhunter Bat; 2 Vampire Sovereign; 2 Vampire Neonate; 2 Gifted Aetherborn; 3 Sovereign's Bite; 2 Douse in Gloom; 2 Vicious Hunger",
    "gil": "4 Walking Corpse; 4 Scathe Zombies; 3 Shambling Goblin; 2 Zombie Goliath; 2 Gravedigger; 2 Cadaver Imp; 3 Raise Dead; 2 Zombify; 1 Disentomb",
    "morrow": "4 Deathbloom Thallid; 3 Sprout; 3 Jade Mage; 2 Deadly Recluse; 2 Thornweald Archer; 2 Giant Spider; 3 Putrefy; 2 Disfigure; 1 Harmonize; 1 Murder",
    "hob": "4 Dusk Legion Zealot; 3 Phyrexian Rager; 3 Gravedigger; 2 Tithebearer Giant; 2 Skeleton Archer; 2 Bog Imp; 2 Night's Whisper; 2 Raise Dead; 2 Hand of Death; 1 Ambition's Cost",
    # Sunspire highlands.
    "edda": "4 Elite Vanguard; 3 Raise the Alarm; 3 Attended Knight; 2 Benalish Marshal; 2 Glorious Anthem; 2 Midnight Haunting; 3 Charge; 2 Inspired Charge; 2 Warrior's Honor",
    "calix": "4 Soul Warden; 3 Lone Missionary; 3 Venerable Monk; 2 Nyx-Fleece Ram; 2 Tireless Missionaries; 2 Angel of Mercy; 2 Seraph of Dawn; 2 Revitalize; 2 Dawnstrike Paladin",
    "lune": "4 Suntail Hawk; 3 Healer's Hawk; 3 Wild Griffin; 3 Aven Sentry; 2 Griffin Sentinel; 2 Shining Aerosaur; 2 Serra Angel; 2 Mighty Leap; 2 Sunlance",
    "osric": "4 Gideon's Lawkeeper; 3 Benalish Trapper; 3 Angelic Wall; 2 Wall of Omens; 2 Standing Troops; 2 Ardent Militia; 2 Yotian Soldier; 2 Serra Angel; 2 Day of Judgment",
    "maelis": "4 Boros Swiftblade; 3 Skyknight Legionnaire; 3 Youthful Knight; 2 Ajani's Sunstriker; 2 Anaba Bodyguard; 3 Lightning Helix; 2 Lightning Bolt; 2 Sure Strike; 1 Uncaged Fury",
    # Emberpeak.
    "brakka": "4 Raging Cougar; 3 Vulshok Berserker; 3 Lightning Elemental; 2 Charging Monstrosaur; 2 Cobblebrute; 2 Borderland Minotaur; 2 Kamahl, Pit Fighter; 2 Brute Strength; 2 Infuriate; 1 Fervor",
    "fizz": "4 Guttersnipe; 3 Kiln Fiend; 2 Prodigal Pyromancer; 4 Shock; 3 Lightning Strike; 2 Searing Spear; 2 Volcanic Hammer; 2 Lightning Blast; 1 Ember Shot",
    "snag": "4 Goblin Instigator; 3 Beetleback Chief; 3 Mogg Fanatic; 2 Ember Hauler; 2 Goblin Fireslinger; 3 Krenko's Command; 2 Hordeling Outburst; 2 Dragon Fodder; 2 Fervor",
    "sora": "4 Furnace Whelp; 3 Volcanic Dragon; 2 Shivan Dragon; 2 Fire Imp; 3 Sparkmage Apprentice; 2 Flametongue Kavu; 2 Lightning Strike; 3 Shock; 2 Flame Slash",
    "kessa": "4 Rip-Clan Crasher; 3 Garruk's Companion; 3 Raging Goblin; 2 Fires of Yavimaya; 2 Charging Monstrosaur; 2 Rumbling Baloth; 2 Kalonian Tusker; 2 Brute Force; 2 Giant Growth; 1 Overrun",
    # Tidewater coast.
    "quill": "4 Storm Crow; 3 Zephyr Sprite; 3 Wind Drake; 3 Cloudkin Seer; 2 Snapping Drake; 2 Mist Raven; 2 Air Elemental; 2 Unsummon; 2 Remove Soul",
    "ambrose": "3 Tolarian Scholar; 3 Lumengrid Warden; 2 Wall of Air; 2 Mahamoti Djinn; 3 Counterspell; 2 Cancel; 2 Negate; 2 Exclude; 2 Concentrate; 2 Jace's Ingenuity; 1 Opportunity",
    "brine": "4 Spectral Sailor; 3 Man-o'-War; 3 Scroll Thief; 2 Thieving Magpie; 2 Aven Fisher; 2 Mist Raven; 3 Unsummon; 2 Repulse; 2 Boomerang",
    "sela": "4 Kraken Hatchling; 3 Fortress Crab; 3 Horned Turtle; 2 Wall of Air; 4 Tome Scour; 3 Mind Sculpt; 2 Millstone; 2 Essence Scatter; 1 Counterspell",
    "ivo": "4 Wee Dragonauts; 3 Kiln Fiend; 2 Prodigal Sorcerer; 3 Shock; 3 Lightning Strike; 2 Unsummon; 2 Divination; 2 Negate; 2 Repulse; 1 Inspiration",
    # Verdant Wilds.
    "thorne": "4 Thornweald Archer; 4 Deadly Recluse; 3 Giant Spider; 3 Mammoth Spider; 2 Sentinel Spider; 2 Plummet; 2 Giant Growth; 2 Titanic Growth; 1 Harmonize",
    "bramblebeard": "4 Llanowar Elves; 3 Elvish Mystic; 2 Vine Trellis; 2 Leatherback Baloth; 2 Craw Wurm; 2 Vastwood Gorger; 2 Colossal Dreadmaw; 2 Enormous Baloth; 2 Pelakka Wurm; 2 Harmonize; 1 Overrun",
    "fenna": "4 Jade Mage; 4 Sprout; 3 Deathbloom Thallid; 3 Llanowar Visionary; 2 Wall of Wood; 2 Overrun; 2 Glorious Anthem; 2 Elvish Visionary; 2 Woodland Mystic",
    "kip": "4 Kalonian Tusker; 3 Trained Armodon; 3 Thrashing Brontodon; 2 Woodland Mystic; 2 Fyndhorn Elves; 2 Ancient Brontodon; 1 Gigantosaurus; 2 Charging Monstrosaur; 2 Titanic Growth; 2 Monstrous Growth",
    "oakhart": "4 Watchwolf; 4 Steward of Valeron; 3 Avacyn's Pilgrim; 2 Rhox War Monk; 2 Knight of Meadowgrain; 2 Serra Angel; 2 Glorious Anthem; 2 Giant Growth; 2 Inspired Charge",
    # The ruins under the Spire.
    "gearlock": "4 Copper Myr; 3 Iron Myr; 3 Alloy Myr; 2 Palladium Myr; 2 Perilous Myr; 2 Darksteel Myr; 2 Filigree Familiar; 2 Gold-Forged Sentinel; 2 Hexplate Golem; 2 Moonglove Extract",
    "sentinel": "4 Steel Wall; 3 Wall of Spears; 3 Yotian Soldier; 2 Guardian Automaton; 2 Pierce Strider; 2 Prizefighter Construct; 2 Ornithopter of Paradise; 2 Hedron Archive; 2 Rod of Ruin; 2 Phyrexian Hulk",
    # Bosses.
    "vesper": "4 Vampire Nighthawk; 3 Gifted Aetherborn; 3 Vampire Sovereign; 2 Bloodhunter Bat; 2 Ravenous Chupacabra; 2 Phyrexian Gargantua; 2 Doom Blade; 2 Murder; 2 Grasp of Darkness; 2 Soul Feast; 1 Drown in Sorrow",
    "aurelia": "4 Healer's Hawk; 3 Seraph of Dawn; 3 Serra Angel; 2 Shepherd of the Lost; 2 Archangel; 1 Zetalpa, Primal Dawn; 2 Angel of Mercy; 2 Inspiring Overseer; 2 Mighty Leap; 2 Sunlance; 1 Wrath of God",
    "pyrrhus": "4 Furnace Whelp; 3 Volcanic Dragon; 2 Shivan Dragon; 2 Shivan Hellkite; 2 Flametongue Kavu; 4 Lightning Bolt; 3 Lightning Strike; 2 Flame Slash; 1 Pyroclasm",
    "oshara": "3 Spectral Sailor; 3 Cloudkin Seer; 2 Mist Raven; 2 Benthic Giant; 2 Mahamoti Djinn; 4 Counterspell; 2 Exclude; 2 Essence Scatter; 2 Repulse; 1 Jace's Ingenuity; 1 Opportunity",
    "thornfather": "4 Llanowar Elves; 3 Birds of Paradise; 2 Sentinel Spider; 2 Leatherback Baloth; 2 Pelakka Wurm; 2 Ancient Brontodon; 1 Gigantosaurus; 2 Kavu Climber; 2 Titanic Growth; 2 Overrun; 1 Harmonize",
    # Endgame.
    "regent": "1 Sol Ring; 2 Mind Stone; 2 Baleful Strix; 2 Tower Gargoyle; 2 Gold-Forged Sentinel; 2 Hexplate Golem; 2 Wall of Omens; 2 Vampire Nighthawk; 2 Absorb; 2 Vindicate; 2 Counterspell; 1 Wrath of God; 1 Murder; 1 Serra Angel",
    "morvane": "2 Birds of Paradise; 2 Alloy Myr; 2 Broodmate Dragon; 2 Lightning Angel; 1 Zetalpa, Primal Dawn; 2 Woolly Thoctar; 2 Rhox War Monk; 2 Vampire Nighthawk; 2 Shivan Hellkite; 2 Lightning Helix; 2 Terminate; 2 Putrefy; 1 Vindicate",
}

out = {
    "starters": {k: {"name": n, "text": deck(s)} for k, (n, s) in STARTERS.items()},
    "duelists": {k: deck(s) for k, s in DUELISTS.items()},
}
path = ROOT / "app/src/adventure/data/decks.json"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(json.dumps(out, indent=1) + "\n")
print(f"{len(out['starters'])} starters, {len(out['duelists'])} duelists -> {path}")
for k, s in DUELISTS.items():
    n = sum(n for n, _ in parse(s))
    if not (21 <= n <= 25): print("spell count", k, n)
