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

/** The miniature Sindri asset root the fixture bakes into, and out of. */
const PROJECT = fileURLToPath(new URL('../fixtures/project/', import.meta.url));

async function standingStone() {
  const path = `${PROJECT}prefabs/standing-stone.isobake.json`;
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
  const stored = decodePng(await readFile(`${PROJECT}${png.path}`));
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
  assert.equal(sheet.contents.toString('utf8'), await readFile(`${PROJECT}${sheet.path}`, 'utf8'));
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

test('the sheet is a grid of one row, named by direction and guttered', async () => {
  const result = bake(await standingStone());
  const gutter = 2;
  assert.deepEqual(result.sheet.document, {
    format_version: 1,
    // Declared rather than left to the format's default, so the sheet says
    // where its frames meet the ground instead of inheriting an answer.
    anchor: 'center',
    grid: {
      columns: 4,
      rows: 1,
      size: [result.sheet.image.width, result.sheet.image.height],
      margin: [gutter, gutter],
      spacing: [2 * gutter, 2 * gutter],
      names: ['south', 'west', 'north', 'east'],
    },
  });
  assert.equal(result.sheet.image.width, (result.canvas.width + 2 * gutter) * 4);
  assert.equal(result.sheet.image.height, result.canvas.height + 2 * gutter);
});

test('the gutter repeats each frame edge rather than leaving a hole', async () => {
  const result = bake(await standingStone());
  const sheet = result.sheet.image;
  const gutter = 2;
  const at = (x: number, y: number) => {
    const i = (y * sheet.width + x) * 4;
    return [sheet.data[i], sheet.data[i + 1], sheet.data[i + 2], sheet.data[i + 3]].join(',');
  };
  // A pixel in the gutter is the frame's own edge pixel repeated. If it were
  // transparent instead, the bilinear minification the renderer does would pull
  // a dark rim into every frame.
  const midY = gutter + Math.floor(result.canvas.height / 2);
  assert.equal(at(0, midY), at(gutter, midY), 'the left gutter repeats the left edge');
  assert.equal(at(gutter - 1, gutter - 1), at(gutter, gutter), 'the corner repeats the corner');
});

test('nothing comes out in a colour the recipe did not declare', async () => {
  const recipe = await standingStone();
  const result = bake(recipe);
  const declared = new Set(result.meshes.flatMap((mesh) => paletteFor(mesh, recipe.render)));

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

test('a colour declared in a different case is still one palette entry', async () => {
  const recipe = await standingStone();
  const shouted = {
    ...recipe,
    render: { ...recipe.render, outline: { enabled: true, colour: '#241D2B' } },
  };
  // The palette decides ties when a blended edge pixel is snapped, so the same
  // colour spelled twice would be two entries and the bake could differ from
  // one that spelled it once.
  const result = bake(shouted);
  assert.deepEqual(result.report.warnings, []);
  const png = bake(recipe).files.find((file) => file.path.endsWith('.png'));
  const shoutedPng = result.files.find((file) => file.path.endsWith('.png'));
  assert.ok(png && shoutedPng && png.contents.equals(shoutedPng.contents));
});

/** A flat-view recipe built in memory, so the assertions are about the view. */
function flat(view: 'top-down' | 'side', pixelsPerUnit: number) {
  return parseRecipe(
    JSON.stringify({
      format_version: 1,
      id: 'probe',
      texture: 'textures/probe.png',
      view,
      pixels_per_unit: pixelsPerUnit,
      directions: 1,
      model: {
        materials: { hull: { colour: '#7fd4ff' } },
        // A unit wide, a unit deep, and deliberately taller than either.
        parts: [{ type: 'box', material: 'hull', position: [0, 1, 0], size: [1, 2, 1] }],
      },
    }),
    'probe',
  );
}

test('a flat bake measures itself, because it stands on no tilemap', () => {
  const result = bake(flat('top-down', 32));
  // The scale a generated prefab is built from: canvas pixels over the camera's
  // own pixels-per-unit, with no tile in the arithmetic anywhere.
  assert.equal(result.report.worldUnitsPerPixel, 1 / 32);
  assert.equal(result.report.spriteScale[0], result.canvas.width / 32);
  assert.equal(result.report.spriteScale[1], result.canvas.height / 32);
  assert.deepEqual(result.report.warnings, [], `${result.report.warnings}`);
});

test('height is invisible from above and is the whole picture from the side', () => {
  // The same model, twice. Looking down, a two-unit-tall box is a one-unit
  // square; looking along, it is a tall rectangle. That difference is the only
  // thing the two views disagree about, and it is the reason both exist.
  const above = bake(flat('top-down', 32)).report;
  const beside = bake(flat('side', 32)).report;

  assert.equal(above.canvas.width, above.canvas.height, 'a unit-square footprint is square from above');
  assert.ok(
    beside.canvas.height > beside.canvas.width,
    `a box twice as tall as it is wide should be taller than it is wide from the side, got ${beside.canvas.width}x${beside.canvas.height}`,
  );
});

test('a model larger than any footprint is fine when nothing hands out ground', () => {
  // Isometric refuses this: a model wider than its footprint covers tiles the
  // game still lets others stand on. A flat view has no tilemap making that
  // promise, so there is nothing to break.
  const recipe = parseRecipe(
    JSON.stringify({
      format_version: 1,
      id: 'wide',
      texture: 'textures/wide.png',
      view: 'top-down',
      pixels_per_unit: 8,
      directions: 1,
      model: {
        materials: { hull: { colour: '#7fd4ff' } },
        parts: [{ type: 'box', material: 'hull', size: [6, 1, 6] }],
      },
    }),
    'wide',
  );
  assert.doesNotThrow(() => bake(recipe));
});
