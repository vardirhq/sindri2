import assert from 'node:assert/strict';
import { test } from 'node:test';

import { bake } from '../src/bake.ts';
import { parseRecipe } from '../src/recipe.ts';

function slab(positionY: number, height: number) {
  return parseRecipe(
    JSON.stringify({
      format_version: 1,
      id: 'slab',
      texture: 'textures/slab.png',
      tile: { width: 88, height: 44 },
      tile_world: { width: 1.1, height: 0.55 },
      directions: 1,
      footprint: { width: 1, height: 1 },
      render: { padding: 0, outline: { enabled: false } },
      model: {
        materials: { ground: { colour: '#3f6047' } },
        parts: [
          {
            type: 'box',
            material: 'ground',
            position: [0, positionY, 0],
            size: [1, height, 1],
          },
        ],
      },
    }),
    'slab',
  );
}

test('a baked slab records how far its geometry hangs below the floor', () => {
  const result = bake(slab(0, 0.2));
  const ratio = result.sheet.document.tile_overhang_ratio;

  assert.ok(ratio !== undefined, 'geometry below y=0 should describe tile overhang');
  // A 0.2-high box centred on y=0 hangs 0.1 world/model units below the floor.
  // Under the 2:1 88x44 isometric camera that projects to this fraction of one
  // logical tile height. Keep the number here so changing camera maths cannot
  // silently change every baked tile's runtime footprint.
  assert.ok(Math.abs(ratio - 0.1224744871391589) < 1e-12, `${ratio}`);
});

test('ordinary scenery above the floor does not invent tile overhang', () => {
  const result = bake(slab(0.1, 0.2));
  assert.equal(result.sheet.document.tile_overhang_ratio, undefined);
});
