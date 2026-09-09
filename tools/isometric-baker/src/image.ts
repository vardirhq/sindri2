/** The one image representation everything in the baker passes around. */
export interface RgbaImage {
  data: Uint8ClampedArray;
  width: number;
  height: number;
}

export function createImage(width: number, height: number): RgbaImage {
  return { data: new Uint8ClampedArray(width * height * 4), width, height };
}

/** A copy that shares nothing with the original. */
export function cloneImage(image: RgbaImage): RgbaImage {
  return { data: new Uint8ClampedArray(image.data), width: image.width, height: image.height };
}

export function imagesEqual(a: RgbaImage, b: RgbaImage): boolean {
  if (a.width !== b.width || a.height !== b.height) return false;
  for (let i = 0; i < a.data.length; i++) {
    if (a.data[i] !== b.data[i]) return false;
  }
  return true;
}

/** Where two images first differ, for a test that has to say what moved. */
export function firstDifference(a: RgbaImage, b: RgbaImage): { x: number; y: number } | null {
  if (a.width !== b.width || a.height !== b.height) return { x: -1, y: -1 };
  for (let i = 0; i < a.data.length; i += 4) {
    for (let channel = 0; channel < 4; channel++) {
      if (a.data[i + channel] !== b.data[i + channel]) {
        const pixel = i / 4;
        return { x: pixel % a.width, y: Math.floor(pixel / a.width) };
      }
    }
  }
  return null;
}
