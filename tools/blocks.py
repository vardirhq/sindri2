"""Generates Causeway's block recipes.

Written down rather than hand-edited because a palette is a table: every block
is the same handful of decisions -- which strata, which cap, how coarse the
grain -- and twenty of them authored by hand drift apart on the third one.

Re-run this, then re-bake:

    python3 tools/blocks.py
    cd tools/isometric-baker
    for r in ../../game/recipes/blocks-*.isobake.json; do node src/cli.ts "$r" --out ../../game/assets; done
"""
import json
import pathlib
import random

# Every material any block draws from, in one place so two blocks meant to be
# the same earth actually are.
MATERIALS = {
    "earth-top":  ("#94643f", 0.46, 0.05,  3, (-0.19, -0.1, 0.0, 0.08)),
    "earth":      ("#7d5132", 0.46, 0.055, 23, (-0.18, -0.09, 0.0, 0.07)),
    "earth-deep": ("#623c1f", 0.40, 0.065, 5, (-0.15, -0.08, 0.0, 0.06)),
    "grass":      ("#5f9741", 0.22, 0.036, 11, (-0.2, -0.1, 0.0, 0.09)),
    "grass-deep": ("#4f8037", 0.22, 0.04,  17, None),
    "stone":      ("#6f6a62", 0.28, 0.045, 7, None),
    "stone-pale": ("#8b867c", 0.30, 0.042, 29, (-0.17, -0.09, 0.0, 0.08)),
    "plank":      ("#a8763f", 0.34, 0.05,  41, (-0.18, -0.09, 0.0, 0.08)),
    "plank-dark": ("#8a5c2d", 0.30, 0.055, 43, None),
    "water":      ("#3f76a8", 0.24, 0.07,  47, (-0.16, -0.08, 0.0, 0.1)),
    "water-deep": ("#2f5d8a", 0.20, 0.08,  53, None),
    # The biomes.
    "sand":       ("#d8c48f", 0.30, 0.045, 101, (-0.16, -0.08, 0.0, 0.07)),
    "sand-deep":  ("#bda874", 0.26, 0.05,  103, None),
    "snow":       ("#e8eef5", 0.18, 0.04,  107, (-0.12, -0.06, 0.0, 0.05)),
    "snow-deep":  ("#c6d3e0", 0.20, 0.05,  109, None),
    "ice":        ("#9fc6dd", 0.20, 0.06,  113, (-0.14, -0.07, 0.0, 0.1)),
    "rock":       ("#585c63", 0.32, 0.05,  127, (-0.18, -0.09, 0.0, 0.08)),
    "rock-dark":  ("#42464c", 0.30, 0.06,  131, None),
    "mud":        ("#4e4433", 0.38, 0.06,  137, (-0.17, -0.09, 0.0, 0.07)),
    "mud-wet":    ("#3b3527", 0.34, 0.07,  139, None),
    "moss":       ("#4f6b34", 0.26, 0.04,  149, (-0.2, -0.1, 0.0, 0.08)),
    "gravel":     ("#7c7468", 0.40, 0.035, 151, (-0.2, -0.1, 0.0, 0.08)),
    "clay":       ("#a9705a", 0.30, 0.05,  157, (-0.16, -0.08, 0.0, 0.07)),
}


def materials(*names):
    out = {}
    for name in names:
        colour, strength, size, seed, ramp = MATERIALS[name]
        entry = {"colour": colour, "grain": {"strength": strength, "size": size, "seed": seed}}
        if ramp:
            entry["ramp"] = {"lightness": list(ramp)}
        out[name] = entry
    return out


