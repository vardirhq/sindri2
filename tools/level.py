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

WANDERER = (2, 7)

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
            "id": "wanderer",
            "name": "Wanderer",
            # The scale the bake reports, which is the frame's own size in
            # world units. The frame is twice the figure's height because the
            # canvas is symmetric about the model's origin -- the walker stands
            # on that origin, so the quad's centre is at its feet and it stands
            # on the ground rather than sinking to the waist.
            "transform_3d": transform(
                (WANDERER[0] * CELL, 0.0, WANDERER[1] * CELL), scale=(0.6, 2.95, 1.0)),
            "components": {
                "sindri.sprite": {"texture": "textures/wanderer.png#bob-0",
                                  "tint": [1.0, 1.0, 1.0, 1.0], "billboard": True},
                "sindri.animation.sprite": {
                    "clips": {"bob": {"frames": ["bob-0", "bob-1", "bob-2", "bob-3"],
                                      "looping": True, "seconds_per_frame": 0.16}},
                    "speed": 1.0,
                },
                # No cell: a walker's place is wherever walking left it, and
                # only its height is the ground's to answer.
                "sindri.grid.placement": {"grid": "floor"},
                # What makes it a thing the pathfinder knows about. Without it
                # the walker is scenery standing on the grid rather than
                # somebody moving across it.
                "sindri.grid.occupant": {"grid": "floor", "footprint": [[0, 0]]},
                "sindri.script": {"source": "scripts/wanderer.decay", "script": "Wanderer",
                                  "properties": {"step_seconds": 0.34}},
            },
        },
        {
            "id": "beacon",
            "name": "Beacon",
            # 66 by 174, so a square scale would squash it to a smudge.
            "transform_3d": transform((0.0, 0.0, 0.0), scale=(0.68, 1.79, 1.0)),
            "components": {
                "sindri.sprite": {"texture": "textures/beacon.png#south",
                                  "tint": [1.0, 1.0, 1.0, 1.0], "billboard": True},
                # Deliberately not an occupant. A goal that fills its own cell
                # is a goal nothing can reach: the pathfinder will not route
                # into an occupied square, so marking the beacon would make the
                # whole game unwinnable in a way nothing else would report.
                "sindri.grid.placement": {"grid": "floor", "cell": list(PILLAR)},
                "sindri.script": {"source": "scripts/beacon.decay", "script": "Beacon",
                                  "properties": {"lit_scale": 1.25}},
            },
        },
        {
            "id": "builder",
            "name": "Builder",
            "transform_3d": transform((0.0, 0.0, 0.0)),
            "components": {
                "sindri.script": {"source": "scripts/builder.decay", "script": "Builder",
                                  "properties": {"stock": 12.0}},
            },
        },
        {
            "id": "music",
            "name": "Music",
            "transform_3d": transform((0.0, 0.0, 0.0)),
            "components": {
                "sindri.audio.source": {
                    "autoplay": True, "clip": "audio/background.wav",
                    "looping": True, "volume": 0.2,
                },
            },
        },
        {
            "id": "title",
            "name": "Title",
            "transform_3d": transform((0.0, 0.0, 2.0)),
            "components": {
                "sindri.ui.text": {
                    "anchor": "top_left", "color": [0.949, 0.722, 0.294, 1.0],
                    "font": "fonts/Inter.ttf", "font_size": 0.0611,
                    "layer": 101, "line_height": 0.0778, "text": "CAUSEWAY",
                },
            },
        },
        {
            "id": "kind",
            "name": "Kind",
            "transform_3d": transform((0.0, 0.0, 2.0)),
            "components": {
                "sindri.ui.text": {
                    "anchor": "top_left", "color": [0.875, 0.902, 0.949, 1.0],
                    "font": "fonts/Inter.ttf", "font_size": 0.0407,
                    "layer": 101, "line_height": 0.0519, "text": "planks",
                },
            },
        },
        {
            "id": "stock",
            "name": "Stock",
            "transform_3d": transform((0.0, 0.0, 2.0)),
            "components": {
                "sindri.ui.text": {
                    "anchor": "top_left", "color": [0.949, 0.722, 0.294, 1.0],
                    "font": "fonts/Inter.ttf", "font_size": 0.0407,
                    "layer": 101, "line_height": 0.0519, "text": "x {}",
                    "values": [12.0],
                },
            },
        },
        {
            "id": "hint",
            "name": "Hint",
            "transform_3d": transform((0.0, 0.0, 2.0)),
            "components": {
                "sindri.ui.text": {
                    "anchor": "bottom_left", "color": [0.604, 0.651, 0.733, 1.0],
                    "font": "fonts/Inter.ttf", "font_size": 0.0346,
                    "layer": 101, "line_height": 0.0481,
                    "text": "Click a block's side to build from it. Right-click to take one back. 1-2-3 to choose.",
                },
            },
        },
        {
            "id": "hud",
            "name": "Hud",
            "transform_3d": transform((0.0, 0.0, 0.0)),
            "components": {
                "sindri.script": {"source": "scripts/hud.decay", "script": "Hud"},
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
