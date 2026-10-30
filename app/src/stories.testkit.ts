// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a story's `play` function checks that axe cannot** (#2901, `docs/component-stories.md`).
 * Each asserts what a person sees, from the page, never a component's internals.
 */

import { waitFor } from "storybook/test";
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

/** The global the story tests put their window-sizer on (`.storybook/cell.setup.ts`). */
export const WINDOW_GLOBAL = "__PURLIS_STORY_WINDOW__";

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

/** How long the keyboard may take to land: an engine may move it a frame or more late. */
const LANDS_WITHIN_MS = 3_000;

/**
 * **The keyboard lands on `target`** (`step` names where in the story). Waits for it, as an
 * engine moves the focus a frame or more after the key that asked for it; throws naming the step
 * and what has the focus instead, so a failure says where the keyboard went.
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

/** Waits until `holds` says the keyboard is where `target` wants it, `on` it or `in` it. */
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
}

/** How long a surface may take to arrive: WebKit draws the next frame late in a busy run. */
const SETTLES_WITHIN_MS = 3_000;

/** The next frame drawn. */
const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

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

/** The families whose native input is hidden inside the part that draws it (#2907). */
const CHOICE_SCOPES = ["checkbox", "radio", "switch", "segment", "option-card", "slider"]
  .map((scope) => `[data-scope="${scope}"]`)
  .join(", ");

/** The points of a box a person looks at: its centre and a point just inside each corner. */
function pointsOf(box: DOMRect): [number, number][] {
  const inset = Math.min(2, box.width / 4, box.height / 4);
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
      (covering.size > 0 ? `: it is under ${[...covering].join(", ")}` : ""),
  );
}

/**
 * **The focus comes into sight** (`step` names where in the story): {@link expectFocusInSight},
 * waited for as long as the keyboard may take to land. A collection's focused item is scrolled
 * in by React Aria on the frame after the keyboard moves to it (`useSelectableCollection`), and
 * WebKit can draw that frame late in a busy run (#4026): checked at once, the item was still out
 * of sight. Throws what `expectFocusInSight` says, after the step.
 */
export async function untilFocusInSight(step: string): Promise<void> {
  try {
    await waitFor(() => expectFocusInSight(), { timeout: LANDS_WITHIN_MS });
  } catch (error) {
    throw new Error(`${step}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function describe(element: Element): string {
  const name = element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 40);
  const role = element.getAttribute("role") ?? element.tagName.toLowerCase();
  return name ? `the ${role} "${name}"` : `a ${role}`;
}
