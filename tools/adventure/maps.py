"""Builds adventure mode's world: the overworld, interiors, dungeons and the Spire.

Maps are tile grids (one character per tile, legend in app/src/adventure/art.ts) plus objects
(people, lost cards, rare reagents, signs, doors). The layout is hand-placed; trees, rocks,
grass and flowers are scattered with a fixed seed so the world is the same every build.

Run:  python3 tools/adventure/maps.py   (writes app/src/adventure/data/world.json)
"""
import json, pathlib, random
from collections import deque

ROOT = pathlib.Path(__file__).resolve().parents[2]
rng = random.Random(1987)

# Tiles you can walk on. Everything else blocks.
WALK = set(".,\":smahgbfckdeEuvxpwi")

class Map:
    def __init__(self, mid, name, w, h, fill, theme):
        self.id, self.name, self.w, self.h, self.theme = mid, name, w, h, theme
        self.t = [[fill] * w for _ in range(h)]
        self.objects = []

    def get(self, x, y):
        return self.t[y][x] if 0 <= x < self.w and 0 <= y < self.h else None

    def set(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.t[y][x] = c

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1):
            for x in range(x0, x1):
                self.set(x, y, c)

    def outline(self, x0, y0, x1, y1, c, th=1):
        for y in range(y0, y1):
            for x in range(x0, x1):
                if x < x0 + th or x >= x1 - th or y < y0 + th or y >= y1 - th:
                    self.set(x, y, c)

    def blob(self, cx, cy, r, c, only=None, rough=0.35):
        for y in range(int(cy - r - 1), int(cy + r + 2)):
            for x in range(int(cx - r - 1), int(cx + r + 2)):
                d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
                if d <= r - rough + rng.random() * rough * 2 and (only is None or self.get(x, y) in only):
                    self.set(x, y, c)

    def scatter(self, x0, y0, x1, y1, c, chance, only):
        for y in range(y0, y1):
            for x in range(x0, x1):
                if self.get(x, y) in only and rng.random() < chance:
                    self.set(x, y, c)

    def path(self, pts, c=":", width=2, keep=""):
        for (ax, ay), (bx, by) in zip(pts, pts[1:]):
            x, y = ax, ay
            while (x, y) != (bx, by):
                for dx in range(width):
                    for dy in range(width):
                        if self.get(x + dx, y + dy) not in keep:
                            self.set(x + dx, y + dy, c)
                if x != bx:
                    x += 1 if bx > x else -1
                elif y != by:
                    y += 1 if by > y else -1
            for dx in range(width):
                for dy in range(width):
                    if self.get(bx + dx, by + dy) not in keep:
                        self.set(bx + dx, by + dy, c)

    def obj(self, kind, x, y, **kw):
        o = {"kind": kind, "x": x, "y": y, **kw}
        self.objects.append(o)
        return o

    def json(self):
        return {"name": self.name, "theme": self.theme, "w": self.w, "h": self.h,
                "tiles": ["".join(r) for r in self.t], "objects": self.objects}

def house(m, x, y, w, roof="O", roof_rows=2, wall_rows=2, door=None, sign=None):
    """A building with its top-left corner at x, y. Returns the door tile."""
    m.rect(x, y, x + w, y + roof_rows, roof)
    m.rect(x, y + roof_rows, x + w, y + roof_rows + wall_rows, "H")
    for wx in range(x + 1, x + w - 1, 3):
        m.set(wx, y + roof_rows, "N")
    dx = x + w // 2 if door is None else door
    dy = y + roof_rows + wall_rows - 1
    m.set(dx, dy, "d")
    m.set(dx, dy - 1 if wall_rows > 1 else dy, "H" if wall_rows > 1 else "d")
    # A clear step in front of the door.
    for sx in (dx - 1, dx, dx + 1):
        if m.get(sx, dy + 1) not in (":",):
            m.set(sx, dy + 1, ":" if sx == dx else m.get(sx, dy + 1) if m.get(sx, dy + 1) in WALK else ".")
    if sign:
        m.set(dx - 2, dy + 1, ".")
        m.obj("sign", dx - 2, dy + 1, text=sign)
    return dx, dy

