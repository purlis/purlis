// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a story's `play` function checks that axe cannot** (#2901, `docs/component-stories.md`).
 * Each asserts what a person sees, from the page, never a component's internals.
 */

import { expect, userEvent, waitFor } from "storybook/test";
import { cellInTest } from "../.storybook/matrix";

/** The global the screenshot gate puts its picture-taker on (`.storybook/screenshots.setup.ts`),
 *  only where pictures are taken. */
export const PICTURE_GLOBAL = "__PURLIS_STORY_PICTURE__";

/**
 * **The story's picture is taken now**, not after its `play` function: for a story whose `play`
 * ends with what it pictures gone, such as a dialog it answers. Compared with the story's
 * baseline as the screenshot gate compares it, once: the gate takes no picture after `play`.
 * Where no picture is taken (a local run, a cell that is not pictured, `screenshot: false`), it
 * does nothing.
 */
export async function pictureNow(): Promise<void> {
  const take = (globalThis as Record<string, unknown>)[PICTURE_GLOBAL];
  if (typeof take === "function") await (take as () => Promise<void>)();
}

/**
 * **Each story frame that does not stand at the top of what it is drawn in, on a whole pixel**
 * (#5463), one entry for each `[data-story-frame]` under `inside`. A frame drawn on a line of
 * text sits on that line's baseline, so its top would move with whatever baseline its story's
 * content gives it, by a fraction of a pixel as often as not, and its picture would take a row
 * more. Measured exactly, with no tolerance: a fraction is the fault, and both engines lay out
 * in whole sixty-fourths, so a frame at its line's top measures 0 from it.
 */
export function framesOffTheirLine(inside: ParentNode = document): string[] {
  return [...inside.querySelectorAll("[data-story-frame]")].flatMap((frame) => {
    const line = frame.parentElement;
    if (line === null) return [];
    const { top } = frame.getBoundingClientRect();
    // The top of the line's content box: past its border and its padding, where the frame goes.
    const drawn = getComputedStyle(line);
    const lineTop =
      line.getBoundingClientRect().top +
      parseFloat(drawn.borderTopWidth) +
      parseFloat(drawn.paddingTop);
    const off: string[] = [];
    if (top !== lineTop)
      off.push(`the story's frame stands ${top - lineTop} px below the top of its line`);
    if (!Number.isInteger(top)) off.push(`the story's frame's top is at ${top} px, between pixels`);
    return off;
  });
}

/** **The story's frame stands at the top of its line**, as `framesOffTheirLine` measures it;
 *  `step` names where in the story. */
export function expectFrameOnItsLine(inside: ParentNode, step: string): void {
  const off = framesOffTheirLine(inside);
  if (off.length > 0) throw new Error(`${step}: ${off.join("; ")}`);
}

/** The global the story tests put their window-sizer on (`.storybook/cell.setup.ts`). */
export const WINDOW_GLOBAL = "__PURLIS_STORY_WINDOW__";

/** The global the story tests put the engine's press on (`.storybook/at-rest.setup.ts`). */
export const ENGINE_PRESS_GLOBAL = "__PURLIS_STORY_ENGINE_PRESS__";

/**
 * **A press with the engine's own pointer** (#5063), which a story's `userEvent` is not: that one
 * only dispatches events, and leaves no `:hover` behind. The pointer stays on `element` for the
 * rest of the story. Says whether it pressed: only a story test can, never Storybook's own page,
 * where the pointer is the person's.
 */
export async function pressWithTheEnginesPointer(element: Element): Promise<boolean> {
  const press = (globalThis as Record<string, unknown>)[ENGINE_PRESS_GLOBAL];
  if (typeof press !== "function") {
    if (cellInTest() !== undefined)
      throw new Error("a story test has no engine's press (.storybook/at-rest.setup.ts)");
    return false;
  }
  await (press as (element: Element) => Promise<void>)(element);
  return true;
}

/** The global the story tests put the engine's keyboard on (`.storybook/at-rest.setup.ts`). */
export const ENGINE_KEYS_GLOBAL = "__PURLIS_STORY_ENGINE_KEYS__";

/**
 * **Keys on the engine's own keyboard** (#5993), which a story's `userEvent` is not: that one only
 * dispatches events, and does nothing the browser does for a key it has no copy of, such as ↑ and
 * ↓ moving a box of lines' caret a line. `keys` are written as `userEvent.keyboard`'s are. Says
 * whether it pressed them: only a story test can, never Storybook's own page.
 */
