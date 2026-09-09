/**
 * Turning a render into a sprite.
 *
 * Ported from IsoGame's `tools/sprite-factory/src/postprocess.ts` (MIT), minus
 * the vertical flip — that exists there because WebGL reads a render target
 * back bottom-up, and this baker's rasteriser already writes top-down — and
 * minus the crop, which Sindri deliberately does not do (see `frames.ts`).
 *
 * The rasteriser draws at an integer multiple of the target size, then this
 * walks it down to pixel art:
 *
 *   1. downsample  - alpha-weighted box filter, so edge pixels average cleanly
 *   2. threshold   - alpha becomes fully on or fully off, no soft halo
 *   3. snap        - every pixel returns to an exact palette entry
 *   4. outline     - optional dark silhouette drawn *inside* the shape
 *
 * Steps 2 and 3 are what separate this from "a small 3D render". Supersampling
 * gives good decisions about *where* an edge is; thresholding and snapping then
 * throw away the blurry evidence of how it got there.
 */

import { type RgbaImage } from './image.ts';
import { type Rgb, hexToRgb, nearestColour } from './palette.ts';

/**
 * Box-filter downsample.
 *
 * Colour is averaged weighted by alpha, so fully transparent background pixels
 * cannot drag an edge colour toward black.
 */
export function downsample(image: RgbaImage, factor: number): RgbaImage {
  if (factor === 1) return image;

  const width = Math.floor(image.width / factor);
  const height = Math.floor(image.height / factor);
  const out = new Uint8ClampedArray(width * height * 4);
  const samples = factor * factor;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;

      for (let sy = 0; sy < factor; sy++) {
        for (let sx = 0; sx < factor; sx++) {
          const i = ((y * factor + sy) * image.width + (x * factor + sx)) * 4;
          const alpha = image.data[i + 3];
          r += image.data[i] * alpha;
          g += image.data[i + 1] * alpha;
          b += image.data[i + 2] * alpha;
          a += alpha;
        }
      }

      const o = (y * width + x) * 4;
      if (a > 0) {
        out[o] = r / a;
        out[o + 1] = g / a;
        out[o + 2] = b / a;
      }
      out[o + 3] = a / samples;
    }
  }

  return { data: out, width, height };
}

/** Alpha becomes binary. `cutoff` is the coverage a pixel needs to survive. */
export function thresholdAlpha(image: RgbaImage, cutoff = 128): RgbaImage {
  const data = new Uint8ClampedArray(image.data);
  for (let i = 3; i < data.length; i += 4) {
    if (data[i] >= cutoff) {
      data[i] = 255;
    } else {
      data[i - 3] = 0;
      data[i - 2] = 0;
      data[i - 1] = 0;
      data[i] = 0;
    }
  }
  return { ...image, data };
}

/** Force every opaque pixel onto the nearest entry of a fixed palette. */
export function snapToPalette(image: RgbaImage, palette: string[]): RgbaImage {
  const entries: Rgb[] = palette.map(hexToRgb);
  if (entries.length === 0) return image;

  const data = new Uint8ClampedArray(image.data);
  const memo = new Map<number, Rgb>();

  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;
    const key = (data[i] << 16) | (data[i + 1] << 8) | data[i + 2];
    let snapped = memo.get(key);
    if (!snapped) {
      snapped = nearestColour({ r: data[i], g: data[i + 1], b: data[i + 2] }, entries);
      memo.set(key, snapped);
    }
    data[i] = snapped.r;
    data[i + 1] = snapped.g;
    data[i + 2] = snapped.b;
  }

  return { ...image, data };
}

/**
 * Recolour the outermost ring of opaque pixels.
 *
 * Drawn inward rather than outward so the silhouette — and therefore the
 * footprint and the anchor — stay exactly where the rasteriser put them.
 */
export function addInnerOutline(image: RgbaImage, colour: string): RgbaImage {
  const { width, height } = image;
  const { r, g, b } = hexToRgb(colour);
  const data = new Uint8ClampedArray(image.data);

  const transparentAt = (x: number, y: number) => {
    if (x < 0 || y < 0 || x >= width || y >= height) return true;
    return image.data[(y * width + x) * 4 + 3] === 0;
  };

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      if (image.data[i + 3] === 0) continue;
      if (
        transparentAt(x - 1, y) ||
        transparentAt(x + 1, y) ||
        transparentAt(x, y - 1) ||
        transparentAt(x, y + 1)
      ) {
        data[i] = r;
        data[i + 1] = g;
        data[i + 2] = b;
      }
    }
  }

  return { ...image, data };
}

export interface ContentBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * The box the drawn pixels occupy, without trimming to it.
 *
 * IsoGame crops here and records where the anchor ended up. Sindri does not
 * crop — the reason is in `frames.ts` — but it still wants the measurement, to
 * report how much of the canvas an asset uses and to catch a model that has
 * grown past its padding.
 */
export function contentBounds(image: RgbaImage): ContentBounds | null {
  const { width, height, data } = image;
  let minX = width;
  let minY = height;
  let maxX = -1;
  let maxY = -1;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (data[(y * width + x) * 4 + 3] === 0) continue;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
  }

  if (maxX < 0) return null;
  return { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1 };
}

/** Every distinct opaque colour in an image, most used first. */
export function usedColours(image: RgbaImage): string[] {
  const counts = new Map<number, number>();
  for (let i = 0; i < image.data.length; i += 4) {
    if (image.data[i + 3] === 0) continue;
    const key = (image.data[i] << 16) | (image.data[i + 1] << 8) | image.data[i + 2];
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0] - b[0])
    .map(([key]) => `#${key.toString(16).padStart(6, '0')}`);
}
