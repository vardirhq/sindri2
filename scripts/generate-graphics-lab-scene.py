#!/usr/bin/env python3
"""Generate the texture-free Graphics Lab composition.

This is intentionally deterministic. The lab is a visual stress/sample project,
so keeping the authored scene reproducible makes screenshots useful in review.
"""

from __future__ import annotations

import json
import math
import random
from pathlib import Path

OUT = Path("games/graphics-lab/assets/graphics-lab.scene.json")
ENTITIES: list[dict] = []


def transform(x=0.0, y=0.0, z=0.0, sx=1.0, sy=None, rot=0.0):
    if sy is None:
        sy = sx
    half = rot / 2.0
    return {
        "position": [round(x, 4), round(y, 4), z],
        "rotation": [0.0, 0.0, round(math.sin(half), 6), round(math.cos(half), 6)],
        "scale": [round(sx, 4), round(sy, 4), 1.0],
    }


def shape(kind="polygon", count=6, fill=(0, 0, 0, 0), stroke=(1, 1, 1, 1), sw=0.05,
          dashes=0, duty=0.5, blend="add", layer=0, points=None):
    result = {
        "kind": kind, "count": float(count), "fill": list(fill), "stroke": list(stroke),
        "stroke_width": float(sw), "corner_radius": 0.0, "dashes": float(dashes),
        "dash_duty": float(duty), "sweep_start": 0.0, "sweep_turns": 1.0,
        "blend": blend, "layer": int(layer),
    }
    if points is not None:
        result["points"] = [[float(x), float(y)] for x, y in points]
    return result


def script(spin=0.0, pulse=0.0, pulse_speed=2.5, phase=0.0, bob_x=0.0, bob_y=0.0,
           bob_speed=1.0, orbit_radius=0.0, orbit_speed=1.0):
    return {
        "source": "scripts/showcase.decay", "script": "Showcase",
        "properties": {
            "spin": spin, "pulse": pulse, "pulse_speed": pulse_speed, "phase": phase,
            "bob_x": bob_x, "bob_y": bob_y, "bob_speed": bob_speed,
            "orbit_radius": orbit_radius, "orbit_speed": orbit_speed,
        },
    }


def add(entity_id, name, x, y, sx=1.0, sy=None, kind="polygon", count=6,
        fill=(0, 0, 0, 0), stroke=(1, 1, 1, 1), sw=0.05, dashes=0, duty=0.5,
        blend="add", layer=0, parent=None, points=None, spin=0.0, pulse=0.0,
        pulse_speed=2.5, phase=0.0, bob_x=0.0, bob_y=0.0, bob_speed=1.0,
        orbit_radius=0.0, orbit_speed=1.0, rot=0.0, scripted=True):
    entity = {"id": entity_id, "name": name}
    if parent:
        entity["parent"] = parent
    entity["transform_3d"] = transform(x, y, 0.0, sx, sy, rot)
    components = {}
    if scripted:
        components["sindri.script"] = script(
            spin, pulse, pulse_speed, phase, bob_x, bob_y, bob_speed, orbit_radius, orbit_speed
        )
    components["sindri.shape"] = shape(
        kind, count, fill, stroke, sw, dashes, duty, blend, layer, points
    )
    entity["components"] = components
    ENTITIES.append(entity)


def ring(parent, prefix, scale, count, stroke, spin, dashes, layer, phase=0.0, sy=None):
    add(prefix, prefix.replace("-", " ").title(), 0, 0, sx=scale, sy=sy, parent=parent,
        kind="ellipse" if count == 0 else "polygon", count=6 if count == 0 else count,
        fill=(0, 0, 0, 0), stroke=stroke, sw=0.04, dashes=dashes, duty=0.32,
        layer=layer, spin=spin, pulse=0.055, pulse_speed=2.6, phase=phase)


