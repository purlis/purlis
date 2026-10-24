// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Motion, as data: how long a change in the window takes and how it moves, named once and read
 * by name — which is what `theme.ts` already does for colour, for the same reasons.
 *
 * **Why this module exists.** The window had two motions before it, each spelled out where it
 * was used — `900ms linear` on the spinner, `120ms ease` on the explorer's twisty — and each
 * with its own `@media (prefers-reduced-motion)` block beside it. That is the state `App.css`
 * was in for colour before M6.1: every value a local decision, every guard something the next
 * component had to remember. Asking for more motion (the operator, twice: *"no icons, visual
 * components, animations"* and *"show pipelines with animation"*) on top of that would have
 * multiplied both. So the timing is a vocabulary first, and the motion is built on it (M7.2).
 *
 * **Semantic, not a scale.** A theme sets `duration.enter`, never `duration-200`. The names say
 * what the motion is FOR — a surface arriving, a mark settling on an answer, a mark saying
 * something is still going — so a theme that wants arrivals quicker changes one number and
 * does not also speed up the spinner that happened to share a rung of a scale with it.
 *
 * **Values are numbers, never text.** A duration is a count of milliseconds and an easing is
 * the four numbers of a cubic Bézier, each held to its bounds by the core's theme engine
 * (`purlis_core::theme`), which reads a theme file and hands the window numbers.
 * `motionVariables` writes the CSS from those numbers, so nothing a theme file says is ever
 * copied into a declaration — the parse-and-re-emit rule (ADR 0041, property 4) that makes a
 * theme safe to take from an extension.
 *
 * **Reduced motion is this module's job and nobody else's.** Under `prefers-reduced-motion:
 * reduce` every duration is written as `0ms`, so every transition and every animation in the
 * window that reads a token finishes as soon as it starts — including one written tomorrow by
 * somebody who never heard of the media query. A looping animation with a zero duration has a
 * zero active duration (Web Animations, "active duration"), so a spinner does not spin and a
 * pulse does not pulse: each is drawn in its resting state, which is why every looping mark in
 * `App.css` is designed to read correctly standing still. `literals.test.ts` refuses a
 * `prefers-reduced-motion` query anywhere else, so there is only ever this one.
 */

/**
 * How long each kind of change takes, in milliseconds.
 *
 * - `quick` — the window answering something the operator just did: a tab lit as it is
 *   selected, a chevron turning. Short enough that it can never be waited on.
 * - `enter` — a surface arriving: a menu, a popover, a dialog, a region brought back.
 * - `settle` — a mark arriving at an answer: a pipeline that has finished, a chat whose state
 *   has just changed. Longer than `enter`, because it is the one motion meant to be noticed.
 * - `spin` — one turn of a mark saying *this is still running*.
 * - `breathe` — one breath of a mark saying *this is queued and not yet running*.
 *
 * **The shell's motions** (R150, #2925), each on the one ease the Purlis theme gives
 * `easing.standard` and `easing.enter`, `cubic-bezier(.22, .8, .24, 1)`:
 *
 * - `slide` — the rail hiding or showing, and a workspace switch sliding the work across (260).
 *   A token since the rail (#2915), as are `reveal` and `leave`.
 * - `panel` — the right panel arriving (240).
 * - `reveal` — a hidden surface coming in: the rail's peek, a bottom half (220).
 * - `leave` — the peek going, and the rail fading out (200).
 * - `view` — a side view's list changing (180).
 *
 * `quick` is the chevron turning and a switch's knob sliding (150 in Purlis).
 */
export const DURATIONS = [
  "duration.quick",
  "duration.enter",
  "duration.settle",
  "duration.spin",
  "duration.breathe",
  "duration.slide",
  "duration.reveal",
  "duration.leave",
] as const;

/**
 * How each kind of change moves, as a cubic Bézier.
 *
 * - `standard` — a state changing in place.
 * - `enter` — decelerating: fast out of nothing and slowing into place, which is how a thing
 *   that arrives reads as having arrived rather than as still coming.
 * - `steady` — no easing at all; a turn that eases reads as a stutter once a second.
 * - `breathe` — symmetric in and out, so a loop has no visible seam.
 */
export const EASINGS = [
  "easing.standard",
  "easing.enter",
  "easing.steady",
  "easing.breathe",
] as const;

export type Duration = (typeof DURATIONS)[number];
export type Easing = (typeof EASINGS)[number];
export type MotionToken = Duration | Easing;

/** The four numbers of `cubic-bezier(x1, y1, x2, y2)`. */
export type Bezier = readonly [number, number, number, number];

/** A theme's motion, complete: every duration and every easing has a value. */
export type Motion = {
  durations: Record<Duration, number>;
  easings: Record<Easing, Bezier>;
};

/** Every motion token, durations first — the order a theme file is easiest to read in. */
export const MOTION_TOKENS: readonly MotionToken[] = [...DURATIONS, ...EASINGS];

/** The custom property a motion token is written to: `duration.enter` becomes
 *  `--motion-duration-enter`. The prefix keeps the two vocabularies from ever sharing a
 *  property, so a colour token called `duration.*` could not collide with one of these. */
export function motionProperty(token: MotionToken): string {
  return `--motion-${token.split(".").join("-")}`;
}

/**
 * The motion as the custom properties the stylesheet reads, written fresh from numbers.
 *
 * `reduced` is the operator's `prefers-reduced-motion: reduce`, and it collapses every
 * duration to `0ms` — see this module's header for why here and only here. The easings are
 * written either way: a curve over no time is no motion, and leaving them in place means a
 * reader of the document sees the same set of properties whatever the setting.
 */
export function motionVariables(motion: Motion, reduced: boolean): Record<string, string> {
  const written: Record<string, string> = {};
  for (const token of DURATIONS) {
    written[motionProperty(token)] = `${reduced ? 0 : motion.durations[token]}ms`;
  }
  for (const token of EASINGS) {
    written[motionProperty(token)] = `cubic-bezier(${motion.easings[token].join(", ")})`;
  }
  return written;
}

/** The media query the operator's reduced-motion setting is read through. */
export const REDUCE = "(prefers-reduced-motion: reduce)";

/** Whether the system asks for less motion. A webview without `matchMedia` (jsdom, for one) is
 *  read as not having asked, which is the platform's own default. */
export function systemReducesMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia(REDUCE).matches;
}

/** Whether the person turned motion off in purlis's own settings (#2925): `theme.ts` sets it
 *  from the person's layer, which the engine reads from the layout file's `appearance`. */
let inPurlis = false;

/** Says whether the person turned motion off in purlis. `theme.ts` draws again after it. */
export function reduceMotionInPurlis(reduced: boolean): void {
  inPurlis = reduced;
}

/**
 * **Whether the operator has asked for less motion**, in the system or in purlis (#2925): the
 * two are layered, and either asking is enough. Under it every duration is `0ms`
 * ({@link motionVariables}) and the root carries {@link MOTION_ATTRIBUTE}, which the stylesheet's
 * one global switch reads (`styles.css`): no transition, and every animation over at once.
 */
export function motionReduced(): boolean {
  return inPurlis || systemReducesMotion();
}

/** The root's attribute that says motion is off: `reduced`, or none. The one global switch in
 *  `styles.css` reads it, so a transition or a keyframe written without a token still stops. */
export const MOTION_ATTRIBUTE = "data-motion";
