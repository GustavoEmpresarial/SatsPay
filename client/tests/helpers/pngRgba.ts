import { inflateSync } from 'node:zlib';

/** Minimal 8-bit RGBA PNG reader (color type 6) for branding regression tests. */
export type PngRgba = {
  width: number;
  height: number;
  /** length = width * height * 4 (RGBA) */
  data: Uint8Array;
};

export function readPngRgba(buf: Buffer): PngRgba {
  if (buf.length < 8 || buf.subarray(0, 8).toString('binary') !== '\x89PNG\r\n\x1a\n') {
    throw new Error('not a PNG');
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = -1;
  const idat: Buffer[] = [];

  let offset = 8;
  while (offset + 8 <= buf.length) {
    const length = buf.readUInt32BE(offset);
    const type = buf.subarray(offset + 4, offset + 8).toString('ascii');
    const data = buf.subarray(offset + 8, offset + 8 + length);
    offset += 12 + length; // len + type + data + crc

    if (type === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
    } else if (type === 'IDAT') {
      idat.push(Buffer.from(data));
    } else if (type === 'IEND') {
      break;
    }
  }

  if (colorType !== 6 || bitDepth !== 8) {
    throw new Error(`expected 8-bit RGBA PNG, got bitDepth=${bitDepth} colorType=${colorType}`);
  }
  if (!width || !height) throw new Error('missing IHDR');

  const inflated = inflateSync(Buffer.concat(idat));
  const stride = width * 4;
  const expected = height * (1 + stride);
  if (inflated.length < expected) {
    throw new Error(`IDAT too short: ${inflated.length} < ${expected}`);
  }

  const out = new Uint8Array(width * height * 4);
  let src = 0;
  let dst = 0;
  const prev = new Uint8Array(stride);

  for (let y = 0; y < height; y++) {
    const filter = inflated[src++];
    const row = inflated.subarray(src, src + stride);
    src += stride;
    const recon = new Uint8Array(stride);

    for (let i = 0; i < stride; i++) {
      const x = row[i];
      const a = i >= 4 ? recon[i - 4] : 0;
      const b = prev[i];
      const c = i >= 4 ? prev[i - 4] : 0;
      let val = 0;
      switch (filter) {
        case 0:
          val = x;
          break;
        case 1:
          val = (x + a) & 255;
          break;
        case 2:
          val = (x + b) & 255;
          break;
        case 3:
          val = (x + ((a + b) >> 1)) & 255;
          break;
        case 4:
          val = (x + paeth(a, b, c)) & 255;
          break;
        default:
          throw new Error(`unsupported PNG filter ${filter}`);
      }
      recon[i] = val;
    }

    out.set(recon, dst);
    dst += stride;
    prev.set(recon);
  }

  return { width, height, data: out };
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

export function pixelAt(png: PngRgba, x: number, y: number): [number, number, number, number] {
  const i = (y * png.width + x) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2], png.data[i + 3]];
}

export function countFullyTransparent(png: PngRgba): number {
  let n = 0;
  for (let i = 3; i < png.data.length; i += 4) {
    if (png.data[i] === 0) n += 1;
  }
  return n;
}
