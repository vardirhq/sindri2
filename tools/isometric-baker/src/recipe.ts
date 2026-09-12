/**
 * The bake recipe: `<name>.isobake.json`.
 *
 * A recipe is the durable half of a baked asset. The PNG and its sheet are
 * derived and can be regenerated; the recipe is the source, and it is what a
 * later change edits. IsoGame keeps its equivalent in TypeScript
 * (`tools/sprite-factory/src/catalog.ts`) because its catalogue is code the game
 * imports. A Sindri asset is a file in a project, so its recipe is a document —
 * which is also what lets the editor own one later without a rebuild.
 *
 * Field naming follows Sindri's other documents: `snake_case` on disk, and no
 * field the reader does not understand (a typo in a recipe is refused, not
 * silently baked at the default).
 */

import { type Direction, type DirectionCount, DIRECTION_COUNTS, isDirection } from './directions.ts';
import { type Footprint, type FrameConfig } from './frames.ts';
import { type TileSize, type View, VIEWS, isView } from './camera.ts';
import {
  type JsonValue,
  RecipeError,
  asArray,
  asBoolean,
  asColour,
  asNumber,
  asObject,
  asString,
  fail,
  optional,
  rejectUnknown,
  required,
} from './json.ts';
import { type ModelSpec } from './model.ts';
import { type PrefabRequest } from './prefab.ts';
import { readModel } from './recipe-model.ts';
import { type ShadingConfig, DEFAULT_SHADING } from './shading.ts';
import { type TileWorldSize } from './sheet.ts';

export { RecipeError };

/** One named model on a sheet. A lone model has no name of its own. */
export interface ModelVariant {
  name: string | null;
  model: ModelSpec;
}

/** How the baked model is split into sheet frames. */
export type OutputMode = 'sprite' | 'block_faces';

/** The version this tool writes and reads. */
export const RECIPE_FORMAT_VERSION = 1;

export interface Recipe {
  formatVersion: number;
  /** Asset name, used for the output file names and the bake report. */
  id: string;
  /** Asset ID of the texture to write, relative to the project's asset root. */
  texture: string;
  /** Which way this bake is looked at. */
  view: View;
  /**
   * The tile an isometric bake's pitch is derived from.
   *
   * `null` for a flat view. A top-down tile is a square and a side-on one is a
   * line, so neither has a ratio to be a pitch, and a recipe that named one
   * would be describing a relationship its picture does not have.
   */
  tile: TileSize | null;
  /**
   * The tilemap this asset is meant to stand on, in world units.
   *
   * `null` for a flat view, which stands on no tilemap.
   */
  tileWorld: TileWorldSize | null;
  /**
   * Pixels per world unit, for a flat view that states its scale directly.
   *
   * `null` for an isometric bake, which derives it from the tile.
   */
  pixelsPerUnit: number | null;
  facing: Direction;
  directions: DirectionCount;
  footprint: Footprint;
  render: FrameConfig;
  /** Whole sprites, or aligned top/south/east layers for a stackable block. */
  output: OutputMode;
  /**
   * The models on this sheet, in the order their frames are packed.
   *
   * Always at least one. A recipe naming a single `model` becomes one variant
   * with no name, whose frames are called by direction; a recipe naming
   * `variants` becomes one per entry, which is what a tile set is.
   */
  variants: ModelVariant[];
  /** The prefab to generate beside the sheet, when one is wanted. */
  prefab?: PrefabRequest;
}

export const DEFAULT_RENDER: FrameConfig = {
  supersample: 4,
  padding: 2,
  outline: { enabled: true, colour: '#241d2b' },
  paletteSnap: true,
  extraPalette: [],
  alphaCutoff: 128,
  shading: DEFAULT_SHADING,
};

/** Gather's tilemap draws 1.1 x 0.55 world-unit tiles, which is 2:1. */
export const DEFAULT_TILE_WORLD: TileWorldSize = { width: 1.1, height: 0.55 };

function readTile(value: JsonValue, path: string): TileSize {
  const source = asObject(value, path);
  rejectUnknown(source, path, ['width', 'height']);
  return {
    width: required(source, 'width', path, asNumber),
    height: required(source, 'height', path, asNumber),
  };
}