def cave(m, x, y, rock="R", door="e"):
    """A rock face 5 wide and 3 tall with an entrance at the bottom middle (x, y)."""
    for yy in range(y - 2, y + 1):
        for xx in range(x - 2, x + 3):
            m.set(xx, yy, rock)
    m.set(x, y, door)
    m.set(x, y + 1, ":")
    m.set(x - 1, y + 1, ":")
    m.set(x + 1, y + 1, ":")

# ------------------------------------------------------------------ overworld
W, H = 100, 80
ow = Map("overworld", "The Vale of Elderreach", W, H, ".", "vale")
REG = {
    "high": (0, 0, 34, 28, "h"), "spire": (34, 0, 66, 28, "g"), "ember": (66, 0, 100, 28, "a"),
    "mire": (0, 28, 34, 54, "m"), "meadow": (34, 28, 66, 54, "."), "coast": (66, 28, 100, 54, "."),
    "wilds": (0, 54, 34, 80, "."), "village": (34, 54, 66, 80, "."), "lake": (66, 54, 100, 80, "."),
}
REGION_NAMES = {
    "high": "Sunspire Highlands", "spire": "The Shattered Reach", "ember": "Emberpeak",
    "mire": "Mirefen", "meadow": "Thistle Meadow", "coast": "Tidewater Coast",
    "wilds": "The Verdant Wilds", "village": "Hearthmoor", "lake": "Willowmere",
}
for k, (x0, y0, x1, y1, g) in REG.items():
    ow.rect(x0, y0, x1, y1, g)

# Region borders: thick lines of trees, rock or cliffs with gaps where the roads cross.
def vborder(x, y0, y1, c, gaps):
    for y in range(y0, y1):
        if not any(g - 1 <= y <= g + 2 for g in gaps):
            ow.set(x - 1, y, c); ow.set(x, y, c)
            if rng.random() < 0.5: ow.set(x + 1 if rng.random() < 0.5 else x - 2, y, c)

def hborder(y, x0, x1, c, gaps):
    for x in range(x0, x1):
        if not any(g - 1 <= x <= g + 2 for g in gaps):
            ow.set(x, y - 1, c); ow.set(x, y, c)
            if rng.random() < 0.5: ow.set(x, y + 1 if rng.random() < 0.5 else y - 2, c)

vborder(34, 0, 28, "P", [14])          # highlands | reach
vborder(66, 0, 28, "R", [14])          # reach | ember
vborder(34, 28, 54, "D", [41])         # mire | meadow
vborder(66, 28, 54, "T", [41])         # meadow | coast
vborder(34, 54, 80, "T", [67])         # wilds | village
vborder(66, 54, 80, "T", [67])         # village | lake
hborder(28, 0, 34, "P", [17])          # highlands / mire
hborder(28, 34, 66, "R", [50])         # reach / meadow
hborder(28, 66, 100, "R", [83])        # ember / coast
hborder(54, 0, 34, "T", [17])          # mire / wilds
hborder(54, 34, 66, "T", [50])         # meadow / village
hborder(54, 66, 100, "T", [80])        # coast / lake

# Edges of the world.
ow.outline(0, 0, W, H, "T", 2)
ow.rect(0, 0, W, 2, "R")
for x in range(2, 66):
    if rng.random() < 0.6: ow.set(x, 2, "R")
ow.rect(66, 0, 100, 28, "a"); ow.outline(66, 0, 100, 28, "R", 2)
vborder(66, 0, 28, "R", [14])
hborder(28, 66, 100, "R", [83])
# The sea along the east coast and the lake shore.
for y in range(28, 80):
    edge = 92 + int(2 * __import__("math").sin(y / 4.0))
    for x in range(edge - 4, edge):
        if ow.get(x, y) not in ("T", "R"): ow.set(x, y, "s")
    for x in range(edge, W):
        ow.set(x, y, "~")

# ---- textures per region
def scatter_region(k, items):
    x0, y0, x1, y1, g = REG[k]
    for c, chance in items:
        ow.scatter(x0 + 2, y0 + 2, x1 - 2, y1 - 2, c, chance, {g})

