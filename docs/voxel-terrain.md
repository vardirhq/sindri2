# Natural voxel terrain

`sindri.voxel_world` can generate its world with `natural_terrain`: continents
and sea, mountain ranges, rivers, biomes, caves, overhangs, beaches, snow, ice
and trees, all decided by one seed. The generator is
`sindri_voxel::NaturalTerrain`; the scene document is
`sindri_scene::NaturalTerrainDocument`. Voxel Lab's scene is the worked example.

Every section is a pure function of the settings and its coordinate, so a
world is the same on every machine, sections can be generated in any order,
and generating one never depends on another.

## How a column is decided

1. **Shape.** A warped, low-frequency field says where land is. A folded ridge
   field, allowed only where a second "uplift" field is high, raises ranges
   along part of the world rather than everywhere. Small detail roughens both.
2. **Climate.** Warmth and moisture are slow fields of their own. Warmth falls
   with height, so ranges are colder than the plains around them.
3. **Biome.** A column takes the biome whose `temperature`/`moisture` point is
   nearest its climate, read through a little noise so edges fray into
   tongues and bays rather than following a line. Heights blend across biome
   edges by climate distance, so a biome's `relief` and `terraces` change the
   land gradually rather than at a cliff.
4. **Rivers.** Where a slow field crosses its middle, the lowlands are cut
   just below the sea, which fills them. Rivers fade out as the ground rises.
5. **Surface.** In order: the sea bed below sea level; a beach a course or two
   above it; snow above the snow line; bare rock above the tree line or on
   slopes of three voxels or more; otherwise the biome's own ground. Every
   height threshold is frayed, as Causeway's are.
6. **Volume.** In the heart of a range, a band of three-dimensional noise
   around the surface undercuts faces into ledges and overhangs. It only
   takes rock from a few voxels into the side of a wall thick enough, both
   ways across, to keep a core, so a thin ridge or spur is never cut through
   into a window. Tunnels run where two fields both cross their middle, and
   caverns open deep below the sea. Neither breaks the surface except in the
   ranges, where a tunnel coming out is a cave mouth, and a tunnel never runs
   through a wall too thin to hold it.
7. **Water and trees.** Open voxels at or below sea level are water, and the
   surface freezes to ice where it is cold. Trees stand at most one to a
   6×6 square, on a biome's own gentle ground, as often as its `trees`
   says; cold biomes grow conifers.

## Authoring

| Field | Meaning |
| --- | --- |
| `seed` | The world. Another seed is another world. |
| `sea_level` | The level water fills to. |
| `relief` | How far above the sea the highest ranges reach (1–1024). |
| `feature_size` | Width of the largest landforms in voxels (8–4096). Continents are a few times this. |
| `tree_line`, `snow_line` | Heights above the sea where ground turns to bare rock, then snow. |
| `caves`, `rivers` | Whether to cut them. |
| `stone_voxel` | Everything below a biome's subsurface. Required. |
| `water_voxel`, `beach_voxel`, `sea_bed_voxel`, `cliff_voxel`, `snow_voxel`, `ice_voxel`, `trunk_voxel`, `leaves_voxel` | Optional. `null` leaves that feature out: no water leaves the sea empty; trees need both trunk and leaves. |
| `biomes[]` | `name`, climate point (`temperature`, `moisture`, 0–1), `surface_voxel`, `subsurface_voxel`, `subsurface_depth`, `trees` (0–1), `relief` (0–4, a multiple of the world's hills), and `terraces` (step height; 0 for none). |

Every voxel field names a block. A world that sets `blocks` to a block set
(`builtin:blocks`, or a project's own `.tileset.json`) names blocks from it by
name — `"surface_voxel": "grass"` — and the engine numbers the set's blocks
itself; each face draws with the block's art, a tile's south face being the
voxel's front. A world with no block set names its own `materials` by number,
as every world did before block sets, and still loads unchanged. In the
inspector each field is a menu of the set's blocks drawn as cubes (or of the
world's materials), and the optional ones also offer *None*. Switching a
world's generator between `layered_terrain` and `natural_terrain` writes the
arriving generator's fields and drops the other's. A new Voxel World component
starts as a natural terrain built from `builtin:blocks`: seven biomes, water,
beaches, cliffs, snow, ice and trees.

## Blocks

`builtin:blocks` is the block set the engine ships, with its art compiled into
`sindri-assets`: grass, dirt, earth, stone, rock, sand, snow, ice, mud, moss,
gravel, clay, planks, log, leaves, water and lava. `builtin:` references are
not asset IDs (a colon cannot appear in one), so no loader looks for them on
disk: a host binds them from `sindri_assets::builtin_textures()` and
`builtin_tile_sets()`, as it binds `procedural:` textures, and the exporter
leaves them out of a build. The editor binds them for every scene.

A block set is an ordinary tile set, the same asset a tile volume uses, so a
block defined once serves both. Selecting one in the Project panel opens the
block set editor: the set's blocks as cubes, and the chosen block's name, a
texture picker for each face (an empty face borrows the opposite one), whether
it hides its neighbours, supports, is walkable, its height and its tags.
*New block set here* starts a project's own set as a copy of the built-in one.
Tags are words a game gives a block (`hot`, `liquid`) that the engine never
reads and scripts ask about with `Grid.tagged`.

A face can move and a block can glow. A face's `animation` lists further frames
(sprites the same size on the same texture as its first) and a speed; the
faces are meshed once, with the first frame, and the renderer shows the others
by shifting where they read, so a lake ripples without being rebuilt. A
block's `glow` lights it itself: shadow takes none of it away, and the bloom
pass picks up what exceeds full brightness. The built-in water and lava are
8-frame loops, and lava glows. Hosts pass the scene's time as
`SceneRuntime::with_seconds`; at zero every animation shows its first frame.
The editor runs its own clock and redraws while a scene has animated blocks.

Whether a block hides its neighbours decides how it is meshed. One that does
(most blocks) is opaque. One that does not is cut out, holes and all, like
leaves; one that does not and is tagged `liquid` hides only the faces between
its own blocks, so a lake has no walls inside it.

An invalid value is reported against the field, for example
``voxel generator `biomes.2.trees` must be from 0 to 1``, and the editor keeps
drawing the last valid world meanwhile.

## Not yet

Water is drawn opaque: a liquid hides only its own inner faces, but light does
not pass through it, because blended rendering needs a sorted pass the voxel
renderer does not have yet. A block's variants are not yet read by the voxel
mesher, and tile volumes do not yet animate or glow. There are no strata, ores
or structures, and water does not flow. Causeway still uses its own generator.
