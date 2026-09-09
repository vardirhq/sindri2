/**
 * Turning a bake into a prefab a scene can use without any custom game code.
 *
 * A prefab is a scene fragment with exactly one root
 * (`crates/sindri-core/src/prefab/document.rs`), written in the same canonical
 * form a scene is. Generating one is what turns a bake from "two files and a
 * number you have to copy off a report" into an asset: the transform scale that
 * makes one baked pixel one intended pixel is arithmetic, and arithmetic is the
 * tool's job rather than the author's.
 *
 * What is deliberately *not* here is anything IsoGame-specific. Its generated
 * furniture definitions carry categories, sit and lay spots, stackability and a
 * collision model, because that is what its game needs from a chair. A Sindri
 * prefab gets what any renderer needs — a transform, a sprite, and optionally
 * which grid cells the thing stands on — and a game says the rest itself.
 */

import { type BakeReport, type BakeResult, sheetIdFor } from './bake.ts';
import { type JsonValue, f32, ordered, toCanonicalJson } from './canonical.ts';
import { type Direction } from './directions.ts';
import { type Recipe } from './recipe.ts';

/** The version `PREFAB_FORMAT_VERSION` in `sindri-core` currently writes. */
export const PREFAB_FORMAT_VERSION = 1;

/** The key the generation record is filed under, inside the entity's editor map. */
export const GENERATOR_KEY = 'sindri.isometric-baker';

/** The version of the generation record itself, which is not the prefab's. */
export const GENERATOR_FORMAT_VERSION = 1;

export interface PrefabRequest {
  /** Asset ID of the prefab to write, relative to the project's asset root. */
  path: string;
  /** What the entity is called. Defaults to the recipe's id. */
  name?: string;
  /** Which baked frame the prefab shows. Defaults to the authored facing. */
  defaultDirection?: Direction;
  /** Draw-order override on the sprite. Omitted from the payload when zero. */
  layer?: number;
  /**
   * The scene entity holding the tilemap this stands on.
   *
   * Required before a grid footprint can be written at all: `sindri.grid.occupant`
   * names its grid by scene ID, and a prefab has no way to know which entity in
   * the scene that is. Without one, no occupancy is claimed — which is right for
   * scenery that nothing needs to path around.
   */
  grid?: string;
  /** Where the recipe lives, recorded so a rebuild can find it. */
  recipe?: string;
}

/**
 * The prefab document for a bake, as canonical text.
 *
 * Text rather than a structure, because the canonical form *is* the contract:
 * a caller that could build the object could also write it a second way.
 */
export function buildPrefab(recipe: Recipe, result: BakeResult, request: PrefabRequest): string {
  const direction = request.defaultDirection ?? recipe.facing;
  const frame = result.frames.find((candidate) => candidate.direction === direction);
  if (!frame) {
    const baked = result.frames.map((candidate) => candidate.direction).join(', ');
    throw new Error(
      `the prefab asks for the "${direction}" frame, but this bake produced ${baked}`,
    );
  }

  const components: Record<string, JsonValue> = {
    'sindri.sprite': sprite(recipe, direction, request.layer),
  };
  const occupant = gridOccupant(recipe, request.grid);
  if (occupant) components['sindri.grid.occupant'] = occupant;

  const entity = ordered([
    ['id', recipe.id],
    ['name', request.name ?? recipe.id],
    ['transform_3d', transform(result.report)],
    ['components', components],
    ['editor', { [GENERATOR_KEY]: generation(recipe, result.report, request) }],
  ]);

  return toCanonicalJson(
    ordered([
      ['format_version', PREFAB_FORMAT_VERSION],
      ['metadata', ordered([['name', request.name ?? recipe.id]])],
      ['entities', [entity]],
    ]),
  );
}

/**
 * The root's transform.
 *
 * At the origin, unrotated, and scaled so the quad is exactly the size of the
 * baked canvas in world units. A Sindri world sprite is a unit quad centred on
 * its transform, so the scale *is* the size — and because the bake put the
 * floor anchor on the exact centre of every frame, a centred quad of this size
 * stands on its tile in every direction with nothing else to say.
 */
function transform(report: BakeReport): JsonValue {
  return ordered([
    ['position', [f32(0), f32(0), f32(0)]],
    ['rotation', [f32(0), f32(0), f32(0), f32(1)]],
    ['scale', [f32(report.spriteScale[0]), f32(report.spriteScale[1]), f32(1)]],
  ]);
}

function sprite(recipe: Recipe, direction: Direction, layer: number | undefined): JsonValue {
  const payload: Record<string, JsonValue> = {
    texture: `${recipe.texture}#${direction}`,
    tint: [f32(1), f32(1), f32(1), f32(1)],
  };
  // Zero is what a sprite that says nothing about layering already means, and
  // this codebase writes nothing where nothing is meant.
  if (layer !== undefined && layer !== 0) payload.layer = layer;
  return payload;
}

/**
 * Which cells the thing stands on, when the recipe says which grid to stand on.
 *
 * A single cell is the component's own default, so a one-tile asset writes no
 * footprint at all rather than restating it.
 */
function gridOccupant(recipe: Recipe, grid: string | undefined): JsonValue | null {
  if (!grid) return null;

  const payload: Record<string, JsonValue> = { grid };
  const { width, height } = recipe.footprint;
  if (width !== 1 || height !== 1) {
    const cells: JsonValue[] = [];
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) cells.push([x, y]);
    }
    payload.footprint = cells;
  }
  return payload;
}

/**
 * What made this file, recorded where the runtime will not read it.
 *
 * An entity's `editor` map is defined as state runtimes must ignore, and a
 * prefab's editor sections are dropped when one is spawned — so this cannot
 * change what the asset does, which is the requirement. It is also the natural
 * place for it: what an editor needs to offer a rebuild is exactly this, and
 * putting it in a component would make a generated prefab carry a payload no
 * game asked for.
 *
 * No timestamp. A bake is compared byte for byte, and a clock reading would
 * make every rebuild a diff.
 */
function generation(recipe: Recipe, report: BakeReport, request: PrefabRequest): JsonValue {
  const record: Record<string, JsonValue> = {
    anchor: [report.anchor[0], report.anchor[1]],
    canvas: [report.canvas.width, report.canvas.height],
    directions: report.frames.map((frame) => frame.direction),
    format_version: GENERATOR_FORMAT_VERSION,
    sheet: sheetIdFor(recipe.texture),
    texture: recipe.texture,
    tile: [recipe.tile.width, recipe.tile.height],
    world_units_per_pixel: report.worldUnitsPerPixel,
  };
  if (request.recipe) record.recipe = request.recipe;
  return record;
}
