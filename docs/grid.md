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

IsoGame's pathfinder allowed diagonal movement with an approximate `1.4` cost
while using a Manhattan heuristic, scanned a set for the lowest score, and
mixed actor exclusion into its passability callback. Sindri's port keeps the
useful A* behaviour but replaces those details with explicit topology and
integer costs, an admissible policy-specific heuristic, and application-owned
passability.

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
- normalized arbitrary and rectangular `GridFootprint` values
- bounded `GridOccupancy<Owner>` with queries, removal, dry-run validation, and
  atomic placement or movement
- explicit placement failures for conflicting occupants, out-of-bounds cells,
  and coordinate overflow
- authored `sindri.grid.navigation` wall edges and `sindri.grid.occupant`
  stable grid references plus relative footprints
- `WorldGridNavigation`, which derives runtime occupancy from world transforms
  and exposes complete-footprint placement and path queries

Decay now exposes continuous logical position through an explicit tilemap
entity: `Grid.position_x`, `Grid.position_y`, and `Grid.place` invert and apply
the same projection plus the tilemap's full world-XY transform. Gather uses that
surface for movement, floor bounds, and collection distance without moving any
game rule into Rust.

Occupancy deliberately stores application-owned identifiers rather than engine
entities. The core can therefore back a headless simulation, an editor preview,
or a world adapter without acquiring an ECS dependency. A move may overlap the
owner's previous cells, but the entire destination is validated before those
old cells are released, so a failed move cannot leave a partial placement.

The renderer-free A* layer now builds directly on that definition of
passability. `GridPathfinder` supports cardinal or eight-way movement, explicit
integer cardinal/diagonal costs, and an explicit corner-cutting policy. Its
Manhattan or octile heuristic remains admissible even when a configured
diagonal costs more than two cardinal steps, and stable neighbour/insertion
ordering makes equal-cost results deterministic.

A caller may supply any cell-passability function; each answer is memoized for
the duration of the search. `GridOccupancy::find_path` supplies the standard
adapter by validating the moving owner's complete footprint at every candidate
anchor while allowing overlap with that owner's current cells. A returned
`GridPath` includes both endpoints and its total cost; an impassable or
disconnected goal is `None`, while invalid endpoints and arithmetic exhaustion
are explicit errors.

Wall edges are a separate primitive rather than hidden in passability.
`GridWallEdge` accepts only cardinal neighbours and normalizes their order, so a
wall has one symmetric identity regardless of which cell asks. `GridWalls`
bounds those edges, makes block/unblock idempotent, and iterates them in stable
coordinate order. Grid bounds already prevent leaving the map, so this set
represents internal walls rather than duplicating the perimeter.

`GridPathfinder::find_path_with_walls` checks a cardinal move against its one
shared edge. A diagonal is allowed only when all four cardinal edges around its
corner are open; the separate corner-cutting policy still decides whether the
two side cells themselves must be passable. Occupancy can combine both rules
through `GridOccupancy::find_path_with_walls`, validating the complete moving
footprint at each anchor and checking the wall crossed by every constituent
footprint cell during each transition.
Mismatched wall/path bounds are rejected explicitly.

The engine adapter deliberately does not serialize occupancy. A
`sindri.grid.occupant` names an authored tilemap by `SceneEntityId`, carries
only its relative footprint, and takes its anchor from the entity's current
world position. `WorldGridNavigation` resolves those stable references into
runtime `EntityId` owners, inverts the tilemap's planar transform and exact
`GridSpace`, builds all walls and placements, and rejects the whole snapshot if
any wall, footprint, or placement is invalid. Rebuilding after a world change
keeps the world authoritative and avoids stale duplicated cell state.

The tilemap remains the sole owner of bounds and projection. The optional
`sindri.grid.navigation` component stores only internal wall endpoints, so a
scene cannot configure one shape for rendering and another for pathfinding.
The adapter targets one explicit tilemap entity, preserving the same
multi-grid rule as Decay's coordinate surface.

Editor authoring, typed Decay access, overlays, and a Gather pathfinding use
case remain cross-layer adapters tracked in the integration matrix.