def box(material, position, size):
    """One box, clamped to stay inside its own cell.

    A part sitting proud of the cube grows the canvas the baker builds, which
    un-squares every frame on the page and leaves transparent rows inside each
    declared rect -- discarded, at run time, into a bright hairline along every
    edge where two blocks touch. Clamping here is what stops a recipe doing
    that by accident.
    """
    half = size[1] / 2.0
    lift = min(max(position[1], -0.5 + half), 0.5 - half)
    return {"type": "box", "material": material,
            "position": [position[0], round(lift, 4), position[2]],
            "size": [round(v, 4) for v in size]}


def strata(deep, middle, top, cap=None, cap_depth=0.16):
    """A block as courses, deepest first, with an optional cap on top."""
    parts = [
        box(deep,   [0.0, -0.34, 0.0], [1.0, 0.32, 1.0]),
        box(middle, [0.0, -0.03, 0.0], [1.0, 0.30, 1.0]),
    ]
    if cap is None:
        parts.append(box(top, [0.0, 0.29, 0.0], [1.0, 0.42, 1.0]))
    else:
        parts.append(box(top, [0.0, 0.21, 0.0], [1.0, 0.26, 1.0]))
        parts.append(box(cap, [0.0, 0.5 - cap_depth / 2.0, 0.0], [1.0, cap_depth, 1.0]))
    return parts


def speckled(parts, material, seed, count, low, high, names):
    """Scatter a few inclusions so a face is not one flat colour."""
    rng = random.Random(seed)
    for _ in range(count):
        side = rng.uniform(low, high)
        parts.append(box(material,
                         [rng.uniform(-0.34, 0.34), rng.uniform(-0.38, 0.30), 0.0],
                         [side, side, 1.0]))
    names.add(material)
    return parts


def turf(seed, patches):
    """Grass over earth, with a fringe hanging into the soil and a few scrapes.

    The fringe is what separates a grass block from a green slab: turf has a
    ragged edge where it meets the cut, and drawing it straight reads as
    painted-on.
    """
    rng = random.Random(seed)
    parts = strata("earth-deep", "earth", "earth-top", cap="grass", cap_depth=0.14)
    for _ in range(3):
        parts.append(box("grass",
                         [rng.uniform(-0.4, 0.4), 0.30, 0.0],
                         [rng.uniform(0.12, 0.2), 0.1, 1.0]))
    for _ in range(patches):
        side = rng.uniform(0.2, 0.34)
        parts.append(box("grass-deep",
                         [rng.uniform(-0.3, 0.3), 0.5 - 0.045, rng.uniform(-0.3, 0.3)],
                         [side, 0.09, side]))
    return {"kind": "primitives",
            "materials": materials("earth-deep", "earth", "earth-top", "grass", "grass-deep"),
            "parts": parts}


def plain(deep, middle, top, seed, inclusion=None, count=3):
    used = {deep, middle, top}
    parts = strata(deep, middle, top)
    if inclusion:
        speckled(parts, inclusion, seed, count, 0.1, 0.2, used)
    return {"kind": "primitives", "materials": materials(*used), "parts": parts}


def capped(deep, middle, top, cap, seed, cap_depth=0.18):
    used = {deep, middle, top, cap}
    parts = strata(deep, middle, top, cap=cap, cap_depth=cap_depth)
    rng = random.Random(seed)
    for _ in range(2):
        side = rng.uniform(0.16, 0.28)
        parts.append(box(cap, [rng.uniform(-0.3, 0.3), 0.5 - 0.05,
                               rng.uniform(-0.3, 0.3)], [side, 0.1, side]))
    return {"kind": "primitives", "materials": materials(*used), "parts": parts}


def liquid(top, bottom):
    return {"kind": "primitives",
            "materials": materials(top, bottom),
            "parts": [box(bottom, [0.0, -0.25, 0.0], [1.0, 0.5, 1.0]),
                      box(top, [0.0, 0.25, 0.0], [1.0, 0.5, 1.0])]}


def planks():
    parts = [box("plank-dark", [0.0, -0.275, 0.0], [1.0, 0.45, 1.0])]
    for index in range(4):
        parts.append(box("plank", [0.0, 0.225, -0.375 + index * 0.25], [1.0, 0.55, 0.215]))
    return {"kind": "primitives", "materials": materials("plank", "plank-dark"), "parts": parts}


