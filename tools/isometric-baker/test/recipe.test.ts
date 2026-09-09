/**
 * A recipe is hand-edited, so the reader's job is half parsing and half saying
 * what is wrong. These are the mistakes worth naming.
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { RecipeError, parseRecipe } from '../src/recipe.ts';

const MINIMAL = {
  format_version: 1,
  id: 'stone',
  texture: 'textures/stone.png',
  model: {
    materials: { rock: { colour: '#808080' } },
    parts: [{ type: 'box', material: 'rock', size: [1, 1, 1] }],
  },
};

function parse(overrides: Record<string, unknown>) {
  return parseRecipe(JSON.stringify({ ...MINIMAL, ...overrides }), 'recipe');
}

test('a minimal recipe fills in the defaults', () => {
  const recipe = parse({});
  assert.deepEqual(recipe.tile, { width: 64, height: 32 });
  assert.equal(recipe.directions, 4);
  assert.equal(recipe.facing, 'south');
  assert.deepEqual(recipe.footprint, { width: 1, height: 1 });
  assert.equal(recipe.render.supersample, 4);
  assert.equal(recipe.render.outline.enabled, true);
});

test('a field nobody reads is refused rather than ignored', () => {
  assert.throws(() => parse({ supersample: 8 }), /unknown field "supersample"/);
  assert.throws(
    () => parse({ render: { supersamples: 8 } }),
    /unknown field "supersamples"/,
    'a typo inside render must be caught too',
  );
});

test('a recipe from a future version of the baker is refused', () => {
  assert.throws(() => parse({ format_version: 2 }), /this baker writes 1/);
});

test('errors name the field they are about', () => {
  try {
    parse({ model: { materials: {}, parts: [{ type: 'box', material: 'rock', size: [1, 'tall', 1] }] } });
    assert.fail('expected a RecipeError');
  } catch (error) {
    assert.ok(error instanceof RecipeError);
    assert.match(error.message, /model\.parts\[0\]\.size\[1\]/);
  }
});

test('shade band edges have to ascend', () => {
  assert.throws(
    () => parse({ render: { shading: { light: [0, 1, 0], thresholds: [0.8, 0.5, 0.25] } } }),
    /must ascend/,
  );
});

test('eight directions need a square footprint', () => {
  assert.throws(() => parse({ directions: 8, footprint: { width: 2, height: 1 } }), /square footprint/);
  assert.doesNotThrow(() => parse({ directions: 8, footprint: { width: 2, height: 2 } }));
});

test('a footprint is whole tiles', () => {
  assert.throws(() => parse({ footprint: { width: 1.5, height: 1 } }), /whole tiles/);
});

test('a model source that is not baked yet says so instead of being skipped', () => {
  assert.throws(
    () => parse({ model: { kind: 'file', parts: [] } }),
    /only "primitives" is baked today/,
  );
});

test('an unknown primitive names the ones that exist', () => {
  assert.throws(
    () => parse({ model: { materials: {}, parts: [{ type: 'torus', material: '#fff' }] } }),
    /expected box, cylinder, cone or sphere/,
  );
});

test('a material may be named or written inline', () => {
  const recipe = parse({
    model: {
      materials: { rock: { colour: '#808080', unlit: true } },
      parts: [
        { type: 'box', material: 'rock', size: [1, 1, 1] },
        { type: 'sphere', material: { colour: '#ff0000' }, radius: 0.2 },
      ],
    },
  });
  assert.equal(recipe.model.materials.rock.unlit, true);
  assert.deepEqual(recipe.model.parts[1].material, { colour: '#ff0000', ramp: undefined, unlit: undefined });
});

test('a texture has to be a png, because that is what a bake writes', () => {
  assert.throws(() => parse({ texture: 'textures/stone.webp' }), /must name a \.png/);
});

test('a mistyped colour is caught where it was written', () => {
  assert.throws(
    () =>
      parse({
        model: {
          materials: { rock: { colour: '#80808' } },
          parts: [{ type: 'box', material: 'rock', size: [1, 1, 1] }],
        },
      }),
    /expected a hex colour/,
  );
  assert.throws(() => parse({ render: { outline: { colour: 'grey' } } }), /expected a hex colour/);
  // Case is spelling, not meaning: an upper-case colour is the same colour.
  assert.doesNotThrow(() => parse({ render: { outline: { colour: '#241D2B' } } }));
});
