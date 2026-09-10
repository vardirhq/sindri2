/**
 * Packing baked frames into one texture, and describing it the way Sindri does.
 *
 * Sindri slices an image with a document beside it: `textures/shrine.png` is cut
 * by `textures/shrine.sheet.json`, and the rule lives in
 * `crates/sindri-core/src/sheet.rs`. Note that the suffix *replaces* the
 * extension rather than following it — a sheet for `shrine.png` is
 * `shrine.sheet.json`, not `shrine.png.sheet.json`.
 *
 * Because every frame of a bake is the same size, the sheet is a plain
 * edge-to-edge grid of one row: `SheetGrid::edge_to_edge`, with a name per cell.
 * A grid that divides an image edge to edge needs no recorded image size and no
 * gutters — the frames already carry a transparent margin, so there is nothing
 * for filtering to bleed in from.
 */

import { type DirectionCount } from './directions.ts';
import { type BakedFrame } from './frames.ts';
import { type RgbaImage, createImage } from './image.ts';
import { type Camera } from './camera.ts';

/** The version `SHEET_FORMAT_VERSION` in `sindri-core` currently writes. */
export const SHEET_FORMAT_VERSION = 1;

export interface SheetGrid {
  columns: number;
  rows: number;
  /** The image this grid was cut against. Required once there are gutters. */
  size?: [number, number];
  margin?: [number, number];
  spacing?: [number, number];
  names: string[];
}

export interface SpriteSheetDocument {
  format_version: number;
  /**
   * Where a frame meets the ground.
   *
   * Always the centre, because that is what every frame is padded to: the
   * canvas is grown symmetrically about the floor centre of tile (0,0) so the
   * middle of the picture *is* the point that stands on the tile. Written out
   * rather than left to the format's default so the sheet says what it is, and
   * so hand-drawn art sitting next to it is visibly making a choice.
   */
  anchor: 'center' | 'bottom' | [number, number];
  grid: SheetGrid;
}

export interface PackedSheet {
  image: RgbaImage;
  document: SpriteSheetDocument;
}

/**
 * What a frame is called on the sheet.
 *
 * A sheet of one model names its frames by direction, which is what a scene
 * saying `#north` wants. A sheet of several — a tile set — names them by model,
 * because "grass" and "path" are the names a tilemap palette is written in. A
 * sheet that is both names them by both, and nothing has to guess which half of
 * the name it is looking at.
 */
export function frameName(frame: BakedFrame, count: DirectionCount): string {
  if (frame.variant === null) return frame.direction;
  return count === 1 ? frame.variant : `${frame.variant}-${frame.direction}`;
}

/**
 * Pixels of gutter around every frame.
 *
 * Not zero, and this is the reason. `TextureFilter::Nearest` in
 * `crates/sindri-render/src/texture.rs` sets `mag_filter` to Nearest but leaves
 * `min_filter` Linear, so a sheet drawn at even slightly under its authored
 * size is sampled bilinearly — and a frame packed edge to edge against its
 * neighbour is then blended with it. On a sprite it is a faint rim; on a floor
 * tile, whose art fills its cell exactly, it is the tile beside it smeared
 * across every cell.
 *
 * The gutter is filled by extending each frame's own edge pixels outward, so
 * what the filter reaches for is the colour that was already there. A
 * transparent gutter would only trade a colour seam for a dark one.
 */
const GUTTER = 2;

/**
 * Lay the frames out as one horizontal strip.
 *
 * A strip and not a clever bin packer: the frames are all one size, four of them
 * is not a packing problem, and a strip stays readable when someone opens the
 * PNG to check the pipeline's work.
 */
