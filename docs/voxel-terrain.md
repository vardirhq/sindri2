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
   around the surface undercuts faces into ledges and overhangs. Tunnels run
   where two fields both cross their middle, and caverns open deep below the
   sea. Neither breaks the surface except in the ranges, where a tunnel
   coming out is a cave mouth.
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

Every voxel field names a material by its ID. In the inspector each is a menu
of the materials the world defines, and the optional ones also offer *None*.
Switching a world's generator between `layered_terrain` and `natural_terrain`
writes the arriving generator's fields and drops the other's. A new Voxel
World component starts as a natural terrain that needs only its three starting
materials.

An invalid value is reported against the field, for example
``voxel generator `biomes.2.trees` must be from 0 to 1``, and the editor keeps
drawing the last valid world meanwhile.

## Not yet

Everything is drawn as opaque cubes, so water and leaves are solid until cutout
and transparent rendering lands; Voxel Lab uses a leaves texture flattened
onto green for that reason. There are no strata, ores or structures, and
water does not flow. Causeway still uses its own generator.
