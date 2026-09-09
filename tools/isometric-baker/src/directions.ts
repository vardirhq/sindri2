/**
 * Compass directions.
 *
 * Ported from IsoGame's `tools/sprite-factory/src/directions.ts` (MIT). Names
 * are *world-axis* based, and they compose:
 *
 *   +X          south        projects down-right
 *   +Z          east         projects down-left
 *   +X +Z       south-east   projects straight down
 *   -X          north        projects up-left
 *
 * Naming frames by where the model points, rather than by how many times it was
 * turned, is what lets a scene say `#north` and mean it — and it survives a
 * change to which way the model was authored facing.
 */

export const DIRECTIONS_8 = [
  'south',
  'south-west',
  'west',
  'north-west',
  'north',
  'north-east',
  'east',
  'south-east',
] as const;

export type Direction = (typeof DIRECTIONS_8)[number];

/** How many frames a bake produces. One is a prop that reads the same all round. */
export type DirectionCount = 1 | 2 | 4 | 8;

export const DIRECTION_COUNTS: DirectionCount[] = [1, 2, 4, 8];

export function isDirection(value: string): value is Direction {
  return (DIRECTIONS_8 as readonly string[]).includes(value);
}

/** How many eighth-turns one step of this granularity is worth. */
export function strideFor(count: DirectionCount): number {
  return DIRECTIONS_8.length / count;
}

/** Which way `direction` points after `steps` rotations at this granularity. */
export function rotateDirection(direction: Direction, steps: number, count: DirectionCount): Direction {
  const from = DIRECTIONS_8.indexOf(direction);
  const total = DIRECTIONS_8.length;
  const offset = steps * strideFor(count);
  return DIRECTIONS_8[(((from + offset) % total) + total) % total];
}

/** The frames a bake of this granularity produces, in rotation order. */
export function framesFor(facing: Direction, count: DirectionCount): Direction[] {
  return Array.from({ length: count }, (_, index) => rotateDirection(facing, index, count));
}
