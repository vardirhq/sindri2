/**
 * Reading the `model` half of a bake recipe.
 *
 * Split from `recipe.ts` because parts are the part that grows: every primitive
 * has its own fields, and a recipe reader that validates all of them beside the
 * camera settings is a file nobody rereads.
 */

import {
  type JsonValue,
  asArray,
  asBoolean,
  asNumber,
  asObject,
  asString,
  asVec3,
  fail,
  optional,
  rejectUnknown,
  required,
} from './json.ts';
import { type ModelPart, type ModelSpec } from './model.ts';
import { type MaterialSpec, type RampOptions } from './shading.ts';

const RAMP_KEYS = ['lightness', 'saturation', 'hue'] as const;

function readRamp(value: JsonValue, path: string): Partial<RampOptions> {
  const source = asObject(value, path);
  rejectUnknown(source, path, [...RAMP_KEYS]);
  const ramp: Partial<RampOptions> = {};
  for (const key of RAMP_KEYS) {
    const list = optional(source, key, path, asArray);
    if (!list) continue;
    if (list.length !== 4) fail(`${path}.${key}`, `expected four deltas, got ${list.length}`);
    ramp[key] = list.map((entry, index) => asNumber(entry, `${path}.${key}[${index}]`)) as [
      number,
      number,
      number,
      number,
    ];
  }
  return ramp;
}

export function readMaterial(value: JsonValue, path: string): MaterialSpec {
  if (typeof value === 'string') return { colour: value };
  const source = asObject(value, path);
  rejectUnknown(source, path, ['colour', 'ramp', 'unlit']);
  return {
    colour: required(source, 'colour', path, asString),
    ramp: optional(source, 'ramp', path, readRamp),
    unlit: optional(source, 'unlit', path, asBoolean),
  };
}

function readMaterialRef(value: JsonValue, path: string): string | MaterialSpec {
  // A bare string is a name in the model's table; anything else is an inline
  // material, which is what a one-off colour should be rather than a table
  // entry used once.
  return typeof value === 'string' ? value : readMaterial(value, path);
}

const SHARED_PART_KEYS = ['type', 'material', 'position', 'rotation'];

function readPart(value: JsonValue, path: string): ModelPart {
  const source = asObject(value, path);
  const type = required(source, 'type', path, asString);
  const base = {
    material: required(source, 'material', path, readMaterialRef),
    position: optional(source, 'position', path, asVec3),
    rotation: optional(source, 'rotation', path, asVec3),
  };

  switch (type) {
    case 'box':
      rejectUnknown(source, path, [...SHARED_PART_KEYS, 'size']);
      return { type, ...base, size: required(source, 'size', path, asVec3) };
    case 'cylinder':
      rejectUnknown(source, path, [...SHARED_PART_KEYS, 'radius', 'radius_top', 'height', 'segments']);
      return {
        type,
        ...base,
        radius: positive(required(source, 'radius', path, asNumber), `${path}.radius`),
        radiusTop: optional(source, 'radius_top', path, asNumber),
        height: positive(required(source, 'height', path, asNumber), `${path}.height`),
        segments: optional(source, 'segments', path, asNumber),
      };
    case 'cone':
      rejectUnknown(source, path, [...SHARED_PART_KEYS, 'radius', 'height', 'segments']);
      return {
        type,
        ...base,
        radius: positive(required(source, 'radius', path, asNumber), `${path}.radius`),
        height: positive(required(source, 'height', path, asNumber), `${path}.height`),
        segments: optional(source, 'segments', path, asNumber),
      };
    case 'sphere':
      rejectUnknown(source, path, [...SHARED_PART_KEYS, 'radius', 'segments', 'scale']);
      return {
        type,
        ...base,
        radius: positive(required(source, 'radius', path, asNumber), `${path}.radius`),
        segments: optional(source, 'segments', path, asNumber),
        scale: optional(source, 'scale', path, asVec3),
      };
    default:
      fail(`${path}.type`, `unknown primitive "${type}" (expected box, cylinder, cone or sphere)`);
  }
}

function positive(value: number, path: string): number {
  if (!(value > 0)) fail(path, `must be greater than zero, got ${value}`);
  return value;
}

export function readModel(value: JsonValue, path: string): ModelSpec {
  const source = asObject(value, path);
  rejectUnknown(source, path, ['kind', 'materials', 'parts']);

  const kind = optional(source, 'kind', path, asString) ?? 'primitives';
  if (kind !== 'primitives') {
    fail(
      `${path}.kind`,
      `only "primitives" is baked today (got "${kind}"); model-file input is a ` +
        'separate change, so a recipe naming one is refused rather than silently ignored',
    );
  }

  const materialSource = optional(source, 'materials', path, asObject) ?? {};
  const materials: Record<string, MaterialSpec> = {};
  for (const [name, entry] of Object.entries(materialSource)) {
    materials[name] = readMaterial(entry, `${path}.materials.${name}`);
  }

  const parts = asArray(required(source, 'parts', path, asArray), `${path}.parts`).map((part, index) =>
    readPart(part, `${path}.parts[${index}]`),
  );
  if (parts.length === 0) fail(`${path}.parts`, 'a model needs at least one part');

  return { materials, parts };
}