def build():
    add("backdrop", "Void", 0, 0, sx=70, sy=70, kind="rect", fill=(0.001, 0.002, 0.008, 1),
        stroke=(0, 0, 0, 0), sw=0, blend="over", layer=-120, scripted=False)
    add("grid-major", "Major Grid", 0, 0, sx=30, sy=30, kind="grid", count=30,
        fill=(0, 0, 0, 0), stroke=(0.02, 0.17, 0.25, 0.32), sw=0.004,
        layer=-110, spin=0.004)
    add("grid-minor", "Minor Grid", 0, 0, sx=30, sy=30, kind="grid", count=60,
        fill=(0, 0, 0, 0), stroke=(0.01, 0.08, 0.14, 0.17), sw=0.002,
        layer=-111, spin=-0.002)
    ENTITIES.append({
        "id": "camera", "name": "Camera", "transform_3d": transform(0, 0, 10),
        "components": {"sindri.camera": {
            "far": 100.0, "near": 0.1, "projection": "orthographic",
            "vertical_size": 12.0, "fit": "shorter",
        }},
    })

    for i, (scale, color, dashes, spin) in enumerate([
        (8.5, (0.08, 0.42, 0.62, 0.20), 24, 0.02),
        (11.5, (0.45, 0.12, 0.65, 0.13), 36, -0.014),
        (15.0, (0.10, 0.25, 0.48, 0.10), 48, 0.009),
    ]):
        add(f"world-halo-{i}", f"World Halo {i}", 0, 0, sx=scale, kind="ellipse",
            fill=(0, 0, 0, 0), stroke=color, sw=0.018, dashes=dashes, duty=0.28,
            layer=-80, spin=spin, pulse=0.01, pulse_speed=0.5 + i * 0.2, phase=i)

    # Central reactor: five concentric layers plus eight animated radial prongs.
    add("core", "Quantum Reactor", 0, 0, sx=1.25, count=8, fill=(0.02, 0.18, 0.28, 0.45),
        stroke=(0.2, 0.9, 1, 1), sw=0.07, layer=20, spin=0.25, pulse=0.035, pulse_speed=2.8)
    for i, (scale, kind, count, color, dashes, spin, pulse, phase) in enumerate([
        (1.35, "ellipse", 6, (0.2, 0.65, 1, 0.7), 16, -0.8, 0.03, 0),
        (1.75, "polygon", 12, (0.9, 0.2, 1, 0.55), 8, 0.55, 0.06, 1.2),
        (2.15, "ellipse", 6, (0.15, 1, 0.78, 0.35), 28, -0.28, 0.04, 2.1),
        (2.6, "polygon", 16, (0.25, 0.5, 1, 0.24), 12, 0.16, 0.03, 3.2),
    ]):
        add(f"core-shell-{i}", f"Core Shell {i}", 0, 0, sx=scale, parent="core", kind=kind,
            count=count, fill=(0, 0, 0, 0), stroke=color, sw=0.04, dashes=dashes,
            duty=0.35 + i * 0.05, layer=19 + i, spin=spin, pulse=pulse,
            pulse_speed=2 + i * 0.4, phase=phase)
    add("core-heart", "Core Heart", 0, 0, sx=0.28, parent="core", kind="ellipse",
        fill=(0.65, 0.96, 1, 0.95), stroke=(1, 1, 1, 1), sw=0.11,
        layer=30, pulse=0.22, pulse_speed=5.4)
    for i in range(8):
        angle = i * math.tau / 8
        add(f"core-prong-{i}", f"Core Prong {i}", math.cos(angle) * 1.55,
            math.sin(angle) * 1.55, sx=0.12, sy=0.55, parent="core", kind="rect",
            fill=(0.06, 0.36, 0.55, 0.24), stroke=(0.25, 0.86, 1, 0.7), sw=0.045,
            layer=21, rot=angle - math.pi / 2, pulse=0.08, pulse_speed=3.2, phase=i * 0.5)

    # Upper-right boss sculpture.
    add("boss", "Crown Engine", 3.7, 4.9, count=10, fill=(0.28, 0.02, 0.12, 0.4),
        stroke=(1, 0.13, 0.46, 1), sw=0.08, layer=20, spin=-0.18, pulse=0.04,
        pulse_speed=2.1, bob_x=0.08, bob_y=0.16, bob_speed=0.7)
    for i, (scale, count, color, spin, dashes) in enumerate([
        (1.35, 6, (0.95, 0.18, 1, 0.7), 1.0, 6),
        (1.72, 12, (1, 0.15, 0.48, 0.55), -0.62, 12),
        (2.1, 8, (0.32, 0.45, 1, 0.34), 0.28, 16),
    ]):
        add(f"boss-shell-{i}", f"Boss Shell {i}", 0, 0, sx=scale, parent="boss",
            count=count, fill=(0, 0, 0, 0), stroke=color, sw=0.045, dashes=dashes,
            duty=0.32, layer=22 + i, spin=spin, pulse=0.06, pulse_speed=2.6 + i * 0.5,
            phase=0.2 + i)
    add("boss-eye", "Boss Eye", 0, 0, sx=0.3, parent="boss", kind="ellipse",
        fill=(1, 0.12, 0.45, 0.95), stroke=(1, 0.8, 0.9, 1), sw=0.12,
        layer=30, pulse=0.25, pulse_speed=6)
    for i in range(10):
        angle = i * math.tau / 10
        add(f"boss-spike-{i}", f"Boss Spike {i}", math.cos(angle) * 1.42,
            math.sin(angle) * 1.42, sx=0.14, sy=0.5, parent="boss", count=3,
            fill=(0.45, 0.02, 0.18, 0.28), stroke=(1, 0.2, 0.5, 0.9), sw=0.05,
            layer=21, rot=angle - math.pi / 2, pulse=0.12, pulse_speed=3.5, phase=i * 0.35)

    # Authored-point ship with layered shields and fake additive exhaust.
    hull = [(0, 1.35), (0.58, -0.75), (0.18, -0.52), (0, -0.95), (-0.18, -0.52), (-0.58, -0.75)]
    add("ship", "Vector Spear", -3.5, -4.6, sx=0.9, count=len(hull), points=hull,
        fill=(0.02, 0.16, 0.28, 0.55), stroke=(0.3, 0.9, 1, 1), sw=0.075,
        layer=20, spin=0.08, pulse=0.025, pulse_speed=2.4, bob_x=0.12, bob_y=0.18, bob_speed=0.9)
    for i, (scale, color, dashes, spin) in enumerate([
        (1.45, (0.2, 0.8, 1, 0.65), 6, -0.9),
        (1.85, (0.15, 0.4, 1, 0.3), 12, 0.32),
    ]):
        add(f"ship-shield-{i}", f"Ship Shield {i}", 0, 0, sx=scale, parent="ship",
            count=6, fill=(0, 0, 0, 0), stroke=color, sw=0.04, dashes=dashes,
            duty=0.55, layer=18 + i, spin=spin, pulse=0.06, pulse_speed=2.8, phase=i)
    add("ship-core", "Ship Core", 0, -0.08, sx=0.24, parent="ship", kind="ellipse",
        fill=(0.55, 0.96, 1, 0.95), stroke=(0.9, 1, 1, 1), sw=0.09,
        layer=28, pulse=0.18, pulse_speed=7)
    for i, (y, sx, sy, alpha) in enumerate([
        (-1.15, 0.38, 0.7, 0.35), (-1.55, 0.25, 0.95, 0.22), (-1.95, 0.15, 1.15, 0.12)
    ]):
        add(f"engine-trail-{i}", f"Engine Trail {i}", 0, y, sx=sx, sy=sy, parent="ship",
            count=3, fill=(0.1, 0.65, 1, alpha), stroke=(0.3, 0.85, 1, alpha + 0.2),
            sw=0.03, layer=16 - i, pulse=0.25, pulse_speed=8 + i, phase=i * 0.7)

    # Top portal and lower-right singularity make the portrait viewport feel inhabited.
    add("portal", "Prism Gate", 0, 8.2, sx=0.75, count=6, fill=(0.08, 0.03, 0.25, 0.25),
        stroke=(0.45, 0.35, 1, 0.95), sw=0.06, layer=18, spin=0.3, pulse=0.08,
        pulse_speed=1.8, bob_y=0.12, bob_speed=0.8)
    for i, (scale, count, color, spin) in enumerate([
        (1.35, 3, (0.2, 0.85, 1, 0.7), -0.75),
        (1.75, 6, (0.75, 0.2, 1, 0.5), 0.46),
        (2.2, 12, (0.15, 1, 0.7, 0.28), -0.22),
    ]):
        add(f"portal-ring-{i}", f"Portal Ring {i}", 0, 0, sx=scale, parent="portal",
            count=count, fill=(0, 0, 0, 0), stroke=color, sw=0.04, dashes=count,
            duty=0.3, layer=18 + i, spin=spin, pulse=0.05, pulse_speed=2 + i)
    add("portal-center", "Portal Center", 0, 0, sx=0.18, parent="portal", kind="ellipse",
        fill=(0.7, 0.8, 1, 0.95), stroke=(1, 1, 1, 1), sw=0.1,
        layer=27, pulse=0.3, pulse_speed=5)

    add("singularity", "Singularity", 3.7, -7.1, sx=0.78, kind="ellipse",
        fill=(0.02, 0.0, 0.08, 0.8), stroke=(0.72, 0.2, 1, 0.9), sw=0.07,
        layer=18, spin=-0.15, pulse=0.04, pulse_speed=1.7, bob_y=0.12, bob_speed=0.6)
    for i, (scale, dashes, color, spin) in enumerate([
        (1.4, 10, (0.65, 0.18, 1, 0.55), 1.2),
        (1.9, 18, (0.2, 0.55, 1, 0.4), -0.65),
        (2.5, 30, (0.1, 1, 0.75, 0.2), 0.3),
    ]):
        add(f"singularity-ring-{i}", f"Singularity Ring {i}", 0, 0, sx=scale,
            sy=scale * 0.55, parent="singularity", kind="ellipse", fill=(0, 0, 0, 0),
            stroke=color, sw=0.04, dashes=dashes, duty=0.25, layer=17 + i,
            spin=spin, pulse=0.1, pulse_speed=2 + i * 0.4, phase=i)
    add("singularity-core", "Singularity Core", 0, 0, sx=0.24, parent="singularity",
        kind="ellipse", fill=(0.5, 0.05, 0.9, 0.9), stroke=(1, 0.55, 1, 1),
        sw=0.12, layer=29, pulse=0.28, pulse_speed=6.5)

    add("prism", "Prism Array", -3.5, 5.8, sx=0.8, count=3, fill=(0.0, 0.18, 0.22, 0.35),
        stroke=(0.1, 1, 0.78, 1), sw=0.07, layer=18, spin=0.26, pulse=0.05,
        pulse_speed=2.2, bob_x=0.08, bob_y=0.12, bob_speed=0.85)
    for i in range(3):
        angle = i * math.tau / 3
        add(f"prism-node-{i}", f"Prism Node {i}", math.cos(angle) * 1.1,
            math.sin(angle) * 1.1, sx=0.34, parent="prism", count=3,
            fill=(0.05, 0.2, 0.18, 0.25), stroke=(0.2, 1, 0.72, 0.9), sw=0.055,
            layer=20, spin=(-1 if i % 2 else 1) * (1.1 + i * 0.2),
            pulse=0.12, pulse_speed=3 + i, phase=i)

    rocks = [
        ("rock-a", -4.7, 1.6, 1.1, 0.85, [(0,1),(.8,.55),(.95,-.2),(.35,-1),(-.4,-.82),(-1,-.1),(-.65,.7)]),
        ("rock-b", 4.7, -1.7, 0.9, 1.15, [(0,1),(.75,.7),(1,0),(.5,-.9),(-.25,-1),(-.9,-.4),(-.85,.4)]),
        ("rock-c", -4.5, -7.4, 0.7, 0.55, [(0,1),(.9,.45),(.75,-.6),(-.1,-1),(-.9,-.45),(-.7,.55)]),
        ("rock-d", 4.4, 7.4, 0.68, 0.82, [(0,1),(.7,.5),(.9,-.35),(.2,-1),(-.65,-.7),(-1,.1),(-.5,.7)]),
    ]
    for i, (entity_id, x, y, sx, sy, points) in enumerate(rocks):
        add(entity_id, f"Procedural Rock {i}", x, y, sx=sx, sy=sy, count=len(points), points=points,
            fill=(0.02, 0.05, 0.11, 0.72), stroke=(0.28, 0.52, 0.78, 0.75),
            sw=0.045, blend="over", layer=3, spin=0.12 if i % 2 == 0 else -0.16,
            pulse=0.025, pulse_speed=1.2 + i * 0.2, bob_y=0.08, bob_speed=0.4 + i * 0.1)
        add(f"{entity_id}-inner", f"Rock Inner {i}", 0.06, -0.03, sx=0.62,
            parent=entity_id, count=5, fill=(0, 0, 0, 0), stroke=(0.18, 0.38, 0.58, 0.46),
            sw=0.03, dashes=3, duty=0.55, blend="over", layer=4,
            spin=-0.22 if i % 2 == 0 else 0.25)

    palette = [(0.15,1,0.7,1),(1,0.62,0.12,1),(0.82,0.28,1,1),(0.22,0.75,1,1),(1,0.18,0.45,1)]
    satellites = [(-4.2,8.6),(-2.8,9.3),(2.3,9.0),(4.3,2.2),(-4.5,-1.2)]
    for i, (x, y) in enumerate(satellites):
        color = palette[i % len(palette)]
        add(f"sat-{i}", f"Satellite {i}", x, y, sx=0.24 + 0.05 * (i % 3),
            count=3 + (i % 5), fill=(color[0]*0.18, color[1]*0.18, color[2]*0.18, 0.3),
            stroke=color, sw=0.06, layer=10, spin=(1.2+i*0.25) * (-1 if i%2 else 1),
            pulse=0.15, pulse_speed=3+i*0.35, phase=i*0.6,
            bob_x=0.08, bob_y=0.12, bob_speed=0.6+i*0.07)

    random.seed(8)
    for i in range(12):
        x, y = random.uniform(-5.1, 5.1), random.uniform(-10.5, 10.5)
        color = palette[i % len(palette)]
        kind = "ellipse" if i % 3 == 0 else "polygon"
        count = 6 if kind == "ellipse" else 3 + i % 4
        size = random.uniform(0.035, 0.11)
        add(f"star-{i}", f"Energy Speck {i}", x, y, sx=size, kind=kind, count=count,
            fill=(color[0],color[1],color[2],0.45), stroke=(color[0],color[1],color[2],0.7),
            sw=0.04, layer=12, spin=random.uniform(-4,4), pulse=random.uniform(0.15,0.5),
            pulse_speed=random.uniform(4,9), phase=random.uniform(0,6.2),
            bob_x=random.uniform(0,0.08), bob_y=random.uniform(0,0.12), bob_speed=random.uniform(0.5,1.5))

    for side in (-1, 1):
        for i in range(4):
            add(f"bolt-{side}-{i}", f"Energy Bolt {side} {i}", side * (0.9 + i * 0.45),
                -1.0 - i * 0.38, sx=0.08, sy=0.2, count=4,
                fill=(0.15,0.75,1,0.45), stroke=(0.55,0.95,1,0.9), sw=0.04,
                layer=16, rot=-0.5 if side < 0 else 0.5, pulse=0.25,
                pulse_speed=8, phase=i*0.4, bob_x=0.05, bob_y=0.08, bob_speed=1.6)


