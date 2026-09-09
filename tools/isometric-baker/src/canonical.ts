/**
 * Writing a Sindri document in the one form Sindri writes it.
 *
 * A canonical scene or prefab is a fixed point — reading one and writing it
 * again produces the same bytes — and a generated document has to already be at
 * that fixed point, or the editor's first save rewrites lines nobody edited.
 * The rules are in `crates/sindri-core/src/scene/canonical.rs`; this reproduces
 * them rather than approximating them:
 *
 *   1. `serde_json`'s pretty printer: two-space indent, `": "` after a key, and
 *      every array element on its own line.
 *   2. an array holding only scalars is folded back onto one line, whenever the
 *      folded form leaves the line under 96 columns.
 *   3. one trailing newline.
 *
 * Key order is the other half. A component payload is a `serde_json::Value`,
 * whose map is a `BTreeMap`, so its keys come back out sorted however they went
 * in — objects here therefore sort by default. The document's own structs are
 * not maps and keep their declared field order, which is what `ordered` is for.
 */

import { formatF32 } from './f32.ts';

/** The column budget for keeping an array of scalars on one line. */
const INLINE_ARRAY_WIDTH = 96;

const F32 = Symbol('f32');
const ORDERED = Symbol('ordered');

export interface F32Value {
  [F32]: number;
}

export interface OrderedObject {
  [ORDERED]: [string, JsonValue][];
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | F32Value
  | OrderedObject
  | JsonValue[]
  | { [key: string]: JsonValue };

/** A number written as an `f32` — with a decimal point, and narrowed first. */
export function f32(value: number): F32Value {
  return { [F32]: value };
}

/**
 * An object whose keys keep the order they are written in.
 *
 * For the document's own structs, whose field order is declared in Rust and is
 * not alphabetical: a `SceneEntity` is `id`, `name`, `parent`, `transform_3d`,
 * `components`, and a sorted object would put `components` first.
 */
export function ordered(entries: [string, JsonValue | undefined][]): OrderedObject {
  return {
    [ORDERED]: entries.filter((entry): entry is [string, JsonValue] => entry[1] !== undefined),
  };
}

/** The canonical text of a document, trailing newline included. */
export function toCanonicalJson(value: JsonValue): string {
  return `${collapseScalarArrays(pretty(value, 0))}\n`;
}

function pretty(value: JsonValue, depth: number): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean' || typeof value === 'number') return JSON.stringify(value);
  if (typeof value === 'string') return JSON.stringify(value);
  if (isF32(value)) return formatF32(value[F32]);

  const indent = '  '.repeat(depth + 1);
  const closing = '  '.repeat(depth);

  if (Array.isArray(value)) {
    if (value.length === 0) return '[]';
    const items = value.map((item) => `${indent}${pretty(item, depth + 1)}`);
    return `[\n${items.join(',\n')}\n${closing}]`;
  }

  const entries = isOrdered(value)
    ? value[ORDERED]
    : Object.entries(value)
        .filter((entry): entry is [string, JsonValue] => entry[1] !== undefined)
        .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));

  if (entries.length === 0) return '{}';
  const lines = entries.map(([key, item]) => `${indent}${JSON.stringify(key)}: ${pretty(item, depth + 1)}`);
  return `{\n${lines.join(',\n')}\n${closing}}`;
}

function isF32(value: object): value is F32Value {
  return F32 in value;
}

function isOrdered(value: object): value is OrderedObject {
  return ORDERED in value;
}

/**
 * Fold arrays that contain only scalars onto one line.
 *
 * A direct port of `collapse_scalar_arrays`, including the detail that the
 * width test is against the line built *so far* — so whether an array folds
 * depends on how deeply it is indented and what key it sits under, and two
 * identical arrays at different depths can be written differently.
 */
function collapseScalarArrays(text: string): string {
  let output = '';
  let index = 0;
  let lineStart = 0;

  while (index < text.length) {
    const character = text[index];

    if (character === '\n') {
      output += '\n';
      index += 1;
      lineStart = output.length;
    } else if (character === '"') {
      const end = stringEnd(text, index);
      output += text.slice(index, end);
      index = end;
    } else if (character === '[') {
      const end = scalarArrayEnd(text, index);
      const inline = end === null ? null : inlineArray(text.slice(index, end));
      if (inline !== null && end !== null && output.length - lineStart + inline.length < INLINE_ARRAY_WIDTH) {
        output += inline;
        index = end;
      } else {
        output += '[';
        index += 1;
      }
    } else {
      output += character;
      index += 1;
    }
  }

  return output;
}

/** Just past the closing quote of the string starting at `start`. */
function stringEnd(text: string, start: number): number {
  let index = start + 1;
  while (index < text.length) {
    if (text[index] === '\\') index += 2;
    else if (text[index] === '"') return index + 1;
    else index += 1;
  }
  return text.length;
}

/** Just past the closing bracket, or null when the array nests something. */
function scalarArrayEnd(text: string, start: number): number | null {
  let index = start + 1;
  while (index < text.length) {
    const character = text[index];
    if (character === '"') index = stringEnd(text, index);
    else if (character === ']') return index + 1;
    else if (character === '[' || character === '{' || character === '}') return null;
    else index += 1;
  }
  return null;
}

/** Rewrite an already validated scalar array as a single line. */
function inlineArray(source: string): string {
  let output = '[';
  let index = 1;

  while (index < source.length) {
    const character = source[index];
    if (character === ']') break;
    if (character === ',') {
      output += ', ';
      index += 1;
    } else if (character === '"') {
      const end = stringEnd(source, index);
      output += source.slice(index, end);
      index = end;
    } else if (/\s/.test(character)) {
      index += 1;
    } else {
      output += character;
      index += 1;
    }
  }

  return `${output}]`;
}
