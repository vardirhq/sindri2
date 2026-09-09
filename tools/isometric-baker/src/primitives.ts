/**
 * Primitive geometry, built as plain triangles.
 *
 * Ported from the geometry half of IsoGame's
 * `tools/sprite-factory/src/model.ts` (MIT), which asks Three.js for a
 * `BoxGeometry` or a `CylinderGeometry`. This tool has no Three.js, so the four
 * shapes it needs are generated here.
 *
 * The shapes are deliberately crude, and that is the pipeline's premise: geometry
 * is authored *around* the limits of a 64-pixel sprite, not shrunk down from a
 * detailed model and hoped over.
 *
 * Conventions, shared with the rest of the baker:
 *   - one unit is one tile edge
 *   - y = 0 is the floor
 *   - a part's `position` is the centre of its own shape
 */

import { type Vec3, cross, normalize, sub, vec } from './vec.ts';

/** A triangle soup with a normal per vertex, in the part's own space. */
export interface Geometry {
  /** Vertex positions, three numbers each. */
  positions: number[];
  /** Vertex normals, three numbers each, matching `positions`. */
  normals: number[];
  /** Triangle corners, three indices each. */
  indices: number[];
}

export function emptyGeometry(): Geometry {
  return { positions: [], normals: [], indices: [] };
}

function pushVertex(geometry: Geometry, position: Vec3, normal: Vec3): number {
  const index = geometry.positions.length / 3;
  geometry.positions.push(position[0], position[1], position[2]);
  geometry.normals.push(normal[0], normal[1], normal[2]);
  return index;
}

/**
 * Add one quad as two triangles, wound counter-clockwise when seen from the
 * side the normal points at.
 *
 * Winding is not used for culling — the baker keeps back faces and lets the
 * depth buffer decide, because a model made of separate primitives is not one
 * closed surface — but keeping it consistent means a normal derived from the
 * corners agrees with the one that was asked for.
 */
function pushQuad(geometry: Geometry, a: Vec3, b: Vec3, c: Vec3, d: Vec3, normal?: Vec3): void {
  const n = normal ?? normalize(cross(sub(b, a), sub(d, a)));
  const ia = pushVertex(geometry, a, n);
  const ib = pushVertex(geometry, b, n);
  const ic = pushVertex(geometry, c, n);
  const id = pushVertex(geometry, d, n);
  geometry.indices.push(ia, ib, ic, ia, ic, id);
}

/** An axis-aligned box centred on the origin. */
export function boxGeometry(size: Vec3): Geometry {
  const [x, y, z] = [size[0] / 2, size[1] / 2, size[2] / 2];
  const geometry = emptyGeometry();

  // +X, -X, +Y, -Y, +Z, -Z. Written out rather than looped: six faces spelled
  // once are easier to check than an axis-permutation trick used six times.
  pushQuad(geometry, vec(x, -y, z), vec(x, -y, -z), vec(x, y, -z), vec(x, y, z), vec(1, 0, 0));
  pushQuad(geometry, vec(-x, -y, -z), vec(-x, -y, z), vec(-x, y, z), vec(-x, y, -z), vec(-1, 0, 0));
  pushQuad(geometry, vec(-x, y, z), vec(x, y, z), vec(x, y, -z), vec(-x, y, -z), vec(0, 1, 0));
  pushQuad(geometry, vec(-x, -y, -z), vec(x, -y, -z), vec(x, -y, z), vec(-x, -y, z), vec(0, -1, 0));
  pushQuad(geometry, vec(-x, -y, z), vec(x, -y, z), vec(x, y, z), vec(-x, y, z), vec(0, 0, 1));
  pushQuad(geometry, vec(x, -y, -z), vec(-x, -y, -z), vec(-x, y, -z), vec(x, y, -z), vec(0, 0, -1));

  return geometry;
}

export interface CylinderOptions {
  radiusTop: number;
  radiusBottom: number;
  height: number;
  segments: number;
}

/**
 * A cylinder or cone, centred on the origin, standing along +Y.
 *
 * Side normals point straight out from the axis rather than being tilted for a
 * taper. On a four-band material the difference is at most one band on a very
 * steep cone, and a radial normal is the one that makes a cylinder terrace in
 * even vertical stripes — which is the look being aimed at.
 */
