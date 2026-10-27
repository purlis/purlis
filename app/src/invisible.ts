// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **purlis's one rule for a character a reader does not see, and its one mark, in the window**
 * (SK-51, #3202): `purlis_unicode`'s `hidden` and `marked`, applied to what the window draws
 * itself.
 *
 * Which characters is never the window's to say: the tables are the core's own generated
 * statics, handed over as `UNICODE_RULE` in `bindings.ts` (`app/src-tauri/src/unicoderule.rs`).
 * Where, in context, is the core's joiner rule (D-1301), written here once more because a rule
 * cannot cross the seam as data; `invisible.cases.json` holds the core's answers for a sweep of
 * the tables and every edge of that rule, and `invisible.test.ts` meets each one, so the two
 * cannot drift apart unnoticed.
 */

import { UNICODE_RULE } from "./bindings";

/** What becomes of a line end and a tab (`purlis_unicode::Breaks`): `"kept"` where they are the
 *  text's own layout (a file, a brief), `"marked"` on one line (a name, a target). */
export type Breaks = "kept" | "marked";

/** One character of a text, with whether a reader would not see it there. */
export type Flagged = { ch: string; hidden: boolean };

type Ranges = readonly (readonly [number, number])[];

/** Is `cp` in one of `table`'s sorted, disjoint, inclusive ranges? (`purlis_unicode::in_table`) */
function inTable(cp: number, table: Ranges): boolean {
  let lo = 0;
  let hi = table.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const [from, to] = table[mid];
    if (cp < from) hi = mid - 1;
    else if (cp > to) lo = mid + 1;
    else return true;
  }
  return false;
}

/** A lone surrogate, which a JavaScript string can hold and a Rust one cannot: general category
 *  `Cs`, which the rule names, and which the core's tables never meet. */
const lone = (cp: number) => cp >= 0xd800 && cp <= 0xdfff;

/** Has `ch` no glyph a reader sees, or does it move the text around it, on its own?
 *  (`purlis_unicode::invisible`) */
export function invisible(ch: string): boolean {
  const cp = ch.codePointAt(0) ?? 0;
  return lone(cp) || inTable(cp, UNICODE_RULE.invisible);
}

/** **The one mark** (`purlis_unicode::marked`): `⟦U+XXXX⟧`, four hex digits at least, upper
 *  case. */
export function markOf(ch: string): string {
  const code = (ch.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0");
  return `⟦U+${code}⟧`;
}

/** Each mark the core or this module wrote in `text` (`⟦U+XXXX⟧`), with where it stands. */
export function marksIn(text: string): { mark: string; at: number }[] {
  return Array.from(text.matchAll(/⟦U\+[0-9A-F]{4,6}⟧/gu), (one) => ({
    mark: one[0],
    at: one.index,
  }));
}

/** Each character of `text`, with whether it is hidden there (`purlis_unicode::hidden`): every
 *  invisible one, but a joiner or a presentation selector doing its one job, the selector or
 *  tags of a keycap or an RGI flag, and, with `"kept"`, a line end and a tab. */
export function flagged(text: string, breaks: Breaks): Flagged[] {
  const chars = Array.from(text);
  const cps = chars.map((ch) => ch.codePointAt(0) ?? 0);
  const spelled = inEmojiSequence(cps);
  return chars.map((ch, at) => {
    const kept = breaks === "kept" && (ch === "\n" || ch === "\t");
    return { ch, hidden: invisible(ch) && !kept && !spelled[at] && !doesItsJob(cps, at) };
  });
}

/** Does `text` hold a character a reader would not see there? (`purlis_unicode::hides`) */
export function hides(text: string, breaks: Breaks): boolean {
  return flagged(text, breaks).some((one) => one.hidden);
}

/** **`text` as a reader is shown it** (`purlis_unicode::marked`): each hidden character as its
 *  mark, the rest as itself. Idempotent: a mark is visible text. */
export function marked(text: string, breaks: Breaks): string {
  return flagged(text, breaks)
    .map((one) => (one.hidden ? markOf(one.ch) : one.ch))
    .join("");
}

/** For each codepoint, whether it is part of a keycap or an RGI tag sequence. */
function inEmojiSequence(cps: readonly number[]): boolean[] {
  const spelled = cps.map(() => false);
  let at = 0;
  while (at < cps.length) {
    const sequence = UNICODE_RULE.emoji_sequences.find(
      (one) => one.length <= cps.length - at && one.every((cp, i) => cps[at + i] === cp),
    );
    if (sequence === undefined) {
      at += 1;
    } else {
      spelled.fill(true, at, at + sequence.length);
      at += sequence.length;
    }
  }
  return spelled;
}

const ZWJ = 0x200d;
const ZWNJ = 0x200c;
/** VARIATION SELECTOR-15 and -16: an emoji drawn as text or as emoji. */
const PRESENTATION = [0xfe0e, 0xfe0f];

/** Is `cps[at]` a joiner or a presentation selector doing its one job between its neighbours?
 *  (`purlis_unicode`'s `does_its_job`, D-1301) */
function doesItsJob(cps: readonly number[], at: number): boolean {
  const before = at > 0 ? cps[at - 1] : undefined;
  const after = at + 1 < cps.length ? cps[at + 1] : undefined;
  const brahmic =
    before !== undefined &&
    (brahmicLetter(before) || virama(before)) &&
    after !== undefined &&
    brahmicLetter(after);
  const cp = cps[at];
  if (cp === ZWJ) {
    let emojiBefore = false;
    if (before !== undefined && PRESENTATION.includes(before)) {
      emojiBefore = at >= 2 && pictographic(cps[at - 2]);
    } else if (before !== undefined) {
      emojiBefore = pictographic(before) || inTable(before, UNICODE_RULE.emoji_modifier);
    }
    return (emojiBefore && after !== undefined && pictographic(after)) || brahmic;
  }
  if (cp === ZWNJ) {
    return (
      (before !== undefined &&
        arabicLetter(before) &&
        after !== undefined &&
        arabicLetter(after)) ||
      brahmic
    );
  }
  if (PRESENTATION.includes(cp)) return before !== undefined && pictographic(before);
  return false;
}

/** A letter of the Brahmic scripts' blocks, Devanagari to Malayalam (U+0900-U+0D7F): the
 *  core's generated table, not this engine's `\p{Alphabetic}`, whose Unicode version is the
 *  engine's own (#3470). */
const brahmicLetter = (cp: number) => inTable(cp, UNICODE_RULE.brahmic_letter);

/** A virama of those scripts: each block's U+0x4D, and Malayalam's two others. */
const virama = (cp: number) =>
  cp >= 0x0900 && cp <= 0x0d7f && ((cp & 0x7f) === 0x4d || cp === 0x0d3b || cp === 0x0d3c);

/** An emoji a presentation selector or a ZWJ may follow (Extended_Pictographic). */
const pictographic = (cp: number) => inTable(cp, UNICODE_RULE.extended_pictographic);

/** A letter of the Arabic script: its blocks and their presentation forms (the core's table). */
const arabicLetter = (cp: number) => inTable(cp, UNICODE_RULE.arabic_letter);
