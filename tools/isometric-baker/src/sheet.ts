/**
 * Packing baked frames into one texture, and describing it the way Sindri does.
 *
 * Sindri slices an image with a document beside it: `textures/shrine.png` is cut
 * by `textures/shrine.sheet.json`, and the rule lives in
 * `crates/sindri-core/src/sheet.rs`. Note that the suffix *replaces* the
 * extension rather than following it — a sheet for `shrine.png` is
 * `shrine.sheet.json`, not `shrine.png.sheet.json`.
 */

import { type DirectionCount } from './directions.ts';
import { type BakedFrame } from './frames.ts';
import { type RgbaImage, createImage } from './image.ts';
import { type Camera } from './camera.ts';
import { type Mesh, boundsOf } from './model.ts';

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
   * middle of the picture *is* the point that stands on the tile.
   */
  anchor: 'center' | 'bottom' | [number, number];
  /** How far tile art hangs below its logical diamond, in tile-height units. */
  tile_overhang_ratio?: number;
  grid: SheetGrid;
}

export interface PackedSheet {
  image: RgbaImage;
  document: SpriteSheetDocument;
}

/** What a frame is called on the sheet. */
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
 * neighbour is then blended with it.
 */
const GUTTER = 2;

/**
 * How much of an isometric model genuinely hangs below the floor it stands on.
 *
 * The model format has a useful contract here: y=0 is the floor. That lets the
 * baker distinguish a slab side from a flower, tuft, tree, or any other detail
 * that rises above the tile. Measuring the final alpha bounds cannot make that
 * distinction and would turn tall decoration into fake floor thickness.
 *
 * The result is expressed relative to the logical tile's screen height so the
 * sheet remains independent of whichever world-space tile size later draws it.
 */
export function tileOverhangRatio(meshes: Mesh[], camera: Camera): number | undefined {
  if (camera.tile === null || meshes.length === 0) return undefined;

  let belowFloor = 0;
  for (const mesh of meshes) {
    belowFloor = Math.max(belowFloor, Math.max(0, -boundsOf(mesh).min[1]));
  }
  if (belowFloor <= 0) return undefined;

  // Height projects along the camera's screen-up axis. Going below y=0 moves
  // the same distance downward, so only the Y contribution of that basis is
  // relevant. X/Z are already represented by the logical diamond itself.
  const pixels = belowFloor * camera.up[1] * camera.pixelsPerUnit;
  const ratio = pixels / camera.tile.height;
  return ratio > 0 ? ratio : undefined;
}

/** Lay the frames out as one horizontal strip. */
export function packSheet(
  frames: BakedFrame[],
  count: DirectionCount = 4,
  gutter: number = GUTTER,
  tileOverhang?: number,
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
  const sheet = createImage(frames.length * (width + 2 * gutter), height + 2 * gutter);

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

  const document: SpriteSheetDocument = {
    format_version: SHEET_FORMAT_VERSION,
    anchor: 'center',
    grid,
  };
  if (tileOverhang !== undefined) document.tile_overhang_ratio = tileOverhang;
  return { image: sheet, document };
}

/**
 * Copy `frame` to (`left`, `top`) and repeat its edge pixels `gutter` deep
 * around it.
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

/** The transform scale a Sindri world sprite needs for one baked frame. */
export function spriteScale(
  canvas: { width: number; height: number },
  camera: Camera,
  tile: TileWorldSize | null,
): { scale: [number, number]; worldUnitsPerPixel: number } {
  const worldUnitsPerPixel =
    tile === null || camera.tile === null ? 1 / camera.pixelsPerUnit : tile.width / camera.tile.width;
  return {
    scale: [canvas.width * worldUnitsPerPixel, canvas.height * worldUnitsPerPixel],
    worldUnitsPerPixel,
  };
}

/** How far a tilemap's tile shape is from the one the bake assumed. */
export function tileRatioMismatch(camera: Camera, tile: TileWorldSize | null): number {
  if (tile === null || camera.tile === null) return 0;
  const baked = camera.tile.height / camera.tile.width;
  const scene = tile.height / tile.width;
  return Math.abs(baked - scene);
}
