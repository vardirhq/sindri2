/**
 * Colour handling.
 *
 * Ported from IsoGame's `tools/sprite-factory/src/palette.ts` (MIT), with the
 * hex/HSL conversions kept identical so a model authored against the Sprite
 * Factory bakes to the same colours here.
 *
 * The pipeline never asks a renderer to invent a colour. Every material
 * declares one base colour, from which a four-entry shade ramp is derived; the
 * shading step can only ever emit one of those entries. Supersampling blends
 * them at edges, so after downsampling every pixel is snapped back onto the
 * ramp set. What comes out has a palette that can be stated exactly, up front.
 */

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export interface RampOptions {
  /** Lightness deltas applied to the base colour, darkest shade first. */
  lightness: [number, number, number, number];
  /** Saturation deltas, darkest first. Shadows gain a little. */
  saturation: [number, number, number, number];
  /** Hue rotation in turns, darkest first. Shadows cool, highlights warm. */
  hue: [number, number, number, number];
}

export const DEFAULT_RAMP: RampOptions = {
  lightness: [-0.24, -0.13, 0, 0.11],
  saturation: [0.1, 0.05, 0, -0.04],
  hue: [-0.04, -0.02, 0, 0.015],
};

export function hexToRgb(hex: string): Rgb {
  let h = hex.trim().replace('#', '');
  if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
  if (!/^[0-9a-fA-F]{6}$/.test(h)) throw new Error(`not a hex colour: ${JSON.stringify(hex)}`);
  const n = Number.parseInt(h, 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

export function rgbToHex({ r, g, b }: Rgb): string {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, '0');
  return `#${c(r)}${c(g)}${c(b)}`;
}

export function rgbToHsl({ r, g, b }: Rgb): [number, number, number] {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];

  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6;
  else if (max === gn) h = ((bn - rn) / d + 2) / 6;
  else h = ((rn - gn) / d + 4) / 6;
  return [h, s, l];
}

export function hslToRgb(hue: number, saturation: number, lightness: number): Rgb {
  const h = ((hue % 1) + 1) % 1;
  const s = clamp01(saturation);
  const l = clamp01(lightness);

  if (s === 0) {
    const v = Math.round(l * 255);
    return { r: v, g: v, b: v };
  }

  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const channel = (offset: number) => {
    let t = offset;
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return {
    r: Math.round(channel(h + 1 / 3) * 255),
    g: Math.round(channel(h) * 255),
    b: Math.round(channel(h - 1 / 3) * 255),
  };
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

/**
 * Derive a four-shade ramp from a base colour, darkest first.
 *
 * Shadows are pushed slightly toward blue and gain saturation; highlights lose
 * a little. That is the pixel-art trick that stops shaded geometry from reading
 * as "the same colour, but greyer".
 */
export function makeRamp(baseHex: string, options: Partial<RampOptions> = {}): string[] {
  const opts: RampOptions = { ...DEFAULT_RAMP, ...options };
  const [h, s, l] = rgbToHsl(hexToRgb(baseHex));
  return [0, 1, 2, 3].map((i) =>
    rgbToHex(hslToRgb(h + opts.hue[i], s + opts.saturation[i], l + opts.lightness[i])),
  );
}

/** Squared distance in a cheap perceptual weighting: green counts most. */
function colourDistance(a: Rgb, b: Rgb): number {
  const dr = a.r - b.r;
  const dg = a.g - b.g;
  const db = a.b - b.b;
  return 2 * dr * dr + 4 * dg * dg + 3 * db * db;
}

/**
 * The palette entry nearest `colour`.
 *
 * Ties go to the earlier entry, which is what makes a snap reproducible: the
 * palette is built in a stated order, so two equally distant shades cannot come
 * out differently on two runs.
 */
export function nearestColour(colour: Rgb, palette: Rgb[]): Rgb {
  let best = palette[0];
  let bestDistance = Number.POSITIVE_INFINITY;
  for (const entry of palette) {
    const d = colourDistance(colour, entry);
    if (d < bestDistance) {
      bestDistance = d;
      best = entry;
    }
  }
  return best;
}

/** Deduplicate hex colours, preserving order. */
export function uniqueColours(hexes: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const hex of hexes) {
    const normalised = rgbToHex(hexToRgb(hex));
    if (!seen.has(normalised)) {
      seen.add(normalised);
      out.push(normalised);
    }
  }
  return out;
}