export async function keysOnTheEnginesKeyboard(keys: string): Promise<boolean> {
  const press = (globalThis as Record<string, unknown>)[ENGINE_KEYS_GLOBAL];
  if (typeof press !== "function") {
    if (cellInTest() !== undefined)
      throw new Error("a story test has no engine's keyboard (.storybook/at-rest.setup.ts)");
    return false;
  }
  await (press as (keys: string) => Promise<void>)(keys);
  return true;
}

/** The short window the operator's complaint was of (#3389): about 500 px tall. */
export const SHORT_WINDOW = { width: 800, height: 500 } as const;

/** A window short enough that 6vh is under 24 px, so the dialog's top is held by the floor. */
export const SHORTEST_WINDOW = { width: 800, height: 360 } as const;

/**
 * **The story's window is `height` px tall** (and `width` px wide), for the rest of its `play`:
 * a story that checks where something sits against a short window, such as a dialog's fixed
 * top and the 24 px it keeps clear of the bottom (#3510). The window is put back to its size
 * after the story. Says whether it was resized: only a story test can, never Storybook's own
 * page, where the window is the person's.
 */
export async function inAWindow(width: number, height: number): Promise<boolean> {
  const size = (globalThis as Record<string, unknown>)[WINDOW_GLOBAL];
  if (typeof size !== "function") {
    // In a story test the window is always resizable: a sizer missing there is a broken setup,
    // which would quietly check the tall window again.
    if (cellInTest() !== undefined)
      throw new Error("a story test has no window sizer (.storybook/cell.setup.ts)");
    return false;
  }
  await (size as (width: number, height: number) => Promise<void>)(width, height);
  await waitFor(() => {
    if (window.innerHeight !== height) throw new Error("not resized yet");
  });
  await nextFrame();
  return true;
}

/** The share of the window's height a dialog's top sits below the window's top (R2, 6vh). */
const DIALOG_TOP_SHARE = 0.06;

/** How tall `length` (a token, `var(--space-7)`) is drawn in this cell, in px. */
function lengthOf(length: string): number {
  const probe = document.createElement("div");
  probe.style.position = "absolute";
  probe.style.blockSize = length;
  document.body.append(probe);
  const px = probe.getBoundingClientRect().height;
  probe.remove();
  return px;
}

/**
 * **The dialog sits where R2 anchors it** (#3389, #3510): its top at 6vh of the window, never
 * closer than `--space-7` (24 px at the default text size), and its bottom that far or more above
 * the window's bottom, so neither its header nor its footer leaves the window, however short.
 */
export function expectAnchored(dialog: Element): void {
  const clear = lengthOf("var(--space-7)");
  // An unresolved token measures 0, which would weaken both checks to nothing.
  if (!(clear > 0)) throw new Error("--space-7 measures 0 px here: the theme's tokens are not set");
  const box = dialog.getBoundingClientRect();
  const top = Math.max(clear, DIALOG_TOP_SHARE * window.innerHeight);
  const floor = window.innerHeight - clear;
  if (Math.abs(box.top - top) > 1)
    throw new Error(
      `the dialog's top is at ${box.top} px, not at ${top} px of a ${window.innerHeight} px window`,
    );
  if (box.bottom > floor + 0.5)
    throw new Error(
      `the dialog's bottom is at ${box.bottom} px, past ${floor} px of a ${window.innerHeight} px window`,
    );
}

/** The next frame drawn. */
const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

/** How long the keyboard may take to land: an engine may move it a frame or more late. */
const LANDS_WITHIN_MS = 3_000;

/**
 * **The keyboard lands on `target`** (`step` names where in the story). Waits for it, as an
 * engine moves the focus a frame or more after the key that asked for it; throws naming the step
 * and what has the focus instead, so a failure says where the keyboard went.
 *
 * It has landed once the frame after the focus moved is drawn: a collection brings the row it
 * moved the keyboard to into sight on that frame (React Aria's `useSelectableCollection` scrolls
 * in a `requestAnimationFrame`), so `expectFocusInSight` right after checks what a person sees,
 * not the list before it scrolled (#4103).
 */
export async function untilFocused(target: () => Element | null, step: string): Promise<void> {
  await untilTheKeyboard(target, step, "on", (wanted, at) => at === wanted);
}

