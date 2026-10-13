import { describe, expect, it } from "vitest";
import {
  alignmentPositions,
  dataCodewords,
  encode,
  formatBits,
  rsRemainder,
  toSvgPath,
  versionBits,
} from "./qr";

/**
 * **The QR encoder draws what a scanner reads** (iOS design §4.2): the standard's own tables
 * for the format and version bits, the alignment grid and the capacities, a Reed–Solomon
 * remainder that divides, and a code with its finders, timing and dark module where they go.
 */

// ISO/IEC 18004 Table C.1: the 15-bit format information, by level and mask.
const FORMAT: Record<"L" | "M", number[]> = {
  L: [0x77c4, 0x72f3, 0x7daa, 0x789d, 0x662f, 0x6318, 0x6c41, 0x6976],
  M: [0x5412, 0x5125, 0x5e7c, 0x5b4b, 0x45f9, 0x40ce, 0x4f97, 0x4aa0],
};

describe("qr", () => {
  it("writes the standard's format bits for every level and mask", () => {
    for (const ecc of ["L", "M"] as const)
      for (let mask = 0; mask < 8; mask++) expect(formatBits(ecc, mask)).toBe(FORMAT[ecc][mask]);
  });

  it("writes the standard's version bits", () => {
    expect(versionBits(7)).toBe(0x07c94);
    expect(versionBits(8)).toBe(0x085bc);
    expect(versionBits(9)).toBe(0x09a99);
    expect(versionBits(10)).toBe(0x0a4d3);
  });

  it("has the standard's data capacity, by version and level", () => {
    const L = [
      19, 34, 55, 80, 108, 136, 156, 194, 232, 274, 324, 370, 428, 461, 523, 589, 647, 721, 795,
      861,
    ];
    const M = [
      16, 28, 44, 64, 86, 108, 124, 154, 182, 216, 254, 290, 334, 365, 415, 453, 507, 563, 627, 669,
    ];
    L.forEach((n, i) => expect(dataCodewords(i + 1, "L"), `L${i + 1}`).toBe(n));
    M.forEach((n, i) => expect(dataCodewords(i + 1, "M"), `M${i + 1}`).toBe(n));
  });

  it("places the alignment patterns where the standard does", () => {
    expect(alignmentPositions(1)).toEqual([]);
    expect(alignmentPositions(2)).toEqual([6, 18]);
    expect(alignmentPositions(7)).toEqual([6, 22, 38]);
    expect(alignmentPositions(10)).toEqual([6, 28, 50]);
    expect(alignmentPositions(14)).toEqual([6, 26, 46, 66]);
    expect(alignmentPositions(32)).toEqual([6, 34, 60, 86, 112, 138]);
  });

  it("appends a remainder that makes the codeword divisible by the generator", () => {
    // GF(2^8) with 0x11d, built here and not borrowed from the encoder.
    const exp = new Array<number>(256).fill(0);
    const log = new Array<number>(256).fill(0);
    for (let i = 0, x = 1; i < 255; i++) {
      exp[i] = x;
      log[x] = i;
      x <<= 1;
      if (x & 0x100) x ^= 0x11d;
    }
    const mul = (a: number, b: number) => (a === 0 || b === 0 ? 0 : exp[(log[a] + log[b]) % 255]);
    const data = Array.from({ length: 26 }, (_, i) => (i * 37 + 11) & 0xff);
    const degree = 10;
    const codeword = data.concat(rsRemainder(data, degree));
    expect(codeword).toHaveLength(36);
    for (let i = 0; i < degree; i++) {
      // c(α^i) over the whole codeword, highest degree first.
      const root = exp[i];
      let value = 0;
      for (const c of codeword) value = mul(value, root) ^ c;
      expect(value, `root α^${i}`).toBe(0);
    }
  });

  it("picks version 1 for a short message", () => {
    const code = encode("A", "L");
    expect(code.version).toBe(1);
    expect(code.size).toBe(21);
  });

  it("draws the finders, the timing and the dark module where a scanner looks", () => {
    const code = encode("purlis-pair:" + "x".repeat(240), "M");
    expect(code.version).toBeGreaterThanOrEqual(10);
    expect(code.version).toBeLessThanOrEqual(14);
    const { size, modules } = code;
    expect(size).toBe(17 + 4 * code.version);
    // Finder at the top left: a 7×7 ring, dark-light-dark.
    for (let i = 0; i < 7; i++) {
      expect(modules[0][i]).toBe(true);
      expect(modules[6][i]).toBe(true);
      expect(modules[i][0]).toBe(true);
      expect(modules[i][6]).toBe(true);
    }
    expect(modules[1][1]).toBe(false);
    expect(modules[3][3]).toBe(true);
    // The separator around it is light.
    expect(modules[7][7]).toBe(false);
    // Timing patterns alternate between the finders, dark at even positions.
    for (let i = 8; i < size - 8; i++) {
      expect(modules[6][i], `row 6, ${i}`).toBe(i % 2 === 0);
      expect(modules[i][6], `column 6, ${i}`).toBe(i % 2 === 0);
    }
    // The dark module beside the bottom-left finder.
    expect(modules[size - 8][8]).toBe(true);
    // The two copies of the format bits agree, and name the level.
    const read = (cells: [number, number][]) =>
      cells.reduce((acc, [x, y], i) => acc | (modules[y][x] ? 1 << i : 0), 0);
    const first = read([
      [8, 0],
      [8, 1],
      [8, 2],
      [8, 3],
      [8, 4],
      [8, 5],
      [8, 7],
      [8, 8],
      [7, 8],
      [5, 8],
      [4, 8],
      [3, 8],
      [2, 8],
      [1, 8],
      [0, 8],
    ]);
    const second = read([
      [size - 1, 8],
      [size - 2, 8],
      [size - 3, 8],
      [size - 4, 8],
      [size - 5, 8],
      [size - 6, 8],
      [size - 7, 8],
      [size - 8, 8],
      [8, size - 7],
      [8, size - 6],
      [8, size - 5],
      [8, size - 4],
      [8, size - 3],
      [8, size - 2],
      [8, size - 1],
    ]);
    expect(first).toBe(second);
    expect(FORMAT.M).toContain(first);
  });

  it("fits a pairing payload of three hundred bytes well inside version 15", () => {
    const payload =
      "purlis-pair:" +
      JSON.stringify({
        k: "f".repeat(64),
        c: "0".repeat(64),
        a: ["192.168.9.155:7777", "100.64.0.9:7777"],
      });
    expect(encode(payload, "M").version).toBeLessThanOrEqual(15);
  });

  it("draws one unit square per dark module", () => {
    const code = encode("A", "L");
    const dark = code.modules.flat().filter(Boolean).length;
    expect(toSvgPath(code).split("M").length - 1).toBe(dark);
  });
});
