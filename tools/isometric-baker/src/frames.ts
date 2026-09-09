/**
 * Rendering every direction of an asset onto one shared canvas.
 *
 * This is where Sindri deliberately parts company with IsoGame. There, each
 * orientation is cropped to its own content and the sheet records an
 * `anchorX`/`anchorY` per frame, because IsoGame's own renderer draws a frame at
 * `(screenX - anchorX, screenY - anchorY)`.
 *
 * Sindri's sprite sheet stores frame rectangles and nothing else
 * (`crates/sindri-core/src/sheet.rs`), and a world sprite is *centred* on its
 * transform (`crates/sindri-scene/src/extract/sprite.rs`). Copying cropped
 * frames in would make each direction a different size around the same centre,
 * so an asset would slide around its tile as it turned.
 *
 * So frames are not cropped. Every direction is rendered onto one canvas, sized
 * symmetrically about the anchor and large enough for the widest rotation, and
 * the anchor therefore lands on the exact centre of every frame. A centred quad
 * of the right size then puts the floor of the model on the tile, in all four
 * directions, with the sheet format Sindri already has.
 *
 * The cost is transparent atlas space. The gain is that this vertical slice
 * needs no engine change at all; per-frame pivots can be an engine feature
 * later, argued on their own merits.
 */

import { type Direction, type DirectionCount, framesFor, strideFor } from './directions.ts';
import { type IsoCamera, originTileCentre, projectToPixels } from './iso.ts';
import { type RgbaImage } from './image.ts';
import { type Mesh, boundsOf } from './model.ts';
import {
  type ContentBounds,
  addInnerOutline,
  contentBounds,
  downsample,
  snapToPalette,
  thresholdAlpha,
} from './postprocess.ts';
import { rasterise } from './raster.ts';
import { type ShadingConfig } from './shading.ts';
import { uniqueColours } from './palette.ts';
import { rotationY, vec } from './vec.ts';

export interface Footprint {
  /** Tiles along +X at the authored rotation. */
  width: number;
  /** Tiles along +Z at the authored rotation. */
  height: number;
}

export interface FrameConfig {
  /** Render scale before downsampling. 1 is raw and jaggy; 4 is the sweet spot. */
  supersample: number;
  /** Transparent margin kept around the model, in final pixels. */
  padding: number;
  outline: { enabled: boolean; colour: string };
  /** Snap colours back onto the material ramps after downsampling. */
  paletteSnap: boolean;
  /** Extra colours the snap step may use, on top of the material ramps. */
  extraPalette: string[];
  /** Coverage a pixel needs to survive alpha thresholding, 0-255. */
  alphaCutoff: number;
  shading: ShadingConfig;
}

export interface BakedFrame {
  direction: Direction;
  /** Rotation step, counted from the authored facing. */
  index: number;
  image: RgbaImage;
  /** Footprint after this rotation, in tiles. */
  footprint: Footprint;
  /** Where the drawn pixels sit inside the canvas, or null for an empty frame. */
  content: ContentBounds | null;
}

export interface Canvas {
  width: number;
  height: number;
}

/**
 * Footprint after a rotation of `eighths` eighth-turns.
 *
 * Odd quarter turns swap the axes. A diagonal orientation is not a quarter turn
 * at all and has no footprint on a square grid, so it keeps the authored one —
 * which is why a recipe asking for eight directions must declare a square
 * footprint (`recipe.ts` refuses the rest rather than guessing).
 */
export function rotateFootprint(footprint: Footprint, eighths: number): Footprint {
  const quarters = eighths / 2;
  if (!Number.isInteger(quarters) || quarters % 2 === 0) {
    return { width: footprint.width, height: footprint.height };
  }
  return { width: footprint.height, height: footprint.width };
}

/**
 * A canvas big enough for every rotation, sized symmetrically about the anchor.
 *
 * Symmetric is the load-bearing word: it is what puts the anchor on the exact
 * centre, which is what lets a centred Sindri sprite stand on its tile without
 * any per-frame pivot to carry.
 */
export function measureCanvas(
  mesh: Mesh,
  footprint: Footprint,
  count: DirectionCount,
  camera: IsoCamera,
  padding: number,
): Canvas {
  let maxX = 0;
  let maxY = 0;
  const stride = strideFor(count);

  for (let index = 0; index < count; index++) {
    const rotation = rotationY(index * stride);
    const box = boundsOf(mesh, rotation);
    const rotated = rotateFootprint(footprint, index * stride);
    const origin = originTileCentre(rotated.width, rotated.height);

    for (const x of [box.min[0], box.max[0]]) {
      for (const y of [box.min[1], box.max[1]]) {
        for (const z of [box.min[2], box.max[2]]) {
          const point = projectToPixels(camera, vec(x, y, z), origin);
          maxX = Math.max(maxX, Math.abs(point.x));
          maxY = Math.max(maxY, Math.abs(point.y));
        }
      }
    }
  }

  return {
    width: 2 * ceilPixels(maxX + padding),
    height: 2 * ceilPixels(maxY + padding),
  };
}

/**
 * A pixel count is rounded up, but not for a rounding error.
 *
 * A model that measures exactly one tile across projects to 32.000000000000007
 * pixels rather than 32, because the projection multiplies two irrational
 * factors that only cancel in exact arithmetic. Rounding that up would give
 * every exactly-fitted asset two pixels of canvas it does not use, and the
 * canvas is what a sprite's world scale is derived from — so the error would
 * end up visible in a scene rather than staying in the arithmetic.
 */
const PIXEL_EPSILON = 1e-6;

function ceilPixels(value: number): number {
  return Math.ceil(value - PIXEL_EPSILON);
}

export interface FrameRequest {
  mesh: Mesh;
  footprint: Footprint;
  facing: Direction;
  count: DirectionCount;
  camera: IsoCamera;
  canvas: Canvas;
  config: FrameConfig;
}

/**
 * The palette a bake may snap to: the model's ramps, plus anything declared.
 *
 * Normalised and deduplicated, because the ramps come out of the colour maths
 * in one spelling and the declared colours come out of a hand-written recipe in
 * whatever spelling someone typed. The order is what breaks a tie when a
 * blended edge pixel is equally close to two entries, so it has to be stated
 * once rather than depending on which of two spellings of the same colour a
 * recipe used.
 */
export function paletteFor(mesh: Mesh, config: FrameConfig): string[] {
  const outline = config.outline.enabled ? [config.outline.colour] : [];
  return uniqueColours([...mesh.palette, ...config.extraPalette, ...outline]);
}

/** Render every direction, in rotation order. */
export function bakeFrames(request: FrameRequest): BakedFrame[] {
  const { mesh, footprint, facing, count, camera, canvas, config } = request;
  const palette = paletteFor(mesh, config);
  const stride = strideFor(count);
  const names = framesFor(facing, count);

  return names.map((direction, index) => {
    const rotated = rotateFootprint(footprint, index * stride);
    let image = rasterise({
      mesh,
      rotation: rotationY(index * stride),
      camera,
      width: canvas.width,
      height: canvas.height,
      supersample: config.supersample,
      origin: originTileCentre(rotated.width, rotated.height),
      shading: config.shading,
    });

    image = downsample(image, config.supersample);
    image = thresholdAlpha(image, config.alphaCutoff);
    if (config.paletteSnap) image = snapToPalette(image, palette);
    if (config.outline.enabled) image = addInnerOutline(image, config.outline.colour);

    return { direction, index, image, footprint: rotated, content: contentBounds(image) };
  });
}