function readFootprint(value: JsonValue, path: string): Footprint {
  const source = asObject(value, path);
  rejectUnknown(source, path, ['width', 'height']);
  const read = (key: string) => {
    const tiles = required(source, key, path, asNumber);
    if (!Number.isInteger(tiles) || tiles < 1) {
      fail(`${path}.${key}`, `a footprint is measured in whole tiles, got ${tiles}`);
    }
    return tiles;
  };
  return { width: read('width'), height: read('height') };
}

function readShading(value: JsonValue, path: string): ShadingConfig {
  const source = asObject(value, path);
  rejectUnknown(source, path, ['light', 'thresholds']);

  const light = asArray(required(source, 'light', path, asArray), `${path}.light`);
  if (light.length !== 3) fail(`${path}.light`, `expected three components, got ${light.length}`);

  const thresholds = asArray(required(source, 'thresholds', path, asArray), `${path}.thresholds`);
  if (thresholds.length !== 3) {
    fail(`${path}.thresholds`, `four shade bands have three edges, got ${thresholds.length}`);
  }
  const edges = thresholds.map((entry, index) => asNumber(entry, `${path}.thresholds[${index}]`));
  if (!(edges[0] <= edges[1] && edges[1] <= edges[2])) {
    fail(`${path}.thresholds`, `band edges must ascend, got ${edges.join(', ')}`);
  }

  return {
    light: light.map((entry, index) => asNumber(entry, `${path}.light[${index}]`)) as [
      number,
      number,
      number,
    ],
    thresholds: edges as [number, number, number],
  };
}

function readRender(value: JsonValue, path: string): FrameConfig {
  const source = asObject(value, path);
  rejectUnknown(source, path, [
    'supersample',
    'padding',
    'outline',
    'palette_snap',
    'extra_palette',
    'alpha_cutoff',
    'shading',
  ]);

  const supersample = optional(source, 'supersample', path, asNumber) ?? DEFAULT_RENDER.supersample;
  if (!Number.isInteger(supersample) || supersample < 1 || supersample > 8) {
    fail(`${path}.supersample`, `expected a whole number from 1 to 8, got ${supersample}`);
  }

  const padding = optional(source, 'padding', path, asNumber) ?? DEFAULT_RENDER.padding;
  if (!Number.isInteger(padding) || padding < 0) {
    fail(`${path}.padding`, `expected a whole number of pixels, got ${padding}`);
  }

  const alphaCutoff = optional(source, 'alpha_cutoff', path, asNumber) ?? DEFAULT_RENDER.alphaCutoff;
  if (alphaCutoff < 1 || alphaCutoff > 255) {
    fail(`${path}.alpha_cutoff`, `expected coverage from 1 to 255, got ${alphaCutoff}`);
  }

  const outlineSource = optional(source, 'outline', path, asObject);
  const outline = outlineSource
    ? (rejectUnknown(outlineSource, `${path}.outline`, ['enabled', 'colour']),
      {
        enabled: optional(outlineSource, 'enabled', `${path}.outline`, asBoolean) ?? true,
        colour:
          optional(outlineSource, 'colour', `${path}.outline`, asColour) ?? DEFAULT_RENDER.outline.colour,
      })
    : DEFAULT_RENDER.outline;

  const extra = optional(source, 'extra_palette', path, asArray);

  return {
    supersample,
    padding,
    outline,
    paletteSnap: optional(source, 'palette_snap', path, asBoolean) ?? DEFAULT_RENDER.paletteSnap,
    extraPalette: extra
      ? extra.map((entry, index) => asColour(entry, `${path}.extra_palette[${index}]`))
      : [],
    alphaCutoff,
    shading: optional(source, 'shading', path, readShading) ?? DEFAULT_RENDER.shading,
  };
}

function readTileWorld(value: JsonValue, path: string): TileWorldSize {
  const source = asObject(value, path);
  rejectUnknown(source, path, ['width', 'height']);
  return {
    width: required(source, 'width', path, asNumber),
    height: required(source, 'height', path, asNumber),
  };
}

/**
 * The prefab block.
 *
 * Optional: a bake that only wants a sheet — a tile set, an atlas something
 * else composes — has no prefab to generate, and writing an unused one would
 * put a file in the project that nothing names.
 */
