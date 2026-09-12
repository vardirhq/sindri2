#!/usr/bin/env python3
"""One-shot authoring pass for Gather's first full-size farm.

The committed scene remains the source of truth and can be edited normally.
This script exists only to make the initial 625-cell terrain and repeated
scenery reviewable as rules rather than as hand-entered integers.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCENE = ROOT / "game/assets/gather.scene.json"
SHED = ROOT / "game/assets/shed.scene.json"
SIZE = 25
CENTRE = 12
FLOOR_Y = round(0.275 * (SIZE - 1), 4)


def grass(col: int, row: int) -> int:
    value = (col * 17 + row * 31 + col * row * 7) % 23
    if value == 0:
        return 4
    if value in (1, 2):
        return 3
    return (col * 5 + row * 3 + col * row) % 3


def is_shore(col: int, row: int) -> bool:
    edge = min(col, row, SIZE - 1 - col, SIZE - 1 - row)
    if edge <= 1:
        return True
    return edge == 2 and (
        (row < 6 and col % 4 != 0)
        or (col > 18 and row % 5 in (0, 1))
        or (row > 19 and col % 5 in (2, 3))
        or (col < 5 and row % 6 == 0)
    )


def terrain() -> list[int]:
    cells = [
        [9 if is_shore(col, row) else grass(col, row) for col in range(SIZE)]
        for row in range(SIZE)
    ]

    # Three deliberately different working plots: dry rows, partly-watered
    # rows, and a smaller orchard plot. They make the future farming states
    # visible now instead of reserving a blank rectangle for later.
    for row in range(4, 10):
        for col in range(4, 10):
            cells[row][col] = 5 if (col + row) % 5 else 6
    for row in range(15, 21):
        for col in range(4, 10):
            cells[row][col] = 6 if row % 3 == 0 else 5
    for row in range(4, 9):
        for col in range(16, 21):
            cells[row][col] = 5

    # A pond with an irregular bank, separate from the sea around the island.
    pond = {
        (17, 16), (18, 16), (19, 16),
        (16, 17), (17, 17), (18, 17), (19, 17), (20, 17),
        (16, 18), (17, 18), (18, 18), (19, 18), (20, 18),
        (17, 19), (18, 19), (19, 19),
    }
    for col, row in pond:
        cells[row][col] = 9

    # Paths are a network, not decoration: every field, the old shrine grove,
    # the farmhouse pad, and the shore are connected to the central crossroads.
    for col in range(2, 23):
        cells[12][col] = 7
    for row in range(2, 23):
        cells[row][12] = 7
    for col in range(9, 13):
        cells[9][col] = 7
    for row in range(9, 13):
        cells[row][9] = 7
    for col in range(9, 13):
        cells[15][col] = 7
    for row in range(12, 16):
        cells[row][9] = 7
    for col in range(12, 17):
        cells[9][col] = 7
    for row in range(9, 13):
        cells[row][16] = 7

    # The farmhouse and village square have a hard, readable footprint.
    for row in range(13, 17):
        for col in range(10, 15):
            cells[row][col] = 8

    return [cell for row in cells for cell in row]


def position(col: float, row: float) -> list[float]:
    return [round(0.55 * (col - row), 4), round(FLOOR_Y - 0.275 * (col + row), 4), 0.0]


def layer(col: float, row: float) -> int:
    return round(1 + 2 * (col + row))


def prop(kind: str, index: int, col: float, row: float) -> dict:
    if kind == "tree":
        texture, scale = "textures/tree.png#south", [1.05, 2.5, 1.0]
    elif kind == "outcrop":
        texture, scale = "textures/standing-stones.png#south", [0.925, 1.4, 1.0]
    elif kind == "wall":
        texture, scale = "textures/stone-wall.png#south", [0.95, 1.075, 1.0]
    else:
        texture, scale = "textures/waystone.png#south", [0.8, 1.0, 1.0]

    components: dict[str, object] = {
        "sindri.sprite": {
            "layer": layer(col, row),
            "texture": texture,
            "tint": [1.0, 1.0, 1.0, 1.0],
        }
    }
    if kind in ("tree", "outcrop"):
        components = {
            "sindri.grid.occupant": {"grid": "floor"},
            **components,
            "sindri.tags": {"tags": ["solid"]},
        }
    return {
        "id": f"farm-{kind}-{index:02d}",
        "name": f"Farm {kind.title()} {index + 1}",
        "transform_3d": {
            "position": position(col, row),
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": scale,
        },
        "components": components,
    }


def collapse_scalar_arrays(pretty: str) -> str:
    out: list[str] = []
    index = 0
    line_start = 0
    while index < len(pretty):
        char = pretty[index]
        if char == "\n":
            out.append(char)
            index += 1
            line_start = len("".join(out))
            continue
        if char == '"':
            end = index + 1
            while end < len(pretty):
                if pretty[end] == "\\":
                    end += 2
                elif pretty[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            out.append(pretty[index:end])
            index = end
            continue
        if char == "[":
            end = index + 1
            nested = False
            in_string = False
            while end < len(pretty):
                current = pretty[end]
                if in_string:
                    if current == "\\":
                        end += 2
                        continue
                    if current == '"':
                        in_string = False
                elif current == '"':
                    in_string = True
                elif current in "[{":
                    nested = True
                    break
                elif current == "]":
                    candidate = json.dumps(
                        json.loads(pretty[index : end + 1]),
                        ensure_ascii=False,
                        separators=(", ", ": "),
                    )
                    column = len("".join(out)) - line_start
                    if column + len(candidate) < 96:
                        out.append(candidate)
                        index = end + 1
                    break
                end += 1
            else:
                nested = True
            if index > end:
                continue
            if not nested and end < len(pretty) and pretty[end] == "]" and index == end + 1:
                continue
        out.append(char)
        index += 1
    return "".join(out)


def write_canonical(path: Path, document: dict) -> None:
    document["entities"].sort(key=lambda entity: entity["id"])
    pretty = json.dumps(document, indent=2, ensure_ascii=False)
    path.write_text(collapse_scalar_arrays(pretty) + "\n")


def main() -> None:
    document = json.loads(SCENE.read_text())
    floor = next(entity for entity in document["entities"] if entity["id"] == "floor")
    floor["transform_3d"]["position"][1] = FLOOR_Y
    tilemap = floor["components"]["sindri.tilemap"]
    tilemap.update({
        "columns": SIZE,
        "palette": [
            "grass", "grass-b", "grass-c", "grass-tuft", "grass-bloom",
            "soil", "soil-wet", "path", "flagstone", "water",
        ],
        "rows": SIZE,
        "tiles": terrain(),
    })
    navigation = floor["components"]["sindri.grid.navigation"]
    navigation["walls"] = [
        {"first": [8 + step, 8 + step], "second": [9 + step, 8 + step]}
        for step in range(4)
    ]

    # The old 9x9 composition remains centred in world space. Its logical cells
    # move by eight in each axis, so only row-derived render layers change.
    for entity in document["entities"]:
        sprite = entity.get("components", {}).get("sindri.sprite")
        transform = entity.get("transform_3d")
        if sprite is None or transform is None:
            continue
        x, y, _ = transform["position"]
        total = (FLOOR_Y - y) / 0.275
        difference = x / 0.55
        sprite["layer"] = layer((total + difference) / 2, (total - difference) / 2)

    door = next(entity for entity in document["entities"] if entity["id"] == "shed-door")
    # Going into the shed still arrives inside its 7x5 grid.
    door["components"]["sindri.script"]["properties"].update({
        "arrive_x": 3.0,
        "arrive_y": 3.0,
    })
    camera = next(entity for entity in document["entities"] if entity["id"] == "world-camera")
    camera["components"]["sindri.script"] = {
        "properties": {"ease": 7.5},
        "script": "CameraFollow",
        "source": "scripts/camera-follow.decay",
    }

    layouts = {
        "tree": [
            (3, 4), (3, 7), (5, 3), (7, 3),
            (17, 3), (19, 3), (21, 5), (21, 8), (18, 10),
            (3, 16), (3, 19), (5, 21), (8, 21),
            (15, 21), (19, 21), (21, 19), (21, 14),
        ],
        "outcrop": [
            (2, 10), (3, 13), (10, 2), (14, 2),
            (22, 11), (22, 14), (11, 22), (14, 22),
            (15, 17), (16, 16), (20, 16), (21, 18), (20, 20),
        ],
        "wall": [
            (3.5, 4), (3.5, 6), (3.5, 8),
            (9.5, 4), (9.5, 6), (9.5, 8),
            (3.5, 16), (3.5, 18), (3.5, 20),
            (9.5, 16), (9.5, 18), (9.5, 20),
            (15.5, 4), (15.5, 6), (20.5, 5), (20.5, 7),
        ],
        "waystone": [(12, 3), (3, 12), (21, 12), (12, 21)],
    }
    document["entities"] = [
        entity for entity in document["entities"]
        if not entity["id"].startswith("farm-")
    ]
    for kind, cells in layouts.items():
        document["entities"].extend(
            prop(kind, index, col, row)
            for index, (col, row) in enumerate(cells)
        )
    write_canonical(SCENE, document)

    shed = json.loads(SHED.read_text())
    exit_door = next(entity for entity in shed["entities"] if entity["id"] == "way-out")
    exit_door["components"]["sindri.script"]["properties"].update({
        "arrive_x": 12.0,
        "arrive_y": 13.0,
    })
    if not any(entity["id"] == "world-camera" for entity in shed["entities"]):
        shed["entities"].append({
            "id": "world-camera",
            "name": "World Camera",
            "transform_3d": {
                "position": [0.0, 0.0, 9.0],
                "rotation": [0.0, -0.0, -0.0, 1.0],
                "scale": [1.0, 1.0, 1.0],
            },
            "components": {
                "sindri.camera": {
                    "far": 60.0,
                    "near": 0.1,
                    "projection": "perspective",
                    "vertical_fov_degrees": 45.0,
                }
            },
        })
    write_canonical(SHED, shed)


if __name__ == "__main__":
    main()
