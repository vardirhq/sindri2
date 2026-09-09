/**
 * Banded "pixel toon" shading, evaluated on the CPU.
 *
 * Ported from IsoGame's `tools/sprite-factory/src/materials.ts` (MIT). There it
 * is a GLSL fragment shader; here it is the same expression in TypeScript,
 * because this baker rasterises without a GPU. The band arithmetic is
 * deliberately identical, down to the half-lambert remap and the `step` order,
 * so a material bakes to the same four colours either way.
 *
 * A physically-shaded render downsampled to 64 pixels looks like mud. Instead
 * each material gets a four-entry ramp and the shading snaps the lambert term
 * into one of those four. Nothing else can come out of it.
 *
 * With the default rig a box lands on three predictable bands:
 *
 *   top face (+Y)   -> shade 3, highlight
 *   left face (+Z)  -> shade 2, base colour
 *   right face (+X) -> shade 1, shadow
 *
 * Curved geometry gets the same four bands, which is exactly the terracing you
 * would hand-paint on a cylinder.
 */

import { type Rgb, hexToRgb, makeRamp, type RampOptions } from './palette.ts';
import { type Vec3, dot, normalize } from './vec.ts';

export type { RampOptions };

export interface MaterialSpec {
  /** Base colour. The ramp is derived from this. */
  colour: string;
  /** Per-material ramp tweaks. */
  ramp?: Partial<RampOptions>;
  /** Ignore lighting and always emit the brightest shade: lamps, screens, glow. */
  unlit?: boolean;
}

export interface ShadingConfig {
  /** Direction from a surface toward the light, in world space. */
  light: [number, number, number];
  /** Band edges on the half-lambert term, ascending. */
  thresholds: [number, number, number];
}

/**
 * The default rig: light above and to the screen-left.
 *
 * Chosen so the three visible faces of a box land cleanly in three separate
 * bands rather than straddling a threshold.
 */
export const DEFAULT_SHADING: ShadingConfig = {
  light: [-0.3, 0.89, 0.35],
  thresholds: [0.25, 0.5, 0.8],
};

/** A material resolved to the only four colours it can emit. */
export interface BandedMaterial {
  /** Ramp entries, darkest first. */
  ramp: string[];
  /** The same entries as bytes, which is what the rasteriser writes. */
  shades: Rgb[];
  unlit: boolean;
}

export function bandMaterial(spec: MaterialSpec): BandedMaterial {
  const ramp = makeRamp(spec.colour, spec.ramp);
  return { ramp, shades: ramp.map(hexToRgb), unlit: spec.unlit === true };
}

/** The light direction, normalised once so the inner loop does not. */
export function lightVector(shading: ShadingConfig): Vec3 {
  return normalize(shading.light as unknown as Vec3);
}

/**
 * Which of the four shades a surface normal lands on.
 *
 * The half-lambert remap (`* 0.5 + 0.5`) is what keeps a surface facing away
 * from the light on the darkest band instead of clamping a whole hemisphere to
 * black, and it is why the thresholds are stated in 0..1 rather than -1..1.
 */
export function shadeIndex(normal: Vec3, light: Vec3, thresholds: ShadingConfig['thresholds']): number {
  const t = dot(normalize(normal), light) * 0.5 + 0.5;
  let index = 0;
  if (t >= thresholds[0]) index = 1;
  if (t >= thresholds[1]) index = 2;
  if (t >= thresholds[2]) index = 3;
  return index;
}
