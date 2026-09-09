/**
 * Where a baked sprite's tile actually is.
 *
 * Everything else in the baker can be right while this is wrong, and the
 * symptom would be an asset that floats or sinks by a few pixels in a game —
 * visible, annoying, and very hard to attribute back to the tool. So it is
 * measured directly: bake a flat plate exactly one tile across and check its
 * silhouette against the diamond a Sindri tilemap draws.
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { bake } from '../src/bake.ts';
import { contentBounds } from '../src/postprocess.ts';
import { parseRecipe } from '../src/recipe.ts';

/**
 * A 1x1 floor plate with no thickness at all.
 *
 * Zero height on purpose: any thickness at all lifts a corner above the floor,
 * which projects a fraction of a pixel past the tile and costs the canvas a
 * whole pixel each way. The point of this fixture is to measure the tile, so it
 * must not carry anything else.
 */
function plateRecipe(tile: { width: number; height: number }): string {
  return JSON.stringify({
    format_version: 1,
    id: 'plate',
    texture: 'textures/plate.png',
    tile,
    directions: 1,
    render: {
      supersample: 4,
      padding: 0,
      palette_snap: false,
      outline: { enabled: false },
    },
    model: {
      materials: { flat: { colour: '#808080' } },
      parts: [{ type: 'box', material: 'flat', position: [0, 0, 0], size: [1, 0, 1] }],
    },
  });
}

test('a one-tile plate bakes to a one-tile diamond around the anchor', () => {
  const tile = { width: 64, height: 32 };
  const result = bake(parseRecipe(plateRecipe(tile), 'plate'));
  const [anchorX, anchorY] = result.report.anchor;

  assert.deepEqual(
    result.canvas,
    { width: tile.width, height: tile.height },
    'a tile-sized plate with no padding should need exactly a tile of canvas',
  );

  const bounds = contentBounds(result.frames[0].image);
  assert.ok(bounds, 'the plate drew something');

  // The diamond's tips land exactly on the canvas edges, half a tile from the
  // anchor in each direction. They are *tips*, though: the outermost column
  // covers a sliver of a pixel, and the alpha threshold rounds a sliver away.
  // So the check is that the silhouette is centred on the anchor and fills the
  // tile to within that one rounded pixel, which is what "the sprite stands on
  // its tile" actually means.
  const inset = 1;
  assert.ok(bounds.x >= anchorX - tile.width / 2 && bounds.x <= anchorX - tile.width / 2 + inset);
  assert.ok(bounds.y >= anchorY - tile.height / 2 && bounds.y <= anchorY - tile.height / 2 + inset);
  assert.equal(
    bounds.x + bounds.width / 2,
    anchorX,
    'the diamond is not centred on the anchor horizontally',
  );
  assert.equal(
    bounds.y + bounds.height / 2,
    anchorY,
    'the diamond is not centred on the anchor vertically',
  );
});

test('a plate baked at a different tile resolution scales with it', () => {
  const result = bake(parseRecipe(plateRecipe({ width: 32, height: 16 }), 'plate'));
  assert.deepEqual(result.canvas, { width: 32, height: 16 });
});

test('the sprite scale converts baked pixels into the tilemap world units', () => {
  // Gather's tilemap draws 1.1 x 0.55 world-unit tiles. One tile of pixels must
  // therefore come out as exactly 1.1 world units wide.
  const result = bake(parseRecipe(plateRecipe({ width: 64, height: 32 }), 'plate'));
  const [width, height] = result.report.spriteScale;
  assert.ok(Math.abs(width - 1.1) < 1e-12, `a one-tile sprite is ${width} units wide, not 1.1`);
  assert.ok(Math.abs(height - 0.55) < 1e-12, `a one-tile sprite is ${height} units tall, not 0.55`);
});

test('a tilemap of the wrong shape is reported rather than baked over', () => {
  const recipe = parseRecipe(plateRecipe({ width: 64, height: 32 }), 'plate');
  const skewed = { ...recipe, tileWorld: { width: 1.1, height: 0.7 } };
  const warnings = bake(skewed).report.warnings;
  assert.ok(
    warnings.some((warning) => warning.includes('not the shape')),
    `expected a tile-shape warning, got ${JSON.stringify(warnings)}`,
  );
});
