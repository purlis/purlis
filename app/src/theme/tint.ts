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
  /** A rule of the accent's words (#3100): measured only once the accent is answered, so it
   *  costs the accent no colour; see {@link wordsChroma}. */
  words?: boolean;
  /** A rule of the accent's marks (#4921): measured only once every other rule is answered, so
   *  it costs the accent and its kin no colour; the mark gives way on its own; see
   *  {@link signalChroma}. */
  signal?: boolean;
};

/**
 * **A ground the accent's words stand on comes no nearer a signal's wash than it was** (#4666,
 * `purlis_core::theme::tint::NoNearer`): each of `tokens`, turned and laid on each of `over` as
 * the screen draws it, is at least as far from each of `from` laid on the same ground as the value
 * it was turned from; where it would come nearer, it stays as it was. See {@link noNearer}.
 */
export type NoNearer = {
  readonly tokens: readonly string[];
  readonly from: readonly string[];
  readonly over: readonly string[];
};

/** One rule as a theme draws it: the value of each side, whether the tint turns each, whether
 *  each is a side of a rule held alone, and the distance it holds
 *  (`purlis_core::theme::Rule`). */
export type Rule = {
  token: string;
  tokenTinted: boolean;
  tokenAlone?: boolean;
  from: string;
  fromTinted: boolean;
  fromAlone?: boolean;
  distance: number;
};

/** Which sides a search moves: `"all"`, or `"alone"`, only the sides held alone. */
type Moves = "all" | "alone";

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
  return remembered([], rules, hue, Number.POSITIVE_INFINITY, "all");
}

/**
 * **The most chroma the sides held alone are given at a hue** (#3388,
 * `purlis_core::theme::alone_chroma_of`), with every other side drawn at `shared`, what
 * {@link apartChroma} answered for the shared rules: `shared` when every rule holds there, else
 * the largest multiple of {@link CHROMA_STEP} under it that holds them all, each side at the
 * chroma it is drawn at; `undefined` when not even a grey does. A chroma is offered only where
 * `kin`, the shared rules, hold too with their sides at it: past that the accent keeps the colour
 * it was written in rather than a grey at a chroma its kin could not take.
 */
export function aloneChroma(
  rules: readonly Rule[],
  kin: readonly Rule[],
  shared: number,
  hue: number,
): number | undefined {
  return remembered(kin, rules, hue, shared, "alone");
}

/**
 * **The most chroma held tokens are given at a hue** (`purlis_core::theme::most_of`): `shared`,
 * what the shared rules answer, and `alone`, for the sides of a rule held alone, with every rule
 * measured as it will be drawn (#3388). Where no amount holds the sides held alone they are drawn
 * as written, and if a rule breaks there too, nothing held is tinted at this hue.
 */
export function mostOf(
  all: readonly Rule[],
  shared: readonly Rule[],
  hue: number,
): { shared: number | undefined; alone: number | undefined } {
  const most = apartChroma(shared, hue);
  if (all.length === shared.length) return { shared: most, alone: most };
  if (most === undefined) return { shared: undefined, alone: undefined };
  const alone = aloneChroma(all, shared, most, hue);
  const holds = alone !== undefined || holdsAsWritten(all, hue, most);
  return { shared: holds ? most : undefined, alone };
}

/**
 * **The chroma the accent's words are drawn at** (#3100, `purlis_core::theme::words_chroma_of`):
 * the accent's own, `alone`, where the words' rules `words` hold at it, the other side of each
 * drawn as it will be (at `alone` for a side held alone, else at `shared`); `undefined`, the words
 * as written, where they do not. Where no chroma holds the accent's words apart from the waiting
 * colour (a yellow or green workspace), they keep the theme's colour, and the accent its tint.
 * The washes the words stand on are answered with them (#4666), so the two never differ in hue.
 */
export function wordsChroma(
  words: readonly Rule[],
  hue: number,
  shared: number | undefined,
  alone: number | undefined,
): number | undefined {
  if (alone === undefined) return undefined;
  const at = (sideAlone: boolean | undefined) => (sideAlone === true ? alone : shared);
  return words.every((rule) => holdsAt(rule, hue, alone, at(rule.fromAlone))) ? alone : undefined;
}

/**
 * **The chroma a mark of the accent is drawn at** (#4921, `purlis_core::theme::signal_chroma_of`):
 * `shared`, the most it is given, where every one of its `rules` holds there; else `alone`, the
 * accent's own, since a link reads as the accent (#3049); else `undefined`, the mark as written in
 * the theme's own colour, as the accent's words are (#3100). Never a search down to grey.
 */
