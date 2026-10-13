/**
 * **A QR code encoder for the pairing code** (iOS design §4.2): byte mode, error correction L
 * or M, versions 1–40, the mask picked by the standard's penalty scores. The structure is
 * ISO/IEC 18004's, laid out as Nayuki's reference does, which is what every scanner reads.
 * Nothing here draws or names a colour: the answer is a square of booleans, and
 * {@link toSvgPath} turns it into one path of unit squares for a `currentColor` fill.
 */

export type Ecc = "L" | "M";

export type Code = {
  version: number;
  size: number;
  /** `modules[y][x]`: `true` is dark. No quiet zone: the page's margin is it. */
  modules: boolean[][];
};

// ISO/IEC 18004 Table 9, as Nayuki tabulates it: by version, index 0 unused.
const ECC_CODEWORDS_PER_BLOCK: Record<Ecc, readonly number[]> = {
  L: [
    0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30,
    30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
  ],
  M: [
    0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
  ],
};
const NUM_ECC_BLOCKS: Record<Ecc, readonly number[]> = {
  L: [
    0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13, 14,
    15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25,
  ],
  M: [
    0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25,
    26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
  ],
};
/** The level's two bits in the format information. */
const ECC_FORMAT_BITS: Record<Ecc, number> = { L: 1, M: 0 };

const MIN_VERSION = 1;
const MAX_VERSION = 40;

/** Every module of the symbol that is not a function pattern, before any codeword is placed. */
function rawDataModules(version: number): number {
  let result = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const numAlign = Math.floor(version / 7) + 2;
    result -= (25 * numAlign - 10) * numAlign - 55;
    if (version >= 7) result -= 36;
  }
  return result;
}

function rawCodewords(version: number): number {
  return Math.floor(rawDataModules(version) / 8);
}

/** How many codewords carry data at `version` and `ecc`. */
export function dataCodewords(version: number, ecc: Ecc): number {
  return (
    rawCodewords(version) - ECC_CODEWORDS_PER_BLOCK[ecc][version] * NUM_ECC_BLOCKS[ecc][version]
  );
}

/** The centres of the alignment patterns along one axis. */
export function alignmentPositions(version: number): number[] {
  if (version === 1) return [];
  const numAlign = Math.floor(version / 7) + 2;
  const size = 17 + 4 * version;
  const step = version === 32 ? 26 : Math.ceil((version * 4 + 4) / (numAlign * 2 - 2)) * 2;
  const result = [6];
  for (let pos = size - 7; result.length < numAlign; pos -= step) result.splice(1, 0, pos);
  return result;
}

/** The 15-bit format information: the level, the mask, their BCH remainder, and the mask
 *  pattern that keeps it from being all zero. */
export function formatBits(ecc: Ecc, mask: number): number {
  const data = (ECC_FORMAT_BITS[ecc] << 3) | mask;
  let rem = data;
  for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
  return ((data << 10) | rem) ^ 0x5412;
}

/** The 18-bit version information, for versions 7 and up. */
export function versionBits(version: number): number {
  let rem = version;
  for (let i = 0; i < 12; i++) rem = (rem << 1) ^ ((rem >>> 11) * 0x1f25);
  return (version << 12) | rem;
}

// --- Reed–Solomon over GF(2^8) with the primitive polynomial 0x11d.

function gfMultiply(x: number, y: number): number {
  let z = 0;
  for (let i = 7; i >= 0; i--) {
    z = (z << 1) ^ ((z >>> 7) * 0x11d);
    z ^= ((y >>> i) & 1) * x;
  }
  return z & 0xff;
}

/** The generator polynomial of `degree`, as its coefficients from x^(degree-1) down to x^0. */
function rsDivisor(degree: number): number[] {
  const result = new Array<number>(degree).fill(0);
  result[degree - 1] = 1;
  let root = 1;
  for (let i = 0; i < degree; i++) {
    for (let j = 0; j < result.length; j++) {
      result[j] = gfMultiply(result[j], root);
      if (j + 1 < result.length) result[j] ^= result[j + 1];
    }
    root = gfMultiply(root, 0x02);
  }
  return result;
}

/** The `degree` error-correction codewords of `data`. */
export function rsRemainder(data: readonly number[], degree: number): number[] {
  const divisor = rsDivisor(degree);
  const result = new Array<number>(degree).fill(0);
  for (const b of data) {
    const factor = b ^ (result.shift() ?? 0);
    result.push(0);
    divisor.forEach((coef, i) => {
      result[i] ^= gfMultiply(coef, factor);
    });
  }
  return result;
}

// --- The data: byte mode, padded, blocked, interleaved.

function appendBits(bits: number[], value: number, count: number): void {
  for (let i = count - 1; i >= 0; i--) bits.push((value >>> i) & 1);
}

function countBits(version: number): number {
  return version <= 9 ? 8 : 16;
}

function smallestVersion(byteCount: number, ecc: Ecc): number {
  for (let version = MIN_VERSION; version <= MAX_VERSION; version++) {
    const needed = 4 + countBits(version) + byteCount * 8;
    if (needed <= dataCodewords(version, ecc) * 8) return version;
  }
  throw new Error(`the text is too long for a QR code (${byteCount} bytes)`);
}

