// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
} from "react";
import type { Said } from "./actions";
import { resultOf } from "./actionResult";
import { Notice, NoticeList, type NoticeAction } from "./Notice";
import type { SettingsLink } from "./settings/links";

/**
 * **What an action answered, as a closable notice** (#3344, the sidebar-views prototype's Q3 A):
 * its title and one plain sentence (`actionResult.ts`), its ways on, a Details disclosure
 * holding what the core said, and a × that closes it, as Esc does while the keyboard is in it.
 * The same notice stands at the top of the content for the newest result and is kept in the
 * Inbox's Notices under Results. A refusal is the trouble tone; a Notice is always a polite
 * `status`, so it is said once and never interrupts.
 */
export function ActionResultNotice({
  said,
  onClose,
  onSettings,
  cause,
  kept = false,
  escape = true,
  refocus,
}: {
  /** Where the keyboard goes when the way it was on goes from under it (a Retry whose notice
   *  turns into its progress, #3649's review): a chip's card hands its region. Else the
   *  notice's own line takes it. */
  refocus?: () => HTMLElement | null;
  /** Esc while the keyboard is in it closes it. Off where what it stands in answers Esc for
   *  every notice it holds: a chip's card (#3648). */
  escape?: boolean;
  /** Listed under Results, where the Inbox's list draws it: its keyboard handler's element is
   *  then no box of its own, since the notice is drawn in the list's place, and it is no live
   *  region, since the notice at the top said it already. */
  kept?: boolean;
  said: Said;
  onClose: () => void;
  onSettings: (link: SettingsLink) => void;
  /** What the line is about, stably: the result's family and which one it is. */
  cause: string;
}) {
  const { title, says, details } = resultOf(said);
  const [open, setOpen] = useState(false);
  // A new result starts with its Details closed, whatever the last one's were (#3360's review):
  // reset during the render, as React's "information from previous renders" pattern does, so
  // the live region stays the one element and no frame draws the new result opened.
  const [shown, setShown] = useState(said);
  if (shown !== said) {
    setShown(said);
    setOpen(false);
  }
  const detailsId = useId();
  const fixes: NoticeAction[] = (said.ways ?? []).map((way) => ({
    label: way.label,
    onPress: way.onPress,
    primary: way.primary,
  }));
  if (details !== undefined)
    fixes.push({
      label: "Details",
      onPress: () => setOpen((was) => !was),
      opens: { id: detailsId, open },
      disclosure: true,
    });
  const settings = said.settings;
  const link =
    settings === undefined
      ? undefined
      : { label: settings.label, onPress: () => onSettings(settings.link) };
  // **The keyboard never falls to the page** (WCAG 2.4.3, #3649's review): when the way it was
  // on is drawn no more (Retry, once its notice turns into its progress), it goes to the
  // notice's line, or where `refocus` says, rather than to `<body>`. A browser may say nothing
  // as a focused element is removed, so every render looks as well as the blur.
  const focused = useRef<{ on: HTMLElement; line: HTMLElement | null } | null>(null);
  const handTo = useRef(refocus);
  useLayoutEffect(() => {
    handTo.current = refocus;
  });
  const restore = () => {
    const had = focused.current;
    if (had === null || had.on.isConnected) return;
    focused.current = null;
    const at = document.activeElement;
    if (at !== null && at !== document.body) return;
    const to = handTo.current?.() ?? (had.line?.isConnected === true ? had.line : null);
    if (to === null) return;
    if (!to.hasAttribute("tabindex")) to.setAttribute("tabindex", "-1");
    to.focus({ preventScroll: true });
  };
  useLayoutEffect(restore);
  const onFocus = (event: FocusEvent) => {
    const on = event.target as HTMLElement;
    focused.current = {
      on,
      line: on.closest<HTMLElement>('[data-scope="notice"][data-part="root"]'),
    };
  };
  const onBlur = (event: FocusEvent) => {
    const on = event.target as HTMLElement;
    // To somewhere else that takes it: nothing to hand back.
    if (event.relatedTarget !== null) {
      focused.current = null;
      return;
    }
    queueMicrotask(() => {
      if (!on.isConnected) restore();
      // A press on the page, which takes no keyboard: the way is still there, so it is the
      // person's own move, and nothing is handed back later.
      else if (focused.current?.on === on) focused.current = null;
    });
  };
  const closeOnEsc = (event: KeyboardEvent) => {
    if (!escape || event.key !== "Escape") return;
    event.stopPropagation();
    onClose();
  };
  return (
    <div
      className={kept ? "action-result-kept" : "action-result"}
      data-from={said.from}
      onKeyDown={closeOnEsc}
      onFocus={onFocus}
      onBlur={onBlur}
    >
      <Notice
        cause={cause}
        tone={said.refused ? "trouble" : "news"}
        title={title}
        // A kept copy is said by the notice that stood at the top already: never twice.
        quiet={kept}
        fixes={fixes.length === 0 ? undefined : (fixes as [NoticeAction, ...NoticeAction[]])}
        link={link}
        onDismiss={onClose}
        under={
          open && details !== undefined ? (
            <pre className="action-result-details" id={detailsId}>
              {details}
            </pre>
          ) : undefined
        }
      >
        {says}
      </Notice>
    </div>
  );
}

/**
 * **Results, in the Inbox's Notices** (#3344): each result the window keeps, as the notice that
 * stood at the top of the content, with the same ways on, listed into `into` (the Inbox's
 * Results, under its heading) in the Notices' order. Its × puts it away.
 */
export function ResultsGroup({
  kept,
  into,
  onDrop,
  onSettings,
}: {
  kept: readonly { id: number; said: Said }[];
  into: Element;
  onDrop: (id: number) => void;
  onSettings: (link: SettingsLink) => void;
}) {
  return (
    <NoticeList into={into}>
      {kept.map((one) => (
        <ActionResultNotice
          kept
          key={one.id}
          cause={`result:${one.said.from}:${one.id}`}
          said={one.said}
          onClose={() => onDrop(one.id)}
          onSettings={onSettings}
        />
      ))}
    </NoticeList>
  );
}

/** What the kept results are called in the Inbox's Notices. */
export const RESULTS = "Results";