# Highlands: pines, boulders, flowers, a cold tarn.
ow.blob(24, 8, 3.2, "~", {"h"})
scatter_region("high", [("P", 0.09), ("r", 0.03), (",", 0.05), ('"', 0.04)])
# The Shattered Reach: ruined stone, crystals, broken pillars, patchy grass.
for _ in range(14):
    ow.blob(rng.randint(37, 63), rng.randint(4, 25), rng.uniform(1.5, 3.5), ".", {"g"})
scatter_region("spire", [("Y", 0.025), ("Q", 0.02), ("r", 0.03)])
ow.scatter(36, 3, 64, 26, "r", 0.02, {"."})
# Emberpeak: ash, lava pools, rocks, dead trees.
for c in [(74, 8, 2.6), (90, 18, 3.0), (78, 21, 2.0), (95, 11, 1.8)]:
    ow.blob(*c, "L", {"a"})
scatter_region("ember", [("R", 0.06), ("r", 0.05), ("D", 0.03)])
# Mirefen: bog pools, dead trees, reeds (tall grass).
for c in [(10, 40, 3.0), (24, 47, 2.6), (6, 49, 2.0), (27, 33, 2.0), (14, 32, 1.6)]:
    ow.blob(*c, "M", {"m"})
scatter_region("mire", [("D", 0.07), ('"', 0.12), ("r", 0.02)])
# Thistle Meadow: open grass, flowers, tall grass, a pond.
ow.blob(42, 34, 2.4, "~", {"."})
scatter_region("meadow", [('"', 0.10), (",", 0.06), ("T", 0.05)])
# Tidewater: palms near the sand, rocks, tide pools.
scatter_region("coast", [("T", 0.05), (",", 0.04), ('"', 0.06)])
ow.scatter(68, 30, 92, 52, "r", 0.03, {"s"})
for c in [(87, 36, 1.4), (85, 50, 1.2)]:
    ow.blob(*c, "~", {"s", "."})
# Verdant Wilds: thick forest with clearings.
scatter_region("wilds", [("T", 0.38), ('"', 0.16), (",", 0.04)])
for c in [(10, 60, 3.0), (24, 64, 3.2), (14, 70, 3.0), (26, 74, 2.6), (8, 76, 2.4), (18, 58, 2.4)]:
    ow.blob(*c, ".", {"T", '"', ","})
ow.blob(23, 69, 1.8, "~", {".", '"', ","})
# Hearthmoor: tidy, a few trees and flowers.
scatter_region("village", [(",", 0.05), ("T", 0.03)])
# Willowmere: the lake.
ow.blob(80, 70, 6.5, "~", {".", ","}, rough=0.8)
ow.blob(80, 70, 7.5, "s", {"."}, rough=0.6)
scatter_region("lake", [("T", 0.06), (",", 0.06), ('"', 0.10)])

# ---- roads
VC = (49, 66)  # village square
ow.path([VC, (49, 41)])                       # village north to the crossroads
ow.path([(49, 41), (17, 41), (17, 14)])       # west into the mire, north to the highlands
ow.path([(49, 41), (83, 41), (83, 14)])       # east to the coast, north to emberpeak
ow.path([(49, 41), (49, 12)])                 # north to the Spire
ow.path([(17, 14), (49, 14), (83, 14)])       # the old high road across the north
ow.path([VC, (17, 66)])                       # west into the wilds
ow.path([VC, (78, 66), (78, 60)])             # east to Willowmere
ow.path([(17, 41), (17, 66)])                 # mire down to the wilds
ow.path([(80, 41), (80, 54), (80, 60)])       # coast down to the lake
# Wooden bridges where roads cross water.
for y in range(H):
    for x in range(W):
        pass

# ---- Hearthmoor
ow.rect(38, 57, 63, 77, ".")
ow.path([VC, (49, 41)])
ow.path([VC, (17, 66)])
ow.path([VC, (78, 66)])
ow.path([(40, 70), (60, 70)])
ow.path([(44, 60), (44, 66)], width=1)
ow.path([(56, 60), (56, 66)], width=1)
shop_door = house(ow, 40, 56, 9, roof="O", roof_rows=3, wall_rows=2, sign="Bram's Curios — cards bought, sold and treasured.")
inn_door = house(ow, 53, 56, 8, roof="U", roof_rows=3, wall_rows=2, sign="The Sleeping Fox — rest your feet.")
houseA = house(ow, 38, 72, 6, roof="G")
houseB = house(ow, 56, 72, 6, roof="O")
ow.obj("waystone", 47, 69, id="hearthmoor")
ow.rect(51, 67, 53, 69, ".")
ow.obj("well", 52, 68)
for x in range(38, 63):
    if ow.get(x, 77) == ".": ow.set(x, 77, "#")
