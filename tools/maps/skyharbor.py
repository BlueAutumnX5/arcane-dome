"""Skyharbor: an original floating-island arena above the clouds.

Two home islands face a big central island carrying a two-deck metal dock
tower. Small stepping islands link them; thrusters lift fighters from the home
islands to high outcrops, and a shaft through hatches in both decks reaches
the tower roof (the powerup). Below everything is open sky: falling is fatal.
"""
import sys
from builder import Map

m = Map(
    "skyharbor", "Skyharbor",
    "Docks lashed to floating rocks high above the clouds. Mind your footing: the only thing below is a very long fall.",
    (70, 40),
    {"sky_top": "#5f9fd6", "sky_bottom": "#dcefff", "far": "#ffffff", "skyline": "clouds",
     "sun": [0.18, 0.18], "sun_color": "#fff3c4"},
    players=(4, 10),
)

def island(x0, x1, top, depth, tip_x=None):
    """A floating rock: flat mossy top, tapering to a point underneath."""
    tip_x = (x0 + x1) / 2 if tip_x is None else tip_x
    w = x1 - x0
    pts = [(x0, top), (x1, top), (x1 - w * 0.08, top + depth * 0.35),
           (x1 - w * 0.25, top + depth * 0.7), (tip_x, top + depth),
           (x0 + w * 0.22, top + depth * 0.65), (x0 + w * 0.06, top + depth * 0.3)]
    m.poly("solid", "moss", pts)

# The void: anything that falls this far is gone.
m.poly("hazard", "void", [(0, 37), (70, 37), (70, 40), (0, 40)], dps=500)

# --- Home islands and their docks ------------------------------------------------
island(2, 16, 24, 7, tip_x=8)
island(54, 68, 24, 7, tip_x=62)
m.slab(16, 19, 24)                    # left dock
m.slab(51, 54, 24)                    # right dock

# Stepping islands toward the centre.
island(19.5, 23.5, 25.5, 3)
island(46.5, 50.5, 25.5, 3)

# --- Central island and its dock tower ------------------------------------------------
island(26, 44, 22, 9, tip_x=35)
m.floor(27, 43, 16, hatch=(34.3, 35.7))   # lower deck
m.floor(29, 41, 10, hatch=(34.3, 35.7))   # roof deck
m.slab(23, 26, 19)                        # side step up to the lower deck (left)
m.slab(43.5, 47, 13)                      # side step up to the roof (right)

# --- High outcrops (thruster lifts) and the high routes to the tower ----------------
island(6, 14, 14, 3.5)
island(56, 64, 14, 3.5)
island(18, 22, 12.5, 2.5)
island(23, 26, 10.5, 2)
island(51.5, 55, 12.5, 2.5)
island(47.5, 50.5, 10.5, 2)

# --- Placements ----------------------------------------------------------------------
m.spawn(5, 24, 0)
m.spawn(12, 24, 0)
m.spawn(58, 24, 1)
m.spawn(65, 24, 1)
for x, y in [(30, 22), (40, 22), (10, 14), (60, 14), (31, 16), (39, 16)]:
    m.spawn(x, y)

m.pickup("health", 21.5, 25.5, respawn=25)
m.pickup("health", 48.5, 25.5, respawn=25)
m.pickup("ammo", 32.5, 22, respawn=15)
m.pickup("ammo", 24.5, 10.5, respawn=15)
m.pickup("weapon", 8, 14, weapon="rail_spike", respawn=40)
m.pickup("weapon", 62, 14, weapon="needle_gun", respawn=30)
m.pickup("weapon", 20, 12.5, weapon="thumper", respawn=30)
m.pickup("weapon", 53.3, 12.5, weapon="hellbarrel", respawn=40)

m.obj("thruster", 4.5, 24, height_tiles=11)
m.obj("thruster", 65.5, 24, height_tiles=11)
m.obj("thruster", 35, 22, height_tiles=12.8)
m.obj("powerup_spot", 32, 10, powerup="divine_shield")
m.obj("weapon_station", 17.5, 24)
m.obj("weapon_station", 52.5, 24)
m.obj("weapon_station", 28.5, 16)
m.obj("weapon_station", 49, 10.5)
m.obj("shield_station", 41.5, 16)
m.obj("shield_station", 38.5, 10, shield=100, min_delay_s=90, max_delay_s=120)

# --- Decor ----------------------------------------------------------------------------
for x in (16.3, 18.7, 51.3, 53.7):
    m.decor("girder", x, 27.5, w=0.25, h=3.5)
for x, y, h in [(8, 31, 4), (62, 31, 4), (35, 31, 5), (21.5, 28.5, 3), (48.5, 28.5, 3), (10, 17.5, 2.5), (60, 17.5, 2.5)]:
    m.decor("chain", x, y, w=1, h=h)
m.decor("tree", 9, 24, w=2.6, h=4.2, color="#4f9a4a")
m.decor("tree", 59.5, 24, w=2.6, h=4.2, color="#4f9a4a")
m.decor("tree", 37.5, 22, w=2.2, h=3.6, color="#4f9a4a")
m.decor("wall", 27.3, 22, w=15.4, h=5.4, color="#4a5466")   # lower deck interior
m.decor("window", 30, 20.5, w=1.6, h=1.2, color="#bfe6ff")
m.decor("window", 38.4, 20.5, w=1.6, h=1.2, color="#bfe6ff")
m.decor("banner", 29.3, 10, w=1.4, h=2.2, color="#2f6db5")
m.decor("banner", 39.4, 10, w=1.4, h=2.2, color="#2f6db5")
m.decor("lamp", 16.5, 24, h=1.8)
m.decor("lamp", 53.5, 24, h=1.8, flip=True)
m.decor("crystal", 12.2, 14, w=1.2, h=1.4, color="#a8e6ff")
m.decor("crystal", 56.8, 14, w=1.2, h=1.4, color="#a8e6ff")

problems = m.headroom_report()
for p in problems:
    print("HEADROOM:", p)
m.write(sys.argv[1])
print("wrote", sys.argv[1], "problems:", len(problems))
