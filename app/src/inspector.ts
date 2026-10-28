// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { browserView } from "./browsers";
import { openViewBesideView, type Tabs, type ViewRef } from "./tabs";

/**
 * **A browser's DevTools, in a view tab** (#2028, spec #2008).
 *
 * The person, driving a browser, presses Inspect on its view: the engine's own DevTools of the
 * page it shows open in the session host, and the window draws them in a view of their own, the
 * view `devtools` keyed by the browser's id, beside the browser's pane. They are frames on a
 * canvas, as the page is: nothing of DevTools runs in the window. They close as the person hands
 * the browser back, and their view says so.
 */

/** The view a browser's DevTools are drawn in. */
export const INSPECTOR_VIEW = "devtools";

/** The size DevTools open at, in CSS pixels, before their view says how much room it has. */
export const INSPECTOR_OPENS_AT = { width: 1000, height: 700 } as const;

/** The smallest DevTools are sized to, in CSS pixels: the engine's own floor for a page. */
export const INSPECTOR_SMALLEST = 200;

/** What a DevTools tab is called. */
export const INSPECTOR_TITLE = "DevTools";

/** The view of browser `program`'s DevTools. */
export function inspectorView(program: string): ViewRef {
  return { from: null, view: INSPECTOR_VIEW, key: program };
}

/** Whether `view` is a browser's DevTools. */
export function isInspector(view: ViewRef): boolean {
  return view.from === null && view.view === INSPECTOR_VIEW && view.key !== "";
}

/** Browser `program`'s DevTools, opened beside its pane, or in a tab of their own where no tab
 *  shows it, on the strip of `workspace`. */
export function openInspector(tabs: Tabs, program: string, workspace: string): Tabs {
  return openViewBesideView(
    tabs,
    browserView(program),
    inspectorView(program),
    INSPECTOR_TITLE,
    workspace,
  );
}

/** Who opens DevTools' view in each project: the project's own window view. */
const listeners = new Map<string, Set<(program: string) => void>>();

/** **The project `plane` opens a DevTools view** for each browser a view of it asks for, from
 *  now until the answer is called. */
export function onInspect(plane: string, open: (program: string) => void): () => void {
  const here = listeners.get(plane) ?? new Set();
  here.add(open);
  listeners.set(plane, here);
  return () => {
    here.delete(open);
    if (here.size === 0) listeners.delete(plane);
  };
}

/** A view of browser `program` in project `plane` asks for its DevTools' view: DevTools are
 *  open in the host by now. */
export function askToInspect(plane: string, program: string): void {
  for (const open of listeners.get(plane) ?? []) open(program);
}
