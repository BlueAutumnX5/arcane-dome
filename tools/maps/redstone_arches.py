"""Redstone Arches: an original desert canyon arena.

Two sandstone mesas face each other across a dune floor. Wooden scaffold
ledges climb their faces; wooden bridges reach out from the mesa tops to a
great natural arch, whose crest is the highest walkway (and holds the main
powerup). Under the arch an oasis pool sits in a dip, with floating boulders
for mid-height fights.
"""
import sys
from builder import Map

m = Map(
    "redstone_arches", "Redstone Arches",
    "A dry canyon under a giant stone arch. Fight across the dunes, climb the mesas, and hold the arch for the best view in the valley.",
    (72, 34),
    {"sky_top": "#2b3a67", "sky_bottom": "#f0a35e", "far": "#8a4b38", "skyline": "mountains",
     "sun": [0.76, 0.6], "sun_color": "#ffd27a"},
    players=(4, 12),
)

# --- Dune floor with the oasis dip ---------------------------------------------
m.poly("solid", "sand", [
    (0, 30), (11, 30), (14, 29.4), (18, 30), (26, 30), (28.5, 31.6), (30.5, 32.5),
    (41.5, 32.5), (43.5, 31.6), (46, 30), (54, 30), (58, 29.4), (61, 30), (72, 30),
    (72, 34), (0, 34)])
m.poly("water", "water", [(27.6, 31.0), (44.4, 31.0), (43.5, 31.6), (41.5, 32.5),
                          (30.5, 32.5), (28.5, 31.6)])

# --- Mesas -----------------------------------------------------------------------
m.poly("solid", "rock", [(0, 17), (9, 17), (10.6, 17.6), (11, 19), (11, 30), (0, 30)])
m.poly("solid", "rock", [(72, 16), (63, 16), (61.4, 16.6), (61, 18), (61, 30), (72, 30)])

# Wooden scaffold ledges up each mesa face (3.5 tiles apart, staggered).
m.slab(11, 14.5, 26.5, material="wood")
m.slab(13, 16.5, 23, material="wood")
m.slab(11, 14, 19.5, material="wood")
m.slab(57.5, 61, 26.5, material="wood")
m.slab(55.5, 59, 23, material="wood")
m.slab(58, 61, 19.5, material="wood")

# Wooden bridges from the mesa tops toward the arch.
m.slab(12.5, 18.5, 14, material="wood")
m.slab(53.5, 59.5, 13, material="wood")

# --- The arch -------------------------------------------------------------------
# In a side view, legs that reach the ground would wall off the space beneath.
# These legs are eroded away near the bottom, ending ~5 tiles up, so the
# canyon floor runs freely under the whole arch.
m.poly("solid", "rock", [
    (20.4, 24.5), (20.5, 22), (22, 13), (28, 11), (36, 10.5), (44, 11), (50, 13),
    (51.5, 22), (51.6, 24.5), (50.4, 25.6), (49, 24.4), (48, 22), (46.5, 14.5),
    (40, 13.2), (32, 13.2), (25.5, 14.5), (24, 22), (23, 24.4), (21.6, 25.6)])

# Floating boulders under the arch.
m.poly("solid", "rock", [(29, 26.5), (33, 26.5), (32.4, 27.6), (30, 28), (29.3, 27.4)])
m.poly("solid", "rock", [(37, 23), (41, 23), (40.6, 24.1), (38.6, 24.6), (37.3, 24)])
m.poly("solid", "rock", [(30, 19.5), (34, 19.5), (33.5, 20.6), (31.6, 21.1), (30.3, 20.5)])

# --- Placements ------------------------------------------------------------------
m.spawn(14.5, 29.5, 0)
m.spawn(4, 17, 0)
m.spawn(57.5, 29.5, 1)
m.spawn(68, 16, 1)
for x, y in [(24.5, 30), (47.6, 30), (16, 14), (56, 13), (30, 10.9), (12.5, 26.5)]:
    m.spawn(x, y)

m.pickup("health", 18, 30, respawn=25)
m.pickup("health", 39, 23, respawn=25)
m.pickup("ammo", 13.5, 23, respawn=15)
m.pickup("ammo", 59.5, 26.5, respawn=15)
m.pickup("weapon", 32.5, 26.5, weapon="scattergun", respawn=25)
m.pickup("weapon", 2, 17, weapon="sunbeam", respawn=35)
m.pickup("weapon", 70, 16, weapon="leechfang", respawn=25)
m.pickup("weapon", 42, 10.8, weapon="hellbarrel", respawn=40)
m.pickup("weapon", 15.5, 23, weapon="needle_gun", respawn=25)

m.obj("powerup_spot", 36, 10.5, powerup="rapid_fire")
m.obj("powerup_spot", 32, 19.5, powerup="divine_shield")
m.obj("weapon_station", 7, 17)
m.obj("weapon_station", 66, 16)
m.obj("weapon_station", 26, 30)
m.obj("weapon_station", 46.3, 30)
m.obj("shield_station", 14.5, 14)
m.obj("shield_station", 57.5, 13)

# --- Decor -----------------------------------------------------------------------
for x, h in [(3, 2.6), (8.2, 1.8), (65, 2.4), (70.5, 1.6)]:
    m.decor("cactus", x - 0.6, 16.95 if x < 36 else 15.95, w=1.2, h=h)
for x, h in [(15.5, 2.0), (55.5, 2.2)]:
    m.decor("cactus", x, 29.95, w=1.2, h=h)
m.decor("tree", 27, 30.4, w=2.4, h=4.2, color="#4f8a3c")
m.decor("tree", 43.5, 30.4, w=2.4, h=3.8, color="#4f8a3c")
m.decor("fence", 12.6, 14, w=5.8, h=0.9, color="#7a5532")
m.decor("fence", 53.6, 13, w=5.8, h=0.9, color="#7a5532")
for x in (11.2, 14.2):
    m.decor("girder", x, 30, w=0.25, h=3.5, color="#6b4a2c")
for x in (58.0, 60.6):
    m.decor("girder", x, 30, w=0.25, h=3.5, color="#6b4a2c")
m.decor("banner", 35.3, 10.5, w=1.4, h=2.4, color="#b8452e", front=False)
m.decor("lamp", 18.2, 14, h=1.8)
m.decor("lamp", 53.8, 13, h=1.8, flip=True)

problems = m.headroom_report()
for p in problems:
    print("HEADROOM:", p)
m.write(sys.argv[1])
print("wrote", sys.argv[1], "problems:", len(problems))
