import { describe, expect, it } from 'vitest';
import { deflateSync } from 'node:zlib';
import { countFullyTransparent, pixelAt, readPngRgba } from '../../helpers/pngRgba.js';

/** Build a tiny 2x2 RGBA PNG (filter=None) for the helper itself. */
function tinyRgbaPng(pixels: number[]): Buffer {
  // pixels: 2x2 * RGBA = 16 values
  const width = 2;
  const height = 2;
  const raw = Buffer.alloc(height * (1 + width * 4));
  let o = 0;
  for (let y = 0; y < height; y++) {
    raw[o++] = 0; // filter None
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      raw[o++] = pixels[i] ?? 0;
      raw[o++] = pixels[i + 1] ?? 0;
      raw[o++] = pixels[i + 2] ?? 0;
      raw[o++] = pixels[i + 3] ?? 0;
    }
  }
  const idat = deflateSync(raw);

  function chunk(type: string, data: Buffer): Buffer {
    const typeBuf = Buffer.from(type, 'ascii');
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crcBuf = Buffer.alloc(4);
    const crc = crc32(Buffer.concat([typeBuf, data]));
    crcBuf.writeUInt32BE(crc >>> 0);
    return Buffer.concat([len, typeBuf, data, crcBuf]);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', idat),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function crc32(buf: Buffer): number {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i]!;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? (0xedb88320 ^ (c >>> 1)) : c >>> 1;
    }
  }
  return ~c;
}

describe('pngRgba helper', () => {
  it('reads RGBA pixels and counts transparency', () => {
    const png = readPngRgba(
      tinyRgbaPng([
        0, 0, 0, 0, // transparent
        255, 0, 0, 255, // red opaque
        0, 255, 0, 128, // green semi
        0, 0, 255, 0, // transparent blue
      ]),
    );
    expect(png.width).toBe(2);
    expect(png.height).toBe(2);
    expect(pixelAt(png, 0, 0)).toEqual([0, 0, 0, 0]);
    expect(pixelAt(png, 1, 0)).toEqual([255, 0, 0, 255]);
    expect(pixelAt(png, 0, 1)[3]).toBe(128);
    expect(countFullyTransparent(png)).toBe(2);
  });

  it('rejects non-PNG buffers', () => {
    expect(() => readPngRgba(Buffer.from('not-a-png'))).toThrow(/not a PNG/);
  });
});
