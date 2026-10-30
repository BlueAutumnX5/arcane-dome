"""Small helpers for hand-designing Arcane Dome shape maps.

Coordinates are world tiles, y grows downward. Floors are given by their TOP
surface height (where feet stand); slabs extend downward by `thick`.
"""
import json

class Map:
    def __init__(self, id, name, description, size, theme, modes=("deathmatch", "elimination", "gun_game", "juggernaut", "king_of_the_hill", "domination", "territories", "capture_the_flag"), players=(4, 12)):
        self.d = {"id": id, "name": name, "description": description, "tile_size": 32,
                  "size": list(size), "theme": theme, "modes": list(modes),
                  "recommended_players": list(players), "shapes": [], "decor": [],
                  "spawns": [], "pickups": [], "objects": [], "zones": [], "flags": []}
        self.solids = []  # (x0, x1, top, bottom) boxes for headroom checks

    # --- terrain -----------------------------------------------------------
    def poly(self, kind, material, pts, dps=None):
        s = {"kind": kind, "material": material, "points": [[round(x, 3), round(y, 3)] for x, y in pts]}
        if dps is not None:
            s["dps"] = dps
        self.d["shapes"].append(s)
        return self

    def slab(self, x0, x1, top, thick=0.6, material="metal", kind="solid"):
        self.poly(kind, material, [(x0, top), (x1, top), (x1, top + thick), (x0, top + thick)])
        if kind == "solid":
            self.solids.append((x0, x1, top, top + thick))
        return self

    def floor(self, x0, x1, top, hatch=None, thick=0.6, material="metal"):
        """A solid floor, optionally with a hatch gap (x0, x1) you can fly up through."""
        if hatch is None:
            return self.slab(x0, x1, top, thick, material)
        h0, h1 = hatch
        self.slab(x0, h0, top, thick, material)
        self.slab(h0, h1, top, 0.25, material, kind="hatch")
        self.slab(h1, x1, top, thick, material)
        return self

    # --- placements --------------------------------------------------------
    def spawn(self, x, y, team=None, by_teams=None):
        """by_teams: other owners per team count, e.g. {"3": 2} or {"3": None}."""
        sp = {"x": x, "y": y, "team": team}
        if by_teams:
            sp["by_teams"] = by_teams
        self.d["spawns"].append(sp)
        return self

    def pickup(self, kind, x, y, weapon=None, respawn=20):
        p = {"type": kind, "x": x, "y": y, "respawn_s": respawn}
        if weapon:
            p["weapon_id"] = weapon
        self.d["pickups"].append(p)
        return self

    def obj(self, kind, x, y, **kw):
        o = {"type": kind, "x": x, "y": y}
        o.update(kw)
        self.d["objects"].append(o)
        return self

    def zone(self, x, y, w=6, h=4):
        """An area to hold or capture, standing on the floor at (x, y)."""
        self.d["zones"].append({"x": x, "y": y, "w": w, "h": h})
        return self

    def flag(self, x, y, team=None, by_teams=None):
        """A team's flag base; by_teams like spawns, e.g. {"3": None}."""
        f = {"x": x, "y": y, "team": team}
        if by_teams:
            f["by_teams"] = by_teams
        self.d["flags"].append(f)
        return self

    def decor(self, kind, x, y, w=1, h=1, **kw):
        o = {"type": kind, "x": x, "y": y, "w": w, "h": h}
        o.update(kw)
        self.d["decor"].append(o)
        return self

    # --- checks ------------------------------------------------------------
    def headroom_report(self):
        """Every placement should have at least 2 tiles of clear space above."""
        problems = []
        places = [("spawn", s) for s in self.d["spawns"]] + [("pickup", p) for p in self.d["pickups"]] + [("object", o) for o in self.d["objects"]]
        places += [("zone", z) for z in self.d["zones"]] + [("flag", f) for f in self.d["flags"]]
        for what, p in places:
            x, y = p["x"], p["y"]
            for (x0, x1, top, bottom) in self.solids:
                if x0 - 0.35 < x < x1 + 0.35 and y - 2.0 < bottom <= y - 0.01:
                    problems.append(f"{what} at ({x},{y}) has a slab overhead at {bottom}")
        return problems

    def write(self, path):
        lines = ["{"]
        keys = ["id", "name", "description", "tile_size", "size", "theme", "modes", "recommended_players"]
        keys += [k for k in ["team_counts"] if k in self.d]
        for k in keys:
            lines.append(f'  "{k}": {json.dumps(self.d[k])},')
        lists = ["shapes", "decor", "spawns", "pickups", "objects"]
        lists += [k for k in ["zones", "flags"] if self.d[k]]
        for k in lists:
            items = self.d[k]
            lines.append(f'  "{k}": [')
            for i, it in enumerate(items):
                lines.append("    " + json.dumps(it) + ("," if i + 1 < len(items) else ""))
            lines.append("  ]" + ("," if k != lists[-1] else ""))
        lines.append("}")
        with open(path, "w") as f:
            f.write("\n".join(lines) + "\n")
