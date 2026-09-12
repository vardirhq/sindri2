/**
 * One recipe in, the files a Sindri project needs out.
 *
 * Nothing here touches the filesystem: a bake is a pure function from a recipe
 * to a list of files, which is what lets the regression test bake twice and
 * compare, and what will let the editor preview a bake it has not written yet.
 */

import { type BakedFrame, type Canvas, bakeFrames, measureCanvas, paletteFor } from './frames.ts';
import { type Camera, createCamera, createFlatCamera } from './camera.ts';
import { type Mesh, buildMesh } from './model.ts';
import { encodePng } from './png.ts';
import { uniqueColours } from './palette.ts';
import { usedColours } from './postprocess.ts';
import { buildPrefab } from './prefab.ts';
import { type Recipe } from './recipe.ts';
import {
  type PackedSheet,
  frameName,
  packSheet,
  spriteScale,
  tileOverhangRatio,
  tileRatioMismatch,
} from './sheet.ts';

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
  camera: Camera;
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
 */
function checkFootprint(meshes: Mesh[], recipe: Recipe): void {
  if (recipe.view !== 'isometric') return;
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

/** The rig this recipe asked for. */
function cameraFor(recipe: Recipe): Camera {
  if (recipe.view === 'isometric') {
    return createCamera(recipe.tile ?? undefined);
  }
  return createFlatCamera(recipe.view, recipe.pixelsPerUnit ?? 0);
}

export function bake(recipe: Recipe): BakeResult {
  const camera = cameraFor(recipe);
  const meshes = recipe.variants.map((variant) => buildMesh(variant.model));
  checkFootprint(meshes, recipe);

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

  const sheet = packSheet(
    frames,
    recipe.directions,
    undefined,
    tileOverhangRatio(frames, camera),
  );
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
 */
export function toJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function report(
  recipe: Recipe,
  camera: Camera,
  canvas: Canvas,
  frames: BakedFrame[],
  declared: string[],
): BakeReport {
  const { scale, worldUnitsPerPixel } = spriteScale(canvas, camera, recipe.tileWorld);
  const warnings: string[] = [];

  const mismatch = tileRatioMismatch(camera, recipe.tileWorld);
  if (mismatch > 1e-6 && recipe.tile && recipe.tileWorld) {
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