for (fx, fy) in [(39, 63), (40, 63), (60, 63), (61, 63), (42, 76), (58, 76)]:
    ow.set(fx, fy, ",")
ow.obj("sign", 47, 54, text="North: Thistle Meadow and the crossroads.\nThe road splits for Mirefen, the coast and the ruins of the Reach.")

# ---- wizard's tower in Thistle Meadow
ow.rect(55, 31, 62, 39, ".")
tower_door = house(ow, 56, 31, 4, roof="J", roof_rows=3, wall_rows=3)
ow.path([(49, 39), (58, 39)], width=1)
ow.obj("sign", 56, 39, text="Ysolde's Tower.\nKnock loudly. Do not touch the cauldron.")

# ---- dungeon entrances
cave(ow, 8, 34, "R", "e")        # Hollow Barrow (mire)
ow.path([(17, 38), (8, 38), (8, 35)], width=1)
cave(ow, 8, 7, "R", "E")         # Gilded Sanctum (highlands)
ow.path([(17, 12), (8, 12), (8, 8)], width=1)
cave(ow, 92, 7, "R", "e")        # Cinder Depths (emberpeak)
ow.path([(84, 12), (92, 12), (92, 8)], width=1)
cave(ow, 89, 32, "R", "e")       # Drowned Grotto (coast)
ow.path([(84, 38), (89, 38), (89, 33)], width=1)
cave(ow, 7, 75, "T", "e")        # Thornheart Hollow (wilds)
ow.path([(17, 72), (7, 72), (7, 76)], width=1)
ow.set(7, 75, "e"); ow.set(7, 76, ":")

# ---- the Spire
ow.rect(43, 3, 57, 12, "g")
for yy in range(3, 9):
    for xx in range(46, 54):
        ow.set(xx, yy, "Z" if yy < 8 else "Z")
ow.set(49, 8, "E"); ow.set(50, 8, "Z")
ow.set(49, 9, "g"); ow.set(48, 9, "g"); ow.set(50, 9, "g")
ow.obj("gate", 49, 9, id="spire")
ow.obj("waystone", 52, 11, id="reach")

# ---- waystones
for wid, x, y in [("mirefen", 19, 43), ("highlands", 19, 16), ("emberpeak", 81, 16), ("tidewater", 81, 43),
                  ("wilds", 19, 64), ("willowmere", 76, 62)]:
    ow.obj("waystone", x, y, id=wid)

# ---- signs at the borders
for x, y, text in [
    (31, 43, "West: Mirefen. Mind the bogs, and the folk who live in them."),
    (68, 43, "East: Tidewater Coast."),
    (19, 30, "North: the Sunspire Highlands."),
    (85, 30, "North: Emberpeak. The ground is warm. Then it is very warm."),
    (51, 30, "North: the Shattered Reach and the Rift Spire.\nNo one who climbed it remembers why they went."),
    (31, 68, "West: the Verdant Wilds. Stay on the path."),
    (68, 68, "East: Willowmere."),
    (10, 39, "Hollow Barrow.\nThe dead are resting. Please do the same."),
    (10, 13, "The Gilded Sanctum. Pilgrims welcome; doubters tolerated."),
    (90, 13, "The Cinder Depths. Turn back."),
    (87, 39, "The Drowned Grotto. Low tide only."),
    (9, 72, "Thornheart Hollow."),
]:
    ow.obj("sign", x, y, text=text)

