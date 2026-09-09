/**
 * The regression test the whole tool exists to be able to write.
 *
 * A baked asset is checked into a Sindri project, so "the pipeline still
 * produces this exact picture" has to be a thing CI can assert. Pixels are
 * compared rather than file bytes: a zlib upgrade that changes the compressed
 * stream and no pixel is not a regression, and a test that failed on it would
 * be turned off within a month.
 */

import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

import { bake, sheetIdFor, toJson } from '../src/bake.ts';
import { firstDifference } from '../src/image.ts';
import { decodePng } from '../src/png.ts';
import { parseRecipe } from '../src/recipe.ts';
import { usedColours } from '../src/postprocess.ts';
import { paletteFor } from '../src/frames.ts';

const FIXTURES = fileURLToPath(new URL('../fixtures/', import.meta.url));

async function standingStone() {
  const path = `${FIXTURES}standing-stone.isobake.json`;
  return parseRecipe(await readFile(path, 'utf8'), path);
}

test('the same recipe bakes the same bytes twice', async () => {
  const recipe = await standingStone();
  const first = bake(recipe);
  const second = bake(recipe);

  assert.equal(first.files.length, second.files.length);
  for (const [index, file] of first.files.entries()) {
    assert.equal(file.path, second.files[index].path);
    assert.ok(file.contents.equals(second.files[index].contents), `${file.path} is not deterministic`);
  }
});

test('the fixture still bakes to the picture checked in beside it', async () => {
  const recipe = await standingStone();
  const result = bake(recipe);

  const png = result.files.find((file) => file.path.endsWith('.png'));
  assert.ok(png, 'a bake writes a texture');
  const stored = decodePng(await readFile(`${FIXTURES}baked/${png.path}`));
  const baked = decodePng(png.contents);

  const at = firstDifference(baked, stored);
  assert.equal(
    at,
    null,
    at && at.x < 0
      ? `the sheet is now ${baked.width}x${baked.height}, was ${stored.width}x${stored.height}`
      : `the bake differs from the fixture at pixel ${at?.x},${at?.y}`,
  );

  const sheet = result.files.find((file) => file.path === sheetIdFor(recipe.texture));
  assert.ok(sheet, 'a bake writes a sheet document');
  assert.equal(sheet.contents.toString('utf8'), await readFile(`${FIXTURES}baked/${sheet.path}`, 'utf8'));
});

test('every frame is the same size, with the anchor on its centre', async () => {
  const result = bake(await standingStone());
  const { canvas, frames, report } = result;

  assert.equal(canvas.width % 2, 0, 'an odd canvas has no integer centre to anchor to');
  assert.equal(canvas.height % 2, 0);
  assert.deepEqual(report.anchor, [canvas.width / 2, canvas.height / 2]);

  for (const frame of frames) {
    assert.equal(frame.image.width, canvas.width, `${frame.direction} is a different width`);
    assert.equal(frame.image.height, canvas.height, `${frame.direction} is a different height`);
  }
});

test('the sheet is a plain grid of one row, named by direction', async () => {
  const result = bake(await standingStone());
  assert.deepEqual(result.sheet.document, {
    format_version: 1,
    grid: { columns: 4, rows: 1, names: ['south', 'west', 'north', 'east'] },
  });
  assert.equal(result.sheet.image.width, result.canvas.width * 4);
  assert.equal(result.sheet.image.height, result.canvas.height);
});

test('nothing comes out in a colour the recipe did not declare', async () => {
  const recipe = await standingStone();
  const result = bake(recipe);
  const declared = new Set(paletteFor(result.mesh, recipe.render));

  for (const frame of result.frames) {
    for (const colour of usedColours(frame.image)) {
      assert.ok(declared.has(colour), `${frame.direction} drew ${colour}, which is not in the palette`);
    }
  }
  assert.deepEqual(result.report.warnings, []);
});

test('turning the model actually changes the picture', async () => {
  const result = bake(await standingStone());
  // The fixture is deliberately lopsided — a patch of moss off to one side —
  // because four identical frames would pass every other test here while the
  // rotation did nothing at all.
  const [south, west] = result.frames;
  assert.notEqual(firstDifference(south.image, west.image), null, 'south and west are identical');
});

test('a sheet id replaces the extension rather than following it', () => {
  assert.equal(sheetIdFor('textures/shrine.png'), 'textures/shrine.sheet.json');
  assert.equal(sheetIdFor('shrine'), 'shrine.sheet.json');
});

test('documents are written with a trailing newline', () => {
  assert.equal(toJson({ a: 1 }), '{\n  "a": 1\n}\n');
});
