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
import { type PackedSheet, frameName, packSheet, spriteScale, tileRatioMismatch } from './sheet.ts';

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
  /** One per variant, in sheet order. A lone model is the only entry. */
  meshes: Mesh[];
  canvas: Canvas;
  frames: BakedFrame[];
  sheet: PackedSheet;
  files: BakeOutput[];
  report: BakeReport;
}

/**
 * Refuses a model that stands on more ground than its footprint claims.
 *
 * A footprint is what the game reserves: collision, placement and the order
 * things draw in all read it, and all of them assume the picture stays inside
 * it. A model wider than its footprint therefore covers tiles it does not own,
 * and a character standing on one of those legally is drawn sliced by scenery
 * it is not touching. Height is free — a tree is meant to tower over its cell —
 * so only the ground is measured.
 *
 * Refused rather than widened for the author: the fix is either a smaller model
 * or a bigger footprint, and which one is right depends on what the thing is.
 */
function checkFootprint(meshes: Mesh[], recipe: Recipe): void {
  const axes = [
    { name: 'x', stride: 0, limit: recipe.footprint.width },
    { name: 'z', stride: 2, limit: recipe.footprint.height },
  ];
  for (const [index, mesh] of meshes.entries()) {
    for (const axis of axes) {
      let reach = 0;
      for (let at = axis.stride; at < mesh.positions.length; at += 3) {
        reach = Math.max(reach, Math.abs(mesh.positions[at]));
      }
      // Measured as a full width about the origin, because that is where the
      // footprint is centred and how a recipe states it.
      const extent = reach * 2;
      if (extent > axis.limit + 1e-6) {
        const name = recipe.variants[index].name ?? recipe.texture;
        throw new Error(
          `${name} stands ${extent.toFixed(3)} tiles across in ${axis.name}, ` +
            `but its footprint claims ${axis.limit}. Shrink the model or widen the footprint: ` +
            `a model wider than its footprint covers ground the game lets others stand on.`,
        );
      }
    }
  }
}

export function bake(recipe: Recipe): BakeResult {
  const camera = createCamera(recipe.tile);
  const meshes = recipe.variants.map((variant) => buildMesh(variant.model));
  checkFootprint(meshes, recipe);

  // One canvas and one palette across the whole sheet. The canvas because
  // frames must be uniform for the anchor to be the centre of each; the palette
  // because a blended edge pixel of one tile must not snap to a shade only the
  // tile beside it declared — two tiles meant to match would then not.
  const canvas = measureCanvas(
    meshes,
    recipe.footprint,
    recipe.directions,
    camera,
    recipe.render.padding,
  );
  const palette = uniqueColours(meshes.flatMap((mesh) => paletteFor(mesh, recipe.render)));

  const frames = meshes.flatMap((mesh, index) =>
    bakeFrames({
      mesh,
      variant: recipe.variants[index].name,
      footprint: recipe.footprint,
      facing: recipe.facing,
      count: recipe.directions,
      camera,
      canvas,
      config: recipe.render,
      palette,
    }),
  );

  const sheet = packSheet(frames, recipe.directions);
  const files: BakeOutput[] = [
    { path: recipe.texture, contents: encodePng(sheet.image) },
    { path: sheetIdFor(recipe.texture), contents: Buffer.from(toJson(sheet.document)) },
  ];

  const result: BakeResult = {
    camera,
    meshes,
    canvas,
    frames,
    sheet,
    files,
    report: report(recipe, camera, canvas, frames, palette),
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
  declared: string[],
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
      warnings.push(`frame ${frameName(frame, recipe.directions)} drew nothing`);
      return { direction: frameName(frame, recipe.directions), content: '0x0', margin: 0 };
    }
    const margin = Math.min(
      frame.content.x,
      frame.content.y,
      canvas.width - (frame.content.x + frame.content.width),
      canvas.height - (frame.content.y + frame.content.height),
    );
    // Only worth saying when a margin was asked for. A recipe with no padding
    // is one whose art is meant to reach the edge — a floor tile has to fill
    // its cell exactly, or the tilemap draws a seam between every pair.
    if (margin <= 0 && recipe.render.padding > 0) {
      warnings.push(
        `frame ${frameName(frame, recipe.directions)} touches the canvas edge; raise ` +
          'render.padding or the silhouette will be clipped',
      );
    }
    return {
      direction: frameName(frame, recipe.directions),
      content: `${frame.content.width}x${frame.content.height}`,
      margin,
    };
  });

  // What the palette promises: every pixel that came out was a colour the
  // recipe declared going in. A colour outside it means the snap step let
  // something through, which is worth saying out loud.
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
