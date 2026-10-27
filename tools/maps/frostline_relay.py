"""Frostline Relay: an original icy mountain arena built for 3 and 4 teams.

A diamond of bases: Blue and Red hold the two cliff tops, Green the relay deck
high in the middle, Yellow the frozen valley basin below. Walkable slopes join
the basin to both cliffs, and 45 degree bridges climb from the side ledges to
the relay deck. A thruster in the basin flies up the relay shaft, through
hatches in the ice island, the deck and the tower roof, to the powerup on top.
Mirror symmetric, so Blue against Red is fair too.
"""
import sys
from builder import Map

m = Map(
    "frostline_relay", "Frostline Relay",
    "A relay station high in frozen mountains. Hold a cliff, the valley or the relay deck, and ride the shaft to the tower roof.",
    (72, 44),
    {"sky_top": "#162540", "sky_bottom": "#a8c4de", "far": "#5d7591", "skyline": "mountains",
     "sun": [0.2, 0.22], "sun_color": "#eef6ff"},
    players=(4, 16),
)
m.d["team_counts"] = [2, 3, 4]

SHAFT = (35, 37)

# --- The valley: two cliffs joined to the basin by walkable slopes ----------------
m.poly("solid", "rock", [(0, 28), (16, 28), (26, 40), (46, 40), (56, 28), (72, 28), (72, 44), (0, 44)])

# --- Ice island floating over the basin (hatch in the middle) -------------------------
m.poly("solid", "ice", [(31, 31), (35, 31), (35, 32.4), (33.4, 33.4), (31.6, 32.6)])
m.poly("solid", "ice", [(37, 31), (41, 31), (40.4, 32.6), (38.6, 33.4), (37, 32.4)])
m.poly("hatch", "metal", [(35, 31), (37, 31), (37, 31.25), (35, 31.25)])

# --- Side climbs: a step, a ledge, then a 45 degree bridge up to the deck ------------
# Each climb leaves 2 tiles of open air beside the floor above.
m.slab(11, 14, 25, material="wood")                  # step, left
m.slab(16, 23, 22)                                   # ledge, left
m.poly("solid", "metal", [(23, 22), (30, 15), (30, 15.6), (23, 22.6)])   # bridge, left
m.slab(58, 61, 25, material="wood")                  # step, right
m.slab(49, 56, 22)                                   # ledge, right
m.poly("solid", "metal", [(49, 22), (42, 15), (42, 15.6), (49, 22.6)])   # bridge, right

# --- The relay: deck and tower roof, both with the shaft hatch ------------------------
m.floor(30, 42, 15, hatch=SHAFT)                     # relay deck
m.floor(33, 39, 8, hatch=SHAFT)                      # tower roof

# --- Spawns ------------------------------------------------------------------------------
for x in (4, 9):
    m.spawn(x, 28, 0, by_teams={"3": None})          # Blue: left cliff
for x in (68, 63):
    m.spawn(x, 28, 1, by_teams={"3": None})          # Red: right cliff
for x in (33.6, 38.4):
    m.spawn(x, 15, 2)                                # Green: relay deck, by the tower
for x in (28.5, 43.5):
    m.spawn(x, 40, 3, by_teams={"3": None})          # Yellow: the basin
# With 3 teams the base in the middle would be every enemy's nearest target,
# so Blue and Red move down to the two ends of the valley instead, close to
# each other, and Green keeps the high deck, the farthest from both.
for x, y, owner in [(28.5, 40, 0), (31, 40, 0), (43.5, 40, 1), (41, 40, 1)]:
    m.spawn(x, y, by_teams={"3": owner})
for x, y in [(19, 22), (53, 22), (12.5, 25), (59.5, 25), (32.5, 31), (39.5, 31)]:
    m.spawn(x, y)