function dataBytes(bytes: readonly number[], version: number, ecc: Ecc): number[] {
  const bits: number[] = [];
  appendBits(bits, 0b0100, 4);
  appendBits(bits, bytes.length, countBits(version));
  for (const b of bytes) appendBits(bits, b, 8);
  const capacity = dataCodewords(version, ecc) * 8;
  appendBits(bits, 0, Math.min(4, capacity - bits.length));
  appendBits(bits, 0, (8 - (bits.length % 8)) % 8);
  for (let pad = 0xec; bits.length < capacity; pad ^= 0xec ^ 0x11) appendBits(bits, pad, 8);
  const out = new Array<number>(bits.length / 8).fill(0);
  bits.forEach((bit, i) => {
    out[i >>> 3] |= bit << (7 - (i & 7));
  });
  return out;
}

function interleave(data: readonly number[], version: number, ecc: Ecc): number[] {
  const numBlocks = NUM_ECC_BLOCKS[ecc][version];
  const blockEccLen = ECC_CODEWORDS_PER_BLOCK[ecc][version];
  const raw = rawCodewords(version);
  const numShortBlocks = numBlocks - (raw % numBlocks);
  const shortBlockLen = Math.floor(raw / numBlocks);
  const blocks: number[][] = [];
  for (let i = 0, k = 0; i < numBlocks; i++) {
    const length = shortBlockLen - blockEccLen + (i < numShortBlocks ? 0 : 1);
    const dat = data.slice(k, k + length);
    k += length;
    const correction = rsRemainder(dat, blockEccLen);
    const block = dat.slice();
    if (i < numShortBlocks) block.push(0);
    blocks.push(block.concat(correction));
  }
  const result: number[] = [];
  for (let i = 0; i < blocks[0].length; i++)
    blocks.forEach((block, j) => {
      // The padding a short block carries is not a codeword.
      if (i !== shortBlockLen - blockEccLen || j >= numShortBlocks) result.push(block[i]);
    });
  return result;
}

// --- The symbol.

class Matrix {
  readonly version: number;
  readonly size: number;
  readonly modules: boolean[][];
  private readonly ecc: Ecc;
  private readonly isFunction: boolean[][];

  constructor(version: number, ecc: Ecc) {
    this.version = version;
    this.ecc = ecc;
    this.size = 17 + 4 * version;
    this.modules = Array.from({ length: this.size }, () =>
      new Array<boolean>(this.size).fill(false),
    );
    this.isFunction = Array.from({ length: this.size }, () =>
      new Array<boolean>(this.size).fill(false),
    );
  }

  private setFunction(x: number, y: number, dark: boolean): void {
    this.modules[y][x] = dark;
    this.isFunction[y][x] = true;
  }

  drawFunctionPatterns(): void {
    for (let i = 0; i < this.size; i++) {
      this.setFunction(6, i, i % 2 === 0);
      this.setFunction(i, 6, i % 2 === 0);
    }
    this.drawFinder(3, 3);
    this.drawFinder(this.size - 4, 3);
    this.drawFinder(3, this.size - 4);
    const align = alignmentPositions(this.version);
    const last = align.length - 1;
    align.forEach((x, i) =>
      align.forEach((y, j) => {
        // Not where a finder is.
        if ((i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0)) return;
        this.drawAlignment(x, y);
      }),
    );
    this.drawFormatBits(0);
    this.drawVersion();
  }

  private drawFinder(x: number, y: number): void {
    for (let dy = -4; dy <= 4; dy++)
      for (let dx = -4; dx <= 4; dx++) {
        const dist = Math.max(Math.abs(dx), Math.abs(dy));
        const xx = x + dx;
        const yy = y + dy;
        if (xx >= 0 && xx < this.size && yy >= 0 && yy < this.size)
          this.setFunction(xx, yy, dist !== 2 && dist !== 4);
      }
  }

  private drawAlignment(x: number, y: number): void {
    for (let dy = -2; dy <= 2; dy++)
      for (let dx = -2; dx <= 2; dx++)
        this.setFunction(x + dx, y + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1);
  }

  drawFormatBits(mask: number): void {
    const bits = formatBits(this.ecc, mask);
    const bit = (i: number) => ((bits >>> i) & 1) !== 0;
    for (let i = 0; i <= 5; i++) this.setFunction(8, i, bit(i));
    this.setFunction(8, 7, bit(6));
    this.setFunction(8, 8, bit(7));
    this.setFunction(7, 8, bit(8));
    for (let i = 9; i < 15; i++) this.setFunction(14 - i, 8, bit(i));
    for (let i = 0; i < 8; i++) this.setFunction(this.size - 1 - i, 8, bit(i));
    for (let i = 8; i < 15; i++) this.setFunction(8, this.size - 15 + i, bit(i));
    this.setFunction(8, this.size - 8, true);
  }