function readPrefab(value: JsonValue, path: string): PrefabRequest {
  const source = asObject(value, path);
  rejectUnknown(source, path, ['path', 'name', 'default_direction', 'layer', 'grid', 'recipe']);

  const target = required(source, 'path', path, asString);
  if (!target.endsWith('.prefab.json')) {
    fail(`${path}.path`, `a prefab is called <name>.prefab.json, got ${JSON.stringify(target)}`);
  }

  const direction = optional(source, 'default_direction', path, asString);
  if (direction !== undefined && !isDirection(direction)) {
    fail(`${path}.default_direction`, `${JSON.stringify(direction)} is not a compass direction`);
  }

  const layer = optional(source, 'layer', path, asNumber);
  if (layer !== undefined && !Number.isInteger(layer)) {
    fail(`${path}.layer`, `a draw layer is a whole number, got ${layer}`);
  }

  return {
    path: target,
    name: optional(source, 'name', path, asString),
    defaultDirection: direction,
    layer,
    grid: optional(source, 'grid', path, asString),
    recipe: optional(source, 'recipe', path, asString),
  };
}

/**
 * The models a sheet holds.
 *
 * `model` and `variants` are the same thing said two ways, and a recipe may say
 * it only once: a document that said both would have to decide which won, and
 * whichever it picked would surprise whoever wrote the other.
 */
function readVariants(root: Record<string, JsonValue>, source: string): ModelVariant[] {
  const single = root.model;
  const many = root.variants;

  if (single !== undefined && many !== undefined) {
    fail(source, 'has both "model" and "variants"; a sheet is described one way or the other');
  }
  if (single !== undefined) {
    return [{ name: null, model: readModel(single, `${source}.model`) }];
  }
  if (many === undefined) fail(source, 'is missing "model"');

  const entries = asArray(many, `${source}.variants`);
  if (entries.length === 0) fail(`${source}.variants`, 'names no models');

  const seen = new Set<string>();
  return entries.map((entry, index) => {
    const path = `${source}.variants[${index}]`;
    const variant = asObject(entry, path);
    rejectUnknown(variant, path, ['name', 'model']);

    const name = required(variant, 'name', path, asString);
    if (!/^[a-z0-9][a-z0-9_-]*$/.test(name)) {
      fail(`${path}.name`, `expected a lowercase name like "long-grass", got ${JSON.stringify(name)}`);
    }
    // Two frames of one name would make a sheet whose rects disagree about
    // which cell that name cuts, which `sindri-core` refuses when it loads it.
    if (seen.has(name)) fail(`${path}.name`, `is used twice; a sheet names each frame once`);
    seen.add(name);

    return { name, model: readModel(required(variant, 'model', path, (value) => value), `${path}.model`) };
  });
}

/**
 * How a bake is looked at, and how big a world unit is when it is.
 *
 * The two questions are one question, because each view measures itself the way
 * that view actually thinks. An isometric bake states a floor tile and the
 * camera's pitch falls out of its ratio; a flat bake has no pitch and no
 * diamond, so it states pixels per world unit and there is nothing to derive.
 *
 * Naming the other view's field is refused rather than ignored. A `tile` on a
 * top-down bake is somebody expecting a tilemap relationship that the picture
 * does not have, and baking it anyway at some default would be the quiet kind
 * of wrong this format exists to avoid.
 */
function readScale(
  root: Record<string, JsonValue>,
  source: string,
): Pick<Recipe, 'view' | 'tile' | 'tileWorld' | 'pixelsPerUnit'> {
  const named = optional(root, 'view', source, asString);
  if (named !== undefined && !isView(named)) {
    fail(`${source}.view`, `expected one of ${VIEWS.join(', ')}, got ${JSON.stringify(named)}`);
  }
  const view: View = (named as View | undefined) ?? 'isometric';

  const only = (keys: string[], why: string) => {
    for (const key of keys) {
      if (root[key] !== undefined) fail(`${source}.${key}`, why);
    }
  };

  if (view === 'isometric') {
    only(
      ['pixels_per_unit'],
      'is for a flat view; an isometric bake gets its scale from "tile", and two ' +
        'sources for one number can disagree',
    );
    return {
      view,
      tile: optional(root, 'tile', source, readTile) ?? { width: 64, height: 32 },
      tileWorld: optional(root, 'tile_world', source, readTileWorld) ?? DEFAULT_TILE_WORLD,
      pixelsPerUnit: null,
    };
  }

  only(
    ['tile', 'tile_world'],
    `describes a floor diamond, and a "${view}" bake draws no diamond; ` +
      'state "pixels_per_unit" instead',
  );
  const pixelsPerUnit = required(root, 'pixels_per_unit', source, asNumber);
  if (!(pixelsPerUnit > 0) || !Number.isFinite(pixelsPerUnit)) {
    fail(`${source}.pixels_per_unit`, `must be a positive number, got ${pixelsPerUnit}`);
  }
  return { view, tile: null, tileWorld: null, pixelsPerUnit };
}