# Every block the world is made of. The generator names these, so adding a
# biome means adding a row here and a rule there.
def palette():
    blocks = [
        ("ground-0", turf(11, 0)),
        ("ground-1", turf(12, 2)),
        ("ground-2", turf(13, 2)),
        ("ground-3", turf(14, 3)),
        ("earth-0", plain("earth-deep", "earth", "earth-top", 100, "stone", 3)),
        ("earth-1", plain("earth-deep", "earth", "earth-top", 101, "stone", 2)),
        ("stone-0", plain("stone", "stone", "stone", 200, "stone-pale", 4)),
        ("stone-1", plain("stone", "stone", "stone", 201, "stone-pale", 3)),
        ("rock-0", plain("rock-dark", "rock", "rock", 300, "stone-pale", 3)),
        ("rock-1", plain("rock-dark", "rock", "rock", 301, "rock-dark", 4)),
        ("sand-0", plain("sand-deep", "sand", "sand", 400, "sand-deep", 3)),
        ("sand-1", plain("sand-deep", "sand", "sand", 401, "sand-deep", 4)),
        ("snow-0", capped("stone", "rock", "snow-deep", "snow", 500)),
        ("snow-1", capped("rock-dark", "rock", "snow-deep", "snow", 501)),
        ("ice-0", liquid("ice", "water-deep")),
        ("mud-0", plain("mud-wet", "mud", "mud", 600, "moss", 3)),
        ("mud-1", plain("mud-wet", "mud", "mud", 601, "mud-wet", 3)),
        ("moss-0", capped("earth-deep", "mud", "earth", "moss", 700, cap_depth=0.16)),
        ("gravel-0", plain("rock-dark", "gravel", "gravel", 800, "stone-pale", 4)),
        ("clay-0", plain("clay", "clay", "clay", 900, "sand-deep", 3)),
        ("water", liquid("water", "water-deep")),
        ("plank", planks()),
    ]
    return [{"name": name, "model": model} for name, model in blocks]


def recipe(view, suffix, variants, ppu=48):
    return {
        "format_version": 1,
        "id": f"blocks-{suffix}",
        "texture": f"textures/blocks-{suffix}.png",
        "view": view,
        "pixels_per_unit": ppu,
        "directions": 1,
        "footprint": {"width": 1, "height": 1},
        "render": {"supersample": 1, "padding": 0, "palette_snap": True,
                   "alpha_cutoff": 128, "outline": {"enabled": False}},
        "variants": variants,
    }


def main():
    variants = palette()
    for view, suffix in (("top-down", "top"), ("side", "side")):
        path = pathlib.Path(f"game/recipes/blocks-{suffix}.isobake.json")
        path.write_text(json.dumps(recipe(view, suffix, variants), indent=2) + "\n")
    # The tile set as well as the art. It was written here and then left
    # uncalled, so the file this generates was in fact maintained by hand and
    # the comment below claiming otherwise was aspirational. A block that
    # exists in the palette and is unnameable in the tile set is exactly what
    # that was meant to prevent.
    document = pathlib.Path("game/assets/causeway.tileset.json")
    document.write_text(json.dumps(tileset(), indent=2) + "\n")
    print(f"wrote {len(variants)} blocks and {len(tileset()['tiles'])} tiles")


