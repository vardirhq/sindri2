"""Generates Causeway's level.

Written down rather than hand-edited because the shape of a shore is a formula
and 400-odd cells of JSON is not something to keep correct by hand. Re-running
this is how the level changes.
"""
import json, math, pathlib, sys
sys.path.insert(0, '/tmp')
from look import look_at

COLUMNS, ROWS, CELL = 160, 160, 1.0
# Where the game starts, in cells. Near the middle, because the world around
# it is generated and every direction is somewhere.
WANDERER = (78, 84)
BEACON = (82, 76)


def transform(position, rotation=None, scale=(1.0, 1.0, 1.0)):
    return {
        "position": [float(v) for v in position],
        "rotation": [float(v) for v in (rotation or (0.0, 0.0, 0.0, 1.0))],
        "scale": [float(v) for v in scale],
    }

def camera():
    """Framed on the walker rather than on the whole world.

    A board small enough to see at once can be framed once; a world this size
    cannot, so the camera sits a fixed distance from where the game starts and
    a script keeps that offset as the walker moves.
    """
    centre = [WANDERER[0] * CELL, 2.0, WANDERER[1] * CELL]
    distance, pitch, yaw = 30.0, math.radians(33.0), math.radians(45.0)
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
                # Left empty on purpose. The cells are generated from a seed
                # before the first frame -- a hundred and sixty on a side is
                # more world than a scene file should carry, and one that is
                # written down is one island for ever.
                "sindri.tile_volume": {"tileset": "causeway.tileset.json", "cells": []},
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
                "sindri.grid.placement": {"grid": "floor", "cell": list(BEACON)},
                "sindri.script": {"source": "scripts/beacon.decay", "script": "Beacon",
                                  "properties": {"lit_scale": 1.25}},
            },
        },
        {
            # Where the walker was last told to go, in play mode.
            #
            # A marker rather than a pair of numbers on the walker, because
            # routing already takes an entity as its destination -- the same
            # call that walks to the beacon walks to this, and tap-to-move
            # needs no new way to ask. Deliberately not an occupant, for the
            # reason the beacon is not: nothing routes into a filled cell, so
            # marking it would make the place you tapped the one place you
            # could not go.
            "id": "target",
            "name": "Target",
            "transform_3d": transform((0.0, 0.0, 0.0)),
            "components": {
                "sindri.grid.placement": {"grid": "floor", "cell": list(WANDERER)},
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
            # Which game you are playing at the moment. One entity so there is
            # one answer, read by the camera, the builder and the walker.
            "id": "modes",
            "name": "Modes",
            "transform_3d": transform((0.0, 0.0, 0.0)),
            "components": {
                "sindri.script": {"source": "scripts/modes.decay", "script": "Modes"},
            },
        },
        {
            # Switching modes without a keyboard, which is the only way to do
            # it on the device this was hardest to play on.
            "id": "mode-button",
            "name": "ModeButton",
            "transform_3d": transform((0.82, 0.88, 0.0), scale=(0.26, 0.1, 1.0)),
            "components": {
                "sindri.ui.button": {"label": "mode"},
                "sindri.ui.shape": {
                    "kind": "rect", "count": 6.0, "fill": [0.09, 0.11, 0.15, 0.94],
                    "stroke": [0.949, 0.722, 0.294, 1.0], "stroke_width": 0.012,
                    "corner_radius": 0.02, "dashes": 0.0, "dash_duty": 0.5,
                    "sweep_start": 0.0, "sweep_turns": 1.0, "blend": "over",
                    "anchor": "center", "layer": 110,
                },
                "sindri.script": {"source": "scripts/mode-button.decay",
                                  "script": "ModeButton"},
            },
        },
        {
            "id": "mode-label",
            "name": "ModeLabel",
            "transform_3d": transform((0.82, 0.88, 0.0)),
            "components": {
                "sindri.ui.text": {
                    "anchor": "center", "color": [0.949, 0.722, 0.294, 1.0],
                    "font": "fonts/Inter.ttf", "font_size": 0.038,
                    "layer": 140, "line_height": 0.045, "text": "BUILD",
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
                    "text": "Tap a block's side to build from it. Hold to take one back. Drag to look. 1-2-3-4 to choose.",
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
                    "projection": "orthographic", "vertical_size": 22.0,
                    "near": 0.1, "far": 400.0,
                },
                "sindri.script": {"source": "scripts/camera-follow.decay",
                                  "script": "CameraFollow",
                                  "properties": {"ease": 4.0}},
            },
        },
    ]

if __name__ == "__main__":
    document = {"format_version": 9, "metadata": {"name": "Causeway"}, "entities": entities()}
    pathlib.Path('game/assets/causeway.scene.json').write_text(
        json.dumps(document, indent=2) + "\n")
    print("wrote the level")
