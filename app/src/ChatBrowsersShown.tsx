// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { memo, useRef } from "react";
import { Globe } from "lucide-react";
import type { Catalogued, Offer } from "./actions";
import type { BrowserDriver, ChatBrowser } from "./bindings";
import { addressChip, chatBrowserTitle, deviceOf } from "./browsers";
import { Menued } from "./Menus";
import { THUMBNAIL_ACROSS, useBrowserOrigin, useBrowserWatch, useWindowSeen } from "./browserWatch";
import { useProjectShown } from "./projectShown";
import { onAMac } from "./tabKeys";
import { Button } from "./ui/button/Button";
import { Count } from "./ui/badge/Badge";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **A chat's browsers, where the window draws them besides their view tab** (#2024, spec
 * #2008): live on their chat's tab, and as rows under their chat in the Chats list. Either
 * opens the browser's view in a tab of its own, where the person watches it whole and takes
 * it over (#2014).
 */

/** A browser of a chat's, as it is drawn: the browser, and its chat by the name it has here,
 *  or for a pinned one whose chat closed, by what that chat was called (#2022). */
export type BrowserShown = { browser: ChatBrowser; chat: string };

/** What a row says of a pinned browser, whole, on its hover (#2022). */
const PINNED =
  "Pinned: it outlives its chat, until you stop it, Stop all ends it, or purlis quits.";

/** What a row's mark says of who drives its browser, and its whole words on hover (#2300). */
export const DRIVEN: Record<BrowserDriver, { mark: string; said: string }> = {
  person: {
    mark: "you are driving",
    said: "You are driving this browser. Its agent is paused until you press Done in its view.",
  },
  purlis: {
    mark: "purlis is signing in",
    said: "purlis is signing in to a page in this browser. Its agent waits until purlis hands it back.",
  },
};

/** **What a browser's row and thumbnail say on hover** (#2300): which browser of which chat,
 *  where its page is on which device, and who drives it where its agent does not. */
export function browserSaid(
  shown: BrowserShown,
  origin: string | null | undefined,
  mac: boolean,
): string[] {
  const driver = shown.browser.driver;
  return [
    chatBrowserTitle(shown.chat, shown.browser.name),
    addressChip(origin ?? null, deviceOf(shown.browser.pool ?? null, mac)).said,
    ...(driver !== null ? [DRIVEN[driver].said] : []),
  ];
}

/** No rows of the catalogue: rows drawn without a menu, in a test. */
const NO_OFFERS: Catalogued = new Map();
const NO_CHATS: readonly number[] = [];

/**
 * **A browser, live and small** (#2024): its thumbnail view, about a frame a second, drawn on
 * a canvas while it can be seen (its project in front and the window not hidden), and nothing
 * asked of the browser while it cannot.
 */
const BrowserThumbnail = memo(function BrowserThumbnail({
  program,
  driver,
}: {
  program: string;
  /** Who drives it, where its agent does not: marked on it (#2300). */
  driver: BrowserDriver | null;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const inFront = useProjectShown();
  const windowSeen = useWindowSeen();
  const seen = inFront && windowSeen;
  const { watched } = useBrowserWatch(program, "thumbnail", canvas, seen, THUMBNAIL_ACROSS);
  return (
    <span
      className="browser-thumbnail"
      data-watched={watched.kind}
      data-driver={driver ?? undefined}
    >
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
      <Tooltip tip={said}>
        <Count role="img" aria-label={said}>
          <Globe aria-hidden="true" />
          <span aria-hidden="true">{used.length}</span>
        </Count>
      </Tooltip>
      {used.map((shown) => (
        <TabBrowser key={shown.browser.program} shown={shown} onOpen={onOpen} />
      ))}
    </span>
  );
}

/** One browser's live thumbnail on its chat's tab, pressed to open its view (#2024). */
function TabBrowser({
  shown,
  onOpen,
}: {
  shown: BrowserShown;
  onOpen: (shown: BrowserShown) => void;
}) {
  const origin = useBrowserOrigin(shown.browser.program);
  const driver = shown.browser.driver;
  return (
    <Button
      variant="ghost"
      excludeFromTabOrder
      aria-label={`Open browser ${shown.browser.name} of ${shown.chat}${
        driver !== null ? `, ${DRIVEN[driver].mark}` : ""
      }`}
      tooltip={browserSaid(shown, origin, onAMac()).join("\n")}
      onPress={() => onOpen(shown)}
      className="tab-browser"
    >
      <BrowserThumbnail program={shown.browser.program} driver={driver} />
    </Button>
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
  label,
  offers = NO_OFFERS,
  onPress,
  handTo = NO_CHATS,
}: {
  /** The chat they are of, by the name its row has. */
  chat: string;
  /** The level they are drawn at: one below the chat's own. */
  level: number;
  browsers: readonly BrowserShown[];
  onOpen: (shown: BrowserShown) => void;
  /** What the group is called: `Browsers of <chat>` unless said. */
  label?: string;
  /** The catalogue, for each row's menu (#2022): Pin, Hand to a chat, Stop. */
  offers?: Catalogued;
  onPress?: (offer: Offer) => void;
  /** The open chats a browser no chat uses may be handed to, by number. */
  handTo?: readonly number[];
}) {
  if (browsers.length === 0) return null;
  return (
    <li role="none" className="chat-browsers" data-level={level}>
      <div role="group" aria-label={label ?? `Browsers of ${chat}`}>
        {browsers.map((shown) => (
          <BrowserRow
            key={shown.browser.program}
            shown={shown}
            onOpen={onOpen}
            offers={offers}
            onPress={onPress}
            handTo={handTo}
          />
        ))}
      </div>
    </li>
  );
});

/** **One browser's row under its chat** (#2024), with its catalogue menu (#2022): where its
 *  page is on its hover, and who drives it where its agent does not (#2300). */
function BrowserRow({
  shown,
  onOpen,
  offers,
  onPress,
  handTo,
}: {
  shown: BrowserShown;
  onOpen: (shown: BrowserShown) => void;
  offers: Catalogued;
  onPress?: (offer: Offer) => void;
  handTo: readonly number[];
}) {
  const origin = useBrowserOrigin(shown.browser.program);
  const driver = shown.browser.driver;
  return (
    <Menued
      on={{
        on: "browser",
        program: shown.browser.program,
        handTo,
        // Its chat's browser timeline, while that chat is open (#2017).
        ...(shown.browser.was === null && !shown.browser.unended
          ? { session: shown.browser.session }
          : {}),
      }}
      offers={offers}
      onPress={onPress ?? (() => undefined)}
    >
      {/* The row's one hint says what each of its marks means: a mark inside a row takes no
          keyboard of its own. */}
      <Tooltip
        tip={[
          ...browserSaid(shown, origin, onAMac()),
          ...(driver !== null ? [DRIVEN[driver].said] : []),
          ...(shown.browser.login !== null ? [`Signed in with ${shown.browser.login}`] : []),
          ...(shown.browser.unended ? [NOT_ENDED_YET] : []),
          ...(shown.browser.pinned ? [PINNED] : []),
        ].join("\n")}
      >
        <button
          type="button"
          className="ui-row ui-tree-row chat-browser"
          // #190: WebKit leaves a button out of the tab sequence without `tabIndex`.
          tabIndex={0}
          onClick={() => onOpen(shown)}
        >
          <Globe className="node-icon" aria-hidden="true" />
          <span className="session ui-text-secondary">Browser {shown.browser.name}</span>
          {/* A pinned one whose chat closed says which chat it was of (#2022). */}
          {shown.browser.was !== null && (
            <span className="chat-browser-was ui-text-meta">{shown.browser.was}</span>
          )}
          {/* Who drives it, where its agent does not (#2300). */}
          {driver !== null && (
            <span className="chat-browser-driver ui-text-meta" data-driver={driver}>
              {DRIVEN[driver].mark}
            </span>
          )}
          {/* Signed in with a login purlis performed (#2022): it goes only to a chat that may
            log in with it. */}
          {shown.browser.login !== null && (
            <span className="chat-browser-login ui-text-meta">signed in</span>
          )}
          {shown.browser.pinned && <span className="chat-browser-pinned ui-text-meta">pinned</span>}
          {/* Could not be ended yet (D-2013-16): its chat no longer uses it. */}
          {shown.browser.unended && (
            <span className="chat-browser-unended ui-text-meta">not ended yet</span>
          )}
        </button>
      </Tooltip>
    </Menued>
  );
}
