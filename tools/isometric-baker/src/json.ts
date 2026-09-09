/**
 * Reading untrusted JSON with errors that name the field.
 *
 * A recipe is hand-edited, so "expected a number at model.parts[2].size[1], got
 * a string" is worth more than a stack trace from deep inside the rasteriser.
 */

export class RecipeError extends Error {}

export type JsonValue = unknown;

export function fail(path: string, message: string): never {
  throw new RecipeError(`${path}: ${message}`);
}

export function asObject(value: JsonValue, path: string): Record<string, JsonValue> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    fail(path, `expected an object, got ${describe(value)}`);
  }
  return value as Record<string, JsonValue>;
}

export function asArray(value: JsonValue, path: string): JsonValue[] {
  if (!Array.isArray(value)) fail(path, `expected an array, got ${describe(value)}`);
  return value;
}

export function asString(value: JsonValue, path: string): string {
  if (typeof value !== 'string') fail(path, `expected a string, got ${describe(value)}`);
  return value;
}

export function asNumber(value: JsonValue, path: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    fail(path, `expected a finite number, got ${describe(value)}`);
  }
  return value;
}

export function asBoolean(value: JsonValue, path: string): boolean {
  if (typeof value !== 'boolean') fail(path, `expected true or false, got ${describe(value)}`);
  return value;
}

export function asVec3(value: JsonValue, path: string): [number, number, number] {
  const list = asArray(value, path);
  if (list.length !== 3) fail(path, `expected three numbers, got ${list.length}`);
  return [asNumber(list[0], `${path}[0]`), asNumber(list[1], `${path}[1]`), asNumber(list[2], `${path}[2]`)];
}

export function optional<T>(
  source: Record<string, JsonValue>,
  key: string,
  path: string,
  read: (value: JsonValue, path: string) => T,
): T | undefined {
  const value = source[key];
  return value === undefined ? undefined : read(value, `${path}.${key}`);
}

export function required<T>(
  source: Record<string, JsonValue>,
  key: string,
  path: string,
  read: (value: JsonValue, path: string) => T,
): T {
  if (source[key] === undefined) fail(path, `is missing "${key}"`);
  return read(source[key], `${path}.${key}`);
}

/**
 * Refuse a key nobody reads.
 *
 * A recipe with `supersamples: 8` in it would otherwise bake at 4 and say
 * nothing, and the person who wrote it would conclude supersampling does not
 * work.
 */
export function rejectUnknown(source: Record<string, JsonValue>, path: string, known: string[]): void {
  for (const key of Object.keys(source)) {
    if (!known.includes(key)) {
      fail(path, `has an unknown field "${key}" (expected one of: ${known.join(', ')})`);
    }
  }
}

function describe(value: JsonValue): string {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'an array';
  return typeof value;
}
