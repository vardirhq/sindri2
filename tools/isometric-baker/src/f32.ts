/**
 * Writing a number the way Sindri writes one.
 *
 * A scene and a prefab store transforms as `[f32; 3]`
 * (`crates/sindri-core/src/transform.rs`), and Sindri writes a document in a
 * canonical form that is a *fixed point*: reading one and writing it again
 * produces the same bytes, so a diff on it is the edit and nothing else. A
 * generated prefab has to already be at that fixed point, or the first time the
 * editor saves the file it rewrites numbers nobody touched.
 *
 * Two things have to match, and neither is what JavaScript does by default.
 *
 * The value must be an `f32`. JavaScript has one number type, and a scale
 * computed in double precision is usually not any `f32` — written out, it would
 * be read back as the nearest one and written again differently.
 * `Math.fround` does that narrowing here instead.
 *
 * The text must be the *shortest decimal that round-trips*, which is what
 * `serde_json` emits through ryu, and it must carry a decimal point:
 * `1.0f32` is written `1.0`, where `JSON.stringify` would say `1`. The
 * shortest-decimal search below is the same idea as ryu's, done by asking for
 * increasing precision until the value survives the round trip.
 */

/** The range this writes plainly. Outside it, ryu switches to exponents. */
const SMALLEST_PLAIN = 1e-4;
const LARGEST_PLAIN = 1e16;

/**
 * `value` narrowed to `f32` and written as Sindri would write it.
 *
 * Throws rather than guessing for a value that ryu would print with an
 * exponent: nothing a bake produces is that small or that large, and a wrong
 * guess would be a file that silently stops being canonical.
 */
export function formatF32(value: number): string {
  if (!Number.isFinite(value)) {
    throw new RangeError(`a transform cannot hold ${value}`);
  }

  const narrowed = Math.fround(value);
  if (Object.is(narrowed, -0)) return '-0.0';
  if (narrowed !== 0) {
    const magnitude = Math.abs(narrowed);
    if (magnitude < SMALLEST_PLAIN || magnitude >= LARGEST_PLAIN) {
      throw new RangeError(
        `${value} is outside the range this writes as a plain decimal ` +
          `(${SMALLEST_PLAIN} to ${LARGEST_PLAIN}); ryu would print it with an exponent`,
      );
    }
  }

  return withDecimalPoint(shortest(narrowed));
}

/** `value` narrowed to `f32`, for measurements that are compared rather than written. */
export function toF32(value: number): number {
  return Math.fround(value);
}

/**
 * The fewest significant digits that still name this exact `f32`.
 *
 * Nine is the most an `f32` can ever need, so the loop always terminates on the
 * value itself.
 */
function shortest(value: number): number {
  for (let digits = 1; digits < 9; digits++) {
    const candidate = Number(value.toPrecision(digits));
    if (Math.fround(candidate) === value) return candidate;
  }
  return Number(value.toPrecision(9));
}

function withDecimalPoint(value: number): string {
  const text = String(value);
  if (text.includes('e') || text.includes('E')) {
    throw new RangeError(`${value} would be written with an exponent`);
  }
  return text.includes('.') ? text : `${text}.0`;
}
