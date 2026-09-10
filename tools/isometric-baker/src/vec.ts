/**
 * The three-component vector maths the baker needs, and nothing else.
 *
 * IsoGame reaches for `THREE.Vector3` here because it is already rendering
 * through Three.js. This tool rasterises on the CPU, so pulling a WebGL library
 * in for a dot product would be the only reason it had any dependency at all —
 * and a Sindri tool with no `node_modules` is one that CI can run without
 * resolving a package tree nobody reviews.
 */

export type Vec3 = readonly [number, number, number];

export function vec(x: number, y: number, z: number): Vec3 {
  return [x, y, z];
}

export function add(a: Vec3, b: Vec3): Vec3 {
  return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}

export function sub(a: Vec3, b: Vec3): Vec3 {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

export function scale(a: Vec3, k: number): Vec3 {
  return [a[0] * k, a[1] * k, a[2] * k];
}

export function dot(a: Vec3, b: Vec3): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

export function cross(a: Vec3, b: Vec3): Vec3 {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}

export function length(a: Vec3): number {
  return Math.sqrt(dot(a, a));
}

/** A zero-length vector normalises to itself rather than to `NaN`. */
export function normalize(a: Vec3): Vec3 {
  const l = length(a);
  return l > 0 ? scale(a, 1 / l) : a;
}

/**
 * A 3x3 matrix in column-major order, which is how a rotation is applied here
 * and how `glam` stores one on the Rust side.
 */
export type Mat3 = readonly [Vec3, Vec3, Vec3];

export const IDENTITY: Mat3 = [vec(1, 0, 0), vec(0, 1, 0), vec(0, 0, 1)];

export function transform(m: Mat3, v: Vec3): Vec3 {
  return [
    m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
    m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
    m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
  ];
}

export function multiply(a: Mat3, b: Mat3): Mat3 {
  return [transform(a, b[0]), transform(a, b[1]), transform(a, b[2])];
}

/**
 * A rotation about +Y by a whole number of eighth-turns.
 *
 * Written as an exact table rather than `Math.cos(index * Math.PI / 4)` for the
 * reason `camera.ts` avoids trigonometry: a quarter turn has to be exactly a
 * quarter turn, or four frames of one asset stop agreeing about where the
 * middle of the tile is.
 */
const EIGHTH_COS = [1, Math.SQRT1_2, 0, -Math.SQRT1_2, -1, -Math.SQRT1_2, 0, Math.SQRT1_2];
const EIGHTH_SIN = [0, Math.SQRT1_2, 1, Math.SQRT1_2, 0, -Math.SQRT1_2, -1, -Math.SQRT1_2];

/** Rotation about +Y that sends +X toward -Z, matching the compass cycle. */
export function rotationY(eighths: number): Mat3 {
  const index = ((eighths % 8) + 8) % 8;
  if (!Number.isInteger(index)) {
    throw new Error(`rotationY takes whole eighth-turns, got ${eighths}`);
  }
  const c = EIGHTH_COS[index];
  const s = EIGHTH_SIN[index];
  return [vec(c, 0, -s), vec(0, 1, 0), vec(s, 0, c)];
}

/** Euler rotation in degrees, applied X then Y then Z, as a model part is authored. */
export function rotationXYZ(degrees: Vec3): Mat3 {
  const [rx, ry, rz] = degrees.map(toRadians) as unknown as Vec3;
  const [cx, sx] = [cosOf(rx, degrees[0]), sinOf(rx, degrees[0])];
  const [cy, sy] = [cosOf(ry, degrees[1]), sinOf(ry, degrees[1])];
  const [cz, sz] = [cosOf(rz, degrees[2]), sinOf(rz, degrees[2])];

  const x: Mat3 = [vec(1, 0, 0), vec(0, cx, sx), vec(0, -sx, cx)];
  const y: Mat3 = [vec(cy, 0, -sy), vec(0, 1, 0), vec(sy, 0, cy)];
  const z: Mat3 = [vec(cz, sz, 0), vec(-sz, cz, 0), vec(0, 0, 1)];
  return multiply(z, multiply(y, x));
}

function toRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

/**
 * Cosine, exact on the quarter turns.
 *
 * Right angles are overwhelmingly the common case in authored parts, and
 * `Math.cos(Math.PI / 2)` is 6.1e-17 rather than zero — enough to tilt a face
 * by a fraction of a pixel and move which side of a threshold its shading lands
 * on. Taking the table for whole quarter turns keeps those exact and leaves
 * every other angle to the library.
 */
function cosOf(radians: number, degrees: number): number {
  const quarter = degrees / 90;
  return Number.isInteger(quarter) ? EIGHTH_COS[(((quarter * 2) % 8) + 8) % 8] : Math.cos(radians);
}

function sinOf(radians: number, degrees: number): number {
  const quarter = degrees / 90;
  return Number.isInteger(quarter) ? EIGHTH_SIN[(((quarter * 2) % 8) + 8) % 8] : Math.sin(radians);
}