const TOP_LEVEL_KEYS = [
  'format_version',
  'id',
  'texture',
  'view',
  'pixels_per_unit',
  'tile',
  'tile_world',
  'facing',
  'directions',
  'footprint',
  'render',
  'output',
  'model',
  'variants',
  'prefab',
];

/**
 * The prefab block, refused on a sheet of several models.
 *
 * A prefab draws one named frame, and on a tile set there is no one frame it
 * could mean — the whole point of the sheet is that a tilemap picks per cell.
 */
function prefab(root: Record<string, JsonValue>, source: string): PrefabRequest | undefined {
  const request = optional(root, 'prefab', source, readPrefab);
  if (request && root.variants !== undefined) {
    fail(
      `${source}.prefab`,
      'a sheet of several models has no single frame for a prefab to draw; ' +
        'bake the prefab from its own recipe',
    );
  }
  return request;
}

export function parseRecipe(json: string, source: string): Recipe {
  let parsed: JsonValue;
  try {
    parsed = JSON.parse(json);
  } catch (error) {
    throw new RecipeError(`${source}: is not valid JSON: ${(error as Error).message}`);
  }

  const root = asObject(parsed, source);
  rejectUnknown(root, source, TOP_LEVEL_KEYS);

  const formatVersion = required(root, 'format_version', source, asNumber);
  if (formatVersion !== RECIPE_FORMAT_VERSION) {
    fail(
      `${source}.format_version`,
      `is ${formatVersion}, but this baker writes ${RECIPE_FORMAT_VERSION}`,
    );
  }

  const id = required(root, 'id', source, asString);
  if (!/^[a-z0-9][a-z0-9_-]*$/.test(id)) {
    fail(`${source}.id`, `expected a lowercase name like "standing-stone", got ${JSON.stringify(id)}`);
  }

  const texture = required(root, 'texture', source, asString);
  if (!texture.endsWith('.png')) {
    fail(`${source}.texture`, `must name a .png, got ${JSON.stringify(texture)}`);
  }

  const facing = optional(root, 'facing', source, asString) ?? 'south';
  if (!isDirection(facing)) {
    fail(`${source}.facing`, `${JSON.stringify(facing)} is not a compass direction`);
  }

  const directions = (optional(root, 'directions', source, asNumber) ?? 4) as DirectionCount;
  if (!DIRECTION_COUNTS.includes(directions)) {
    fail(`${source}.directions`, `expected one of ${DIRECTION_COUNTS.join(', ')}, got ${directions}`);
  }

  const output = optional(root, 'output', source, asString) ?? 'sprite';
  if (output !== 'sprite' && output !== 'block_faces') {
    fail(`${source}.output`, `expected "sprite" or "block_faces", got ${JSON.stringify(output)}`);
  }
  if (output === 'block_faces' && directions !== 1) {
    fail(`${source}.directions`, 'block-face output has one fixed view; set "directions" to 1');
  }

  const footprint = optional(root, 'footprint', source, readFootprint) ?? { width: 1, height: 1 };
  if (directions === 8 && footprint.width !== footprint.height) {
    fail(
      `${source}.footprint`,
      'eight directions need a square footprint: a diagonal orientation has no ' +
        'rectangle on a square grid, and guessing one would put the anchor in the wrong place',
    );
  }

  const scale = readScale(root, source);
  const request = prefab(root, source);
  if (output === 'block_faces' && request) {
    fail(`${source}.prefab`, 'block-face output is a tile-set atlas, not one placeable sprite');
  }
  if (scale.view !== 'isometric' && request?.grid) {
    fail(
      `${source}.prefab.grid`,
      `names a tilemap for the prefab to occupy, and a "${scale.view}" bake stands on none`,
    );
  }

  return {
    formatVersion,
    id,
    texture,
    ...scale,
    facing,
    directions,
    footprint,
    render: optional(root, 'render', source, readRender) ?? DEFAULT_RENDER,
    output,
    variants: readVariants(root, source),
    prefab: request,
  };
}