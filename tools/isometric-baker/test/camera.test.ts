/**
 * The camera is the contract between a bake and a Sindri tilemap: if a unit
 * tile does not project to exactly one tile of pixels, nothing baked with it
 * stands where it should, and every other test would still pass.
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  basisError,
  createCamera,
  createFlatCamera,
  originTileCentre,
  projectToPixels,
} from '../src/camera.ts';
import { vec } from '../src/vec.ts';

test('the camera basis is orthonormal', () => {
  for (const tile of [
    { width: 64, height: 32 },
    { width: 32, height: 16 },
    { width: 96, height: 48 },
    { width: 64, height: 37 },
  ]) {
    assert.ok(basisError(createCamera(tile)) < 1e-12, `${tile.width}x${tile.height} is not orthonormal`);
  }
});

test('a unit floor tile projects to exactly one tile of pixels', () => {
  const camera = createCamera({ width: 64, height: 32 });
  const centre = originTileCentre(1, 1);

  // The four corners of the tile the model stands on, at floor level.
  const east = projectToPixels(camera, vec(0.5, 0, -0.5), centre);
  const west = projectToPixels(camera, vec(-0.5, 0, 0.5), centre);
  const south = projectToPixels(camera, vec(0.5, 0, 0.5), centre);
  const north = projectToPixels(camera, vec(-0.5, 0, -0.5), centre);

  assert.ok(Math.abs(east.x - west.x - 64) < 1e-9, `diamond is ${east.x - west.x}px across, not 64`);
  assert.ok(Math.abs(south.y - north.y - 32) < 1e-9, `diamond is ${south.y - north.y}px tall, not 32`);
  // Left and right corners sit level with the anchor; near and far are the tips.
  assert.ok(Math.abs(east.y) < 1e-9 && Math.abs(west.y) < 1e-9);
  assert.ok(Math.abs(south.x) < 1e-9 && Math.abs(north.x) < 1e-9);
});

test('height projects straight up the screen', () => {
  const camera = createCamera();
  const origin = originTileCentre(1, 1);
  const raised = projectToPixels(camera, vec(0, 1, 0), origin);
  assert.equal(raised.x, 0, 'raising something must not move it sideways');
  assert.ok(raised.y < 0, 'raising something must move it up the screen');
});

test('a tile that is not wider than it is tall has no isometric pitch', () => {
  assert.throws(() => createCamera({ width: 64, height: 64 }), /wider than it is tall/);
  assert.throws(() => createCamera({ width: 0, height: 0 }), /must be positive/);
});

test('a flat camera basis is orthonormal', () => {
  for (const view of ['top-down', 'side'] as const) {
    for (const scale of [1, 16, 32, 45.25]) {
      assert.ok(
        basisError(createFlatCamera(view, scale)) < 1e-12,
        `${view} at ${scale} px/unit is not orthonormal`,
      );
    }
  }
});

test('top-down draws the ground square and gives height no size at all', () => {
  const camera = createFlatCamera('top-down', 32);
  const origin = originTileCentre(1, 1);

  // The same four ground corners the isometric test measures as a diamond.
  const east = projectToPixels(camera, vec(0.5, 0, -0.5), origin);
  const west = projectToPixels(camera, vec(-0.5, 0, 0.5), origin);
  assert.ok(Math.abs(east.x - west.x - 32) < 1e-9, 'a unit of ground is not 32px across');
  assert.ok(Math.abs(west.y - east.y - 32) < 1e-9, 'a unit of ground is not 32px down');

  // Looking straight down, a thing's height is not visible. This is the whole
  // character of the view, and the reason a top-down bake shows only top faces.
  // Zero, not "close to zero": the axes are exact, so this is not a tolerance.
  // Written with `abs` only because a signed zero is still zero and
  // `strictEqual` disagrees.
  const raised = projectToPixels(camera, vec(0, 1, 0), origin);
  assert.ok(Math.abs(raised.x) === 0 && Math.abs(raised.y) === 0);
});

test('side draws height up the screen and gives depth no size at all', () => {
  const camera = createFlatCamera('side', 32);
  const origin = originTileCentre(1, 1);

  const raised = projectToPixels(camera, vec(0, 1, 0), origin);
  assert.ok(Math.abs(raised.x) === 0, 'raising something must not move it sideways');
  assert.ok(Math.abs(raised.y + 32) < 1e-9, 'a unit of height is not 32px up the screen');

  const behind = projectToPixels(camera, vec(0, 0, 1), origin);
  assert.ok(Math.abs(behind.x) === 0 && Math.abs(behind.y) === 0);
});

test('a flat camera needs a real scale, since it has none to derive', () => {
  assert.throws(() => createFlatCamera('side', 0), /positive number/);
  assert.throws(() => createFlatCamera('top-down', Number.NaN), /positive number/);
});

test('the isometric tile guard points at the views that have no pitch', () => {
  assert.throws(() => createCamera({ width: 64, height: 64 }), /top-down/);
});
