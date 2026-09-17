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
  /** Break the face into texels of slightly different shade. */
  grain?: GrainSpec;
}

/**
 * Surface texture, made of the ramp a material already has.
 *
 * A flat-shaded box gives three flat faces. That reads as a *shape*, and the
 * blocks it makes look like coloured cardboard: a face of grass and a face of
 * stone differ only in hue. What tells them apart in the voxel games this art
 * is copying is not the lighting — it is that each face is a small grid of
 * texels that disagree slightly about their colour.
 *
 * So rather than a texture map the baker has no way to author, a material may
 * say its surface is grainy, and the rasteriser shifts each texel a band along
 * the ramp the material already has. The result stays inside the four colours
 * the palette promised, which is what keeps a grainy material honest about
 * `palette_snap` and the bake report.
 */
export interface GrainSpec {
  /**
   * Texel size in tile units, so it is the block that decides how coarse its
   * own surface is rather than the output resolution.
   */
  size: number;
  /**
   * How much of the surface is shifted off its lit band, from zero to one.
   *
   * At zero nothing moves and the material is flat. At one nearly every texel
   * is a band away from its neighbours, which is noise rather than texture; the
   * useful range is small.
   */
  strength: number;
  /** Which stream of variation to take, so two materials do not agree. */
  seed?: number;
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
  grain?: GrainSpec;
}

export function bandMaterial(spec: MaterialSpec): BandedMaterial {
  const ramp = makeRamp(spec.colour, spec.ramp);
  return {
    ramp,
    shades: ramp.map(hexToRgb),
    unlit: spec.unlit === true,
    grain: spec.grain,
  };
}

/**
 * Which way one texel's shade is pushed, as -1, 0 or 1.
 *
 * Hashed from the texel's own position in model space, so the grain belongs to
 * the block rather than to the camera: bake the same model twice, or from four
 * directions, and the same texel is the same shade every time. A random number
 * generator would give a prettier spread and a different sprite each run, which
 * is not a trade an asset baker gets to make.
 */
export function grainShift(grain: GrainSpec, x: number, y: number, z: number): number {
  if (!(grain.size > 0) || !(grain.strength > 0)) return 0;
  const texel = (value: number) => Math.floor(value / grain.size);
  const noise = hash3(texel(x), texel(y), texel(z), grain.seed ?? 0);
  // Two thresholds either side of the middle, so a low strength leaves most of
  // the face on its lit band and moves a scattering of texels off it.
  const half = Math.min(1, grain.strength) * 0.5;
  if (noise < half) return -1;
  if (noise > 1 - half) return 1;
  return 0;
}

/**
 * A stable hash of three integers into the unit interval.
 *
 * Integer mixing rather than anything trigonometric: `Math.sin` noise is a
 * different number on a different engine, and a baked sprite that changes with
 * the runtime is not a baked sprite.
 */
function hash3(x: number, y: number, z: number, seed: number): number {
  let h = (seed | 0) ^ 0x9e3779b9;
  for (const value of [x | 0, y | 0, z | 0]) {
    h = Math.imul(h ^ value, 0x85ebca6b);
    h ^= h >>> 13;
    h = Math.imul(h, 0xc2b2ae35);
    h ^= h >>> 16;
  }
  return (h >>> 0) / 4294967296;
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
