# iOS Phase 1D: Settings → Devices Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The desktop window's side of pairing and phone access (iOS design §4.2, §6.4): a Settings › You › Devices page that turns phone access on and off, names extra addresses, shows the pairing QR code and asks the person to confirm a phone, lists and unpairs paired phones, and holds the keep-awake setting — over the nine `remote_*` commands 1C built, plus one more (`remote_pair_stop`).

**Architecture:** One new Settings group builder, `app/src/settings/devices.tsx`, in the shape of `thisMachine.tsx` (one shared read of `remote_status`, read again after every action and on focus; one action at a time), with a pairing panel that polls the status once a second while a window is open and raises a Radix `AlertDialog` when a phone offers to pair. The QR code is drawn by a small in-repo encoder, `app/src/qr.ts` (byte mode, EC L/M, versions 1–40, ISO/IEC 18004 structure as Nayuki's reference lays it out), as SVG rects coloured by the text colour, so no colour is named outside the theme. A `Copy pairing text` button beside the QR carries the same payload for a phone that cannot scan. One Rust addition: `remote_pair_stop` closes a window the person gave up on.

**Tech Stack:** React 19 + TypeScript (vitest, Testing Library, `@tauri-apps/api/mocks`), Radix AlertDialog, the settings set in `app/src/settings/components.tsx`; Rust/Tauri for the one new command; `app/src/bindings.ts` + `uiRpc.ts` regenerated.

**Spec:** `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` — §4.2 (pairing: QR with the public key, the addresses and a one-time 5-minute code; the window asks *Pair 'Noro's iPhone'? Key fingerprint 3F2A…* with Pair and Cancel; unpair from Settings), §4.1 (`machine.json` keys), §6.4 (keep-awake, off by default), §7 Phase 1, §8 Testing ("Settings → Devices" is a thing a person notices → a changelog fragment).

## Global Constraints

- **No colour is written anywhere but `app/src/theme/`** (AGENTS.md): the QR's rects use `fill="currentColor"`; the quiet zone is the page's own background.
- **Window copy follows `docs/ui-copy.md`:** sentence case, *purlis* lowercase, no "please"/"just"/"successfully", no exclamation marks; a button is a verb and what it acts on; an error says what happened and what to do.
- **A change people would notice gets a changelog fragment** `changes/<slug>.md`, never a CHANGELOG.md edit.
- **The Settings catalogue** (`app/src/settings/catalogue.ts`) lists every group; `catalogue.test.ts` reads the builders' sources, so the new builder joins its `BUILDERS` list and its `id:`/`label:` lines are written on consecutive lines as that test's regex expects.
- **In a React test, a precondition waits with Testing Library's `waitFor`**, not `vi.waitFor`; controls are found by role.
- **Nothing parses harness output; no `unsafe`; commit messages plain.** Keep-awake's label says *Mac* on a Mac and *machine* elsewhere (`onAMac()`).
- **The bindings are generated:** after the Rust command is added, `cargo test -p purlis-app --lib -- --ignored regenerate_the_typescript_the_ui_imports` rewrites `app/src/bindings.ts`; `uiRpc.ts` is derived from it by its own test.

## Review Focus

1. A QR the phone cannot scan: the encoder's structure must be the standard's (format and version bits, alignment positions, interleaving, masking). Task 0's tests pin the known-answer tables; the live check is the phone in 1F, and `Copy pairing text` is the way around it meanwhile.
2. A pairing window left open: the window ends on the host after five minutes whatever the page shows; the page's countdown reaches 0:00 and offers Pair a phone… again; Stop pairing calls `remote_pair_stop` so a window the person abandoned does not stay live.
3. An offer that arrives while the page is away: the status poll runs only while the pairing panel is open and the window is shown (`useWhileShown`); an offer that times out on the host (5 min) is withdrawn there, and the page learns so at its next read.
4. Phone access that refuses to start (no address, port in use): the toggle stays as the store says it (on) while the row's alert says why nothing listens, and the status line says *Not listening*.
5. Unpair while a phone is linked: the host closes the link at once (1B/1C); the page reads the list again and the row is gone.

---

### Task 0: The QR encoder

**Files:**
- Create: `app/src/qr.ts`, `app/src/qr.test.ts`

**Interfaces:**
- Produces: `encode(text: string, ecc?: "L" | "M"): Code` where `Code = { version: number; size: number; modules: boolean[][] }` (`modules[y][x]`, `true` is dark); `formatBits(ecc, mask): number`; `versionBits(version): number`; `dataCodewords(version, ecc): number`; `alignmentPositions(version): number[]`; `rsRemainder(data: number[], degree: number): number[]`; `toSvgPath(code: Code): string` (one path `d` of unit squares, for `<path fill="currentColor">`).

- [ ] **Step 1: Write the failing tests** (`app/src/qr.test.ts`)

```ts
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
    const L = [19, 34, 55, 80, 108, 136, 156, 194, 232, 274, 324, 370, 428, 461, 523, 589, 647, 721, 795, 861];
    const M = [16, 28, 44, 64, 86, 108, 124, 154, 182, 216];
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
      x = x << 1;
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

  it("pads a short message as the standard does and picks version 1", () => {
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
      [8, 0], [8, 1], [8, 2], [8, 3], [8, 4], [8, 5], [8, 7], [8, 8],
      [7, 8], [5, 8], [4, 8], [3, 8], [2, 8], [1, 8], [0, 8],
    ]);
    const second = read([
      [size - 1, 8], [size - 2, 8], [size - 3, 8], [size - 4, 8], [size - 5, 8], [size - 6, 8], [size - 7, 8], [size - 8, 8],
      [8, size - 7], [8, size - 6], [8, size - 5], [8, size - 4], [8, size - 3], [8, size - 2], [8, size - 1],
    ]);
    expect(first).toBe(second);
    expect(FORMAT.M).toContain(first);
  });

  it("fits a pairing payload of three hundred bytes well inside version 15", () => {
    expect(encode("purlis-pair:" + JSON.stringify({ k: "f".repeat(64), c: "0".repeat(64), a: ["192.168.9.155:7777", "100.64.0.9:7777"] }), "M").version).toBeLessThanOrEqual(15);
  });

  it("draws one unit square per dark module", () => {
    const code = encode("A", "L");
    const dark = code.modules.flat().filter(Boolean).length;
    expect(toSvgPath(code).split("M").length - 1).toBe(dark);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run (outside the sandbox, `app/`): `npx vitest run src/qr.test.ts`
Expected: FAIL: cannot find module `./qr`.

- [ ] **Step 3: Write `app/src/qr.ts`**

```ts
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
  L: [0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  M: [0, 10, 16, 18, 22, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28],
};
const NUM_ECC_BLOCKS: Record<Ecc, readonly number[]> = {
  L: [0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25],
  M: [0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49],
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
  return rawCodewords(version) - ECC_CODEWORDS_PER_BLOCK[ecc][version] * NUM_ECC_BLOCKS[ecc][version];
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
    const ecc = rsRemainder(dat, blockEccLen);
    const block = dat.slice();
    if (i < numShortBlocks) block.push(0);
    blocks.push(block.concat(ecc));
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

class Symbol {
  readonly size: number;
  readonly modules: boolean[][];
  private readonly isFunction: boolean[][];

  constructor(
    readonly version: number,
    private readonly ecc: Ecc,
  ) {
    this.size = 17 + 4 * version;
    this.modules = Array.from({ length: this.size }, () => new Array<boolean>(this.size).fill(false));
    this.isFunction = Array.from({ length: this.size }, () => new Array<boolean>(this.size).fill(false));
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
          case 0: invert = (x + y) % 2 === 0; break;
          case 1: invert = y % 2 === 0; break;
          case 2: invert = x % 3 === 0; break;
          case 3: invert = (x + y) % 3 === 0; break;
          case 4: invert = (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0; break;
          case 5: invert = ((x * y) % 2) + ((x * y) % 3) === 0; break;
          case 6: invert = (((x * y) % 2) + ((x * y) % 3)) % 2 === 0; break;
          default: invert = (((x + y) % 2) + ((x * y) % 3)) % 2 === 0;
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
        if (c === this.modules[y][x + 1] && c === this.modules[y + 1][x] && c === this.modules[y + 1][x + 1]) result += 3;
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
  const symbol = new Symbol(version, ecc);
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
```

- [ ] **Step 4: Run (the user), lint, commit**

Run: `npx vitest run src/qr.test.ts` → 9 pass; `npm run lint`, `npx prettier --check src/qr.ts src/qr.test.ts`.

```bash
git add app/src/qr.ts app/src/qr.test.ts && git commit -q -m "App: a QR code encoder for the pairing code"
```

---

### Task 1: `remote_pair_stop`

**Files:**
- Modify: `app/src-tauri/src/remote/gateway.rs` (one command), `app/src-tauri/src/ipc_commands.rs` (one line), `app/src/bindings.ts` + `app/src/uiRpc.ts` (regenerated: one `remotePairStop` line each)

- [ ] **Step 1: Write the failing test** (`app/src-tauri/src/remote/gateway_tests.rs`, append)

```rust
#[test]
fn stopping_a_pairing_closes_the_window_so_no_phone_can_take_it() {
    let config = tempfile::tempdir().unwrap();
    let remote = Remote::new(Some(config.path().to_path_buf()), None)
        .with_key(LinkKey::from_private([9u8; 32]));
    let desk = remote.desk_for_tests();
    desk.open_window(&LinkKey::from_private([9u8; 32]), HOST, vec![], Instant::now());
    assert!(desk.window(Instant::now()).is_some());
    remote.pair_stop();
    assert!(desk.window(Instant::now()).is_none(), "the window is closed");
}
```

- [ ] **Step 2: Implement**

In `gateway.rs`, on `Remote`: `pub fn pair_stop(&self) { self.desk.close_window(); }` and `#[cfg(test)] pub fn desk_for_tests(&self) -> Arc<Desk> { Arc::clone(&self.desk) }`; the command:

```rust
/// Stop pairing: the window closes, and a code shown on screen is spent.
#[tauri::command]
#[specta::specta]
pub fn remote_pair_stop(remote: tauri::State<'_, Remote>) {
    remote.pair_stop();
}
```

`ipc_commands.rs`: `remote::gateway::remote_pair_stop,` after `remote_pair_cancel`. Regenerate the bindings (Global Constraints) and port the one-line hunk into both files (`remotePairStop: () => __TAURI_INVOKE<null>("remote_pair_stop"),` after `remotePairCancel`).

- [ ] **Step 3: Run, commit**

`cargo test -p purlis-app remote::` (the bind test outside the sandbox); clippy; fmt.

```bash
git add app/src-tauri app/src/bindings.ts app/src/uiRpc.ts && git commit -q -m "App: stop a pairing from the window"
```

---

### Task 2: The Devices page

**Files:**
- Create: `app/src/settings/devices.tsx`, `app/src/settings/Devices.test.tsx`
- Modify: `app/src/settings/you.tsx` (the group after This machine), `app/src/settings/catalogue.ts` (`{ id: "you.devices", label: "Devices" }` after `you.machine`), `app/src/settings/catalogue.test.ts` (`"devices.tsx"` in `BUILDERS`), `app/src/App.css` (the `ui-devices-*` classes, at the end)

**Interfaces:**
- Consumes: `commands.remoteStatus/remoteSetOn/remoteSetAddresses/remotePairStart/remotePairStop/remotePairConfirm/remotePairCancel/remoteUnpair/remoteSetKeepAwake`, `RemoteStatus`, `PeerRow`, `PendingOffer` (bindings); `encode`/`toSvgPath` (Task 0); `Choice`, `Field` (components); `useWhileShown` (`../whileShown`); `onAMac` (`../tabKeys`); Radix `AlertDialog`.
- Produces: `devicesGroup(): SettingsGroup` (`you.devices`), `devicesChanged(): void` (read again from elsewhere).

- [ ] **Step 1: Write the failing tests** (`Devices.test.tsx`)

```tsx
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import type { PendingOffer, RemoteStatus } from "../bindings";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **Settings › You › Devices** (iOS design §4.2, §6.4), in the whole window: phone access on
 * and off with where it listens, the extra addresses, the paired phones and Unpair, Pair a
 * phone… with its QR code and the window's own Pair/Cancel on an offer, Stop pairing, and
 * the keep-awake setting — each reaching the core as the command it is, and the status read
 * again after it.
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => <div data-testid="pane">session {session}</div>,
}));

const PHONE = "01J9PHONE00000000000000000";
const QR = 'purlis-pair:{"version":1,"host_device":"01J9DEVCE00000000000000000","host_key":"ab","addresses":["192.168.9.155:7777"],"code":"cd"}';

let status: RemoteStatus;
let sent: { cmd: string; args: Record<string, unknown> }[];
let refuse: string | undefined;
let offer: PendingOffer | null;

function act(cmd: string, args: Record<string, unknown>): unknown {
  sent.push({ cmd, args });
  if (refuse !== undefined) throw refuse;
  if (cmd === "remote_set_on") {
    status.on = args.on as boolean;
    status.running = status.on;
    status.bound = status.on ? ["192.168.9.155:7777"] : [];
  }
  if (cmd === "remote_set_addresses") status.addresses = args.addresses as string[];
  if (cmd === "remote_set_keep_awake") status.keep_awake = args.on as boolean;
  if (cmd === "remote_unpair") status.peers = status.peers.filter((p) => p.device !== args.device);
  if (cmd === "remote_pair_start") return QR;
  if (cmd === "remote_pair_confirm") {
    status.peers.push({ device: PHONE, name: "Noro's iPhone", fingerprint: "3f2a9c1e0b7d4455", paired: 1_760_000_000 });
    offer = null;
    return null;
  }
  if (cmd === "remote_pair_cancel") offer = null;
  if (cmd === "remote_pair_stop") return null;
  return structuredClone(status);
}

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  status = {
    on: false,
    running: false,
    bound: [],
    port: 7777,
    addresses: [],
    device: "01J9DEVCE00000000000000000",
    fingerprint: null,
    peers: [{ device: "01J9OLDPHONE00000000000000", name: "Old phone", fingerprint: "0011223344556677", paired: 1_750_000_000 }],
    keep_awake: false,
    pending: null,
  };
  sent = [];
  refuse = undefined;
  offer = null;
  mockIPC(
    (cmd, args) => {
      if (cmd === "plane_at_launch") return { plane: null, from: null, why: "no plane here" };
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "planes_to_restore") return { windows: [], dropped: [], gone: [] };
      if (cmd === "remote_status") return { ...structuredClone(status), pending: offer };
      if (cmd.startsWith("remote_")) return act(cmd, (args ?? {}) as Record<string, unknown>);
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

/** Settings, at You › Devices, from the palette of a window with no project open. */
async function devices() {
  render(<App />);
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard("Settings…");
  await userEvent.keyboard("{Enter}");
  const nav = await screen.findByRole("navigation", { name: "Groups" });
  await userEvent.click(within(nav).getByRole("button", { name: "Devices" }));
  return screen.findByRole("region", { name: "Devices" });
}

const phones = () =>
  within(screen.getByRole("group", { name: "Paired phones" }))
    .queryAllByRole("listitem")
    .map((item) => item.textContent);

describe("You › Devices", () => {
  it("lists the paired phones, says access is off, and holds keep-awake off", async () => {
    await devices();
    await waitFor(() => expect(phones()).toEqual(["Old phone0011 2233 4455 6677Unpair Old phone"]));
    expect(screen.getByRole("switch", { name: "Phone access" })).not.toBeChecked();
    expect(screen.getByText("Not listening.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: /Keep this (Mac|machine) awake while chats are running/ })).not.toBeChecked();
  });

  it("turns phone access on and says where it listens", async () => {
    await devices();
    await userEvent.click(await screen.findByRole("switch", { name: "Phone access" }));
    await waitFor(() => expect(screen.getByText("Listening on 192.168.9.155:7777.")).toBeInTheDocument());
    expect(sent).toEqual([{ cmd: "remote_set_on", args: { on: true } }]);
  });

  it("says why access did not start, beside the switch", async () => {
    refuse = "no address could be listened on: 192.168.9.155:7777: address in use";
    await devices();
    await userEvent.click(await screen.findByRole("switch", { name: "Phone access" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("address in use");
  });

  it("keeps the extra addresses, one per line", async () => {
    await devices();
    const box = await screen.findByRole("textbox", { name: "Also listen on" });
    await userEvent.type(box, "100.64.0.9{Enter}10.0.0.5:7800");
    await userEvent.tab();
    await waitFor(() =>
      expect(sent).toEqual([{ cmd: "remote_set_addresses", args: { addresses: ["100.64.0.9", "10.0.0.5:7800"] } }]),
    );
  });

  it("pairs a phone: the QR code, the offer the window confirms, and the phone listed after", async () => {
    status.on = true;
    status.running = true;
    status.bound = ["192.168.9.155:7777"];
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Pair a phone…" }));
    expect(await screen.findByRole("img", { name: "Pairing code for your phone" })).toBeInTheDocument();
    expect(screen.getByText(/works for 5 minutes, once/)).toBeInTheDocument();
    expect(sent.at(-1)).toEqual({ cmd: "remote_pair_start", args: {} });

    offer = { ticket: "7", device: PHONE, name: "Noro's iPhone", fingerprint: "3f2a9c1e0b7d4455" };
    const asking = await screen.findByRole("alertdialog", { name: "Pair Noro's iPhone?" }, { timeout: 3000 });
    expect(asking).toHaveTextContent("3f2a 9c1e 0b7d 4455");
    await userEvent.click(within(asking).getByRole("button", { name: "Pair" }));

    await waitFor(() => expect(phones()).toContain("Noro's iPhone3f2a 9c1e 0b7d 4455Unpair Noro's iPhone"));
    expect(sent).toContainEqual({ cmd: "remote_pair_confirm", args: { ticket: "7" } });
    expect(screen.queryByRole("img", { name: "Pairing code for your phone" })).not.toBeInTheDocument();
  });

  it("cancels an offer, and stops a pairing", async () => {
    status.on = true;
    status.running = true;
    status.bound = ["192.168.9.155:7777"];
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Pair a phone…" }));
    await screen.findByRole("img", { name: "Pairing code for your phone" });
    offer = { ticket: "8", device: PHONE, name: "Noro's iPhone", fingerprint: "3f2a9c1e0b7d4455" };
    const asking = await screen.findByRole("alertdialog", { name: "Pair Noro's iPhone?" }, { timeout: 3000 });
    await userEvent.click(within(asking).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(sent).toContainEqual({ cmd: "remote_pair_cancel", args: { ticket: "8" } }));
    await userEvent.click(screen.getByRole("button", { name: "Stop pairing" }));
    await waitFor(() => expect(sent).toContainEqual({ cmd: "remote_pair_stop", args: {} }));
    expect(screen.queryByRole("img", { name: "Pairing code for your phone" })).not.toBeInTheDocument();
  });

  it("refuses to pair while access is off, and says so", async () => {
    refuse = "turn on phone access first: nothing is listening";
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Pair a phone…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("turn on phone access first");
  });

  it("unpairs a phone", async () => {
    await devices();
    await userEvent.click(await screen.findByRole("button", { name: "Unpair Old phone" }));
    await waitFor(() => expect(phones()).toEqual([]));
    expect(sent).toEqual([{ cmd: "remote_unpair", args: { device: "01J9OLDPHONE00000000000000" } }]);
  });

  it("keeps the machine awake while chats run, when asked", async () => {
    await devices();
    await userEvent.click(await screen.findByRole("switch", { name: /awake while chats are running/ }));
    await waitFor(() => expect(sent).toEqual([{ cmd: "remote_set_keep_awake", args: { on: true } }]));
    expect(screen.getByRole("switch", { name: /awake while chats are running/ })).toBeChecked();
  });
});
```

- [ ] **Step 2: Run to see it fail** (`npx vitest run src/settings/Devices.test.tsx`) — no group "Devices".

- [ ] **Step 3: Write `devices.tsx`**

```tsx
import { useState, useSyncExternalStore, type ReactNode } from "react";
import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { commands, type PendingOffer, type RemoteStatus } from "../bindings";
import { onAMac } from "../tabKeys";
import { useWhileShown } from "../whileShown";
import { encode, toSvgPath } from "../qr";
import { Choice, Field } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Devices** (iOS design §4.2, §6.4): phone access on this machine — whether it listens
 * and where, the paired phones, pairing a new one, and keeping the machine awake while chats
 * run. Nothing here is a project's: it is the machine store's `gateway` and `keep_awake` keys
 * and `peers.json` beside it.
 *
 * **One read, shared by the group's rows** (`remote_status`), read again after every action,
 * on focus, and once a second while a pairing window is open (the offer a phone makes arrives
 * in it). Each row's last refusal is said in that row, in the core's words.
 *
 * **Pairing is the window's to confirm** (ADR 0090): the QR code pairs nothing by itself; a
 * phone that proves the code is offered here, by name and key fingerprint, with Pair and
 * Cancel. The window on the host lasts five minutes whatever this page shows; Stop pairing
 * closes it sooner.
 */
export function devicesGroup(): SettingsGroup {
  return {
    id: "you.devices",
    label: "Devices",
    help: "A phone paired with this machine steers its chats over your network, straight to this machine: on your Wi-Fi, or over Tailscale. Nothing goes through anyone else.",
    settings: [access, addresses, phones, pair, awake],
  };
}

export const PAIRING_MINUTES = 5;

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; status: RemoteStatus };

/** A pairing window this page opened: the QR's text and when it was opened. */
type Pairing = { text: string; openedAt: number };

type Held = {
  read: Read;
  refused: Readonly<Record<string, readonly string[]>>;
  pairing: Pairing | undefined;
};

const START: Held = { read: { state: "reading" }, refused: {}, pairing: undefined };
let held: Held = START;
const listeners = new Set<() => void>();
let reading = 0;

function set(change: (was: Held) => Held) {
  held = change(held);
  for (const one of listeners) one();
}

async function reread() {
  const mine = ++reading;
  const said = await commands
    .remoteStatus()
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (mine !== reading) return;
  set((was) => ({
    ...was,
    read:
      said.status === "error"
        ? { state: "trouble", trouble: said.error }
        : { state: "read", status: said.data },
  }));
}

function cameBack() {
  if (document.visibilityState === "hidden") return;
  void reread();
}

/** Something elsewhere changed phone access: read again if the group is up. */
export function devicesChanged(): void {
  if (listeners.size > 0) void reread();
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = { ...START, pairing: held.pairing };
    void reread();
    window.addEventListener("focus", cameBack);
    document.addEventListener("visibilitychange", cameBack);
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
    if (listeners.size > 0) return;
    window.removeEventListener("focus", cameBack);
    document.removeEventListener("visibilitychange", cameBack);
  };
}

type Done = { status: "ok" } | { status: "error"; error: string };

/** Every action, one at a time, each followed by a read. */
let queue: Promise<void> = Promise.resolve();

function acting(setting: string, act: () => Promise<Done>) {
  const work = async () => {
    const why = await act()
      .then((done) => (done.status === "error" ? done.error : undefined))
      .catch((err: unknown) => String(err));
    set((was) => {
      const refused = { ...was.refused };
      if (why === undefined) Reflect.deleteProperty(refused, setting);
      else refused[setting] = [why];
      return { ...was, refused };
    });
    await reread();
  };
  queue = queue.then(work, work);
}

function useDevices(setting: string) {
  const now = useSyncExternalStore(subscribe, () => held);
  return { read: now.read, error: now.refused[setting], pairing: now.pairing };
}

function waiting(read: Read): ReactNode | undefined {
  if (read.state === "reading") return <p className="ui-setting-help">Reading…</p>;
  if (read.state === "trouble")
    return (
      <p className="ui-setting-help" role="alert">
        {read.trouble}
      </p>
    );
  return undefined;
}

/** A key fingerprint in groups of four, as the phone shows it, so the two can be read against
 *  each other. */
export function spaced(fingerprint: string): string {
  return fingerprint.replace(/(.{4})(?=.)/g, "$1 ");
}

const access: LiveSetting = {
  id: "you.devices.access",
  label: "Phone access",
  help: "Listens for your paired phones on this machine's network address, and on the addresses below. Off, no phone can connect. It does not reach past your network: pair over Wi-Fi, or over Tailscale.",
  useControl: function useAccess() {
    const id = "you.devices.access";
    const { read, error } = useDevices(id);
    const status = read.state === "read" ? read.status : undefined;
    return {
      error,
      control: (ids) => (
        <>
          <Choice
            kind="toggle"
            ids={ids}
            checked={status?.on ?? false}
            disabled={status === undefined}
            onCheckedChange={(to) => acting(id, () => commands.remoteSetOn(to))}
          />
          <p className="ui-setting-help ui-devices-standing">
            {status === undefined
              ? "Reading…"
              : status.bound.length > 0
                ? `Listening on ${status.bound.join(", ")}.`
                : "Not listening."}
          </p>
        </>
      ),
    };
  },
};

const addresses: LiveSetting = {
  id: "you.devices.addresses",
  label: "Also listen on",
  help: "One address per line, with a port after a colon where it is not the default: a Tailscale address, say. Never every interface. It applies the next time access starts, or now while it is on.",
  useControl: function useAddresses() {
    const id = "you.devices.addresses";
    const { read, error } = useDevices(id);
    const onDisk = read.state === "read" ? read.status.addresses.join("\n") : "";
    const [draft, setDraft] = useState<string | undefined>(undefined);
    const commit = () => {
      if (draft === undefined) return;
      const lines = draft
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line !== "");
      setDraft(undefined);
      if (lines.join("\n") === onDisk) return;
      acting(id, () => commands.remoteSetAddresses(lines));
    };
    return {
      error,
      control: (ids) => (
        <Field
          ids={ids}
          kind="list"
          value={draft ?? onDisk}
          minRows={2}
          onChange={setDraft}
          onCommit={commit}
        />
      ),
    };
  },
};

const phones: LiveSetting = {
  id: "you.devices.phones",
  label: "Paired phones",
  help: "Each phone that may steer this machine's chats. Unpair takes it off and closes its link at once; pair it again to let it back.",
  useControl: function usePhones() {
    const id = "you.devices.phones";
    const { read, error } = useDevices(id);
    return {
      grouped: true,
      error,
      control: (ids) => {
        const instead = waiting(read);
        if (instead !== undefined || read.state !== "read") return instead;
        const { peers } = read.status;
        return (
          <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
            {peers.length === 0 ? (
              <p className="ui-setting-help">No phone is paired. Pair one below.</p>
            ) : (
              <ul className="ui-machine-list">
                {peers.map((peer) => (
                  <li key={peer.device} className="ui-machine-entry">
                    <span className="ui-machine-name">{peer.name}</span>
                    <span className="ui-machine-path">{spaced(peer.fingerprint)}</span>
                    <button
                      type="button"
                      className="ui-setting-reset"
                      tabIndex={0}
                      aria-label={`Unpair ${peer.name}`}
                      onClick={() => acting(id, () => commands.remoteUnpair(peer.device))}
                    >
                      Unpair
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        );
      },
    };
  },
};

/** The time left on a pairing window, as `m:ss`, never below 0:00. */
function timeLeft(openedAt: number, now: number): string {
  const left = Math.max(0, PAIRING_MINUTES * 60 - Math.floor((now - openedAt) / 1000));
  return `${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`;
}

function QrCode({ text }: { text: string }) {
  const code = encode(text, "M");
  return (
    <svg
      className="ui-devices-qr"
      role="img"
      aria-label="Pairing code for your phone"
      viewBox={`0 0 ${code.size} ${code.size}`}
      shapeRendering="crispEdges"
    >
      <path d={toSvgPath(code)} fill="currentColor" />
    </svg>
  );
}

/** The window's own question on a phone's offer (iOS design §4.2 step 3). */
function Offer({ offer, onDone }: { offer: PendingOffer; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const answer = (confirm: boolean) => {
    setBusy(true);
    acting("you.devices.pair", async () => {
      const done = confirm
        ? await commands.remotePairConfirm(offer.ticket)
        : await commands.remotePairCancel(offer.ticket);
      if (confirm && done.status === "ok") onDone();
      return done;
    });
  };
  return (
    <AlertDialog.Root open onOpenChange={(open) => !open && !busy && answer(false)}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="asking" />
        <AlertDialog.Content className="warning">
          <AlertDialog.Title>Pair {offer.name}?</AlertDialog.Title>
          <AlertDialog.Description>
            Its key fingerprint is <code>{spaced(offer.fingerprint)}</code>. Check it against
            the phone's screen; pair only when the two match.
          </AlertDialog.Description>
          <div className="ui-setting-actions">
            <AlertDialog.Cancel asChild>
              <button type="button" className="ui-setting-reset" disabled={busy} onClick={() => answer(false)}>
                Cancel
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button type="button" className="ui-setting-reset" disabled={busy} onClick={() => answer(true)}>
                Pair
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

const pair: LiveSetting = {
  id: "you.devices.pair",
  label: "Pair a phone",
  help: "Shows a code for purlis on your phone to scan. It works for five minutes, once, and the phone is paired only after you confirm it here by its name and key fingerprint.",
  useControl: function usePair() {
    const id = "you.devices.pair";
    const { read, error, pairing } = useDevices(id);
    const status = read.state === "read" ? read.status : undefined;
    const [now, setNow] = useState(() => Date.now());
    // While a window is open: the status once a second (the offer arrives in it) and the clock.
    useWhileShown(
      1000,
      () => {
        setNow(Date.now());
        void reread();
      },
      pairing !== undefined,
    );
    const expired = pairing !== undefined && now - pairing.openedAt >= PAIRING_MINUTES * 60_000;
    const stop = () => {
      set((was) => ({ ...was, pairing: undefined }));
      acting(id, () => commands.remotePairStop().then(() => ({ status: "ok" as const })));
    };
    return {
      error,
      control: (ids) => (
        <div className="ui-devices-pairing">
          {pairing === undefined || expired ? (
            <button
              type="button"
              id={ids.id}
              className="ui-setting-reset"
              tabIndex={0}
              aria-describedby={ids.describedBy}
              onClick={() =>
                acting(id, async () => {
                  const started = await commands.remotePairStart();
                  if (started.status === "ok")
                    set((was) => ({ ...was, pairing: { text: started.data, openedAt: Date.now() } }));
                  return started;
                })
              }
            >
              Pair a phone…
            </button>
          ) : (
            <>
              <QrCode text={pairing.text} />
              <p className="ui-setting-help">
                Scan it with purlis on your phone. The code works for 5 minutes, once:{" "}
                {timeLeft(pairing.openedAt, now)} left.
              </p>
              <div className="ui-setting-actions">
                <button
                  type="button"
                  className="ui-setting-reset"
                  tabIndex={0}
                  onClick={() => void navigator.clipboard?.writeText(pairing.text)}
                >
                  Copy pairing text
                </button>
                <button type="button" className="ui-setting-reset" tabIndex={0} onClick={stop}>
                  Stop pairing
                </button>
              </div>
            </>
          )}
          {expired && <p className="ui-setting-help">The last code ran out. Pair a phone… shows a new one.</p>}
          {pairing !== undefined && status?.pending && (
            <Offer offer={status.pending} onDone={() => set((was) => ({ ...was, pairing: undefined }))} />
          )}
        </div>
      ),
    };
  },
};

const awake: LiveSetting = {
  id: "you.devices.awake",
  label: `Keep this ${onAMac() ? "Mac" : "machine"} awake while chats are running`,
  help: "While a chat is working or waiting on you, this machine does not go to sleep on its own; the display still may. Off, it sleeps as it always did, and a phone away from it loses the link.",
  useControl: function useAwake() {
    const id = "you.devices.awake";
    const { read, error } = useDevices(id);
    const status = read.state === "read" ? read.status : undefined;
    return {
      error,
      control: (ids) => (
        <Choice
          kind="toggle"
          ids={ids}
          checked={status?.keep_awake ?? false}
          disabled={status === undefined}
          onCheckedChange={(to) => acting(id, () => commands.remoteSetKeepAwake(to))}
        />
      ),
    };
  },
};
```

`you.tsx`: `import { devicesGroup } from "./devices";` and `devicesGroup(),` after `thisMachineGroup(),`. `catalogue.ts`: `{ id: "you.devices", label: "Devices" },` after `you.machine`. `catalogue.test.ts`: `"devices.tsx",` after `"thisMachine.tsx",`. `App.css` (end):

```css
/* You › Devices (iOS design §4.2): the pairing code, drawn in the text colour on the page's
   own ground, which is its quiet zone. */
.ui-devices-pairing {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.5rem;
}

.ui-devices-qr {
  width: 12rem;
  height: 12rem;
  padding: 0.75rem;
  background: var(--surface-raised);
  border: 1px solid var(--border-subtle);
  border-radius: 4px;
}

.ui-devices-standing {
  margin: 0.25rem 0 0;
}
```

Facts to check while writing: `Choice kind="toggle"` renders `role="switch"` (the tests name it so; else `checkbox`); `Field kind="list"` commits on blur through `onCommit` (see `AskPersona.tsx`); the catalogue test's `pair` regex needs `id:` and `label:` on consecutive lines in `devicesGroup()`; `navigator.clipboard` may be undefined in jsdom (the `?.` guards it); the AlertDialog's `className="warning"` is what `DeleteWorkspace.tsx` uses.

- [ ] **Step 4: Run (the user), lint, commit**

`npx vitest run src/settings/Devices.test.tsx src/settings/catalogue.test.ts src/copy.test.ts src/settings/ThisMachine.test.tsx` → pass; `npm run typecheck`; `npm run lint`; `npx prettier --check src/settings/devices.tsx src/settings/Devices.test.tsx`.

```bash
git add app/src && git commit -q -m "App: Settings › You › Devices: phone access, pairing and keep-awake"
```

---

### Task 3: The changelog fragment and the dossier

**Files:**
- Create: `changes/pair-a-phone-from-settings.md`
- Modify: `docs/superpowers/specs/2026-10-11-ios-mobile-design.md` (§7 row: "**1D, Settings → Devices, is built**"), `~/purlis-research/README.md` §10

```markdown
### Added

- **Pair a phone with this machine.** Settings › You › Devices turns phone access on, shows a
  pairing code for purlis on your phone to scan, asks you to confirm the phone by its name and
  key fingerprint, and lists the phones you paired, each with Unpair. A phone pairs only while a
  five-minute code is live, and only after you confirm it here.
- **Keep this Mac awake while chats are running.** A setting on the same page, off by default:
  while a chat is working or waiting on you, the machine does not sleep on its own (the display
  still may).
```

```bash
git add changes docs && git commit -q -m "Changelog: pairing and keep-awake; design: Phase 1D built"
```

## Self-review (done while writing)

- **Spec coverage:** §4.2 steps 1 and 3 (QR; the window's Pair/Cancel by name and fingerprint) → Task 2; "Unpair (desktop Settings)" → Task 2; §6.4 the setting, off by default → Task 2 (the holding of the assertion is 1C's); §8 "a thing a person notices" → Task 3's fragment. The listener's addresses (1C's `remote_set_addresses`) get a row so a Tailscale address can be named without editing `machine.json`.
- **Placeholders:** none; every step carries its code.
- **Type consistency:** `RemoteStatus`/`PendingOffer`/`PeerRow` as 1C's bindings export them (`ticket: string`, `paired: number`); `encode`/`toSvgPath` as Task 0 names them; `remotePairStop` as Task 1 adds it.
- **Review Focus:** each line names where it is pinned or why it is not.