export function cylinderGeometry(options: CylinderOptions): Geometry {
  const { radiusTop, radiusBottom, height, segments } = options;
  const count = Math.max(3, Math.round(segments));
  const geometry = emptyGeometry();
  const halfHeight = height / 2;

  const ring = (index: number) => {
    const angle = (index / count) * Math.PI * 2;
    return { cos: Math.cos(angle), sin: Math.sin(angle) };
  };

  for (let i = 0; i < count; i++) {
    const a = ring(i);
    const b = ring(i + 1);
    const na = vec(a.cos, 0, a.sin);
    const nb = vec(b.cos, 0, b.sin);

    const bottomA = vec(a.cos * radiusBottom, -halfHeight, a.sin * radiusBottom);
    const bottomB = vec(b.cos * radiusBottom, -halfHeight, b.sin * radiusBottom);
    const topB = vec(b.cos * radiusTop, halfHeight, b.sin * radiusTop);
    const topA = vec(a.cos * radiusTop, halfHeight, a.sin * radiusTop);

    // A cone's top ring is a single point, so the quad degenerates to a
    // triangle. Emitting it as a quad anyway costs one zero-area triangle and
    // keeps the loop one case rather than three.
    const ia = pushVertex(geometry, bottomA, na);
    const ib = pushVertex(geometry, bottomB, nb);
    const ic = pushVertex(geometry, topB, nb);
    const id = pushVertex(geometry, topA, na);
    geometry.indices.push(ia, ib, ic, ia, ic, id);

    if (radiusTop > 0) {
      const centre = pushVertex(geometry, vec(0, halfHeight, 0), vec(0, 1, 0));
      const ta = pushVertex(geometry, topA, vec(0, 1, 0));
      const tb = pushVertex(geometry, topB, vec(0, 1, 0));
      geometry.indices.push(centre, ta, tb);
    }
    if (radiusBottom > 0) {
      const centre = pushVertex(geometry, vec(0, -halfHeight, 0), vec(0, -1, 0));
      const ba = pushVertex(geometry, bottomB, vec(0, -1, 0));
      const bb = pushVertex(geometry, bottomA, vec(0, -1, 0));
      geometry.indices.push(centre, ba, bb);
    }
  }

  return geometry;
}

export interface SphereOptions {
  radius: number;
  segments: number;
  /** Non-uniform squash, applied to positions after the sphere is built. */
  scale?: Vec3;
}

/** A UV sphere centred on the origin. */
export function sphereGeometry(options: SphereOptions): Geometry {
  const width = Math.max(3, Math.round(options.segments));
  const height = Math.max(2, Math.round(width / 2));
  const geometry = emptyGeometry();
  const grid: number[][] = [];

  for (let iy = 0; iy <= height; iy++) {
    const row: number[] = [];
    const v = iy / height;
    const phi = v * Math.PI;
    for (let ix = 0; ix <= width; ix++) {
      const u = ix / width;
      const theta = u * Math.PI * 2;
      const normal = vec(
        -Math.cos(theta) * Math.sin(phi),
        Math.cos(phi),
        Math.sin(theta) * Math.sin(phi),
      );
      // Squashing moves the surface but not the normal. For the shallow squashes
      // this is used for — a dome, a river stone — the band it lands on is the
      // same, and an exactly-derived normal would need the inverse transpose for
      // no visible gain at four bands.
      const scaled = options.scale ?? vec(1, 1, 1);
      const position = vec(
        normal[0] * options.radius * scaled[0],
        normal[1] * options.radius * scaled[1],
        normal[2] * options.radius * scaled[2],
      );
      row.push(pushVertex(geometry, position, normal));
    }
    grid.push(row);
  }

  for (let iy = 0; iy < height; iy++) {
    for (let ix = 0; ix < width; ix++) {
      const a = grid[iy][ix + 1];
      const b = grid[iy][ix];
      const c = grid[iy + 1][ix];
      const d = grid[iy + 1][ix + 1];
      if (iy !== 0) geometry.indices.push(a, b, d);
      if (iy !== height - 1) geometry.indices.push(b, c, d);
    }
  }

  return geometry;
}
