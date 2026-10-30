"""Redstone Arches: an original desert canyon arena built for 2 teams.

Blue and Red hold the two mesa tops. Each mesa's inner face is a long walkable
slope down into the canyon, where dunes run past an oasis pool and three
floating boulders give mid-height fights. A great natural arch spans the whole
canyon overhead, resting on rock legs at the map edges: a thruster on each
mesa flies up through a hatch onto it, the walkway rises to a crest in the
middle (the powerup), and two drop hatches lead back down into the canyon.
Mirror symmetric.
"""
import sys
from builder import Map

W = 72
m = Map(
    "redstone_arches", "Redstone Arches",
    "A dry canyon under a giant stone arch. Hold the mesas, brawl by the oasis, and fight for the crest of the arch.",
    (W, 36),
    {"sky_top": "#2b3a67", "sky_bottom": "#f0a35e", "far": "#8a4b38", "skyline": "mountains",
     "sun": [0.8, 0.18], "sun_color": "#ffd27a"},
    players=(4, 12),
)


def mirror(pts):
    return [(W - x, y) for x, y in reversed(pts)]


def lerp_line(line, x):
    """Height of a polyline [(x, y), ...] at x."""
    for (x0, y0), (x1, y1) in zip(line, line[1:]):
        if x0 <= x <= x1:
            t = 0 if x1 == x0 else (x - x0) / (x1 - x0)
            return y0 + (y1 - y0) * t
    raise ValueError(x)


# --- The canyon: mesas with walkable inner slopes, dunes and the oasis dip ------------
left = [(0, 19), (11, 19), (24, 31), (28, 31), (30, 32.4)]
ground = left + mirror(left)
m.poly("solid", "sand", ground + [(W, 36), (0, 36)])
m.poly("water", "water", [(28.43, 31.3), (W - 28.43, 31.3), (W - 30, 32.4), (30, 32.4)])

# --- The arch: walkway on top, curving underside, legs on the mesa tops -----------------
TOP = [(0, 11.5), (10, 11.5), (18, 10), (28, 8.6), (36, 8.2), (44, 8.6), (54, 10), (62, 11.5), (72, 11.5)]
BOT = [(0, 14.5), (2.5, 14.5), (6, 13.4), (16, 12.2), (26, 11), (36, 10.6), (46, 11), (56, 12.2),
       (66, 13.4), (69.5, 14.5), (72, 14.5)]
# Hatches: over each mesa thruster, and two drops into the canyon.
HATCHES = [(8, 10), (22, 24), (48, 50), (62, 64)]


def arch_piece(xa, xb):
    xs = sorted({xa, xb} | {x for x, _ in TOP if xa < x < xb} | {x for x, _ in BOT if xa < x < xb})
    top = [(x, lerp_line(TOP, x)) for x in xs]
    bot = [(x, lerp_line(BOT, x)) for x in reversed(xs)]
    m.poly("solid", "rock", top + bot)


edges = [0] + [x for h in HATCHES for x in h] + [W]
for a, b in zip(edges[0::2], edges[1::2]):
    arch_piece(a, b)
for a, b in HATCHES:
    ya, yb = lerp_line(TOP, a), lerp_line(TOP, b)
    m.poly("hatch", "wood", [(a, ya), (b, yb), (b, yb + 0.25), (a, ya + 0.25)])
m.poly("solid", "rock", [(0, 14.5), (2.5, 14.5), (2.5, 19), (0, 19)])        # left leg
m.poly("solid", "rock", [(W - 2.5, 14.5), (W, 14.5), (W, 19), (W - 2.5, 19)])  # right leg

# --- Floating boulders under the arch -------------------------------------------------------
SIDE = [(24.5, 26.5), (29, 26.5), (28.4, 27.6), (26.6, 28.2), (25, 27.5)]
m.poly("solid", "rock", SIDE)
m.poly("solid", "rock", mirror(SIDE))
m.poly("solid", "rock", [(32.5, 24), (39.5, 24), (38.8, 25.2), (36, 25.9), (33.2, 25.2)])

