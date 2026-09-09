/**
 * The PNG codec exists so a bake can be compared by pixels rather than by
 * bytes. These check both halves of that: what it writes is a PNG, and what it
 * reads back is what went in.
 */

import assert from 'node:assert/strict';
import { deflateSync } from 'node:zlib';
import { test } from 'node:test';

import { createImage } from '../src/image.ts';
import { decodePng, encodePng } from '../src/png.ts';

function noisy(width: number, height: number) {
  const image = createImage(width, height);
  // A fixed pattern rather than a random one: a codec test that fails on one
  // run in fifty is worse than no codec test.
  for (let i = 0; i < image.data.length; i += 4) {
    const pixel = i / 4;
    image.data[i] = (pixel * 7) & 0xff;
    image.data[i + 1] = (pixel * 13) & 0xff;
    image.data[i + 2] = (pixel * 29) & 0xff;
    image.data[i + 3] = pixel % 5 === 0 ? 0 : 255;
  }
  return image;
}

test('an encoded image decodes back to the same pixels', () => {
  const image = noisy(37, 19);
  const back = decodePng(encodePng(image));
  assert.equal(back.width, image.width);
  assert.equal(back.height, image.height);
  assert.deepEqual([...back.data], [...image.data]);
});

test('encoding is deterministic', () => {
  const image = noisy(16, 16);
  assert.ok(encodePng(image).equals(encodePng(image)));
});

test('the file really is a PNG', () => {
  const buffer = encodePng(noisy(4, 4));
  assert.deepEqual([...buffer.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  assert.equal(buffer.toString('ascii', 12, 16), 'IHDR');
  assert.equal(buffer.toString('ascii', buffer.length - 8, buffer.length - 4), 'IEND');
});

test('rows written with a predictor are still read', () => {
  // Nothing here writes filter 4, but a fixture could be re-saved by another
  // tool, and a reader that only understood its own output would reject it.
  const width = 3;
  const height = 2;
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 1; // Sub
    for (let x = 0; x < stride; x++) raw[y * (stride + 1) + 1 + x] = x < 4 ? 10 : 1;
  }

  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8;
  header[9] = 6;
  const png = rebuild(header, deflateSync(raw));

  const decoded = decodePng(png);
  // Each channel accumulates: 10, then 11, 12 across the row.
  assert.deepEqual([...decoded.data.subarray(0, 12)], [10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 12]);
});

/** Rebuild a PNG around a raw IDAT, using the encoder's own chunk framing. */
function rebuild(header: Buffer, idat: Buffer): Buffer {
  const template = encodePng(createImage(1, 1));
  const signature = template.subarray(0, 8);
  return Buffer.concat([signature, chunk('IHDR', header), chunk('IDAT', idat), chunk('IEND', Buffer.alloc(0))]);
}

function chunk(type: string, data: Buffer): Buffer {
  const out = Buffer.alloc(data.length + 12);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, 'ascii');
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}

function crc32(data: Buffer): number {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
