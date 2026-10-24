// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Step } from "./workspaceSwitch";

/**
 * **A workspace switch slides every workspace part together** (#2916, R114, #2868·10): the side
 * panel, the work with its docked tabs, the right panel and the bottom halves come in 28px from
 * the side the switch went towards, fading in, over `duration.slide` on the enter ease
 * (`App.css`, `data-slide`). The activity bars, the rail and the title bar stay still: they are
 * outside what slides. Every switch slides, whatever made it (a swipe, a chip, a key, a
 * switcher's row; the ruling over the prototype, audit §7.6). Reduced motion has none: its
 * duration is written as none (`theme/motion.ts`), so the slide is over as it starts.
 *
 * **Which way** is the pin order's: a step says its own way ({@link slideTowards}, which a swipe
 * round the end of the pins needs), and any other switch goes the way the new workspace stands
 * from the old one in the project's order.
 */

/** The way the next switch was asked to slide, by the step that asked for it. */
let asked: Step | undefined;

/** The next workspace switch slides this way: a step's own, round the ends of the pins. */
export function slideTowards(step: Step): void {
  asked = step;
}

/** The way the switch now being drawn was asked to slide, once. */
export function takeSlide(): Step | undefined {
  const step = asked;
  asked = undefined;
  return step;
}

/** The way a switch goes from `from` to `to` in the project's `order`: on, or back. */
export function wayOf(order: readonly string[], from: string, to: string): Step {
  return order.indexOf(to) >= order.indexOf(from) ? 1 : -1;
}

/**
 * **Slides `parts` in from `step`'s side**, once: it is marked with the way (`data-slide`), which
 * the stylesheet plays over `duration.slide` (`App.css`), and the mark goes when the slide ends,
 * so nothing that draws again replays it. Marked afresh each switch, so a switch during a slide
 * starts it again.
 */
export function slideIn(parts: HTMLElement | null, step: Step): void {
  if (parts === null) return;
  delete parts.dataset.slide;
  // Read once between the two, so the stylesheet starts the slide again.
  void parts.offsetWidth;
  parts.dataset.slide = step > 0 ? "on" : "back";
  const done = (event: AnimationEvent) => {
    if (event.target !== parts) return;
    parts.removeEventListener("animationend", done);
    parts.removeEventListener("animationcancel", done);
    delete parts.dataset.slide;
  };
  parts.addEventListener("animationend", done);
  parts.addEventListener("animationcancel", done);
}

/**
 * **Whether the window moves at all**: not where the motion layer has written every duration as
 * none, which is reduced motion (`theme/motion.ts`). What a scroll into view asks for (R104).
 */
export function moves(): boolean {
  const written = getComputedStyle(document.documentElement)
    .getPropertyValue("--motion-duration-slide")
    .trim();
  return written !== "" && Number.parseFloat(written) > 0;
}
