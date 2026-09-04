// Just enough PNG to look at a screenshot's pixels.
//
// The smoke check needs to tell a drawn frame from an empty one, and the only
// image it can get from a WebGPU canvas is the one the browser composited --
// `drawImage` reads nothing back from such a canvas, so the screenshot is the
// evidence. Decoding it needs a reader, and a dependency for one check that
// runs in CI is a dependency the whole repository then carries; `zlib` is
// already in node.
//
// Deliberately narrow: 8-bit RGB or RGBA, non-interlaced, which is what
// Playwright emits. Anything else throws rather than guessing.
import { inflateSync } from 'node:zlib';

const SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  return pb <= pc ? b : c;
}

/// Decodes `buffer` into `{ width, height, channels, data }`, where `data` is
/// one byte per channel per pixel in row order.
export function decodePng(buffer) {
  if (!buffer.subarray(0, 8).equals(SIGNATURE)) throw new Error('not a PNG');

  let width = 0;
  let height = 0;
  let channels = 0;
  const parts = [];

  for (let at = 8; at + 8 <= buffer.length; ) {
    const length = buffer.readUInt32BE(at);
    const type = buffer.toString('ascii', at + 4, at + 8);
    const body = buffer.subarray(at + 8, at + 8 + length);
    at += 12 + length; // length, type, body, CRC

    if (type === 'IHDR') {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      const depth = body[8];
      const colour = body[9];
      const interlace = body[12];
      if (depth !== 8) throw new Error(`unsupported bit depth ${depth}`);
      if (interlace !== 0) throw new Error('interlaced PNGs are not supported');
      if (colour === 2) channels = 3;
      else if (colour === 6) channels = 4;
      else throw new Error(`unsupported colour type ${colour}`);
    } else if (type === 'IDAT') {
      parts.push(body);
    } else if (type === 'IEND') {
      break;
    }
  }

  if (width === 0 || height === 0) throw new Error('no IHDR');

  const raw = inflateSync(Buffer.concat(parts));
  const stride = width * channels;
  const data = Buffer.alloc(stride * height);

  // Each row is prefixed by the filter it was written with, and every filter
  // but the first refers to the row above -- so rows have to be undone in
  // order, into the output that earlier rows already wrote.
  for (let row = 0; row < height; row += 1) {
    const filter = raw[row * (stride + 1)];
    const from = row * (stride + 1) + 1;
    const to = row * stride;
    const above = to - stride;
    for (let i = 0; i < stride; i += 1) {
      const value = raw[from + i];
      const left = i >= channels ? data[to + i - channels] : 0;
      const up = row > 0 ? data[above + i] : 0;
      const corner = row > 0 && i >= channels ? data[above + i - channels] : 0;
      let restored;
      if (filter === 0) restored = value;
      else if (filter === 1) restored = value + left;
      else if (filter === 2) restored = value + up;
      else if (filter === 3) restored = value + ((left + up) >> 1);
      else if (filter === 4) restored = value + paeth(left, up, corner);
      else throw new Error(`unknown row filter ${filter}`);
      data[to + i] = restored & 0xff;
    }
  }

  return { width, height, channels, data };
}

/// How much was drawn: the number of distinct colours, and the mean channel
/// value across the image.
///
/// Two numbers rather than one because they fail differently. A frame cleared
/// to a single flat colour is bright but has one colour; a frame drawn almost
/// entirely in shadow has many colours but little light.
export function imageStatistics(buffer) {
  const { width, height, channels, data } = decodePng(buffer);
  const seen = new Set();
  let total = 0;
  for (let i = 0; i < data.length; i += channels) {
    seen.add((data[i] << 16) | (data[i + 1] << 8) | data[i + 2]);
    total += (data[i] + data[i + 1] + data[i + 2]) / 3;
  }
  return { width, height, colours: seen.size, mean: total / (width * height) };
}