# ---- duelists on the overworld (facing, sight range)
DUELISTS_OW = [
    ("rowan", 47, 47, "down", 4), ("tam", 52, 36, "left", 4),
    ("pip", 75, 64, "left", 3), ("ida", 88, 63, "down", 3), ("nell", 72, 69, "right", 3),
    ("mosswick", 22, 41, "down", 4), ("vey", 14, 46, "right", 4), ("gil", 19, 33, "left", 3),
    ("edda", 17, 21, "right", 4), ("calix", 26, 14, "down", 3), ("lune", 12, 18, "right", 4),
    ("brakka", 85, 21, "left", 4), ("snag", 76, 14, "down", 3), ("kessa", 89, 15, "left", 3),
    ("quill", 74, 41, "down", 4), ("brine", 86, 45, "left", 4), ("ivo", 83, 34, "right", 3),
    ("thorne", 24, 66, "up", 4), ("kip", 13, 61, "right", 3), ("oakhart", 17, 70, "left", 3),
    ("gearlock", 44, 18, "right", 4), ("sentinel", 51, 20, "left", 3),
]
for nid, x, y, d, sight in DUELISTS_OW:
    ow.obj("npc", x, y, id=nid, dir=d, sight=sight)

# ---- townsfolk
TOWNS = [
    ("mira", 45, 67, "down"), ("old_tom", 58, 69, "left"), ("kid_ash", 41, 69, "right"),
    ("guard_hal", 49, 56, "down"), ("fisher_wen", 79, 61, "down"), ("pilgrim", 20, 24, "down"),
    ("smith", 80, 24, "left"), ("hermit", 28, 50, "left"), ("scholar", 40, 22, "right"),
    ("sailor", 87, 41, "left"),
]
for nid, x, y, d in TOWNS:
    ow.obj("npc", x, y, id=nid, dir=d, sight=0)

# ---- lost cards (visible) and hidden ones (in grass, glinting now and then)
LOST = {
    "meadow": [(38, 32), (60, 46), (45, 50), (63, 30)],
    "village": [(62, 59), (36, 75)],
    "lake": [(70, 76), (95, 60), (72, 57), (90, 76)],
    "mire": [(4, 44), (29, 36), (12, 52), (30, 48)],
    "high": [(4, 22), (29, 4), (22, 25), (31, 19)],
    "ember": [(70, 4), (96, 24), (88, 3), (72, 25)],
    "coast": [(70, 31), (90, 52), (77, 50), (71, 47)],
    "wilds": [(4, 58), (30, 77), (12, 66), (29, 58)],
    "spire": [(38, 5), (62, 24), (60, 6)],
}
HIDDEN = {
    "meadow": [(40, 44), (58, 50), (36, 37)], "lake": [(85, 61), (68, 73)], "mire": [(7, 30), (25, 51), (20, 37)],
    "high": [(5, 15), (27, 22)], "ember": [(95, 4), (70, 18)], "coast": [(78, 35), (73, 52)],
    "wilds": [(4, 70), (21, 76), (31, 62)], "spire": [(36, 25), (63, 13)], "village": [(64, 77)],
}
n = 0
for region, spots in LOST.items():
    for x, y in spots:
        n += 1
        ow.obj("card", x, y, id=f"ow{n}", region=region)
for region, spots in HIDDEN.items():
    for x, y in spots:
        n += 1
        ow.obj("card", x, y, id=f"ow{n}", region=region, hidden=True)
        if ow.get(x, y) in WALK and ow.get(x, y) not in ':':
            ow.set(x, y, '"')

# ---- rare reagents out in the world
for rid, x, y in [("moonpetal", 4, 4), ("phoenix_ember", 97, 2), ("leviathan_pearl", 95, 30),
                  ("mandrake", 3, 63), ("starfall_shard", 63, 3), ("sunstone", 30, 2)]:
    ow.obj("reagent", x, y, id=rid)

# Doors.
def warp(m, x, y, to, tx, ty, d):
    m.obj("warp", x, y, to=to, tx=tx, ty=ty, dir=d)

# ------------------------------------------------------------------ interiors
def interior(mid, name, w, h, theme, floor="f", wall="W"):
    m = Map(mid, name, w, h, floor, theme)
    m.rect(0, 0, w, 2, wall)
    m.outline(0, 0, w, h, wall)
    return m

