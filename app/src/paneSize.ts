// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useState } from "react";
import { PANE } from "./paneDrag";
import type { Size } from "./paneDrop";
import { type Beside, besideOf } from "./viewBeside";

/**
 * **A pane's size on screen** (#4769), for Open beside's judgement before the press: the pane
 * `pane` as the window draws it (`dropPane`'s mark), or the focused pane of what is on screen
 * where no pane is named. A pane that is not drawn, or drawn at no size (a project kept behind
 * the one in front, a test with no layout), has none, and `besideOf` then lets it split.
 */
export function paneSize(pane?: number): Size | undefined {
  const marked =
    pane === undefined
      ? document.querySelectorAll(`[${PANE}][data-focused]`)
      : document.querySelectorAll(`[${PANE}="${pane}"]`);
  for (const one of marked) {
    const r = one.getBoundingClientRect();
    if (r.width > 0 && r.height > 0) return { width: r.width, height: r.height };
  }
  return undefined;
}

/** **Open beside, judged for `pane` as it is resized**: what the beside icon and the press's
 *  menu say before anything is pressed. */
export function useBeside(pane?: number): Beside {
  const [beside, setBeside] = useState<Beside>(() => besideOf(undefined));
  useLayoutEffect(() => {
    const measure = () => {
      const now = besideOf(paneSize(pane));
      setBeside((was) => (was.ok === now.ok && was.direction === now.direction ? was : now));
    };
    measure();
    const element = pane === undefined ? undefined : document.querySelector(`[${PANE}="${pane}"]`);
    if (element == null || typeof ResizeObserver === "undefined") return;
    const watching = new ResizeObserver(measure);
    watching.observe(element);
    return () => watching.disconnect();
  }, [pane]);
  return beside;
}
