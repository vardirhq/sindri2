/**
 * Isometric camera geometry.
 *
 * Adapted from IsoGame's `tools/sprite-factory/src/iso.ts` (MIT). The maths is
 * the same; what changed is that nothing here calls a trigonometric function.
 *
 * Everything follows from one fact: a 1x1 floor tile is drawn as a diamond
 * `tileWidth` pixels across and `tileHeight` pixels tall. That ratio *is* the
 * camera's pitch, so the pitch never has to be stated separately:
 *
 *   sin(elevation) = tileHeight / tileWidth
 *   cos(elevation) = sqrt(1 - sin^2)
 *
 * Deriving the basis from the ratio rather than from `asin` and `sin` of it
 * matters here in a way it does not in a browser: baked PNGs are compared
 * byte-for-byte by a regression test, and `Math.sin` is only specified to be
 * "an implementation-approximation", so its last bit may move between engine
 * versions. Square roots are exactly specified, so this does not.
 *
 * Axis mapping, which is IsoGame's and is kept so ported code still reads:
 *
 *   grid column  ->  +X   (projects down-right on screen)
 *   grid row     ->  +Z   (projects down-left on screen)
 *   height       ->  +Y   (projects straight up on screen)
 *
 * One world unit is one tile edge. The tile's *diagonal* is what measures
 * `tileWidth` pixels, so one unit is `tileWidth / sqrt(2)` pixels long.
 */

import { type Vec3, cross, dot, sub, vec } from './vec.ts';

/** Sindri bakes 2:1 tiles by default, the ratio Gather's tilemap already uses. */
export const DEFAULT_TILE = { width: 64, height: 32 } as const;

export interface TileSize {
  /** Pixels across the diamond's long axis. */
  width: number;
  /** Pixels down its short axis. */
  height: number;
}

/**
 * The fixed rig every frame of a bake is rendered through.
 *
 * Orthographic, so there is no perspective to make one direction's frame
 * disagree with another's, and a pure function of the tile size.
 */
export interface IsoCamera {
  tile: TileSize;
  /** Screen pixels per world unit. Uniform in both axes under orthographic. */
  pixelsPerUnit: number;
  /** Unit vector from the scene toward the camera. */
  direction: Vec3;
  /** World direction that is screen +x. */
  right: Vec3;
  /** World direction that is screen -y (up the screen). */
  up: Vec3;
}

export function createCamera(tile: TileSize = DEFAULT_TILE): IsoCamera {
  if (!(tile.width > 0) || !(tile.height > 0)) {
    throw new Error(`tile size must be positive, got ${tile.width}x${tile.height}`);
  }
  if (tile.height >= tile.width) {
    throw new Error(
      `a tile must be wider than it is tall (got ${tile.width}x${tile.height}); ` +
        'a square or tall tile has no isometric pitch to derive',
    );
  }

  const sinE = tile.height / tile.width;
  const cosE = Math.sqrt(1 - sinE * sinE);
  const half = Math.SQRT1_2;

  return {
    tile,
    pixelsPerUnit: tile.width * half,
    direction: vec(cosE * half, sinE, cosE * half),
    right: vec(half, 0, -half),
    up: vec(-sinE * half, cosE, -sinE * half),
  };
}

export interface PixelPoint {
  x: number;
  y: number;
}

/**
 * Project a world point to pixels relative to `origin`.
 *
 * Canvas convention: +x is right, +y is *down*.
 */
export function projectToPixels(camera: IsoCamera, point: Vec3, origin: Vec3): PixelPoint {
  const d = sub(point, origin);
  return {
    x: dot(d, camera.right) * camera.pixelsPerUnit,
    y: -dot(d, camera.up) * camera.pixelsPerUnit,
  };
}

/** How far along the view axis a point is. Larger is nearer the camera. */
export function depthOf(camera: IsoCamera, point: Vec3): number {
  return dot(point, camera.direction);
}

/**
 * World-space centre of footprint tile (0, 0), for a model authored centred on
 * its own footprint.
 *
 * This is the point a baked sprite is anchored to: the floor under the middle
 * of the first tile the thing stands on.
 */
export function originTileCentre(width: number, height: number): Vec3 {
  return vec(-(width - 1) / 2, 0, -(height - 1) / 2);
}

/**
 * The camera basis as an orthonormal check, exported for tests.
 *
 * The three axes must be mutually perpendicular and unit length, or a bake's
 * pixels and its anchors are measured in different spaces.
 */
export function basisError(camera: IsoCamera): number {
  const axes = [camera.right, camera.up, camera.direction];
  let worst = 0;
  for (let i = 0; i < axes.length; i++) {
    worst = Math.max(worst, Math.abs(dot(axes[i], axes[i]) - 1));
    for (let j = i + 1; j < axes.length; j++) {
      worst = Math.max(worst, Math.abs(dot(axes[i], axes[j])));
    }
  }
  // Right x up should point at the camera, not away from it.
  worst = Math.max(worst, 1 - dot(cross(camera.right, camera.up), camera.direction));
  return worst;
}