shop = interior("shop", "Bram's Curios", 13, 10, "shop")
shop.rect(1, 1, 12, 2, "B")
shop.rect(1, 4, 9, 5, "K")
shop.set(3, 1, "W"); shop.set(9, 1, "W")
shop.rect(10, 2, 12, 4, "C")
shop.set(1, 7, "C"); shop.set(11, 7, "C")
shop.rect(4, 6, 9, 9, "k")
shop.set(6, 9, "d")
shop.obj("npc", 4, 3, id="bram", dir="down", sight=0)
warp(shop, 6, 9, "overworld", shop_door[0], shop_door[1] + 1, "down")
warp(ow, shop_door[0], shop_door[1], "shop", 6, 8, "up")

inn = interior("inn", "The Sleeping Fox", 12, 10, "shop")
inn.rect(1, 1, 11, 2, "B")
inn.rect(1, 4, 5, 5, "K")
inn.rect(7, 3, 11, 4, "C")
inn.rect(7, 6, 11, 7, "C")
inn.rect(4, 7, 8, 9, "k")
inn.set(6, 9, "d")
inn.obj("npc", 2, 3, id="innkeep", dir="down", sight=0)
inn.obj("npc", 9, 5, id="bard", dir="left", sight=0)
warp(inn, 6, 9, "overworld", inn_door[0], inn_door[1] + 1, "down")
warp(ow, inn_door[0], inn_door[1], "inn", 6, 8, "up")

ha = interior("house_a", "Granny Wenna's cottage", 9, 8, "shop")
ha.rect(1, 1, 8, 2, "B"); ha.set(6, 3, "C"); ha.set(1, 3, "C")
ha.set(4, 7, "d")
ha.obj("npc", 2, 4, id="wenna", dir="right", sight=0)
warp(ha, 4, 7, "overworld", houseA[0], houseA[1] + 1, "down")
warp(ow, houseA[0], houseA[1], "house_a", 4, 6, "up")

hb = interior("house_b", "The Ferrow house", 9, 8, "shop")
hb.rect(1, 1, 8, 2, "B"); hb.set(1, 3, "C"); hb.set(7, 3, "C")
hb.set(4, 7, "d")
hb.obj("npc", 6, 4, id="ferrow", dir="left", sight=0)
warp(hb, 4, 7, "overworld", houseB[0], houseB[1] + 1, "down")
warp(ow, houseB[0], houseB[1], "house_b", 4, 6, "up")

tower = interior("tower", "Ysolde's Tower", 11, 11, "tower", floor="c")
tower.rect(1, 1, 10, 2, "B")
tower.set(1, 3, "B"); tower.set(9, 3, "B"); tower.set(1, 4, "B"); tower.set(9, 4, "B")
tower.rect(3, 4, 8, 7, "k")
tower.obj("cauldron", 5, 5)
tower.obj("npc", 5, 3, id="ysolde", dir="down", sight=0)
tower.set(1, 8, "Y"); tower.set(9, 8, "Y")
tower.set(5, 10, "d")
warp(tower, 5, 10, "overworld", tower_door[0], tower_door[1] + 1, "down")
warp(ow, tower_door[0], tower_door[1], "tower", 5, 9, "up")

# ------------------------------------------------------------------ dungeons
def dungeon(mid, name, theme, entrance, acolytes, boss, chest_reagent, card_spots, extra=None):
    """A winding dungeon: entrance at the bottom, the boss's chamber at the top."""
    w, h = 31, 27
    m = Map(mid, name, w, h, "W", theme)
    rooms = [
        (12, 21, 19, 26),   # entrance hall
        (3, 15, 12, 21),    # west cave
        (19, 14, 28, 20),   # east cave
        (3, 6, 11, 12),     # north-west
        (11, 2, 20, 9),     # boss chamber
        (21, 5, 28, 10),    # treasure nook
    ]
    for x0, y0, x1, y1 in rooms:
        m.rect(x0, y0, x1, y1, "c")
    m.path([(15, 21), (15, 18), (8, 18)], "c", 2)
    m.path([(15, 18), (23, 18), (23, 16)], "c", 2)
    m.path([(6, 15), (6, 10)], "c", 2)
    m.path([(8, 8), (12, 8)], "c", 2)
    m.path([(24, 14), (24, 9)], "c", 2)
    m.path([(19, 7), (22, 7)], "c", 1)
    # Decoration.
    m.scatter(1, 1, w - 1, h - 1, "r", 0.05, {"c"})
    for x0, y0, x1, y1 in rooms:
        m.set(x0, y0, "Y"); m.set(x1 - 1, y0, "Y")
    m.rect(14, 2, 17, 4, "k")
    m.set(15, 26, "c")
    m.set(15, 25, "x")
    warp(m, 15, 25, "overworld", entrance[0], entrance[1] + 1, "down")
    for x0, y0, x1, y1 in rooms:
        for x in range(x0, x1):
            for y in range(y0, y1):
                pass
    # Keep corridors clear of rocks.
    for x in range(w):
        for y in range(h):
            if m.get(x, y) == "r" and sum(1 for dx, dy in ((1,0),(-1,0),(0,1),(0,-1)) if m.get(x+dx, y+dy) == "W") >= 2:
                m.set(x, y, "c")
    (a1, a1d), (a2, a2d) = acolytes
    m.obj("npc", 9, 18, id=a1, dir="right", sight=4)
    m.obj("npc", 6, 12, id=a2, dir="down", sight=4)
    m.obj("npc", 15, 3, id=boss, dir="down", sight=0)
    m.obj("reagent", 26, 6, id=chest_reagent, chest=True)
    for i, (x, y) in enumerate(card_spots):
        m.obj("card", x, y, id=f"{mid}{i}", region=theme, hidden=(i == len(card_spots) - 1))
    m.obj("waystone", 13, 23, id=mid)
    return m