def write():
    build()

    # Only scene roots receive direct pointer hit tests. Their descendants
    # inherit the resulting transform burst through hierarchy, so one tap
    # animates the entire sculpture without every child competing for input.
    interactions = {
        "core": (1.85, 1.0),
        "boss": (1.75, 2.0),
        "ship": (1.55, 3.0),
        "portal": (1.6, 4.0),
        "singularity": (1.55, 5.0),
        "asteroid-0": (1.0, 2.0),
        "asteroid-1": (1.0, 3.0),
        "asteroid-2": (1.0, 4.0),
        "asteroid-3": (1.0, 5.0),
    }
    for entity in ENTITIES:
        interaction = interactions.get(entity["id"])
        if interaction is None:
            continue
        script_component = entity.get("components", {}).get("sindri.script")
        if script_component is None:
            continue
        tap_radius, tap_mode = interaction
        script_component["properties"]["tap_radius"] = tap_radius
        script_component["properties"]["tap_mode"] = tap_mode
    lines = ["{", '  "format_version": 9,', '  "metadata": {"name": "Graphics Lab Overdrive"},', '  "entities": [']
    for index, entity in enumerate(ENTITIES):
        suffix = "," if index + 1 < len(ENTITIES) else ""
        lines.append("    " + json.dumps(entity, separators=(",", ":")) + suffix)
    lines.extend(["  ]", "}"])
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT} with {len(ENTITIES)} entities")


if __name__ == "__main__":
    write()
