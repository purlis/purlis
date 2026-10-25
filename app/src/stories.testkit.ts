// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What a story's `play` function checks that axe cannot** (#2901, `docs/component-stories.md`).
 * Each asserts what a person sees, from the page, never a component's internals.
 */

import { waitFor } from "storybook/test";

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

/** How long the keyboard may take to land: an engine may move it a frame or more late. */
const LANDS_WITHIN_MS = 3_000;

/**
 * **The keyboard lands on `target`** (`step` names where in the story). Waits for it, as an
 * engine moves the focus a frame or more after the key that asked for it; throws naming the step
 * and what has the focus instead, so a failure says where the keyboard went.
 */
export async function untilFocused(target: () => Element | null, step: string): Promise<void> {
  try {
    await waitFor(
      () => {
        const wanted = target();
        if (wanted === null || document.activeElement !== wanted) throw new Error("not yet");
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
    throw new Error(
      `${step}: the keyboard is on ${at === null || at === document.body ? "nothing" : describe(at)}` +
        `, not on ${wanted === null ? "an element that is not drawn" : describe(wanted)}`,
    );
  }
}

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
  const box = focused.getBoundingClientRect();
  if (box.width === 0 || box.height === 0)
    throw new Error(`the focus is on ${describe(focused)}, which is not drawn`);
  const covering = new Set<string>();
  for (const [x, y] of pointsOf(box)) {
    if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight) continue;
    const top = document.elementFromPoint(x, y);
    if (top === null) continue;
    if (top === focused || focused.contains(top) || top.contains(focused)) return;
    covering.add(describe(top));
  }
  throw new Error(
    `the focus is on ${describe(focused)}, and none of it is in sight` +
      (covering.size > 0 ? `: it is under ${[...covering].join(", ")}` : ""),
  );
}

function describe(element: Element): string {
  const name = element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 40);
  const role = element.getAttribute("role") ?? element.tagName.toLowerCase();
  return name ? `the ${role} "${name}"` : `a ${role}`;
}