export function packSheet(
  frames: BakedFrame[],
  count: DirectionCount = 4,
  gutter: number = GUTTER,
): PackedSheet {
  if (frames.length === 0) throw new Error('a sheet needs at least one frame');

  const { width, height } = frames[0].image;
  for (const frame of frames) {
    if (frame.image.width !== width || frame.image.height !== height) {
      throw new Error(
        `frame ${frame.direction} is ${frame.image.width}x${frame.image.height}, ` +
          `but the sheet is packed at ${width}x${height}; frames must be uniform`,
      );
    }
  }

  // A margin of one gutter and a spacing of two leaves every frame with exactly
  // `gutter` pixels of its own on all four sides, which is the arrangement
  // `SheetGrid` measures cells against.
  const sheet = createImage(
    frames.length * (width + 2 * gutter),
    height + 2 * gutter,
  );

  frames.forEach((frame, column) => {
    const left = gutter + column * (width + 2 * gutter);
    extrudeInto(sheet, frame.image, left, gutter, gutter);
  });

  const grid: SheetGrid = {
    columns: frames.length,
    rows: 1,
    names: frames.map((frame) => frameName(frame, count)),
  };
  if (gutter > 0) {
    grid.size = [sheet.width, sheet.height];
    grid.margin = [gutter, gutter];
    grid.spacing = [2 * gutter, 2 * gutter];
  }

  return { image: sheet, document: { format_version: SHEET_FORMAT_VERSION, anchor: 'center', grid } };
}

/**
 * Copy `frame` to (`left`, `top`) and repeat its edge pixels `gutter` deep
 * around it.
 *
 * Written as one clamped loop over the padded rectangle rather than a copy plus
 * four border passes: the corners are then the same case as the edges, and the
 * case that gets forgotten in the four-pass version is the corners.
 */
function extrudeInto(
  sheet: RgbaImage,
  frame: RgbaImage,
  left: number,
  top: number,
  gutter: number,
): void {
  for (let y = -gutter; y < frame.height + gutter; y++) {
    const sourceY = Math.min(Math.max(y, 0), frame.height - 1);
    for (let x = -gutter; x < frame.width + gutter; x++) {
      const sourceX = Math.min(Math.max(x, 0), frame.width - 1);
      const from = (sourceY * frame.width + sourceX) * 4;
      const to = ((top + y) * sheet.width + (left + x)) * 4;
      sheet.data[to] = frame.data[from];
      sheet.data[to + 1] = frame.data[from + 1];
      sheet.data[to + 2] = frame.data[from + 2];
      sheet.data[to + 3] = frame.data[from + 3];
    }
  }
}

export interface TileWorldSize {
  /** World units across the tile diamond, matching a tilemap's `tile_size[0]`. */
  width: number;
  /** World units down it, matching `tile_size[1]`. */
  height: number;
}

/**
 * The transform scale a Sindri world sprite needs to draw one frame at exactly
 * one baked pixel per intended pixel.
 *
 * A sprite is a unit quad centred on its transform
 * (`crates/sindri-render/src/sprite_batch/mod.rs`), so its scale *is* its size in
 * world units. The bake knows how many pixels one tile is across; a tilemap
 * knows how many world units it is across; the ratio converts between them.
 *
 * Uniform in both axes on purpose: the camera is orthographic, so a pixel is the
 * same size vertically as horizontally, and using the tile's height ratio for Y
 * would squash every baked sprite by the isometric foreshortening a second time.
 */
export function spriteScale(
  canvas: { width: number; height: number },
  camera: Camera,
  tile: TileWorldSize | null,
): { scale: [number, number]; worldUnitsPerPixel: number } {
  // An isometric bake is measured against the tilemap it stands on: so many
  // world units across the diamond, so many pixels across the diamond. A flat
  // view stands on nothing, so it measures itself — the camera's own scale is
  // the whole relationship between a baked pixel and a world unit.
  const worldUnitsPerPixel =
    tile === null || camera.tile === null ? 1 / camera.pixelsPerUnit : tile.width / camera.tile.width;
  return {
    scale: [canvas.width * worldUnitsPerPixel, canvas.height * worldUnitsPerPixel],
    worldUnitsPerPixel,
  };
}

/**
 * How far a tilemap's own tile shape is from the one the bake assumed.
 *
 * A tilemap drawing 1.1 x 0.55 world-unit tiles is a 2:1 diamond and matches a
 * 64x32 bake exactly. One drawing 1.1 x 0.6 does not, and a sprite baked for the
 * first will stand a little wrong on the second — worth reporting rather than
 * discovering in a capture.
 */
export function tileRatioMismatch(camera: Camera, tile: TileWorldSize | null): number {
  // A flat view claims no relationship to a tilemap, so there is none to be
  // wrong about.
  if (tile === null || camera.tile === null) return 0;
  const baked = camera.tile.height / camera.tile.width;
  const scene = tile.height / tile.width;
  return Math.abs(baked - scene);
}