/**
 * **The keyboard lands in `target`**, on it or on anything inside it (`step` names where in the
 * story): a surface that puts the keyboard on one of its own (a popover's card, a submenu) as it
 * opens. Waits and fails as `untilFocused` does (#3610).
 */
export async function untilFocusWithin(target: () => Element | null, step: string): Promise<void> {
  await untilTheKeyboard(target, step, "in", (wanted, at) => at !== null && wanted.contains(at));
}

/** Waits until `holds` says the keyboard is where `target` wants it, `on` it or `in` it, then
 *  for the next frame, which brings it into sight. */
async function untilTheKeyboard(
  target: () => Element | null,
  step: string,
  where: "on" | "in",
  holds: (wanted: Element, at: Element | null) => boolean,
): Promise<void> {
  try {
    await waitFor(
      () => {
        const wanted = target();
        if (wanted === null || !holds(wanted, document.activeElement)) throw new Error("not yet");
      },
      { timeout: LANDS_WITHIN_MS },
    );
  } catch {
    let wanted: Element | null = null;
    try {
      wanted = target();
    } catch {
      // Not drawn: said below.
    }
    const at = document.activeElement;
    const on = at === null || at === document.body ? "nothing" : describe(at);
    throw new Error(
      `${step}: the keyboard is on ${on}, not ${where} ` +
        (wanted === null ? "an element that is not drawn" : describe(wanted)),
    );
  }
  // The frame the engine scrolls the focus into sight on: its callback was asked for as the focus
  // moved, before this one, so it has run once this one does.
  await nextFrame();
}

/** How long a surface may take to arrive: WebKit draws the next frame late in a busy run. */
const SETTLES_WITHIN_MS = 3_000;

/**
 * **The surface `inside` is on has settled** (#3414): a popover or a menu done arriving (React
 * Aria's `data-entering` gone, none of its animations running) and placed, its box the same on
 * two frames running. For a story that pictures a surface while it is open, as a picture taken
 * while it fades in or moves to its place differs run to run.
 */
export async function untilSettled(inside: Element): Promise<void> {
  const surface = inside.closest('[data-scope="popover"][data-part="root"]') ?? inside;
  let was: DOMRect | undefined;
  await waitFor(
    async () => {
      if (surface.hasAttribute("data-entering")) throw new Error("still arriving");
      if (surface.getAnimations({ subtree: true }).some((one) => one.playState === "running"))
        throw new Error("still moving");
      await nextFrame();
      const box = surface.getBoundingClientRect();
      const same =
        was !== undefined &&
        box.x === was.x &&
        box.y === was.y &&
        box.width === was.width &&
        box.height === was.height;
      was = box;
      if (!same) throw new Error("still being placed");
    },
    { timeout: SETTLES_WITHIN_MS },
  );
}

/**
 * **Words in a dialog, once they are seen** (#6360): a dialog fades in as it opens, so words it
 * shows can be found while it is still transparent. This waits for the dialog's scrim to settle
 * (`untilSettled`), then reads whether the words are seen until they are, as the fade's end is
 * the engine's (stories run 42857403995).
 */
export async function untilSeenInDialog(words: Element): Promise<void> {
  await untilSettled(words.closest('[data-scope="dialog"][data-part="scrim"]') ?? words);
  await waitFor(() => expect(words).toBeVisible(), { timeout: SETTLES_WITHIN_MS });
}

/** The families whose native input is hidden inside the part that draws it (#2907). */
const CHOICE_SCOPES = ["checkbox", "radio", "switch", "segment", "option-card", "slider"]
  .map((scope) => `[data-scope="${scope}"]`)
  .join(", ");

/** **A pen that writes in `element`'s font**: a canvas context whose `measureText` says how
 *  much room the element's words take as the element draws them. */
export function penFor(element: Element): CanvasRenderingContext2D {
  const pen = document.createElement("canvas").getContext("2d");
  if (pen === null) throw new Error("no canvas to measure words with");
  const style = getComputedStyle(element);
  pen.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
  return pen;
}

/** The points of a box a person looks at: its centre and a point just inside each corner, at
 *  least `round` px in for a box whose corners are rounded that far. */
function pointsOf(box: DOMRect, round = 0): [number, number][] {
  const inset = Math.min(Math.max(2, round), box.width / 4, box.height / 4);
  return [
    [box.left + box.width / 2, box.top + box.height / 2],
    [box.left + inset, box.top + inset],
    [box.right - inset, box.top + inset],
    [box.left + inset, box.bottom - inset],
    [box.right - inset, box.bottom - inset],
  ];
}

