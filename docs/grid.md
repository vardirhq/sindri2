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

Both include a configurable plane origin. `unproject` is the algebraic inverse,
and `plane_to_grid` rounds the continuous result to the containing cell. Exact
half-cell ties follow Rust's `f64::round`: away from zero. Tests cover every
cell in a 257×257 region on both projections, negative coordinates, fractional
positions, non-square cells, and a non-zero origin.

The projected plane is intentionally not named world or screen space. A 2D
sprite game may map it to world XY, an orthographic 3D game may map it to world
XZ, and a UI may map it to pixels. Those adapters own axis direction, camera,
pan, zoom, and viewport centring. Keeping them outside this crate is what lets
the gameplay grid be shared.

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

The existing `sindri.tilemap` projection remains separate for now. It answers
where a render component draws its rows in local XY (including the engine's
upward world-Y convention); `sindri-grid` answers gameplay geometry on a neutral
plane. An adapter should replace duplicated formulae only after Gather proves
the orientation and centre conventions together.

## Current and next

This first slice provides:

- `GridCoord`, continuous `GridPoint`, and `PlanePoint`
- finite `GridBounds` and stable bounded/unbounded neighbour iteration
- validated orthogonal and isometric `GridSpace`
- reversible continuous projection and cell lookup
- explicit errors for invalid sizes, non-finite points, overflow, and values
  outside the integer coordinate domain

The next slice should add the engine world/screen adapters and move Gather's
logical coordinates onto this crate. Footprints and occupancy should follow
before A*, because pathfinding needs a truthful definition of passability.
