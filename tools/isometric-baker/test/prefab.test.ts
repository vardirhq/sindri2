/**
 * The generated prefab.
 *
 * The claim that matters most — that the file is byte-for-byte what Sindri's
 * own canonical writer produces — cannot be checked from here, because Sindri
 * is what defines canonical form. It is checked against the real implementation
 * in `crates/sindri-core/tests/baked_documents_are_canonical.rs`. What is here
 * is everything else: the arithmetic, the shape, and the boundaries.
 */

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

import { bake } from '../src/bake.ts';
import { f32, ordered, toCanonicalJson } from '../src/canonical.ts';
import { formatF32 } from '../src/f32.ts';
import { type Recipe, parseRecipe } from '../src/recipe.ts';

const PROJECT = fileURLToPath(new URL('../fixtures/project/', import.meta.url));

async function standingStone(): Promise<Recipe> {
  const path = `${PROJECT}prefabs/standing-stone.isobake.json`;
  return parseRecipe(await readFile(path, 'utf8'), path);
}

function generated(prefab: string): Record<string, unknown> {
  return JSON.parse(prefab) as Record<string, unknown>;
}

test('a bake writes the prefab its recipe asks for', async () => {
  const recipe = await standingStone();
  const result = bake(recipe);
  const paths = result.files.map((file) => file.path);
  assert.deepEqual(paths, [
    'textures/standing-stone.png',
    'textures/standing-stone.sheet.json',
    'prefabs/standing-stone.prefab.json',
  ]);
});

test('a recipe with no prefab block writes no prefab', async () => {
  const recipe = await standingStone();
  const { prefab: _dropped, ...rest } = recipe;
  const paths = bake(rest).files.map((file) => file.path);
  assert.deepEqual(paths, ['textures/standing-stone.png', 'textures/standing-stone.sheet.json']);
});

test('the prefab scale is the canvas measured in world units', async () => {
  const recipe = await standingStone();
  const result = bake(recipe);
  const prefab = generated(result.files[2].contents.toString('utf8')) as {
    entities: { transform_3d: { scale: number[] } }[];
  };

  const [width, height] = prefab.entities[0].transform_3d.scale;
  // Re-derived from the recipe rather than read back off the report, so this is
  // an independent computation of the same number. The fixture is an isometric
  // bake, which is what makes a tile the right thing to derive it from.
  assert.ok(recipe.tile && recipe.tileWorld, 'the fixture stands on a tilemap');
  const perPixel = recipe.tileWorld.width / recipe.tile.width;
  // Compared as the `f32` the field holds, not as the decimal it is spelled
  // with: the file carries the shortest decimal that names that `f32`, which is
  // a different `f64` from the one the engine will load. Not "close to",
  // though — the sprite is a unit quad, so this *is* its size, and being a
  // pixel out is a sprite that does not line up with the tilemap under it.
  assert.equal(Math.fround(width), Math.fround(result.canvas.width * perPixel));
  assert.equal(Math.fround(height), Math.fround(result.canvas.height * perPixel));
});

test('the prefab draws a named frame of the generated sheet', async () => {
  const result = bake(await standingStone());
  const prefab = generated(result.files[2].contents.toString('utf8')) as {
    entities: { components: Record<string, { texture: string }> }[];
  };
  assert.equal(
    prefab.entities[0].components['sindri.sprite'].texture,
    'textures/standing-stone.png#south',
  );
});

test('a prefab asking for a frame the bake did not produce says so', async () => {
  const recipe = await standingStone();
  const single = { ...recipe, directions: 1 as const, prefab: { ...recipe.prefab!, defaultDirection: 'north' as const } };
  assert.throws(() => bake(single), /asks for the "north" frame/);
});

test('grid occupancy is written only when the recipe names a grid', async () => {
  const recipe = await standingStone();
  const withoutGrid = generated(bake(recipe).files[2].contents.toString('utf8')) as {
    entities: { components: Record<string, unknown> }[];
  };
  assert.deepEqual(Object.keys(withoutGrid.entities[0].components), ['sindri.sprite']);

  // `sindri.grid.occupant` names its grid by scene ID, and a prefab cannot know
  // which entity in the scene that is — so without one, nothing is claimed.
  const onGrid = bake({
    ...recipe,
    footprint: { width: 2, height: 3 },
    prefab: { ...recipe.prefab!, grid: 'floor' },
  });
  const occupied = generated(onGrid.files[2].contents.toString('utf8')) as {
    entities: { components: Record<string, { grid?: string; footprint?: number[][] }> }[];
  };
  const occupant = occupied.entities[0].components['sindri.grid.occupant'];
  assert.equal(occupant.grid, 'floor');
  assert.deepEqual(occupant.footprint, [
    [0, 0],
    [1, 0],
    [0, 1],
    [1, 1],
    [0, 2],
    [1, 2],
  ]);
});

test('a single-cell footprint restates nothing', async () => {
  const recipe = await standingStone();
  const result = bake({ ...recipe, prefab: { ...recipe.prefab!, grid: 'floor' } });
  const prefab = generated(result.files[2].contents.toString('utf8')) as {
    entities: { components: Record<string, Record<string, unknown>> }[];
  };
  const occupant = prefab.entities[0].components['sindri.grid.occupant'];
  assert.deepEqual(Object.keys(occupant), ['grid'], 'one cell is the component default');
});

test('the generation record carries no clock reading', async () => {
  const result = bake(await standingStone());
  const text = result.files[2].contents.toString('utf8');
  // IsoGame's generator metadata records a `renderedAt`. That is right for a
  // build artefact and wrong for a file in version control: it would make every
  // rebuild a diff, and the determinism check meaningless.
  assert.ok(!/\d{4}-\d{2}-\d{2}T/.test(text), 'a timestamp leaked into a generated document');
});

test('an f32 is written as the shortest decimal that names it', () => {
  // What `serde_json` emits through ryu, which is what the file has to match.
  assert.equal(formatF32(1), '1.0');
  assert.equal(formatF32(0), '0.0');
  assert.equal(formatF32(-0), '-0.0');
  assert.equal(formatF32(2 / 3), '0.6666667');
  assert.equal(formatF32(50 * (1.1 / 64)), '0.859375');
  // Nothing a bake produces needs an exponent, and guessing at ryu's exponent
  // form would be a file that silently stops being canonical.
  assert.throws(() => formatF32(1e-9), /exponent/);
  assert.throws(() => formatF32(Number.NaN), /cannot hold/);
});

test('a scalar array folds onto one line only while it fits', () => {
  const short = toCanonicalJson(ordered([['position', [f32(0), f32(0), f32(0)]]]));
  assert.equal(short, '{\n  "position": [0.0, 0.0, 0.0]\n}\n');

  // The width test is against the line built so far, so the same array folds at
  // one depth and not at another. That is Sindri's rule, not a simplification
  // of it: matching it is the difference between canonical and nearly.
  const long = toCanonicalJson({ ['x'.repeat(80)]: [f32(1.5), f32(2.5), f32(3.5)] });
  assert.ok(long.includes('[\n'), `expected the long line to stay expanded, got ${long}`);
});

test('object keys are sorted, because a component payload is a BTreeMap', () => {
  const text = toCanonicalJson({ zebra: 1, apple: 2 });
  assert.equal(text, '{\n  "apple": 2,\n  "zebra": 1\n}\n');
});