/**
 * **The focus is in sight** (WCAG 2.2 SC 2.4.11, Focus Not Obscured): something has the focus,
 * and some part of it is on the screen and not under anything else the page draws, such as a
 * dialog's sticky footer or a toolbar. Throws, saying what has the focus and what covers it.
 */
export function expectFocusInSight(): void {
  const focused = document.activeElement;
  if (!(focused instanceof HTMLElement) || focused === document.body)
    throw new Error("nothing has the focus");
  // A choice's native input is hidden inside the part that draws it (a checkbox's row, a
  // slider's thumb, #2907): that part is what a person sees the focus on, so it is what is held
  // in sight. Only a choice family's: any other input, and anything else, is held as itself.
  const choice =
    focused instanceof HTMLInputElement ? focused.closest<HTMLElement>(CHOICE_SCOPES) : null;
  const drawn =
    choice === null ? null : (focused.closest<HTMLElement>("[data-scope][data-part]") ?? choice);
  const seen = drawn ?? focused;
  const box = seen.getBoundingClientRect();
  if (box.width === 0 || box.height === 0)
    throw new Error(`the focus is on ${describe(focused)}, which is not drawn`);
  const covering = new Set<string>();
  for (const [x, y] of pointsOf(box)) {
    if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) continue;
    const top = document.elementFromPoint(x, y);
    if (top === null) continue;
    if (top === seen || seen.contains(top) || top.contains(seen)) return;
    covering.add(describe(top));
  }
  throw new Error(
    `the focus is on ${describe(focused)}, and none of it is in sight` +
      (covering.size > 0 ? `: it is under ${[...covering].join(", ")}` : "") +
      ` (${whereItIs(seen)})`,
  );
}

/** A box, rounded, as `x,y wxh`. */
function boxSaid(box: DOMRect): string {
  const r = Math.round;
  return `${r(box.x)},${r(box.y)} ${r(box.width)}x${r(box.height)}`;
}

/**
 * **Where an element out of sight is** (#4672): its box, the window's size and scroll, and every
 * ancestor that clips or scrolls it, with that ancestor's box and scroll. So one failure says
 * whether the window is too small, a frame did not scroll, or a frame scrolled the wrong way.
 */
function whereItIs(element: Element): string {
  const said = [
    `its box ${boxSaid(element.getBoundingClientRect())}`,
    `the window ${window.innerWidth}x${window.innerHeight} scrolled ${Math.round(window.scrollX)},${Math.round(window.scrollY)}`,
  ];
  for (let at = element.parentElement; at !== null; at = at.parentElement) {
    if (at.scrollHeight <= at.clientHeight && at.scrollWidth <= at.clientWidth) continue;
    const style = getComputedStyle(at);
    if (style.overflowX === "visible" && style.overflowY === "visible") continue;
    const part = at.getAttribute("data-part");
    const scope = at.getAttribute("data-scope");
    const name =
      (scope !== null ? `${scope}${part !== null ? `.${part}` : ""}` : null) ??
      at.getAttribute("data-testid") ??
      at.tagName.toLowerCase();
    said.push(
      `${name} ${boxSaid(at.getBoundingClientRect())} scrolled ${Math.round(at.scrollLeft)},${Math.round(at.scrollTop)} of ${at.scrollWidth}x${at.scrollHeight}`,
    );
  }
  return said.join("; ");
}

/**
 * **The focus comes into sight** (`step` names where in the story): {@link expectFocusInSight},
 * waited for as long as the keyboard may take to land. A collection's focused item is scrolled
 * in by React Aria on the frame after the keyboard moves to it (`useSelectableCollection`), and
 * WebKit can draw that frame late in a busy run (#4026): checked at once, the item was still out
 * of sight. Throws what `expectFocusInSight` says, after the step.
 */
export async function untilFocusInSight(step: string): Promise<void> {
  let why: unknown;
  try {
    await waitFor(() => expectFocusInSight(), { timeout: LANDS_WITHIN_MS });
    return;
  } catch (error) {
    why = error;
  }
  // Said after the step, with what the check last found.
  throw new Error(`${step}: ${why instanceof Error ? why.message : String(why)}`);
}

