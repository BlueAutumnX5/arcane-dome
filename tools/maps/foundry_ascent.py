"""Foundry Ascent: an original steel-mill arena.

Left: a four-storey steel tower. Climb it by staggered side ledges, or ride
the thruster shaft that flies straight up through hatches in every floor.
Centre: a lava pit crossed by stepping plates, a long bridge above it, and a
high perch (the powerup) reached by two short climbs.
Right: a rock hill and a stack of catwalks, with a second thruster up to a
sniper nest, linked back to the centre by a high walkway.
"""
import sys
from builder import Map

m = Map(
    "foundry_ascent", "Foundry Ascent",
    "A steel mill built around a pit of molten slag. Climb the old smelter tower, or ride its thruster shaft straight to the roof.",
    (64, 36),
    {"sky_top": "#150f12", "sky_bottom": "#5c2f1c", "far": "#2c1d1a", "skyline": "girders",
     "sun": [0.78, 0.2], "sun_color": "#ff8a3c"},
    players=(4, 12),
)
# Four corners: Blue tower base, Red hill, Green tower roof, Gold sniper nest.
# With 3 teams the corners would put two teams on the left, so Green moves
# to the steps beside the centre perch: a triangle.
m.d["team_counts"] = [2, 3, 4]

# --- Ground and the lava pit -------------------------------------------------
# The pit has steep walls: walk off the rim and you are in the slag. It is
# shallow (2 tiles), so one quick jump gets you back out.
m.poly("solid", "rock", [(0, 32), (24.8, 32), (24.8, 34), (39.2, 34), (39.2, 36), (0, 36)])
m.poly("hazard", "lava", [(24.8, 33.2), (39.2, 33.2), (39.2, 34), (24.8, 34)], dps=60)
m.poly("solid", "rock", [(39.2, 36), (39.2, 32), (43, 32), (51, 28), (57, 28),
                         (60, 30), (64, 30), (64, 36)])
# Stepping plates over the lava.
m.slab(27.5, 30, 30.5)
m.slab(34, 36.5, 30.5)

# --- Left tower: four floors with a hatch shaft at x 8.9..11.1 ----------------
SHAFT = (8.9, 11.1)
m.floor(4, 15, 26, hatch=SHAFT)      # F1
m.floor(5, 18, 20, hatch=SHAFT)      # F2
m.floor(4, 17, 14, hatch=SHAFT)      # F3
m.floor(4, 14, 8, hatch=SHAFT)       # roof
# Staggered side ledges: ground -> F1 -> F2 -> F3 -> roof, 3 tiles each.
# Each climb has a 2 tile wide column of open air beside the floor above,
# so nobody has to squeeze past a corner.
m.slab(17, 20.5, 29)                 # L1 (right side)
m.slab(0, 2, 23)                     # L2 (left wall)
m.slab(20, 23, 17)                   # L3 (right side)
m.slab(0, 2, 11)                     # L4 (left wall)

# --- Centre: bridge and the climb to the perch ---------------------------------
m.slab(24, 40, 18)                   # bridge over the pit
m.slab(25, 27.5, 14.5)               # step up, left
m.slab(28, 29.5, 11.5)
m.slab(36.5, 39, 14.5)               # step up, right
m.slab(34.5, 36, 11.5)
m.slab(30.5, 33.5, 8.5)              # the perch

# --- Right: catwalk stack, nest and the high walkway ---------------------------
# The zigzag A -> B -> C leaves 2 tiles of open air beside B at each step,
# so every climb rises clear of the catwalk above.
m.slab(47, 53, 24.5)                 # catwalk A (over the hill top)
m.floor(55, 62, 21)                  # catwalk B (thruster at x=60)
m.slab(46, 53, 17.5)                 # catwalk C
m.slab(41, 44, 18)                   # link to the bridge
m.floor(56, 63, 12, hatch=(58.9, 61.1))  # sniper nest
m.slab(41, 53, 11.5)                 # high walkway back to the centre

# --- Placements ------------------------------------------------------------
for x, y in [(6, 32), (12.5, 26)]:
    m.spawn(x, y, 0)
