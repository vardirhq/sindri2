# Tile System 2

Status: block-volume foundation implemented; terrain rules, collision, and
navigation remain in progress.

Gather reached the limit of `sindri.tilemap`. The grid arithmetic held up; the
representation around it did not. One component currently owns logical cells,
projection, sprite selection, visual bounds, terrain thickness, draw order, and
the editor palette. `tile_overhang` is the clearest symptom: a visual size is
being asked to mean terrain elevation.

Tile System 2 keeps `sindri-grid` and replaces that mixed representation.

## Invariants

1. A cell coordinate is logical data. It does not move when art changes.
2. Elevation is an integer Z coordinate. It is never inferred from PNG height.
   A tile may fill part of its cell, and says so as a number rather than by
   being drawn shorter; nothing occupies more than the cell it is in.
3. A visual has an explicit ground anchor and may overflow its cell without
   changing collision, navigation, picking, or elevation.
4. Painter order is global across textures. Batching may combine adjacent draws
   only after ordering has been decided.
5. Terrain meaning is independent of the sprite variant chosen to show it.
6. Orthogonal and isometric maps share storage, terrain, collision, navigation,
   and authoring. Projection changes coordinate conversion and presentation.
7. Editor painting writes terrain and elevation, not edge-frame names.

## Four levels

### Grid

Pure geometry and bounds:

- projection: `orthogonal` or `isometric`
- columns and rows
- logical cell size
- grid-to-plane and plane-to-grid conversion

The existing `sindri-grid` types remain the authority for this math.

### Tile volume

Sparse 3D cell storage attached to a grid:

- tile ID at an integer `(column, row, level)` coordinate
- optional deliberate visual override
- volume-wide visibility and ordering policy

Empty is absence, not a reserved tile ID. The serialized form starts sparse so
an empty sky costs nothing; chunked or dense runtime storage may replace its
index later without changing what one cell means.

A surface is a derived view of a volume, not its storage model. Terrain tools
usually edit the highest occupied level in a column and may fill supporting
strata automatically. A block tool edits one exact 3D cell. Both therefore
produce the same data, and stacked blocks, caves, bridges, walls, and overhangs
need no second map system.

### Tile set and terrain rules

Project asset describing semantic terrain:

- stable tile IDs such as `grass`, `earth`, `water`, `soil`, and `stone`
- how much of its cell each tile fills, from its floor upward
- navigation and collision metadata
- face visuals for top, bottom, and four horizontal neighbours
- adjacency groups and allowed transitions
- deterministic weighted variants

Tile sets are assets rather than components so several scenes and volumes can
share one definition without copying it into every scene. Optional terrain
rules describe high-level surface painting: the surface tile, supporting fill,
transitions, and which tiles count as the same terrain family.

### Tile visual

One representation a tile or terrain rule may choose:

- texture and named sheet sprite
- role: top, bottom, north, east, south, west, corner, transition, or decoration
- explicit ground anchor
- visual bounds/overflow
- repeat or stretch policy for an exposed vertical span

A visual says how it is drawn, never how high the cell is.

## Derived rendering

For each occupied 3D cell, the renderer:

1. resolves its tile definition;
2. culls every face whose neighbouring 3D cell occludes it;
3. chooses deterministic face variants from the tile and its neighbours;
4. emits the remaining top, side, transition, and decoration visuals;
5. assigns deterministic painter keys from volume, projected cell depth, level,
   face role, and stable cell coordinate;
6. globally sorts emitted visuals and only then groups adjacent equal textures
   into draw calls.

Depth belongs to the *column*, not to the drawn face. Every face of a cell takes
the depth of the point where its column meets the ground, so raising a block
moves it up the screen without moving it toward the viewer, and a cell's own
faces are ordered by face role rather than sorted against each other. What
decides between draws at equal depth — the levels of one column, and everything
in a view laid out along the camera's depth axis — is the painter key, whose
leading term is the projection's own idea of distance: `x + y` along an
isometric diagonal, `y` alone down orthogonal rows.

An isometric column two blocks taller than its neighbour therefore exposes two
side faces naturally. Removing the lower block creates a real opening rather
than a special cliff case. No sprite is stretched into a slab and no overhang
value is consulted.

Projection supplies three basis vectors, and also decides which faces a view can
see at all: an isometric view is turned between two axes and shows a cell's top
and two sides, while an orthogonal view looks straight down the rows and shows
the top and the side facing the viewer — its east and west faces are edge-on and
have no width to draw. Neither shows an underside. Asking the projection rather
than drawing whatever a tile set happens to define is what lets one set of tiles
serve both, instead of painting an isometric side flat across an orthogonal
block.

Orthogonal and isometric views differ
in the screen-space X/Y basis; both give integer Z a configured vertical step.
This remains sprite rendering: a block is resolved into visible 2D face visuals,
not submitted as runtime 3D geometry.

## Authoring

The editor's first complete block workflow is now available on an entity that
carries both `sindri.tile_grid` and `sindri.tile_volume`. Its Build inspector
loads the volume's validated tile set and offers two undoable tools:

- **Surface** picks the frontmost exposed top diamond. Place stacks one block
  above it; Remove deletes the block that produced it. Empty space starts a
  foundation at level zero. Surface gestures are click-only so holding a drag
  cannot accidentally grow or drill an entire column.