# --- Pickups ------------------------------------------------------------------------------
m.pickup("health", 12.5, 25, respawn=25)
m.pickup("health", 59.5, 25, respawn=25)
m.pickup("health", 36, 31, respawn=25)
m.pickup("ammo", 21.5, 22, respawn=15)
m.pickup("ammo", 50.5, 22, respawn=15)
m.pickup("ammo", 36, 40, respawn=15)
m.pickup("weapon", 17.5, 22, weapon="needle_gun", respawn=25)
m.pickup("weapon", 54.5, 22, weapon="needle_gun", respawn=25)
m.pickup("weapon", 41, 15, weapon="thumper", respawn=30)
m.pickup("weapon", 32, 40, weapon="scattergun", respawn=25)
m.pickup("weapon", 40, 40, weapon="hail_rifle", respawn=25)
m.pickup("weapon", 37.5, 8, weapon="rail_spike", respawn=40)

# --- Gadgets ---------------------------------------------------------------------------------
m.obj("thruster", 36, 40, height_tiles=32.5)         # the relay shaft, up to the roof
m.obj("thruster", 36, 15, height_tiles=7.6)          # deck to roof: Green's home lift
m.obj("powerup_spot", 34.5, 8, powerup="divine_shield")
m.obj("weapon_station", 6.5, 28)
m.obj("weapon_station", 65.5, 28)
m.obj("weapon_station", 31, 15)
m.obj("weapon_station", 29.5, 40)
m.obj("shield_station", 22, 22)
m.obj("shield_station", 50, 22)
m.obj("shield_station", 39.5, 31, shield=100, min_delay_s=90, max_delay_s=120)

# --- Decor --------------------------------------------------------------------------------------
m.decor("wall", 33.4, 15, w=5.2, h=7, color="#3a4a5e")          # the relay tower body
for wy in (13.2, 10.6):
    m.decor("window", 34.3, wy, w=1.2, h=1.0, color="#bfe6ff")
    m.decor("window", 36.5, wy, w=1.2, h=1.0, color="#bfe6ff")
m.decor("girder", 35.75, 8, w=0.5, h=6.5)                       # the antenna mast
m.decor("girder", 33.2, 8, w=0.3, h=3)
m.decor("girder", 38.5, 8, w=0.3, h=3)
m.decor("lamp", 30.4, 15, h=1.6)
m.decor("lamp", 41.6, 15, h=1.6, flip=True)
m.decor("pipe", 30, 16.2, w=12, h=0.4, color="#5b6878")
for x in (24.5, 27.5, 44.5, 47.5):                              # bridge supports
    m.decor("chain", x, 19.4 if x in (24.5, 47.5) else 16.6, w=1, h=3)
m.decor("tank", 1.2, 28, w=2.2, h=3.0, color="#6d7f94")         # cliff stations
m.decor("tank", 68.6, 28, w=2.2, h=3.0, color="#6d7f94")
m.decor("fence", 0, 28, w=16, h=0.8, color="#55657a")
m.decor("fence", 56, 28, w=16, h=0.8, color="#55657a")
for x, h in [(14.5, 1.6), (26.5, 2.2), (44.5, 1.8), (57, 1.5), (35.8, 1.2)]:
    m.decor("crystal", x, 40 if 20 < x < 50 and x != 35.8 else (28 if x < 20 or x > 50 else 31), w=1.2, h=h,
            color="#bfe9ff")
m.decor("girder", 16.3, 28, w=0.4, h=5.4)                       # ledge struts
m.decor("girder", 55.3, 28, w=0.4, h=5.4)

# --- Objectives ----------------------------------------------------------------
# Zones: the two terraces and the ice island between them.
m.zone(20, 22, w=6).zone(36, 31, w=8).zone(52, 22, w=6)
# Flags: Blue and Red on their plateaus (in the valley with 3 teams), Green on
# the tower, Gold in the valley.
m.flag(2.5, 28, team=0, by_teams={"3": None})
m.flag(69.5, 28, team=1, by_teams={"3": None})
m.flag(27.5, 40, by_teams={"3": 0})
m.flag(44.5, 40, by_teams={"3": 1})
m.flag(31.5, 15, team=2)
m.flag(33.5, 40, team=3, by_teams={"3": None})

problems = m.headroom_report()
for p in problems:
    print("HEADROOM:", p)
m.write(sys.argv[1])
print("wrote", sys.argv[1], "problems:", len(problems))
