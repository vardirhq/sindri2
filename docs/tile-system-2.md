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

An isometric column two blocks taller than its neighbour therefore exposes two
side faces naturally. Removing the lower block creates a real opening rather
than a special cliff case. No sprite is stretched into a slab and no overhang
value is consulted.

Projection supplies three basis vectors. Orthogonal and isometric views differ
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
Collision rebuilds from occupied cells and tile metadata. Navigation walks
surfaces with authored climb, drop, and clearance limits rather than flattening
the volume back into a 2D obstacle map.

## Migration boundary

`sindri.tilemap` remains readable only while repository scenes are migrated.
New authoring targets Tile System 2 and never writes `tile_overhang`. A flat old
map can be translated mechanically into a grid, one tile volume at `z = 0`, and
an imported tile set. A slab map cannot be translated honestly because sprite
overhang does not state elevation; migration must require an explicit level and
fill choice.

Once all checked-in projects have moved, `sindri.tilemap`, its editor palette,
and `tile_overhang` are removed together. The new format will not carry a
compatibility field that preserves the ambiguity.

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
