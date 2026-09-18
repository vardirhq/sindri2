"""Generates Causeway's level.

Written down rather than hand-edited because the shape of a shore is a formula
and 400-odd cells of JSON is not something to keep correct by hand. Re-running
this is how the level changes.
"""
import json, math, pathlib, sys
sys.path.insert(0, '/tmp')
from look import look_at

COLUMNS, ROWS, CELL = 18, 16, 1.0
NEAR = (3.6, 7.5, 3.1, 5.2)   # centre column, centre row, half width, half depth
FAR = (13.6, 7.5, 3.1, 5.2)
PILLAR = (14, 7)
STONES = ((8, 5), (9, 11))

def island(column, row):
    """A shore, as an ellipse rather than a table.

    The rounded corners are the whole of it: a rectangle of grass in a square
    sea reads as a board somebody laid out, and nothing about a straight edge
    invites you to build off it.
    """
    for cx, cy, rx, ry in (NEAR, FAR):
        if ((column - cx) / rx) ** 2 + ((row - cy) / ry) ** 2 <= 1.0:
            return True
    return False

def cells():
    out = []
    for row in range(ROWS):
        for column in range(COLUMNS):
            if (column, row) in STONES:
                out.append({"position": [column, row, -1], "tile": "stone"})
                out.append({"position": [column, row, 0], "tile": "stone"})
            elif island(column, row):
                # Earth under grass, so a shore seen from the side is a cut
                # through soil rather than a green slab floating on the sea.
                out.append({"position": [column, row, -1], "tile": "earth"})
                out.append({"position": [column, row, 0], "tile": "ground"})
            else:
                out.append({"position": [column, row, -1], "tile": "water"})
    # The beacon stands three courses above the far shore, so crossing the
    # water is only half of it: a walker may step up one block at a time, and
    # the way up is something the player builds.
    for level in (1, 2, 3):
        out.append({"position": [PILLAR[0], PILLAR[1], level], "tile": "stone"})
    out.sort(key=lambda cell: (cell["position"][2], cell["position"][1], cell["position"][0]))
    return out

def transform(position, rotation=None, scale=(1.0, 1.0, 1.0)):
    return {
        "position": [float(v) for v in position],
        "rotation": [float(v) for v in (rotation or (0.0, 0.0, 0.0, 1.0))],
        "scale": [float(v) for v in scale],
    }

def camera():
    centre = [COLUMNS * CELL / 2.0, 0.0, ROWS * CELL / 2.0]
    distance, pitch, yaw = 24.0, math.radians(33.0), math.radians(45.0)
    eye = [
        round(centre[0] + distance * math.cos(pitch) * math.sin(yaw), 4),
        round(centre[1] + distance * math.sin(pitch), 4),
        round(centre[2] + distance * math.cos(pitch) * math.cos(yaw), 4),
    ]
    return eye, look_at(eye, centre)

def entities():
    eye, rotation = camera()
    return [
        {
            "id": "floor",
            "name": "Floor",
            "transform_3d": transform((0.0, 0.0, 0.0)),
            "components": {
                "sindri.tile_grid": {
                    "columns": COLUMNS, "rows": ROWS,
                    "cell_size": [CELL, CELL], "cell_height": CELL,
                    "projection": "isometric", "space": "solid",
                },
                "sindri.tile_volume": {
                    "tileset": "causeway.tileset.json",
                    "variant_seed": 1207,
                    "cells": cells(),
                },
                # One block up and no more, which is what makes the pillar a
                # thing to build a way up rather than a thing to walk up.
                "sindri.grid.navigation": {"max_step": 1.0},
            },
        },
        {
            "id": "world-camera",
            "name": "World Camera",
            "transform_3d": transform(eye, rotation),
            "components": {
                "sindri.camera": {
                    "projection": "orthographic", "vertical_size": 14.0,
                    "near": 0.1, "far": 200.0,
                },
            },
        },
    ]

if __name__ == "__main__":
    document = {"format_version": 9, "metadata": {"name": "Causeway"}, "entities": entities()}
    pathlib.Path('game/assets/causeway.scene.json').write_text(
        json.dumps(document, indent=2) + "\n")
    print("wrote the level")