# --- Spawns ------------------------------------------------------------------------------------
for x in (4.5, 7):
    m.spawn(x, 19, 0)                                   # Blue: left mesa
    m.spawn(W - x, 19, 1)                               # Red: right mesa
for x, y in [(26, 31), (46, 31), (27, 26.5), (45, 26.5), (36, 24), (36, 8.2)]:
    m.spawn(x, y)


def on_arch(x):
    return lerp_line(TOP, x)


# --- Pickups --------------------------------------------------------------------------------
m.pickup("health", 25.6, 26.5, respawn=25)
m.pickup("health", W - 25.6, 26.5, respawn=25)
m.pickup("ammo", 27, 31, respawn=15)
m.pickup("ammo", W - 27, 31, respawn=15)
m.pickup("ammo", 4, on_arch(4), respawn=15)
m.pickup("ammo", W - 4, on_arch(W - 4), respawn=15)
m.pickup("weapon", 13, on_arch(13), weapon="needle_gun", respawn=25)
m.pickup("weapon", W - 13, on_arch(W - 13), weapon="needle_gun", respawn=25)
m.pickup("weapon", 25, 31, weapon="scattergun", respawn=25)
m.pickup("weapon", W - 25, 31, weapon="scattergun", respawn=25)
m.pickup("weapon", 36, 32.4, weapon="hellbarrel", respawn=40)    # at the bottom of the oasis
m.pickup("weapon", 34.5, 24, weapon="rail_spike", respawn=40)

# --- Gadgets ---------------------------------------------------------------------------------
m.obj("thruster", 9, 19, height_tiles=8)
m.obj("thruster", W - 9, 19, height_tiles=8)
m.obj("powerup_spot", 36, 8.2, powerup="rapid_fire")
m.obj("weapon_station", 5.8, 19)
m.obj("weapon_station", W - 5.8, 19)
m.obj("weapon_station", 28, 26.5)
m.obj("weapon_station", W - 28, 26.5)
m.obj("shield_station", 24.5, 31)
m.obj("shield_station", W - 24.5, 31)
m.obj("shield_station", 37.5, 24, shield=100, min_delay_s=90, max_delay_s=120)

# --- Decor --------------------------------------------------------------------------------------
for x, h in [(3.2, 2.0), (W - 4.4, 2.2), (17, 1.6)]:
    m.decor("cactus", x, 19 if x < 11 or x > W - 11 else 23.6, w=1.2, h=h)
m.decor("cactus", W - 18.2, 23.6, w=1.2, h=1.6)
m.decor("tree", 29.2, 31.2, w=2.4, h=4.2, color="#4f8a3c")         # palms by the oasis
m.decor("tree", W - 31.6, 31.2, w=2.4, h=3.8, color="#4f8a3c")
m.decor("fence", 2.5, 19, w=8.5, h=0.8, color="#7a5532")
m.decor("fence", W - 11, 19, w=8.5, h=0.8, color="#7a5532")
m.decor("banner", 34.9, 11.4, w=1.4, h=2.4, color="#b8452e")       # hanging under the crest
m.decor("chain", 30, 12.8, w=1, h=1.8)
m.decor("chain", W - 31, 12.8, w=1, h=1.8)
m.decor("lamp", 10.6, 19, h=1.8)
m.decor("lamp", W - 10.6, 19, h=1.8, flip=True)
m.decor("tank", 0.2, 11.5, w=2, h=2.4, color="#8b6a46")             # water tanks on the arch ends
m.decor("tank", W - 2.2, 11.5, w=2, h=2.4, color="#8b6a46")

# --- Objectives ----------------------------------------------------------------
# Zones: the two lower ledges and the floating platform in the middle.
m.zone(25.5, 31, w=5).zone(36, 24, w=6).zone(46.5, 31, w=5)
m.flag(3, 19, team=0)
m.flag(69, 19, team=1)

problems = m.headroom_report()
for p in problems:
    print("HEADROOM:", p)
m.write(sys.argv[1])
print("wrote", sys.argv[1], "problems:", len(problems))
