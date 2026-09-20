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

Empty is absence, not a reserved tile ID. The serialized form stays a readable
sparse cell list. Runtime code shares a 16×16 `TileChunkCoord` and
`TileChunkStore`, so generators and render culling use the same seams while a
scene or save needs no second chunk-shaped file format. Causeway exercises the
first streaming slice: its camera materializes deterministic terrain chunks as
Build pans or Play follows, and sparse navigation derives frontier walls from
loaded walkable columns rather than scanning the enormous declared envelope.

This is not complete residency management yet. Loaded Causeway chunks remain
resident for the run; eviction, persisted player deltas, asynchronous generation,
and budgets for generation/rebuild work are the next layer. The current slice
fixes the finite-map camera failure and establishes the engine unit those
systems will manage without pretending an ever-growing resident set is endless.

A surface is a derived view of a volume, not its storage model. Terrain tools
usually edit the highest occupied level in a column and may fill supporting
strata automatically. A block tool edits one exact 3D cell. Both therefore
produce the same data, and stacked blocks, caves, bridges, walls, and overhangs
need no second map system.

### Tile set and terrain rules

Project asset describing semantic terrain:

- stable tile IDs such as `grass`, `earth`, `water`, `soil`, and `stone`
- how much of its cell each tile fills: a `height` from the floor upward, or an
  `extent` box naming the part of the cell the tile occupies as fractions of it,
  `[across, up, into]`. A height is the full footprint by definition, so
  everything it can describe is a slab of some thickness — a post, a fence, a
  kerb, a rail and a step are all outside what it can say, and a rail is outside
  it twice over because a height always starts at the floor. A document naming
  both is refused rather than having one of them quietly win
- whether a tile's top holds anything up, and separately whether a walker can
  stand on it — water does the first and not the second, and one flag saying
  "solid" could not tell a pond from a hole
- face visuals for top, bottom, and four horizontal neighbours
- adjacency groups and allowed transitions
- deterministic weighted variants: one tile ID with several looks, chosen from
  a hash of the cell, the tile's name and the volume's seed. The scene stores
  what a cell *is* and the renderer decides what it looks like, so variation
  costs nothing per cell and is the same on every machine and after every
  reload — which a render capture depends on. A volume's `variant_seed` rerolls
  the whole thing at once. The hash is written in-repo rather than taken from
  `std`, whose default hasher may change between releases and would rearrange
  every scene that used it

A box changes what covering means. A height could only ask whether a neighbour
was tall enough, which stops being the question the moment a tile does not fill
its footprint: a post against a wall is tall, and hides almost none of it.
Culling now asks whether the two faces meet at the wall between the cells at
all, and whether the neighbour covers the whole of the face rather than part of
it. Corner darkening asks the narrower question still — only a neighbour filling
its cell crowds a corner — because a fence post is not a wall and should not
shade like one.

A box is the shape the tile *is*, not the shape of its art. Its faces are still
six quads placed on the box's own sides, so a fence post is a thin box wearing a
fence texture rather than a full block whose picture happens to be mostly
transparent. Picking is unchanged and still treats a cell as a whole cell, so a
click near a post's cell finds the post.

Where one terrain gives way to another, the threshold that decides it is frayed
rather than sharp: near the crossing the answer comes from a field sampled at a
few cells' scale rather than from the comparison alone. A threshold on a smooth
field draws a smooth line, and a smooth line through a landscape reads as a
border on a map rather than as ground changing. Deciding each column on its own
would fray it too, but into salt and pepper — single cells of snow scattered
through grass read as dirt on the screen, not as snow lying in the hollows.

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
but not walkable, so the island is bounded by the shape of its own ground
rather than by an authored wall.

That began as one flag. Water was "not solid", which answered the navigation
question correctly and took the moat's surface away with it, so three rocks
standing off the shore were left on nothing and had to be given a sandbar. A
pond holds a boat up and a hole does not, and `supports` and `walkable` are now
separate because no single flag can say which of those a cell is. Gather's
water supports and is unwalkable: 625 columns of the island hold something up,
387 of them can be stood on, and the 238 between them are the moat and the
pond. Navigation reads the second number and is unchanged. And a
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
