import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";

/**
 * **A window whose right side opens on the Inbox**, for the window tests of a project's
 * Notices: they stood under the tab strip, always on screen, and since #1695 they are listed in
 * the Inbox, which a person opens. The layout file the window is handed at its creation names
 * the Inbox as the side's view, so what the person would see is what the test can press.
 */
export function inboxOpenAtLaunch(): void {
  asideOpenAtLaunch("inbox");
}

/**
 * **A window whose right side opens on Personas, the Inbox behind it**: for the window tests of
 * a pane's own Notices, which the Inbox lists too. The side opens on the Inbox since #1738, and a
 * Notice on a pane and its line in the Inbox are two of the same name on screen.
 */
export function inboxAwayAtLaunch(): void {
  asideOpenAtLaunch("personas");
}

function asideOpenAtLaunch(view: "inbox" | "personas"): void {
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
        ],
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