- **Level** addresses one explicit integer Z level and supports click-drag
  painting for floors, shelves, and deliberate overhangs.

Both modes preview the exact target cell in the Scene view, keep cell ordering
deterministic, preserve unknown component fields, and commit through the
editor's command history so a stroke can be undone and saved normally. Tile-set
choices are cached for the selected asset rather than reread every frame.

This is intentionally the block half of the authoring contract. Side-face
picking, terrain-name painting, automatic supporting strata, slice/isolation
views, and generated collision/navigation are still later Tile System 2 work.

The primary terrain tool paints a terrain ID on a column. A height gesture
raises or lowers its surface and the terrain rule fills or removes supporting
tiles. A block tool and level/slice control edit one exact `(x, y, z)` cell for
construction, caves, and overhangs. Neighbour rules update the preview without
rewriting cells into edge variants. An advanced override may pin a visual
variant, and clearing it returns the cell to automatic resolution.

Block authoring follows a "LEGO, not CAD" rule. The implemented top-face tool
places the selected block above that face and removal targets its source cell;
the explicit-level tool paints snapped planes. Extending that same contract to
side faces will place blocks in their horizontal adjacent cells. Slice and
isolation views will reveal hidden levels without making authors type
coordinates. Every gesture previews the same derived baked faces the running
game draws.

Picking tests emitted faces front-to-back and returns the 3D cell and face that
caused the hit. Selection may inspect a derived face, but edits the source cell.
Collision rebuilds from occupied cells and tile metadata.

Navigation walks surfaces rather than flattening the volume back into a 2D
obstacle map. A column's surface is the top of its highest solid cell, as a
fraction rather than a level so a slab is half a cell lower than a block, and a
column with nothing solid in it is a hole rather than ground at level zero. Each
edge between two columns is then a step a walker can take or a wall, which is a
thing `sindri-grid` already understands, so the volume contributes walls instead
of the pathfinder learning about levels.

What is authored is one symmetric `max_step`, not separate climb and drop
limits: grid walls block an edge rather than a direction, so a rule letting a
walker fall further than it can climb has nowhere to be recorded. Clearance and
overhangs are the same gap from the other side — only the highest solid cell of
a column is consulted, so a cave is a roof over nothing rather than a second
walkable level. Both wait on directional edges in `sindri-grid`.

## Migration boundary

`sindri.tilemap` remains readable only while repository scenes are migrated.
New authoring targets Tile System 2 and never writes `tile_overhang`.

Geometry is the one thing the two systems already share, and it is shared
deliberately rather than duplicated. Columns, rows, cell size and projection
mean the same thing in `sindri.tilemap` and `sindri.tile_grid`, and both put a
cell's origin in the same place and run their Y axis the same way, so anything
that only asks *where a cell is* takes either component: `Grid.position_x`,
`Grid.place`, `Grid.can_reach`, `Grid.step_toward`, `Grid.columns`, `Grid.rows`,
and the navigation snapshot behind walls, occupancy and pathfinding. An entity
carrying both — a migration in progress — answers from the flat map, so adding
a volume beside an existing floor changes nothing until the flat map is removed.

What is *not* shared is what a cell holds. `Grid.tile` and `Grid.set_tile` index
a palette in a flat array, which is the old model rather than a translation of
it, and they refuse on a grid that carries only `sindri.tile_grid` instead of
pretending a volume has one level and a palette. A flat old
map can be translated mechanically into a grid, one tile volume at `z = 0`, and
an imported tile set. A slab map cannot be translated honestly because sprite
overhang does not state elevation; migration must require an explicit level and
fill choice.

Gather has now moved. Its island is 625 blocks at `z = 0` with the outcrop
stacked on top, the shed is 35 more, and no scene in the repository carries
`sindri.tilemap`. The translation was the mechanical one this section describes:
each palette entry became a tile of the same name — all five kinds of grass, so
the floor kept the variation the scene had already authored — and every filled
cell became a block at level zero.

Two things the move surfaced that a flat map had made unaskable. Water is opaque
but not solid, so the moat and the pond became holes and the island is now
bounded by the shape of its own ground rather than by an authored wall; three
rocks standing off the shore were left on nothing, and sit on a sandbar. And a
volume in a 2D scene needs somewhere to be in the draw order, which is what
`layer_step` is: viewed straight on there is no depth but the render layer, so a
volume that interleaves with sprites places its cells along it.

`sindri.tilemap`, its editor palette, and `tile_overhang` can now be removed
together whenever that change is worth making on its own; nothing shipped here
depends on them. The new format will not carry a compatibility field that
preserves the ambiguity.

## Acceptance test

In the Sindri editor, starting from an empty scene:

1. create one isometric grid;
2. paint grass, water, soil, and paths by terrain name;
3. raise and lower arbitrary columns through the terrain tool;
4. stack individual blocks into a wall and bridge, then cut an opening beneath;
5. see correct tops, transitions, and occluded/exposed faces update automatically;
6. place a character and props with larger visual bounds;
7. walk above, below, in front of, and behind them with stable ordering;
8. pick any visible face and edit the exact 3D cell that produced it;
9. collide and pathfind with climb, drop, and clearance limits;
10. save, reopen, and reproduce the same scene and render.

Passing requires no manual edge selection, sprite offsets, overhang values, or
draw-order repair.
