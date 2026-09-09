/**
 * A PNG reader and writer, in the one shape the baker needs: 8-bit RGBA,
 * non-interlaced.
 *
 * IsoGame encodes through `canvas.toDataURL`, which is a browser API. Node has
 * DEFLATE in its standard library and PNG is a thin wrapper around it, so this
 * costs less than a dependency would and makes the output *ours* — the bytes a
 * bake writes are decided here rather than by whatever a browser build happened
 * to emit.
 *
 * Every row is written with filter 0 (None). A predictor would compress better,
 * but the images are small, and a bake that writes the same pixels twice must
 * write the same file twice — so there is nothing to be gained by giving the
 * encoder a choice to make.
 */

import { deflateSync, inflateSync } from 'node:zlib';

import { type RgbaImage } from './image.ts';

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

export function encodePng(image: RgbaImage): Buffer {
  const { width, height } = image;
  const stride = width * 4;
  const raw = Buffer.allocUnsafe((stride + 1) * height);

  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0;
    Buffer.from(image.data.buffer, image.data.byteOffset + y * stride, stride).copy(
      raw,
      y * (stride + 1) + 1,
    );
  }

  const header = Buffer.allocUnsafe(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 6; // colour type: truecolour with alpha
  header[10] = 0; // compression: deflate
  header[11] = 0; // filter method
  header[12] = 0; // no interlacing

  return Buffer.concat([
    SIGNATURE,
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function chunk(type: string, data: Buffer): Buffer {
  const out = Buffer.allocUnsafe(data.length + 12);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, 'ascii');
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}

/**
 * Decode a PNG this module could have written.
 *
 * Deliberately narrow: it exists so the regression test can compare *pixels*
 * rather than file bytes. Comparing bytes would make the test assert something
 * it does not mean — that this machine's zlib produces the same stream as the
 * machine that recorded the fixture — and fail on a compression-library upgrade
 * that changed no pixel at all.
 */
export function decodePng(buffer: Buffer): RgbaImage {
  if (!buffer.subarray(0, 8).equals(SIGNATURE)) throw new Error('not a PNG');

  let width = 0;
  let height = 0;
  const idat: Buffer[] = [];
  let offset = 8;

  while (offset < buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const type = buffer.toString('ascii', offset + 4, offset + 8);
    const data = buffer.subarray(offset + 8, offset + 8 + length);

    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      if (data[8] !== 8 || data[9] !== 6) {
        throw new Error(`only 8-bit RGBA PNGs are read here (depth ${data[8]}, type ${data[9]})`);
      }
      if (data[12] !== 0) throw new Error('interlaced PNGs are not read here');
    } else if (type === 'IDAT') {
      idat.push(data);
    } else if (type === 'IEND') {
      break;
    }

    offset += 12 + length;
  }

  const raw = inflateSync(Buffer.concat(idat));
  return unfilter(raw, width, height);
}

/** Undo the per-row predictors PNG allows, including ones we never write. */
function unfilter(raw: Buffer, width: number, height: number): RgbaImage {
  const stride = width * 4;
  const out = new Uint8ClampedArray(stride * height);

  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = y * (stride + 1) + 1;
    for (let x = 0; x < stride; x++) {
      const value = raw[line + x];
      const left = x >= 4 ? out[y * stride + x - 4] : 0;
      const up = y > 0 ? out[(y - 1) * stride + x] : 0;
      const upLeft = y > 0 && x >= 4 ? out[(y - 1) * stride + x - 4] : 0;

      let restored: number;
      switch (filter) {
        case 0:
          restored = value;
          break;
        case 1:
          restored = value + left;
          break;
        case 2:
          restored = value + up;
          break;
        case 3:
          restored = value + ((left + up) >> 1);
          break;
        case 4:
          restored = value + paeth(left, up, upLeft);
          break;
        default:
          throw new Error(`unknown PNG row filter ${filter}`);
      }
      out[y * stride + x] = restored & 0xff;
    }
  }

  return { data: out, width, height };
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  return pb <= pc ? b : c;
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(data: Buffer): number {
  let crc = 0xffffffff;
  for (const byte of data) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}