# What the world is made of, as the game names it: a tile ID, the frames its
# looks are drawn from, and the flags that say what it does to a walker.
#
# Generated from the same table the art is, so a block cannot exist in the
# palette and be unnameable by the generator, or be named and not exist.
TILES = {
    "ground": {"looks": ["ground-0", "ground-1", "ground-2", "ground-3"], "buried": "earth-0"},
    "earth":  {"looks": ["earth-0", "earth-1"]},
    "stone":  {"looks": ["stone-0", "stone-1"]},
    "rock":   {"looks": ["rock-0", "rock-1"]},
    "sand":   {"looks": ["sand-0", "sand-1"]},
    "snow":   {"looks": ["snow-0", "snow-1"], "buried": "rock-0"},
    "ice":    {"looks": ["ice-0"]},
    "mud":    {"looks": ["mud-0", "mud-1"]},
    "moss":   {"looks": ["moss-0"], "buried": "mud-1"},
    "gravel": {"looks": ["gravel-0"]},
    "clay":   {"looks": ["clay-0"]},
    "plank":  {"looks": ["plank"]},
    # Supports without being walkable: something floats on a pond and falls
    # through a hole, and those are not the same place.
    "water":  {"looks": ["water"], "walkable": False},
}

TOP = "textures/blocks-top.png"
SIDE = "textures/blocks-side.png"


def faces(look):
    one = [1.0, 1.0]
    return {
        "top":   {"sprite": f"{TOP}#{look}",  "size": one},
        "south": {"sprite": f"{SIDE}#{look}", "size": one},
        "east":  {"sprite": f"{SIDE}#{look}", "size": one},
    }


def tileset():
    tiles = {}
    for name, spec in TILES.items():
        looks = spec["looks"]
        buried = spec.get("buried")
        entry = {"faces": faces(looks[0])}
        if buried:
            # A block with something standing on it wears its buried look. A
            # course of turf halfway up a cliff still carrying a green fringe
            # reads as a stack of lawns.
            entry["covered"] = faces(buried)
        if len(looks) > 1:
            entry["variants"] = [
                dict({"faces": faces(look)}, **({"covered": faces(buried)} if buried else {}))
                for look in looks[1:]
            ]
        if spec.get("walkable") is False:
            entry["walkable"] = False
            entry["supports"] = True
        tiles[name] = entry
    # The walkway the player lays: half a cell, so a causeway sits below the
    # shore it leaves rather than reading as masonry.
    tiles["plank-slab"] = {
        "faces": {
            "top":   {"sprite": f"{TOP}#plank-slab", "size": [1.0, 1.0]},
            "south": {"sprite": "textures/slabs-side.png#plank-slab", "size": [1.0, 0.5]},
            "east":  {"sprite": "textures/slabs-side.png#plank-slab", "size": [1.0, 0.5]},
        },
        "height": 0.5,
    }
    # Shapes a height cannot describe. A height fills its cell's whole
    # footprint from the floor up, so everything authored with one is a slab of
    # some thickness -- there is no post, no rail, no kerb. A box says which
    # part of the cell the tile is, in fractions of it, as [across, up, into].
    #
    # None of these occlude or support: you see past a railing, and nothing
    # stands on top of one. Saying so is what keeps them from culling the faces
    # of whatever they are set against and from darkening its corners.
    for name, extent in {
        # A railing post: a fifth of the cell across and into, full height.
        "plank-post": {"min": [0.4, 0.0, 0.4], "max": [0.6, 1.0, 0.6]},
        # The rail between two posts: a slice held partway up, touching
        # neither the floor nor the ceiling of its cell. This is the shape a
        # height cannot express at all, because a height always starts at the
        # floor.
        "plank-rail": {"min": [0.0, 0.55, 0.42], "max": [1.0, 0.7, 0.58]},
        # A kerb along one edge of the cell: low, and only part of the way in.
        "stone-kerb": {"min": [0.0, 0.0, 0.0], "max": [1.0, 0.3, 0.3]},
    }.items():
        look = "plank" if name.startswith("plank") else "stone-0"
        tiles[name] = {
            "faces": faces(look),
            "extent": extent,
            "occludes": False,
            "supports": False,
            "walkable": False,
        }
    return {"format_version": 1, "tiles": tiles}


if __name__ == "__main__":
    main()