for x, y in [(62, 30), (57.5, 21)]:
    m.spawn(x, y, 1)
for x, y in [(6, 8), (12.5, 8)]:            # Green: the tower roof
    m.spawn(x, y, 2, by_teams={"3": None})
for x, y in [(57.5, 12), (61.5, 12)]:       # Gold: the sniper nest
    m.spawn(x, y, 3)
for x, y in [(35.2, 11.5), (37.7, 14.5)]:   # Green with 3 teams: the right-hand steps
    m.spawn(x, y, by_teams={"3": 2})
for x, y in [(20, 32), (45, 31), (32, 18), (50, 24.5), (7, 20), (48, 17.5)]:
    m.spawn(x, y)

m.pickup("health", 1, 23, respawn=25)
m.pickup("health", 28.7, 30.5, respawn=25)
m.pickup("ammo", 62.5, 30, respawn=15)
m.pickup("ammo", 12, 8, respawn=15)
m.pickup("weapon", 26, 18, weapon="thumper", respawn=30)
m.pickup("weapon", 6, 14, weapon="needle_gun", respawn=25)
m.pickup("weapon", 52, 24.5, weapon="arc_rifle", respawn=25)
m.pickup("weapon", 57.5, 12, weapon="rail_spike", respawn=40)

m.obj("thruster", 10, 32, height_tiles=24.5)
m.obj("thruster", 60, 21, height_tiles=9.8)
m.obj("powerup_spot", 32, 8.5, powerup="critical_strike")
m.obj("weapon_station", 14, 26)
m.obj("weapon_station", 42, 32)
m.obj("weapon_station", 21.5, 17)
m.obj("weapon_station", 50, 11.5)
m.obj("shield_station", 6, 8)
m.obj("shield_station", 42.5, 18)
m.obj("shield_station", 62, 12, shield=100, min_delay_s=90, max_delay_s=120)

# --- Decor -------------------------------------------------------------------
m.decor("wall", 2.5, 32, w=16, h=24.5, color="#241c1e")      # tower interior back wall
for wx in (5, 12):
    for wy in (30, 24, 18, 12):
        m.decor("window", wx, wy, w=1.6, h=1.2, color="#ff9a4a")
m.decor("girder", 2.2, 32, w=0.6, h=24.5)
m.decor("girder", 17.6, 32, w=0.6, h=24.5)
for cx in (26, 30, 34, 38):
    m.decor("chain", cx, 18.6, w=1, h=4.5)
m.decor("lamp", 24.6, 18, h=2.2)
m.decor("lamp", 39.4, 18, h=2.2, flip=True)
m.decor("tank", 52, 28, w=2.2, h=3.4)
m.decor("pipe", 41, 32, w=23, h=0.45, color="#5b5e66")
m.decor("pipe", 63.3, 21, w=0.45, h=9)
m.decor("banner", 31.2, 12.5, w=1.6, h=2.6, color="#9a3a2a")
m.decor("girder", 46, 32, w=0.5, h=14.5)
m.decor("girder", 52.6, 32, w=0.5, h=14.5)
m.decor("lamp", 62.5, 12, h=1.8, flip=True)

# --- Objectives ----------------------------------------------------------------
# Zones, left to right: the tower's middle floor, the long bridge over the
# pit, the ledge below the right catwalks. Territories runs Blue's tower to
# Red's hill along them.
m.zone(12, 20, w=8).zone(32, 18, w=10).zone(50, 24.5, w=6)
# Flags at each team's corner. Green's moves to the centre perch with 3 teams.
m.flag(3.5, 32, team=0)
m.flag(63, 30, team=1)
m.flag(7.5, 8, team=2, by_teams={"3": None})
m.flag(35.2, 11.5, by_teams={"3": 2})
m.flag(62.5, 12, team=3)

problems = m.headroom_report()
for p in problems:
    print("HEADROOM:", p)
m.write(sys.argv[1])
print("wrote", sys.argv[1], "problems:", len(problems))
