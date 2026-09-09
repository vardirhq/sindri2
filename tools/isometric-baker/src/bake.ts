/**
 * One recipe in, the files a Sindri project needs out.
 *
 * Nothing here touches the filesystem: a bake is a pure function from a recipe
 * to a list of files, which is what lets the regression test bake twice and
 * compare, and what will let the editor preview a bake it has not written yet.
 */

import { type BakedFrame, type Canvas, bakeFrames, measureCanvas, paletteFor } from './frames.ts';
import { type IsoCamera, createCamera } from './iso.ts';
import { type Mesh, buildMesh } from './model.ts';
import { encodePng } from './png.ts';
import { uniqueColours } from './palette.ts';
import { usedColours } from './postprocess.ts';
import { buildPrefab } from './prefab.ts';
import { type Recipe } from './recipe.ts';
import { type PackedSheet, packSheet, spriteScale, tileRatioMismatch } from './sheet.ts';

/** The suffix that turns a texture's ID into its sheet's ID, as `sindri-core` does. */
export function sheetIdFor(texture: string): string {
  const dot = texture.lastIndexOf('.');
  const stem = dot < 0 ? texture : texture.slice(0, dot);
  return `${stem}.sheet.json`;
}

export interface BakeOutput {
  /** Path relative to the project's asset root, and to the bake's output directory. */
  path: string;
  contents: Buffer;
}

export interface FrameReport {
  direction: string;
  /** Pixels the drawn content occupies, as `width x height`. */
  content: string;
  /** Smallest gap between the content and the canvas edge, in pixels. */
  margin: number;
}

export interface BakeReport {
  id: string;
  canvas: Canvas;
  /** The anchor: the floor centre of tile (0,0), at the exact centre of every frame. */
  anchor: [number, number];
  pixelsPerUnit: number;
  /** Transform scale for a Sindri world sprite drawing one frame. */
  spriteScale: [number, number];
  worldUnitsPerPixel: number;
  palette: string[];
  frames: FrameReport[];
  warnings: string[];
}

export interface BakeResult {
  camera: IsoCamera;
  mesh: Mesh;
  canvas: Canvas;
  frames: BakedFrame[];
  sheet: PackedSheet;
  files: BakeOutput[];
  report: BakeReport;
}

export function bake(recipe: Recipe): BakeResult {
  const camera = createCamera(recipe.tile);
  const mesh = buildMesh(recipe.model);
  const canvas = measureCanvas(
    mesh,
    recipe.footprint,
    recipe.directions,
    camera,
    recipe.render.padding,
  );

  const frames = bakeFrames({
    mesh,
    footprint: recipe.footprint,
    facing: recipe.facing,
    count: recipe.directions,
    camera,
    canvas,
    config: recipe.render,
  });

  const sheet = packSheet(frames);
  const files: BakeOutput[] = [
    { path: recipe.texture, contents: encodePng(sheet.image) },
    { path: sheetIdFor(recipe.texture), contents: Buffer.from(toJson(sheet.document)) },
  ];

  const result: BakeResult = {
    camera,
    mesh,
    canvas,
    frames,
    sheet,
    files,
    report: report(recipe, camera, canvas, frames, mesh),
  };

  // Last, because a prefab is written from the finished measurements: the
  // canvas decides the sprite's scale, and the canvas is not known until every
  // direction has been measured.
  if (recipe.prefab) {
    files.push({
      path: recipe.prefab.path,
      contents: Buffer.from(buildPrefab(recipe, result, recipe.prefab)),
    });
  }

  return result;
}

/**
 * JSON as a Sindri document is written: two-space indent, one trailing newline.
 *
 * Stated here rather than left to a caller because a bake's output is compared
 * byte-for-byte, and whitespace is part of the bytes.
 */
export function toJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function report(
  recipe: Recipe,
  camera: IsoCamera,
  canvas: Canvas,
  frames: BakedFrame[],
  mesh: Mesh,
): BakeReport {
  const { scale, worldUnitsPerPixel } = spriteScale(canvas, camera, recipe.tileWorld);
  const warnings: string[] = [];

  const mismatch = tileRatioMismatch(camera, recipe.tileWorld);
  if (mismatch > 1e-6) {
    warnings.push(
      `the bake's ${recipe.tile.width}x${recipe.tile.height} tile is not the shape of the ` +
        `${recipe.tileWorld.width}x${recipe.tileWorld.height} world tile it is meant to stand on; ` +
        'the sprite will not sit flat on that tilemap',
    );
  }

  const frameReports = frames.map((frame) => {
    if (!frame.content) {
      warnings.push(`frame ${frame.direction} drew nothing`);
      return { direction: frame.direction, content: '0x0', margin: 0 };
    }
    const margin = Math.min(
      frame.content.x,
      frame.content.y,
      canvas.width - (frame.content.x + frame.content.width),
      canvas.height - (frame.content.y + frame.content.height),
    );
    if (margin <= 0) {
      warnings.push(
        `frame ${frame.direction} touches the canvas edge; raise render.padding or the ` +
          'silhouette will be clipped',
      );
    }
    return {
      direction: frame.direction,
      content: `${frame.content.width}x${frame.content.height}`,
      margin,
    };
  });

  // What the palette promises: every pixel that came out was a colour the
  // recipe declared going in. A colour outside it means the snap step let
  // something through, which is worth saying out loud.
  const declared = paletteFor(mesh, recipe.render);
  const used = uniqueColours(frames.flatMap((frame) => usedColours(frame.image)));
  for (const colour of used) {
    if (!declared.includes(colour)) {
      warnings.push(`${colour} is in the output but not in the declared palette`);
    }
  }

  return {
    id: recipe.id,
    canvas,
    anchor: [canvas.width / 2, canvas.height / 2],
    pixelsPerUnit: camera.pixelsPerUnit,
    spriteScale: scale,
    worldUnitsPerPixel,
    palette: used,
    frames: frameReports,
    warnings,
  };
}
