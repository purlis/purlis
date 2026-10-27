// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A workspace's colour, as a hue shift of one colour** (charter-app#281).
 *
 * The operator: *"workspace collor can add some filter and make same theme but with different
 * collor"*. So a colour is not a second theme and not a palette of its own: it is a **hue**, and
 * a token it tints keeps everything else it had. Which tokens, and the hue each workspace colour
 * gives, are the theme's recipe, which the core's theme engine resolves and hands the window with
 * the theme (`purlis_core::theme::Tint`, #2329, #2393); this file is only the arithmetic, the
 * same as the engine's and held to it by the shared fixture (`theme/tint-cases.json`).
 *
 * # Why OKLCH, and why the luminance is kept exactly
 *
 * The shift is done in OKLCH — lightness, chroma, hue — because a hue turned there keeps its
 * perceived lightness, where HSL's does not (HSL's yellow and blue at one "lightness" are
 * nothing alike). But perceived lightness is still not WCAG's relative luminance, and WCAG's is
 * what `contrast.test.ts` holds every pair to. So after the hue is set, the lightness is searched
 * for until the tinted colour has **the relative luminance the original had**, and each channel
 * is rounded to its byte on the side of that luminance away from the middle (`rounded`): every
 * contrast ratio a theme cleared between a dark shade and a light one, its tinted shades clear
 * too, whatever the hue. The tests sweep every whole degree on both built-ins to hold that.
 *
 * **A grey gets a little colour; a colour keeps its own amount.** The layer shades are neutral
 * greys with no hue to turn, so they are given {@link NEUTRAL_CHROMA} — a tint, felt rather than
 * noticed, which is ADR 0036's rule for the strips — and the accent keeps the chroma it had. A
 * colour the screen cannot show at that chroma is brought into sRGB by lowering the chroma, never
 * the lightness. White stays white: nothing brighter than white is a colour.
 *
 * Pure functions of strings and numbers; no theme is imported, so `theme.ts` can import this.
 */

/** How much colour a neutral grey is given: a step a person feels on a strip rather than sees. */
export const NEUTRAL_CHROMA = 0.03;

/** Below this chroma a colour counts as grey, and has no hue of its own worth keeping. */
const GREY = 0.02;

/** `#rrggbb` and nothing else: what a workspace writes for a colour of its own. */
const CUSTOM = /^#[0-9a-fA-F]{6}$/;

type Vec3 = [number, number, number];

/**
 * The hue of a workspace's own colour, a `#rrggbb`. `undefined` for anything else (a palette
 * name's hue is the theme's, `theme.ts`' `hueIn`), and for a grey, which has no hue to give.
 */
export function hueOf(colour: string | null | undefined): number | undefined {
  if (colour === null || colour === undefined || !CUSTOM.test(colour)) return undefined;
  const [, a, b] = oklab(parse(colour).rgb);
  if (Math.hypot(a, b) < GREY) return undefined;
  return degrees(Math.atan2(b, a));
}

/** The rule a tint's hue is chosen under, as the engine hands it: `token`, tinted, stays at
 *  least `distance` from `from` in OKLab. */
export type Apart = {
  token: string;
  from: string;
  distance: number;
  /** A rule held alone gives less colour only to its own sides (#3130). */
  alone?: boolean;
};

/** One rule as a theme draws it: the value of each side, whether the tint turns each, and the
 *  distance it holds (`purlis_core::theme::Rule`). */
export type Rule = {
  token: string;
  tokenTinted: boolean;
  from: string;
  fromTinted: boolean;
  distance: number;
};

/** The step the chroma a tint gives is lowered by to hold its accent apart. */
const CHROMA_STEP = 0.005;

/**
 * **The most chroma a tint at `hue` gives** (`purlis_core::theme::apart_chroma_of`): no limit
 * (`Infinity`) when every rule holds as tinted; else the largest multiple of
 * {@link CHROMA_STEP} under the tinted sides' own chroma that holds them all, so the hue stays
 * the workspace's; `undefined` when not even a grey does. Every rule holds whichever side a
 * theme tints.
 */
