// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browser } from "@wdio/globals";

/**
 * **The window's gutter, in px, as the theme in force draws it** (#2805, #3034): the room the
 * theme's `window.gutter` keeps between two regions, and between a region and the window's edge
 * (`App.css`, `.workbench`). Purlis Classic's is none, so regions sit flush; Purlis, the default,
 * floats them 8px apart. A spec that measures where one region ends and the next begins allows
 * it, read from the theme rather than spelled, as `tab-strip-centre.e2e.ts` reads the control
 * height.
 */
export async function gutter(): Promise<number> {
  return browser.execute(() => {
    const probe = document.createElement("div");
    probe.style.width = "var(--window-gutter)";
    document.body.append(probe);
    const width = probe.getBoundingClientRect().width;
    probe.remove();
    return width;
  });
}
