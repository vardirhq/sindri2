/**
 * The camera is the contract between a bake and a Sindri tilemap: if a unit
 * tile does not project to exactly one tile of pixels, nothing baked with it
 * stands where it should, and every other test would still pass.
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { basisError, createCamera, originTileCentre, projectToPixels } from '../src/iso.ts';
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
