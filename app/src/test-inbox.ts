// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act } from "@testing-library/react";
import { vi } from "vitest";
import { UNDO_MS } from "./undo";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import type { PaneNoticesShown } from "./paneNotices";

/**
 * **A window whose Panel's right half opens on Notices**, for the window tests of a project's
 * Notices: they stood under the tab strip, always on screen, since #1695 they were listed in
 * the Inbox, and since #5645 in the Notices view, which a person opens under Asks. The layout
 * file the window is handed at its creation names Notices as the half's view, so what the
 * person would see is what the test can press.
 */
export function inboxOpenAtLaunch(): void {
  asideOpenAtLaunch("asks", "expanded", "notices");
}

/**
 * **A window whose right side opens on Personas, the Inbox behind it**: for the window tests of
 * a pane's own Notices, which the Inbox lists too. The side opens on the Inbox since #1738, and a
 * Notice on a pane and its line in the Inbox are two of the same name on screen.
 */
export function inboxAwayAtLaunch(notices?: unknown): void {
  // Passed as a hook (`beforeEach(inboxAwayAtLaunch)`), its argument is the test's context.
  asideOpenAtLaunch("personas", notices === "minimised" ? "minimised" : "expanded");
}

/**
 * The layout file a test's window is handed. **Its panes' notices are expanded** (#4754), so a
 * test of a note on a pane reads the note; a test of the fold hands the file without them, as a
 * new machine's has (`minimised`, the window's default, which the file leaves out).
 */
function asideOpenAtLaunch(
  view: "asks" | "personas",
  notices: PaneNoticesShown = "expanded",
  half?: "notices",
): void {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "/home/op/.config/purlis/layout.json",
      found: true,
      document: {
        version: 2,
        regions: [
          { id: "navigation", side: "left", order: 0, collapsed: false },
          { id: "aside", side: "right", order: 0, collapsed: false, view },
          ...(half === undefined
            ? []
            : [{ id: "panel-right", side: "right", order: 0, collapsed: false, view: half }]),
        ],
        ...(notices === "expanded" ? { notices } : {}),
      },
      trouble: null,
    },
    theme: { path: "", found: false, document: null, trouble: null },
  };
}

/** Takes back what {@link inboxOpenAtLaunch} handed the next window. */
export function forgetInboxOpen(): void {
  Reflect.deleteProperty(globalThis, GLOBAL);
  forgetThisLaunch();
}

/**
 * **Waits out the Undo of a Dismiss** (#5650): a line dismissed in Updates or Notices goes at
 * once, and its own Dismiss is carried out (kept on this machine) only when its Undo has stood
 * for {@link UNDO_MS}. Needs the test's timers faked, `setTimeout` among them.
 */
export function waitOutTheUndo(): void {
  act(() => {
    vi.advanceTimersByTime(UNDO_MS + 1);
  });
}

/**
 * **Opens every ask's Answer** (#5651), for the tests of what an ask's form does: its answers,
 * its reply box, its Notice. Since #5651 an ask's form is drawn only once its Answer link is
 * pressed; these tests are about the form, not the link, so each is opened as the person would
 * open it, and the keyboard the opening moved into the form is handed back.
 */
export function openEveryAnswer(): void {
  const closed = [
    ...document.querySelectorAll<HTMLElement>('li.inbox-ask button[aria-expanded="false"]'),
  ].filter((one) => one.textContent === "Answer");
  if (closed.length === 0) return;
  act(() => {
    for (const one of closed) one.click();
  });
  act(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });
}
