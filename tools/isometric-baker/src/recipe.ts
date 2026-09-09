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
import { type TileSize } from './iso.ts';
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
import { readModel } from './recipe-model.ts';
import { type ShadingConfig, DEFAULT_SHADING } from './shading.ts';
import { type TileWorldSize } from './sheet.ts';

export { RecipeError };

/** The version this tool writes and reads. */
export const RECIPE_FORMAT_VERSION = 1;

export interface Recipe {
  formatVersion: number;
  /** Asset name, used for the output file names and the bake report. */
  id: string;
  /** Asset ID of the texture to write, relative to the project's asset root. */
  texture: string;
  tile: TileSize;
  /** The tilemap this asset is meant to stand on, in world units. */
  tileWorld: TileWorldSize;
  facing: Direction;
  directions: DirectionCount;
  footprint: Footprint;
  render: FrameConfig;
  model: ModelSpec;
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
    light: light.map((entry, index) => asNumber(entry, `${path}.light[${index}]`)) as [number, number, number],
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

const TOP_LEVEL_KEYS = [
  'format_version',
  'id',
  'texture',
  'tile',
  'tile_world',
  'facing',
  'directions',
  'footprint',
  'render',
  'model',
];

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

  const footprint = optional(root, 'footprint', source, readFootprint) ?? { width: 1, height: 1 };
  if (directions === 8 && footprint.width !== footprint.height) {
    fail(
      `${source}.footprint`,
      'eight directions need a square footprint: a diagonal orientation has no ' +
        'rectangle on a square grid, and guessing one would put the anchor in the wrong place',
    );
  }

  return {
    formatVersion,
    id,
    texture,
    tile: optional(root, 'tile', source, readTile) ?? { width: 64, height: 32 },
    tileWorld: optional(root, 'tile_world', source, readTileWorld) ?? DEFAULT_TILE_WORLD,
    facing,
    directions,
    footprint,
    render: optional(root, 'render', source, readRender) ?? DEFAULT_RENDER,
    model: required(root, 'model', source, readModel),
  };
}