export function signalChroma(
  rules: readonly Rule[],
  hue: number,
  shared: number | undefined,
  alone: number | undefined,
): number | undefined {
  return [shared, alone].find(
    (chroma) => chroma !== undefined && rules.every((rule) => holdsAt(rule, hue, chroma, chroma)),
  );
}

/**
 * **Whether `turned` comes no nearer a signal's wash than `written`** (#4666,
 * `purlis_core::theme::tint::no_nearer`): the nearest `turned` comes to any of `rule.from`, each
 * laid with it on each ground of `rule.over` as the screen draws them, alpha and all, in OKLab,
 * is no nearer than the nearest `written` comes. `value` gives those washes and grounds as they
 * are drawn. A value that is not a colour has nothing to be near.
 */
export function noNearer(
  turned: string,
  written: string,
  value: (token: string) => string | undefined,
  rule: NoNearer,
): boolean {
  if (!isColour(turned) || !isColour(written)) return true;
  return nearestSignal(turned, value, rule) >= nearestSignal(written, value, rule);
}

/** The nearest `ground` comes to one of `rule.from`, both laid on each of `rule.over`. */
function nearestSignal(
  ground: string,
  value: (token: string) => string | undefined,
  rule: NoNearer,
): number {
  let nearest = Number.POSITIVE_INFINITY;
  for (const underToken of rule.over) {
    const under = value(underToken);
    if (under === undefined || !isColour(under)) continue;
    for (const fromToken of rule.from) {
      const from = value(fromToken);
      if (from === undefined || !isColour(from)) continue;
      nearest = Math.min(nearest, oklabDistance(onto(ground, under), onto(from, under)));
    }
  }
  return nearest;
}

/**
 * `wash` laid over `under` as the screen draws it, opaque (`purlis_core::theme::check::onto`):
 * each channel mixed by the wash's alpha and rounded to its byte, a half up.
 */