/**
 * **`element` scrolled to the middle of the pane it scrolls in**, and nothing else moved: the
 * nearest ancestor that scrolls, never the page or anything around the pane, as
 * `scrollIntoView` would, so the frame that is pictured stays where the story drew it (#5016).
 * For a story that pictures what sits under its pane's fold.
 */
export function scrollIntoItsPane(element: Element): void {
  let pane = element.parentElement;
  while (pane !== null && !scrolls(pane)) pane = pane.parentElement;
  if (pane === null) throw new Error(`${describe(element)} is in no pane that scrolls`);
  // A pane with nothing under its fold stays as it is: the browser holds `scrollTop` at 0.
  const [box, room] = [element.getBoundingClientRect(), pane.getBoundingClientRect()];
  pane.scrollTop += box.top - room.top - (room.height - box.height) / 2;
}

/** Whether `element` is a pane its content scrolls up and down in, overflowing now or not. */
function scrolls(element: Element): boolean {
  const { overflowY } = getComputedStyle(element);
  return overflowY === "auto" || overflowY === "scroll";
}

/** Why `part` is not wholly in sight, or `null` when every point a person looks at is. */
function outOfSight(part: Element): string | null {
  const box = part.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return `${describe(part)} is not drawn`;
  // A rounded part is hit only inside its curve: its corners are looked at as far in as it rounds.
  // A radius in px; a percentage reads as its number, held inside the box by `pointsOf`.
  const round = parseFloat(getComputedStyle(part).borderTopLeftRadius) || 0;
  for (const [x, y] of pointsOf(box, round)) {
    if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight)
      return `${describe(part)} runs past the window`;
    // The part or something inside it: an ancestor there, such as a pane that clips it, is
    // drawn where the part is not.
    const top = document.elementFromPoint(x, y);
    if (top === null || !part.contains(top))
      return `${describe(part)} is ${top === null ? "not drawn there" : `under ${describe(top)}`}`;
  }
  return null;
}

/**
 * **Each of `parts` comes wholly into sight** (`step` names where in the story): its centre and
 * each corner on the screen and not under anything else the page draws, such as a dialog's
 * sticky footer or the pane that clips it. For a story that scrolls what it pictures into view
 * before `pictureNow` (#5016), waited for as WebKit may draw the scroll late. Throws, after the
 * step, the first part out of sight and what is over it.
 */
export async function untilInSight(parts: Element[], step: string): Promise<void> {
  let why: string | null = null;
  try {
    await waitFor(
      () => {
        why = parts.map(outOfSight).find((one) => one !== null) ?? null;
        if (why !== null) throw new Error(why);
      },
      { timeout: LANDS_WITHIN_MS },
    );
  } catch {
    throw new Error(`${step}: ${why ?? "out of sight"}`);
  }
}

/**
 * **`target` comes into view** (`step` names where in the story): drawn, and neither it nor
 * anything around it hidden or wholly transparent, as `toBeVisible` reads it. Waited for, as a
 * dialog's scrim fades in from nothing (`charter-appear`) and WebKit can start that fade late in
 * a busy run (#5762): checked at once, a line drawn in a dialog not yet fading in was not
 * visible. Throws, after the step, what the check last found.
 */
export async function untilVisible(target: () => Element | null, step: string): Promise<void> {
  let why: unknown;
  try {
    await waitFor(() => expect(target()).toBeVisible(), { timeout: SETTLES_WITHIN_MS });
    return;
  } catch (error) {
    why = error;
  }
  throw new Error(`${step}: ${why instanceof Error ? why.message : String(why)}`);
}

function describe(element: Element): string {
  const name = element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 40);
  const role = element.getAttribute("role") ?? element.tagName.toLowerCase();
  return name ? `the ${role} "${name}"` : `a ${role}`;
}

/**
 * **Tab, until the keyboard is inside `target`**, at most `steps` times. Always ends in
 * `untilFocusWithin`, landed or not, so the frame React Aria scrolls a focused row or cell into
 * sight on (`useSelectableCollection`, a frame after the focus, late in WebKit) has run before
 * the story reads where the focus is or its picture is taken (#4672, #6038).
 */
export async function tabInto(target: () => Element | null, steps = 60): Promise<void> {
  for (let step = 0; step < steps; step++) {
    const into = target();
    if (into !== null && into.contains(document.activeElement)) break;
    await userEvent.tab();
  }
  await untilFocusWithin(target, "the keyboard reaches it by Tab");
}
