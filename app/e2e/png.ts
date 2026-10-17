// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { inflateSync } from "node:zlib";

/**
 * **A PNG, read back into pixels**, for a scenario test that looks at a picture the app took
 * (the theme specimen, #2330). Node's own inflate and the five row filters of the PNG standard:
 * 8-bit RGB or RGBA, not interlaced, which is what WebKit and cairo write. Anything else is
 * refused in a sentence rather than read wrong.
 */
export type Pixels = { width: number; height: number; at: (x: number, y: number) => number[] };

export function readPng(png: Buffer): Pixels {
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  if (!signature.every((byte, n) => png[n] === byte)) throw new Error("not a PNG");
  let width = 0;
  let height = 0;
  let channels = 0;
  const data: Buffer[] = [];
  for (let at = 8; at < png.length; ) {
    const length = png.readUInt32BE(at);
    const kind = png.toString("latin1", at + 4, at + 8);
    const body = png.subarray(at + 8, at + 8 + length);
    if (kind === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      const [depth, colour, , , interlace] = body.subarray(8, 13);
      if (depth !== 8 || interlace !== 0 || (colour !== 2 && colour !== 6))
        throw new Error(`a PNG this reader does not read: depth ${depth}, colour type ${colour}`);
      channels = colour === 6 ? 4 : 3;
    } else if (kind === "IDAT") data.push(body);
    else if (kind === "IEND") break;
    at += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(data));
  const stride = width * channels;
  const pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? pixels[y * stride + x - channels] : 0;
      const b = y > 0 ? pixels[(y - 1) * stride + x] : 0;
      const c = x >= channels && y > 0 ? pixels[(y - 1) * stride + x - channels] : 0;
      const p = a + b - c;
      const [pa, pb, pc] = [Math.abs(p - a), Math.abs(p - b), Math.abs(p - c)];
      const paeth = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      const predicted = [0, a, b, (a + b) >> 1, paeth][filter];
      if (predicted === undefined) throw new Error(`a PNG row filter this reader does not know: ${filter}`);
      pixels[y * stride + x] = (line[x] + predicted) & 0xff;
    }
  }
  return {
    width,
    height,
    at: (x, y) => [...pixels.subarray((y * width + x) * channels, (y * width + x) * channels + 3)],
  };
}