DUNGEONS = [
    dungeon("barrow", "Hollow Barrow", "barrow", (8, 34), [("morrow", "right"), ("hob", "down")], "vesper", "grave_lily", [(4, 20), (26, 16), (21, 9)]),
    dungeon("sanctum", "The Gilded Sanctum", "sanctum", (8, 7), [("osric", "right"), ("maelis", "down")], "aurelia", "seraph_feather", [(4, 20), (26, 16), (21, 9)]),
    dungeon("cinder", "The Cinder Depths", "cinder", (92, 7), [("fizz", "right"), ("sora", "down")], "pyrrhus", "wyrm_scale", [(4, 20), (26, 16), (21, 9)]),
    dungeon("grotto", "The Drowned Grotto", "grotto", (89, 32), [("ambrose", "right"), ("sela", "down")], "oshara", "tide_glass", [(4, 20), (26, 16), (21, 9)]),
    dungeon("hollow", "Thornheart Hollow", "hollow", (7, 75), [("bramblebeard", "right"), ("fenna", "down")], "thornfather", "heartwood", [(4, 20), (26, 16), (21, 9)]),
]
for d, (ex, ey) in zip(DUNGEONS, [(8, 34), (8, 7), (92, 7), (89, 32), (7, 75)]):
    warp(ow, ex, ey, d.id, 15, 24, "up")

# Theme touches.
DUNGEONS[2].scatter(1, 1, 30, 26, "L", 0.03, {"c"})
DUNGEONS[3].scatter(1, 1, 30, 26, "~", 0.03, {"c"})
DUNGEONS[4].scatter(1, 1, 30, 26, '"', 0.12, {"c"})

# ------------------------------------------------------------------ the Rift Spire
sp1 = Map("spire1", "The Rift Spire — the Iron Hall", 17, 17, "W", "spire")
sp1.rect(2, 2, 15, 15, "p")
sp1.rect(6, 3, 11, 13, "k")
for y in (4, 7, 10, 13):
    sp1.set(3, y, "Q"); sp1.set(13, y, "Q")
sp1.set(8, 15, "p"); sp1.set(8, 16, "W")
sp1.set(8, 14, "x")
sp1.set(8, 2, "u")
sp1.obj("npc", 8, 3, id="regent", dir="down", sight=0)
warp(sp1, 8, 14, "overworld", 49, 9, "down")
warp(sp1, 8, 2, "spire2", 8, 13, "up")
warp(ow, 49, 8, "spire1", 8, 13, "up")
sp1.obj("waystone", 5, 13, id="spire")

sp2 = Map("spire2", "The Rift Spire — the Hall of Forgetting", 17, 17, "W", "rift")
sp2.rect(2, 2, 15, 15, "p")
sp2.blob(8, 7, 4.5, "k", {"p"}, rough=0.2)
for x, y in [(3, 3), (13, 3), (3, 12), (13, 12)]:
    sp2.set(x, y, "Y")