  private drawVersion(): void {
    if (this.version < 7) return;
    const bits = versionBits(this.version);
    for (let i = 0; i < 18; i++) {
      const bit = ((bits >>> i) & 1) !== 0;
      const a = this.size - 11 + (i % 3);
      const b = Math.floor(i / 3);
      this.setFunction(a, b, bit);
      this.setFunction(b, a, bit);
    }
  }

  drawCodewords(data: readonly number[]): void {
    let i = 0;
    for (let right = this.size - 1; right >= 1; right -= 2) {
      if (right === 6) right = 5;
      for (let vert = 0; vert < this.size; vert++)
        for (let j = 0; j < 2; j++) {
          const x = right - j;
          const upward = ((right + 1) & 2) === 0;
          const y = upward ? this.size - 1 - vert : vert;
          if (!this.isFunction[y][x] && i < data.length * 8) {
            this.modules[y][x] = ((data[i >>> 3] >>> (7 - (i & 7))) & 1) !== 0;
            i++;
          }
        }
    }
  }

  applyMask(mask: number): void {
    for (let y = 0; y < this.size; y++)
      for (let x = 0; x < this.size; x++) {
        let invert: boolean;
        switch (mask) {
          case 0:
            invert = (x + y) % 2 === 0;
            break;
          case 1:
            invert = y % 2 === 0;
            break;
          case 2:
            invert = x % 3 === 0;
            break;
          case 3:
            invert = (x + y) % 3 === 0;
            break;
          case 4:
            invert = (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0;
            break;
          case 5:
            invert = ((x * y) % 2) + ((x * y) % 3) === 0;
            break;
          case 6:
            invert = (((x * y) % 2) + ((x * y) % 3)) % 2 === 0;
            break;
          default:
            invert = (((x + y) % 2) + ((x * y) % 3)) % 2 === 0;
        }
        if (!this.isFunction[y][x] && invert) this.modules[y][x] = !this.modules[y][x];
      }
  }

  /** The standard's four penalties (ISO/IEC 18004 §7.8.3): long runs, 2×2 blocks, finder-like
   *  runs with a light margin, and an unbalanced dark share. */
  penalty(): number {
    let result = 0;
    const size = this.size;
    const line = (at: (i: number) => boolean) => {
      let run = 0;
      let last: boolean | undefined;
      const colours: boolean[] = [];
      for (let i = 0; i < size; i++) {
        const c = at(i);
        colours.push(c);
        if (c === last) {
          run++;
          if (run === 5) result += 3;
          else if (run > 5) result += 1;
        } else {
          run = 1;
          last = c;
        }
      }
      // 1011101 with four light modules on either side.
      const pattern = [true, false, true, true, true, false, true];
      for (let i = 0; i + 7 <= size; i++) {
        let match = true;
        for (let k = 0; k < 7 && match; k++) if (colours[i + k] !== pattern[k]) match = false;
        if (!match) continue;
        const before = i >= 4 && colours.slice(i - 4, i).every((c) => !c);
        const after = i + 11 <= size && colours.slice(i + 7, i + 11).every((c) => !c);
        if (before || after) result += 40;
      }
    };
    for (let y = 0; y < size; y++) line((x) => this.modules[y][x]);
    for (let x = 0; x < size; x++) line((y) => this.modules[y][x]);
    for (let y = 0; y + 1 < size; y++)
      for (let x = 0; x + 1 < size; x++) {
        const c = this.modules[y][x];
        if (
          c === this.modules[y][x + 1] &&
          c === this.modules[y + 1][x] &&
          c === this.modules[y + 1][x + 1]
        )
          result += 3;
      }
    const dark = this.modules.flat().filter(Boolean).length;
    const total = size * size;
    const k = Math.ceil(Math.abs(dark * 20 - total * 10) / total) - 1;
    result += k * 10;
    return result;
  }

  pickMask(): number {
    let best = 0;
    let least = Infinity;
    for (let mask = 0; mask < 8; mask++) {
      this.applyMask(mask);
      this.drawFormatBits(mask);
      const score = this.penalty();
      if (score < least) {
        least = score;
        best = mask;
      }
      this.applyMask(mask);
    }
    this.applyMask(best);
    this.drawFormatBits(best);
    return best;
  }
}

/** The QR code of `text`'s UTF-8, at the smallest version that holds it. */
export function encode(text: string, ecc: Ecc = "M"): Code {
  const bytes = Array.from(new TextEncoder().encode(text));
  const version = smallestVersion(bytes.length, ecc);
  const symbol = new Matrix(version, ecc);
  symbol.drawFunctionPatterns();
  symbol.drawCodewords(interleave(dataBytes(bytes, version, ecc), version, ecc));
  symbol.pickMask();
  return { version, size: symbol.size, modules: symbol.modules };
}

/** One SVG path of unit squares, one per dark module, for a `viewBox` of `0 0 size size`. */
export function toSvgPath(code: Code): string {
  const parts: string[] = [];
  code.modules.forEach((row, y) =>
    row.forEach((dark, x) => {
      if (dark) parts.push(`M${x} ${y}h1v1h-1z`);
    }),
  );
  return parts.join("");
}