export function apartChroma(rules: readonly Rule[], hue: number): number | undefined {
  // The same rules at the same hue give the same answer: a strip of tabs, a list of personas and
  // every theme the core answers again ask it over and over, and where a rule is not held at
  // full colour (Purlis's red, orange and pink, #3034) the answer is a search.
  const key = `${hue} ${rules
    .map((one) => `${one.token}${one.tokenTinted} ${one.from}${one.fromTinted} ${one.distance}`)
    .join(" ")}`;
  if (answered.has(key)) return answered.get(key);
  const answer = searchedChroma(rules, hue);
  if (answered.size >= MOST_ANSWERED) answered.delete(answered.keys().next().value as string);
  answered.set(key, answer);
  return answer;
}

/** Each answer {@link apartChroma} gave, by its rules and hue, the oldest out first. */
const answered = new Map<string, number | undefined>();

/** How many answers {@link answered} keeps. */
const MOST_ANSWERED = 1024;

/** {@link apartChroma}, worked out. */
function searchedChroma(rules: readonly Rule[], hue: number): number | undefined {
  const side = (value: string, tinted: boolean, most: number) =>
    tinted ? tintHex(value, hue, most) : value;
  const far = (most: number) =>
    rules.every((rule) => {
      const token = side(rule.token, rule.tokenTinted, most);
      const from = side(rule.from, rule.fromTinted, most);
      // A value that is not a colour has nothing to be near.
      if (!token.startsWith("#") || !from.startsWith("#")) return true;
      return apartIn(oklab(parse(token).rgb), oklab(parse(from).rgb)) >= rule.distance;
    });
  if (far(Number.POSITIVE_INFINITY)) return Number.POSITIVE_INFINITY;
  // The most colour any tinted side is given: a grey is given {@link NEUTRAL_CHROMA}.
  let had = 0;
  for (const rule of rules) {
    for (const [value, tinted] of [
      [rule.token, rule.tokenTinted],
      [rule.from, rule.fromTinted],
    ] as const) {
      if (!tinted || !value.startsWith("#")) continue;
      const [, a, b] = oklab(parse(value).rgb);
      const own = Math.hypot(a, b);
      had = Math.max(had, own < GREY ? NEUTRAL_CHROMA : own);
    }
  }
  for (let step = 80; step >= 0; step--) {
    const most = step * CHROMA_STEP;
    if (most < had && far(most)) return most;
  }
  return undefined;
}

/**
 * **The most chroma a tint gives `token`** when {@link apartChroma} answers `most`
 * (`purlis_core::theme::chroma_for`, #3034): `most` for a side of one of the `rules` the theme
 * hands (`undefined`, untinted, where no amount holds them), and no limit for every other token,
 * which no rule measures, so a workspace's strip keeps its colour's full hue.
 */
export function chromaFor(
  token: string,
  most: number | undefined,
  rules: readonly Apart[],
  alone: number | undefined = most,
): number | undefined {
  const side = (rule: Apart) => rule.token === token || rule.from === token;
  // A side of a rule held alone gives way to every rule together (#3130): `alone`.
  if (rules.some((rule) => rule.alone === true && side(rule))) return alone;
  return rules.some(side) ? most : Number.POSITIVE_INFINITY;
}

/** How far two theme colours are in OKLab, their alpha left out. */
export function oklabDistance(a: string, b: string): number {
  return apartIn(oklab(parse(a).rgb), oklab(parse(b).rgb));
}

/** The OKLab distance between two colours, written out as the engine's sum is. */
function apartIn(a: Vec3, b: Vec3): number {
  const [l, x, y] = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  return Math.sqrt(l * l + x * x + y * y);
}

/**
 * `hex` turned to `hue`, with the relative luminance it had and at most `most` chroma. `hex` is a
 * theme's value, so it is `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`; an alpha it had, it keeps.
 */