sp2.set(8, 14, "v")
sp2.obj("npc", 8, 4, id="morvane", dir="down", sight=0)
warp(sp2, 8, 14, "spire1", 8, 3, "down")
sp2.obj("reagent", 13, 7, id="voidglass", chest=True)

MAPS = [ow, shop, inn, ha, hb, tower, *DUNGEONS, sp1, sp2]

# ------------------------------------------------------------------ make sure everything is reachable
def walkable(m, x, y):
    c = m.get(x, y)
    return c is not None and c in WALK

def blocked_by_object(m):
    return {(o["x"], o["y"]) for o in m.objects if o["kind"] in ("npc", "sign", "waystone", "well", "cauldron") or (o["kind"] in ("card", "reagent") and not o.get("hidden"))}

def reach(m, start):
    objs = blocked_by_object(m)
    seen = {start}
    q = deque([start])
    while q:
        x, y = q.popleft()
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            p = (x + dx, y + dy)
            if p not in seen and walkable(m, *p) and p not in objs:
                seen.add(p); q.append(p)
    return seen

def ensure_reachable(m, start, ground):
    for _ in range(80):
        seen = reach(m, start)
        missing = None
        for o in m.objects:
            if o["kind"] == "warp":
                ok = (o["x"], o["y"]) in seen or any((o["x"] + dx, o["y"] + dy) in seen for dx, dy in ((1,0),(-1,0),(0,1),(0,-1)))
            else:
                ok = any((o["x"] + dx, o["y"] + dy) in seen for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))) or (o.get("hidden") and (o["x"], o["y"]) in seen)
            if not ok:
                missing = o; break
        if not missing:
            return
        # Carve toward the nearest reachable tile.
        tx, ty = missing["x"], missing["y"]
        best = min(seen, key=lambda p: abs(p[0] - tx) + abs(p[1] - ty))
        x, y = tx, ty + (1 if missing["kind"] != "warp" else 0)
        if missing["kind"] == "npc" or missing["kind"] == "reagent" or missing["kind"] == "card" or missing["kind"] == "sign" or missing["kind"] == "waystone":
            x, y = tx, ty + 1
        while (x, y) != best:
            if not walkable(m, x, y):
                m.set(x, y, ground(x, y))
            if x != best[0]: x += 1 if best[0] > x else -1
            elif y != best[1]: y += 1 if best[1] > y else -1
    raise SystemExit(f"{m.id}: could not connect {missing}")

def ow_ground(x, y):
    for k, (x0, y0, x1, y1, g) in REG.items():
        if x0 <= x < x1 and y0 <= y < y1:
            return g if g != "g" else "."
    return "."

# Objects must stand on walkable ground.
for m in MAPS:
    for o in m.objects:
        if o["kind"] in ("npc", "card", "reagent", "sign", "waystone", "gate", "well") and not walkable(m, o["x"], o["y"]):
            m.set(o["x"], o["y"], ow_ground(o["x"], o["y"]) if m is ow else "c" if m.theme not in ("shop",) else "f")

ensure_reachable(ow, (49, 63), ow_ground)
for m in MAPS[1:]:
    start = next((o["tx"], o["ty"]) for mm in MAPS for o in mm.objects if o["kind"] == "warp" and o["to"] == m.id)
    ensure_reachable(m, start, lambda x, y, m=m: "c" if m.theme not in ("shop", "spire", "rift") else "f" if m.theme == "shop" else "p")

out = {"start": {"map": "shop", "x": 4, "y": 5, "dir": "up"},
       "regions": {k: {"name": REGION_NAMES[k], "rect": list(v[:4])} for k, v in REG.items()},
       "maps": {m.id: m.json() for m in MAPS}}
path = ROOT / "app/src/adventure/data/world.json"
path.write_text(json.dumps(out, separators=(",", ":")) + "\n")
npcs = sum(1 for m in MAPS for o in m.objects if o["kind"] == "npc")
cards = sum(1 for m in MAPS for o in m.objects if o["kind"] == "card")
reag = sum(1 for m in MAPS for o in m.objects if o["kind"] == "reagent")
print(f"{len(MAPS)} maps, {npcs} people, {cards} lost cards, {reag} reagents -> {path}")
