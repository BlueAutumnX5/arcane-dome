"""Draw a shape map plus its bot navigation graph to a PNG.

usage: preview.py map.json nav.json out.png [scale]
"""
import json, sys
from PIL import Image, ImageDraw

mp = json.load(open(sys.argv[1]))
nav = json.load(open(sys.argv[2]))
out = sys.argv[3]
S = float(sys.argv[4]) if len(sys.argv) > 4 else 16.0
W, H = mp["size"]

def hexc(s):
    return tuple(int(s[i:i + 2], 16) for i in (1, 3, 5))

top, bot = hexc(mp["theme"]["sky_top"]), hexc(mp["theme"]["sky_bottom"])
im = Image.new("RGB", (int(W * S), int(H * S)))
d = ImageDraw.Draw(im, "RGBA")
for y in range(int(H * S)):
    t = y / (H * S)
    d.line([(0, y), (W * S, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bot)))

for dec in mp.get("decor", []):
    x, y, w, h = dec["x"], dec["y"], dec.get("w", 1), dec.get("h", 1)
    d.rectangle([x * S, (y - h) * S, (x + w) * S, y * S], outline=(255, 255, 255, 40))

COL = {"rock": (88, 94, 118), "metal": (118, 126, 142), "wood": (150, 110, 70), "ice": (200, 230, 245),
       "sand": (196, 160, 104), "moss": (80, 110, 70), "sludge": (110, 124, 64), "glass": (160, 210, 230),
       "lava": (230, 100, 40), "spikes": (200, 70, 70), "water": (60, 130, 220)}
for s in mp["shapes"]:
    pts = [(x * S, y * S) for x, y in s["points"]]
    c = COL.get(s.get("material", "rock"), (120, 120, 120))
    if s["kind"] == "hatch":
        d.polygon(pts, fill=c + (140,), outline=(255, 220, 120, 255))
    elif s["kind"] == "water":
        d.polygon(pts, fill=c + (120,))
    else:
        d.polygon(pts, fill=c + (255,))

nodes = nav["nodes"]
EC = {"w": (120, 255, 120, 70), "j": (255, 220, 80, 50), "f": (120, 180, 255, 70),
      "d": (200, 120, 255, 120), "t": (80, 255, 220, 160)}
for a, b, k in nav["edges"]:
    if k == "w":
        continue
    pa, pb = nodes[a], nodes[b]
    d.line([(pa[0] * S, (pa[1] - 0.2) * S), (pb[0] * S, (pb[1] - 0.2) * S)], fill=EC[k], width=1)
for x, y, core in nodes:
    r = 2
    d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], fill=(80, 255, 80) if core else (255, 60, 60))

for sp in mp["spawns"]:
    c = {0: (90, 160, 255), 1: (255, 100, 90)}.get(sp["team"], (240, 240, 240))
    d.rectangle([sp["x"] * S - 3, (sp["y"] - 1.8) * S, sp["x"] * S + 3, sp["y"] * S], fill=c)
for p in mp["pickups"]:
    c = {"health": (90, 230, 120), "ammo": (240, 200, 60), "weapon": (255, 150, 60)}[p["type"]]
    d.ellipse([p["x"] * S - 4, (p["y"] - 0.6) * S - 4, p["x"] * S + 4, (p["y"] - 0.6) * S + 4], fill=c)
for o in mp["objects"]:
    c = {"thruster": (80, 255, 220), "weapon_station": (255, 160, 60), "shield_station": (90, 220, 255),
         "powerup_spot": (190, 110, 255)}[o["type"]]
    d.rectangle([o["x"] * S - 5, (o["y"] - 0.5) * S - 5, o["x"] * S + 5, (o["y"] - 0.5) * S + 5], outline=c, width=2)
im.save(out)
print("saved", out)
