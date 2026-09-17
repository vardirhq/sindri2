/**
 * Surface texture made out of a material's own ramp.
 *
 * The thing being protected is that grain is a *texture* rather than noise: it
 * has to be the same picture every time the asset is baked, and it has to stay
 * inside the four colours the material promised, or `palette_snap` and the bake
 * report both start lying.
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { bake } from '../src/bake.ts';
import { decodePng } from '../src/png.ts';
import { parseRecipe } from '../src/recipe.ts';
import { grainShift } from '../src/shading.ts';

function recipe(grain: unknown): ReturnType<typeof parseRecipe> {
  const source = {
    format_version: 1,
    id: 'grain-probe',
    texture: 'textures/grain-probe.png',
    view: 'isometric',
    tile: { width: 32, height: 16 },
    tile_world: { width: 1, height: 0.5 },
    directions: 1,
    variants: [
      {
        name: 'block',
        model: {
          materials: { body: grain ? { colour: '#8e8e89', grain } : { colour: '#8e8e89' } },
          parts: [{ type: 'box', material: 'body', position: [0, 0.5, 0], size: [1, 1, 1] }],
        },
      },
    ],
  };
  return parseRecipe(JSON.stringify(source), 'grain-probe.isobake.json');
}

function pixels(grain: unknown): Uint8Array {
  const baked = bake(recipe(grain));
  const png = baked.files.find((file) => file.path.endsWith('.png'));
  assert.ok(png, 'a bake writes a texture');
  return decodePng(png.contents).data;
}

/** How many distinct opaque colours a baked block uses. */
function colours(data: Uint8Array): Set<string> {
  const seen = new Set<string>();
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;
    seen.add(`${data[i]},${data[i + 1]},${data[i + 2]}`);
  }
  return seen;
}

test('grain is a texture rather than noise, so it bakes the same twice', () => {
  const grain = { size: 0.125, strength: 0.5, seed: 3 };
  assert.deepEqual(pixels(grain), pixels(grain));
});

test('grain breaks a flat face up without leaving the ramp', () => {
  const flat = colours(pixels(null));
  const grainy = colours(pixels({ size: 0.125, strength: 0.5, seed: 3 }));
  assert.ok(
    grainy.size > flat.size,
    `a grainy face should use more of its ramp than a flat one: ${grainy.size} vs ${flat.size}`,
  );
  // Four ramp entries and one outline colour is everything a block may emit.
  assert.ok(grainy.size <= 5, `grain must stay inside the ramp, got ${grainy.size} colours`);
});

test('a different seed is a different surface, and the same seed is the same one', () => {
  const first = pixels({ size: 0.125, strength: 0.5, seed: 1 });
  const again = pixels({ size: 0.125, strength: 0.5, seed: 1 });
  const other = pixels({ size: 0.125, strength: 0.5, seed: 2 });
  assert.deepEqual(first, again);
  assert.notDeepEqual(first, other);
});

test('a texel is decided by where it is on the model, not by the pixel it lands on', () => {
  // The same point, asked twice, is the same answer; a point a texel away is
  // free to differ. This is what makes grain survive being baked from four
  // directions, or at a different supersample.
  const grain = { size: 0.25, strength: 1 };
  assert.equal(grainShift(grain, 0.1, 0.1, 0.1), grainShift(grain, 0.2, 0.2, 0.2));
  const shifts = new Set<number>();
  for (let i = 0; i < 40; i++) shifts.add(grainShift(grain, i * 0.25, 0, 0));
  assert.ok(shifts.size > 1, 'a whole row of texels should not agree');
});

test('a material with no grain, or none worth having, is left alone', () => {
  assert.equal(grainShift({ size: 0, strength: 1 }, 1, 2, 3), 0);
  assert.equal(grainShift({ size: 0.1, strength: 0 }, 1, 2, 3), 0);
});
