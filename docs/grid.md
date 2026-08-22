# Grid coordinates and projection

`sindri-grid` is the renderer-independent geometry shared by sprite-isometric
and orthographic-3D games. It owns logical cells, finite bounds, neighbour
queries, and the reversible projection of a logical grid onto a two-dimensional
plane. It does not know about worlds, cameras, sprites, tilemaps, editors, or
Decay.

That boundary is the feature. Gameplay should be able to ask which cells are
adjacent or occupied without linking a renderer, and a renderer should not be
the authority on where gameplay believes a character stands.

## Coordinate convention

`GridCoord { x, y }` names an integer cell. The integer coordinate is the
**centre** of the cell; half-integers are its boundaries. `GridPoint` carries a
continuous position in the same axes.

`GridSpace` places that logical grid on a plane:

- orthogonal: `plane = (x * width, y * height)`
- isometric: `plane = ((x - y) * width / 2, (x + y) * height / 2)`

Both include a configurable plane origin and an explicit plane-Y direction:
screen-like planes grow down, while Sindri world XY grows up. `unproject` is the algebraic inverse,
and `plane_to_grid` assigns the continuous result to a half-open cell: a shared
boundary belongs to the cell in the positive grid direction. Tests cover every
cell in a 257×257 region on both projections, negative coordinates, fractional
positions, non-square cells, and a non-zero origin.

The projected plane is intentionally not named world or screen space. A 2D
sprite game may map it to world XY, an orthographic 3D game may map it to world
XZ, and a UI may map it to pixels. `PlaneYAxis` handles the first distinction;
camera projection, pan, zoom, and viewport centring remain presentation
adapters. Keeping those outside this crate is what lets the gameplay grid be
shared.

## What came from the earlier projects

The legacy Sindri engine's `Grid<T>` demonstrated the useful small core: signed
cell coordinates, rectangular bounds, deterministic row-major traversal, and
four/eight-way neighbour queries. It also tied coordinate conversion to its
render `Vec2`, stored `cell_size` beside cell data, and used unchecked casts and
allocation. Sindri Next keeps the concepts, not that coupling or implementation.

IsoGame's `CoordinateUtils` contains the standard reversible diamond formulas
used above. It also combines them with room dimensions, canvas dimensions, pan,
zoom, and centring. Those are presentation transforms and become editor/camera
adapters later; putting them in the grid would make headless pathfinding depend
on a viewport that may not exist.

IsoGame's pathfinder is not ported in this slice. It allows diagonal movement
with an approximate `1.4` cost while using a Manhattan heuristic, scans a set
for the lowest score, and mixes actor exclusion into its passability callback.
The new A* layer should instead accept an explicit movement topology and cost
policy, use an admissible heuristic for that policy, and know nothing about
players.

`sindri.tilemap` is the first engine adapter. It keeps projection and tile size
in the serialized component, then exposes the `GridSpace` and `GridBounds` that
rendering and editor picking use. The adapter flips the neutral grid into
Sindri's upward world Y, and extraction composes every cell with the map's full
transform so a rotated or scaled map draws where picking says it is.

## Current and next

This first slice provides:

- `GridCoord`, continuous `GridPoint`, and `PlanePoint`
- finite `GridBounds` and stable bounded/unbounded neighbour iteration
- validated orthogonal and isometric `GridSpace`, with upward/downward plane Y
- reversible continuous projection and cell lookup
- a tilemap adapter shared by frame extraction and editor picking
- explicit errors for invalid sizes, non-finite points, overflow, and values
  outside the integer coordinate domain

The next slice should give Decay a typed grid-position surface and move Gather's
logical coordinates onto this crate without putting gameplay rules into Rust.
Footprints and occupancy should follow before A*, because pathfinding needs a
truthful definition of passability.
