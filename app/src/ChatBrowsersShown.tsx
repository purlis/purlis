// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, useRef } from "react";
import { Globe } from "lucide-react";
import type { ChatBrowser } from "./bindings";
import { chatBrowserTitle } from "./browsers";
import { THUMBNAIL_ACROSS, useBrowserWatch, useWindowSeen } from "./browserWatch";
import { useProjectShown } from "./projectShown";

/**
 * **A chat's browsers, where the window draws them besides their view tab** (#2024, spec
 * #2008): live on their chat's tab, and as rows under their chat in the Chats list. Either
 * opens the browser's view in a tab of its own, where the person watches it whole and takes
 * it over (#2014).
 */

/** A browser of a chat's, as it is drawn: the browser, and its chat by the name it has here. */
export type BrowserShown = { browser: ChatBrowser; chat: string };

/**
 * **A browser, live and small** (#2024): its thumbnail view, about a frame a second, drawn on
 * a canvas while it can be seen (its project in front and the window not hidden), and nothing
 * asked of the browser while it cannot.
 */
const BrowserThumbnail = memo(function BrowserThumbnail({ program }: { program: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const inFront = useProjectShown();
  const windowSeen = useWindowSeen();
  const seen = inFront && windowSeen;
  const watched = useBrowserWatch(program, "thumbnail", canvas, seen, THUMBNAIL_ACROSS);
  return (
    <span className="browser-thumbnail" data-watched={watched.kind}>
      {/* Its mark until its first frame, and after its view ends. */}
      {watched.kind !== "live" && <Globe aria-hidden="true" />}
      <canvas ref={canvas} aria-hidden="true" hidden={watched.kind !== "live"} />
    </span>
  );
});

/** What a tab's count of its chats' browsers is called: `1 browser`, `2 browsers`. */
export function browsersSaid(count: number): string {
  return count === 1 ? "1 browser" : `${count} browsers`;
}

/** What a row says of a browser that could not be ended yet, whole, on its hover. */
const NOT_ENDED_YET =
  "Its chat no longer uses it, and purlis keeps trying to end it. Stop all in the title bar ends it now.";

/**
 * **A chat's browsers on its tab** (#2024): how many, as the tab's count pill (`CountPill`'s
 * look, with a browser's mark), then each a live thumbnail, pressed to open its view. Beside
 * the tab's button, as the fresh mark is: a button holds no button. Out of the strip's Tab
 * order, as that mark is; the keyboard opens a browser from its row in the Chats list. One that
 * could not be ended yet is its chat's no more, and is on its row alone.
 */
export function TabBrowsers({
  browsers,
  onOpen,
}: {
  browsers: readonly BrowserShown[];
  onOpen: (shown: BrowserShown) => void;
}) {
  const used = browsers.filter((shown) => !shown.browser.unended);
  if (used.length === 0) return null;
  const said = browsersSaid(used.length);
  return (
    <span className="tab-browsers">
      <span className="count-pill tab-browsers-count" role="img" aria-label={said} title={said}>
        <Globe aria-hidden="true" />
        <span className="count-pill-chats" aria-hidden="true">
          {used.length}
        </span>
      </span>
      {used.map((shown) => (
        <button
          key={shown.browser.program}
          type="button"
          className="ui-button ui-button-ghost tab-browser"
          tabIndex={-1}
          aria-label={`Open browser ${shown.browser.name} of ${shown.chat}`}
          title={chatBrowserTitle(shown.chat, shown.browser.name)}
          onClick={() => onOpen(shown)}
        >
          <BrowserThumbnail program={shown.browser.program} />
        </button>
      ))}
    </span>
  );
}

/**
 * **A chat's browsers under its row in the Chats list** (#2024): one row each, by the name the
 * chat knows it by, pressed to open its view. Not rows of the tree, as a chat's finished tasks
 * are not (`FinishedTasks.tsx`): the arrows stop on the chats, and Tab reaches these.
 */
export const BrowserRows = memo(function BrowserRows({
  chat,
  level,
  browsers,
  onOpen,
}: {
  /** The chat they are of, by the name its row has. */
  chat: string;
  /** The level they are drawn at: one below the chat's own. */
  level: number;
  browsers: readonly BrowserShown[];
  onOpen: (shown: BrowserShown) => void;
}) {
  if (browsers.length === 0) return null;
  return (
    <li role="none" className="chat-browsers" data-level={level}>
      <div role="group" aria-label={`Browsers of ${chat}`}>
        {browsers.map((shown) => (
          <button
            key={shown.browser.program}
            type="button"
            className="ui-row ui-tree-row chat-browser"
            // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
            tabIndex={0}
            title={
              shown.browser.unended
                ? `${chatBrowserTitle(shown.chat, shown.browser.name)}\n${NOT_ENDED_YET}`
                : chatBrowserTitle(shown.chat, shown.browser.name)
            }
            onClick={() => onOpen(shown)}
          >
            <Globe className="node-icon" aria-hidden="true" />
            <span className="session ui-text-secondary">Browser {shown.browser.name}</span>
            {/* Could not be ended yet (D-2013-16): its chat no longer uses it. */}
            {shown.browser.unended && (
              <span className="chat-browser-unended ui-text-meta" title={NOT_ENDED_YET}>
                not ended yet
              </span>
            )}
          </button>
        ))}
      </div>
    </li>
  );
});
