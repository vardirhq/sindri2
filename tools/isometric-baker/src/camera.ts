/**
 * The camera geometry every frame of a bake is rendered through.
 *
 * Three views, one basis. Whatever the view, a camera is an orthographic rig —
 * a scale in pixels per world unit and three orthonormal axes — so everything
 * downstream (the rasteriser, the canvas fitting, the packing, the palette)
 * works the same for all of them and knows about none of them.
 *
 * **Isometric** is adapted from IsoGame's `tools/sprite-factory/src/iso.ts`
 * (MIT). The maths is the same; what changed is that nothing here calls a
 * trigonometric function.
 *
 * Everything about it follows from one fact: a 1x1 floor tile is drawn as a
 * diamond `tileWidth` pixels across and `tileHeight` pixels tall. That ratio
 * *is* the camera's pitch, so the pitch never has to be stated separately:
 *
 *   sin(elevation) = tileHeight / tileWidth
 *   cos(elevation) = sqrt(1 - sin^2)
 *
 * Deriving the basis from the ratio rather than from `asin` and `sin` of it
 * matters here in a way it does not in a browser: baked PNGs are compared
 * pixel-for-pixel by a regression test, and `Math.sin` is only specified to be
 * "an implementation-approximation", so its last bit may move between engine
 * versions. Square roots are exactly specified, so this does not.
 *
 * **Top-down** and **side** are the flat views, and they are flat in the exact
 * sense that they have no pitch to derive. A top-down camera looks straight
 * down and a side camera looks straight along an axis, so neither draws a floor
 * tile as a diamond — a top-down tile is a square and a side-on one is a line.
 * That is why they cannot be reached by choosing a tile ratio, and why they
 * state their scale directly instead: `pixelsPerUnit` is the whole of it.
 *
 * Axis mapping, which is IsoGame's and is kept so ported code still reads:
 *
 *   grid column  ->  +X
 *   grid row     ->  +Z
 *   height       ->  +Y   (projects straight up the screen in every view but
 *                          top-down, where height projects to nothing)
 */

import { type Vec3, cross, dot, sub, vec } from './vec.ts';

/** Sindri bakes 2:1 tiles by default, the ratio Gather's tilemap already uses. */
export const DEFAULT_TILE = { width: 64, height: 32 } as const;

/** Which way a bake is looked at. */
export type View = 'isometric' | 'top-down' | 'side';

export const VIEWS: View[] = ['isometric', 'top-down', 'side'];

export function isView(value: string): value is View {
  return (VIEWS as string[]).includes(value);
}

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
 * disagree with another's.
 */
export interface Camera {
  view: View;
  /**
   * The tile this camera's pitch was derived from, for an isometric bake.
   *
   * `null` for a flat view, which has no tile: it is what says a bake has no
   * relationship to a tilemap, rather than a relationship nobody checked.
   */
  tile: TileSize | null;
  /** Screen pixels per world unit. Uniform in both axes under orthographic. */
  pixelsPerUnit: number;
  /** Unit vector from the scene toward the camera. */
  direction: Vec3;
  /** World direction that is screen +x. */
  right: Vec3;
  /** World direction that is screen -y (up the screen). */
  up: Vec3;
}

/** The isometric rig, whose pitch is the tile's own ratio. */
export function createCamera(tile: TileSize = DEFAULT_TILE): Camera {
  if (!(tile.width > 0) || !(tile.height > 0)) {
    throw new Error(`tile size must be positive, got ${tile.width}x${tile.height}`);
  }
  if (tile.height >= tile.width) {
    throw new Error(
      `a tile must be wider than it is tall (got ${tile.width}x${tile.height}); ` +
        'a square or tall tile has no isometric pitch to derive. ' +
        'For a view with no pitch at all, bake "view": "top-down" or "side"',
    );
  }

  const sinE = tile.height / tile.width;
  const cosE = Math.sqrt(1 - sinE * sinE);
  const half = Math.SQRT1_2;

  return {
    view: 'isometric',
    tile,
    // The tile's *diagonal* measures `tileWidth` pixels, so one unit — one tile
    // edge — is `tileWidth / sqrt(2)` pixels long.
    pixelsPerUnit: tile.width * half,
    direction: vec(cosE * half, sinE, cosE * half),
    right: vec(half, 0, -half),
    up: vec(-sinE * half, cosE, -sinE * half),
  };
}

/**
 * A flat rig: straight down, or straight along an axis.
 *
 * Both are axis-aligned, which is the whole difference from the isometric one.
 * There is no diagonal and no foreshortening, so a world unit is
 * `pixelsPerUnit` pixels in both screen axes and nothing has to be derived.
 */
export function createFlatCamera(view: 'top-down' | 'side', pixelsPerUnit: number): Camera {
  if (!(pixelsPerUnit > 0) || !Number.isFinite(pixelsPerUnit)) {
    throw new Error(`pixels per unit must be a positive number, got ${pixelsPerUnit}`);
  }
  const axes =
    view === 'top-down'
      ? {
          // Looking straight down: the ground plane fills the screen, and a
          // model's height projects to nothing. +Z runs down the screen so a
          // grid row still reads downward, as it does isometrically.
          direction: vec(0, 1, 0),
          right: vec(1, 0, 0),
          up: vec(0, 0, -1),
        }
      : {
          // Looking straight along +Z at the XY plane: height is up the screen
          // and depth projects to nothing.
          direction: vec(0, 0, 1),
          right: vec(1, 0, 0),
          up: vec(0, 1, 0),
        };
  return { view, tile: null, pixelsPerUnit, ...axes };
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
export function projectToPixels(camera: Camera, point: Vec3, origin: Vec3): PixelPoint {
  const d = sub(point, origin);
  return {
    x: dot(d, camera.right) * camera.pixelsPerUnit,
    y: -dot(d, camera.up) * camera.pixelsPerUnit,
  };
}

/** How far along the view axis a point is. Larger is nearer the camera. */
export function depthOf(camera: Camera, point: Vec3): number {
  return dot(point, camera.direction);
}

/**
 * World-space centre of footprint tile (0, 0), for a model authored centred on
 * its own footprint.
 *
 * This is the point a baked sprite is anchored to: the floor under the middle
 * of the first tile the thing stands on. A flat view anchors to the same place
 * for the same reason — it is the model's own centre on the ground — so the
 * canvas stays symmetric about it and the sheet's `center` anchor stays true.
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
export function basisError(camera: Camera): number {
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