export function tintHex(hex: string, hue: number, most = Number.POSITIVE_INFINITY): string {
  const { rgb, alpha } = parse(hex);
  const target = luminance(rgb);
  const [, a, b] = oklab(rgb);
  const had = Math.hypot(a, b);
  const chroma = Math.min(had < GREY ? NEUTRAL_CHROMA : had, most);
  const at = (lightness: number) => shown(lightness, chroma, hue);
  // Luminance rises with lightness at a fixed chroma and hue, so the lightness that gives the
  // luminance the colour had is found by halving the interval.
  let low = 0;
  let high = 1;
  for (let step = 0; step < 32; step++) {
    const middle = (low + high) / 2;
    if (luminance(at(middle)) < target) low = middle;
    else high = middle;
  }
  return `#${rounded(at((low + high) / 2), target)
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")}${alpha}`;
}

/** WCAG's relative luminance of a colour in linear sRGB. */
function luminance([r, g, b]: Vec3): number {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The colour at this OKLCH, in linear sRGB — with the chroma lowered until the screen can
 *  show it, when it cannot. */
function shown(lightness: number, chroma: number, hue: number): Vec3 {
  const at = (c: number) => linear([lightness, c * cos(hue), c * sin(hue)]);
  const whole = at(chroma);
  if (inGamut(whole)) return whole;
  let low = 0;
  let high = chroma;
  for (let step = 0; step < 24; step++) {
    const middle = (low + high) / 2;
    if (inGamut(at(middle))) low = middle;
    else high = middle;
  }
  return at(low).map((v) => Math.min(1, Math.max(0, v))) as Vec3;
}

function inGamut(rgb: Vec3): boolean {
  return rgb.every((v) => v >= -1e-6 && v <= 1 + 1e-6);
}

const cos = (deg: number) => Math.cos((deg * Math.PI) / 180);
const sin = (deg: number) => Math.sin((deg * Math.PI) / 180);
const degrees = (rad: number) => ((rad * 180) / Math.PI + 360) % 360;

/** A theme value as linear sRGB, and its alpha as the two hex digits it was written with. */
function parse(hex: string): { rgb: Vec3; alpha: string } {
  const digits = hex.slice(1);
  const long = digits.length <= 4 ? [...digits].map((d) => d + d).join("") : digits;
  const rgb = [0, 2, 4].map((at) => toLinear(Number.parseInt(long.slice(at, at + 2), 16) / 255));
  return { rgb: rgb as Vec3, alpha: long.slice(6, 8) };
}

/** WCAG's middle luminance: a colour this bright contrasts with black exactly as much as with
 *  white. A darker colour is read against lighter ones, and a lighter against darker ones. */
const MIDDLE = Math.sqrt(1.05 * 0.05) - 0.05;

/**
 * **A colour in linear sRGB as three bytes, its luminance kept on its own side**
 * (`purlis_core::theme::tint`'s `rounded`): each channel rounded down or up, whichever of the
 * eight ways gives the luminance nearest `target`, the luminance the colour had, without passing
 * it toward {@link MIDDLE}. A dark shade is never rounded lighter than it was and a light one never
 * darker.
 */
function rounded(rgb: Vec3, target: number): Vec3 {
  const exact = rgb.map((v) => gamma(v) * 255);
  const lighter = target >= MIDDLE;
  let best: Vec3 = [0, 0, 0];
  let bestPast = true;
  let bestOff = Number.POSITIVE_INFINITY;
  for (let pick = 0; pick < 8; pick++) {
    const bytes = exact.map((value, at) =>
      Math.min(255, Math.max(0, Math.floor(value) + ((pick >> (2 - at)) & 1))),
    ) as Vec3;
    const shade = luminance(bytes.map((byte) => toLinear(byte / 255)) as Vec3);
    const past = lighter ? shade < target : shade > target;
    const off = Math.abs(shade - target);
    if ((!past && bestPast) || (past === bestPast && off < bestOff)) {
      best = bytes;
      bestPast = past;
      bestOff = off;
    }
  }
  return best;
}

/** One linear channel gamma-encoded, 0 to 1. */
function gamma(linearValue: number): number {
  const v = Math.min(1, Math.max(0, linearValue));
  return v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055;
}

function toLinear(v: number): number {
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

/** Linear sRGB to OKLab (Björn Ottosson's matrices). */
function oklab([r, g, b]: Vec3): Vec3 {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

/** OKLab to linear sRGB. */
function linear([lightness, a, b]: Vec3): Vec3 {
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}
