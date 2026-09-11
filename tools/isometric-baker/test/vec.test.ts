/**
 * What a part's `rotation` means, pinned.
 *
 * A recipe carries three bare numbers and no unit, and the failure mode when
 * the unit is wrong is quiet: radians read as degrees leave every angle within
 * six degrees of zero, so parts keep their positions, lose their orientations,
 * and the model still bakes to something plausible-looking. Every orbital
 * recipe in this repository was authored that way once. These assert the unit
 * loudly enough that the next author finds out from a test.
 */

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { type Vec3, rotationXYZ, transform, vec } from '../src/vec.ts';

/** Compares to the nearest billionth, which is exactness for this purpose. */
function assertVec(actual: Vec3, expected: Vec3, what: string): void {
  for (let axis = 0; axis < 3; axis += 1) {
    assert.ok(
      Math.abs(actual[axis] - expected[axis]) < 1e-9,
      `${what}: axis ${axis} was ${actual[axis]}, wanted ${expected[axis]}`,
    );
  }
}

test('rotation is in degrees, so a quarter turn is 90', () => {
  // Local +Z, turned a quarter turn about +Y, is world +X exactly.
  assertVec(transform(rotationXYZ([0, 90, 0]), vec(0, 0, 1)), vec(1, 0, 0), 'a quarter turn');
  assertVec(transform(rotationXYZ([0, 180, 0]), vec(0, 0, 1)), vec(0, 0, -1), 'a half turn');
});

test('a quarter turn authored in radians is very nearly no turn at all', () => {
  // The bug this file exists for: 1.5708 is a quarter turn in radians and an
  // almost imperceptible nudge in degrees, and both bake without complaint.
  const turned = transform(rotationXYZ([0, Math.PI / 2, 0]), vec(0, 0, 1));
  assert.ok(turned[2] > 0.999, 'radians-as-degrees leaves the part facing where it started');
});

test('whole quarter turns are exact rather than nearly exact', () => {
  // `Math.cos(Math.PI / 2)` is 6.1e-17, which is enough to tilt a face across a
  // shading threshold. Whole quarter turns take the table instead.
  for (const degrees of [90, 180, 270, -90, -180, -270]) {
    const turned = transform(rotationXYZ([0, degrees, 0]), vec(0, 0, 1));
    for (const component of turned) {
      assert.ok(
        component === 0 || Math.abs(component) === 1,
        `${degrees} degrees produced ${component}, which is not exact`,
      );
    }
  }
});