export function onto(wash: string, under: string): string {
  const top = long(wash);
  if (top.length === 6) return wash;
  const bottom = long(under);
  const byte = (digits: string, at: number) => Number.parseInt(digits.slice(at, at + 2), 16);
  const alpha = byte(top, 6) / 255;
  return `#${[0, 2, 4]
    .map((at) =>
      Math.floor(byte(top, at) * alpha + byte(bottom, at) * (1 - alpha) + 0.5)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}

/** `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`: a theme's colour. */
const COLOUR = /^#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

/** Whether `value` is a theme's colour (`purlis_core::theme::vocabulary::is_colour`). */
export function isColour(value: string): boolean {
  return COLOUR.test(value);
}

/** A colour's hex digits as six or eight. */
function long(hex: string): string {
  const digits = hex.slice(1);
  return digits.length <= 4 ? [...digits].map((d) => d + d).join("") : digits;
}

/** The search for `rules`, remembered: the same rules at the same hue give the same answer. */
function remembered(
  kin: readonly Rule[],
  rules: readonly Rule[],
  hue: number,
  ceiling: number,
  moves: Moves,
): number | undefined {
  // A strip of tabs, a list of personas and every theme the core answers again ask it over and
  // over, and where a rule is not held at full colour (Purlis's red, orange and pink, #3034) the
  // answer is a search.
  const written = (list: readonly Rule[]) =>
    list
      .map(
        (one) =>
          `${one.token}${one.tokenTinted}${one.tokenAlone === true} ` +
          `${one.from}${one.fromTinted}${one.fromAlone === true} ${one.distance}`,
      )
      .join(" ");
  const key = `${moves} ${ceiling} ${hue} ${written(rules)} / ${written(kin)}`;
  if (answered.has(key)) return answered.get(key);
  const answer = searchedChroma(rules, kin, hue, ceiling, moves);
  if (answered.size >= MOST_ANSWERED) answered.delete(answered.keys().next().value as string);
  answered.set(key, answer);
  return answer;
}

/** Each answer {@link apartChroma} gave, by its rules and hue, the oldest out first. */
const answered = new Map<string, number | undefined>();

/** How many answers {@link answered} keeps. */
const MOST_ANSWERED = 1024;

/** Whether `rule`'s sides, drawn at those chromas, are as far apart as it asks. `undefined` is
 *  a side drawn as written. */
function holdsAt(
  rule: Rule,
  hue: number,
  tokenAt: number | undefined,
  fromAt: number | undefined,
): boolean {
  const side = (value: string, tinted: boolean, most: number | undefined) =>
    tinted && most !== undefined ? tintHex(value, hue, most) : value;
  const token = side(rule.token, rule.tokenTinted, tokenAt);
  const from = side(rule.from, rule.fromTinted, fromAt);
  // A value that is not a colour has nothing to be near.
  if (!token.startsWith("#") || !from.startsWith("#")) return true;
  return apartIn(oklab(parse(token).rgb), oklab(parse(from).rgb)) >= rule.distance;
}

/** Whether every rule holds with the sides held alone as written and every other side at
 *  `shared`. */
function holdsAsWritten(rules: readonly Rule[], hue: number, shared: number): boolean {
  const chroma = (alone: boolean | undefined) => (alone === true ? undefined : shared);
  return rules.every((rule) => holdsAt(rule, hue, chroma(rule.tokenAlone), chroma(rule.fromAlone)));
}

/** The search both answers are (`purlis_core::theme::tint`'s `searched`): `ceiling` when every
 *  rule holds there, else the largest multiple of {@link CHROMA_STEP} under it and under the
 *  colour the moving sides have. A side `moves` is tried at each chroma; any other stays at
 *  `ceiling`. */
function searchedChroma(
  rules: readonly Rule[],
  kin: readonly Rule[],
  hue: number,
  ceiling: number,
  moves: Moves,
): number | undefined {
  const moving = (alone: boolean | undefined) => moves === "all" || alone === true;
  const far = (most: number) => {
    if (!kin.every((rule) => holdsAt(rule, hue, most, most))) return false;
    const chroma = (alone: boolean | undefined) => (moving(alone) ? most : ceiling);
    return rules.every((rule) =>
      holdsAt(rule, hue, chroma(rule.tokenAlone), chroma(rule.fromAlone)),
    );
  };
  if (far(ceiling)) return ceiling;
  // The most colour any tinted side is given: a grey is given {@link NEUTRAL_CHROMA}.
  let had = 0;
  for (const rule of rules) {
    for (const [value, tinted, alone] of [
      [rule.token, rule.tokenTinted, rule.tokenAlone],
      [rule.from, rule.fromTinted, rule.fromAlone],
    ] as const) {
      if (!tinted || !moving(alone) || !value.startsWith("#")) continue;
      const [, a, b] = oklab(parse(value).rgb);
      const own = Math.hypot(a, b);
      had = Math.max(had, own < GREY ? NEUTRAL_CHROMA : own);
    }
  }
  for (let step = 80; step >= 0; step--) {
    const most = step * CHROMA_STEP;
    if (most < had && most < ceiling && far(most)) return most;
  }
  return undefined;
}

/**
 * **The most chroma a tint gives `token`** when {@link apartChroma} answers `most`
 * and {@link aloneChroma} `alone` (`purlis_core::theme::chroma_of`, #3034): `alone` for a side of
 * a rule held alone, `most` for a side of any other of the `rules` the theme hands, and no limit
 * for every other token, which no rule measures, so a workspace's strip keeps its colour's full
 * hue. `undefined` is untinted, where no amount holds them: `alone` is not optional, because
 * `undefined` there says the sides held alone are drawn as written, whatever `most` says.
 */
export function chromaFor(
  token: string,
  most: number | undefined,
  rules: readonly Apart[],
  alone: number | undefined,
  words: number | undefined,
  withWords: readonly string[] = [],
  signal: Readonly<Record<string, number | undefined>> = {},
): number | undefined {
  const side = (rule: Apart) => rule.token === token || rule.from === token;
  // The accent's words, and the washes they stand on, at what {@link wordsChroma} answers (#3100,
  // #4666).
  if (withWords.includes(token)) return words;
  if (rules.some((rule) => rule.words === true && rule.token === token)) return words;
  // A mark of the accent at what {@link signalChroma} answers for it (#4921).
  if (rules.some((rule) => rule.signal === true && rule.token === token)) return signal[token];
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

/**
 * **`under` turned toward `accent`** (#4666, `purlis_core::theme::tint::toward`): to the accent's
 * OKLCH hue, at no more chroma than the accent has, with the luminance and alpha `under` had. How
 * an accent's grounds follow an accent given over a parent: a grey accent gives grey grounds. An
 * accent or a ground that is not a colour leaves `under` as it is.
 */
export function toward(under: string, accent: string): string {
  if (!isColour(accent) || !isColour(under)) return under;
  const [, a, b] = oklab(parse(accent).rgb);
  return tintHex(under, degrees(Math.atan2(b, a)), Math.hypot(a, b));
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
