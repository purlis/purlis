// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState, type KeyboardEvent } from "react";
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
  opens,
}: {
  /** **A way to the workspace it is about**, where that workspace has no chip to hang it from
   *  (#3651, D-3648-16): "Open X", drawn as its link, so it is never a notice about a workspace
   *  that offers no way there. */
  opens?: NoticeAction;
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
  const toSettings =
    settings === undefined
      ? undefined
      : { label: settings.label, onPress: () => onSettings(settings.link) };
  // A notice has one link: the setting that puts a refusal right keeps it, and Open is a way on.
  const link = toSettings ?? opens;
  if (toSettings !== undefined && opens !== undefined) fixes.unshift(opens);
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
  openOf,
}: {
  /** The way to the workspace a result is about, where it has no chip (#3651). */
  openOf?: (said: Said) => NoticeAction | undefined;
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
          opens={openOf?.(one.said)}
        />
      ))}
    </NoticeList>
  );
}

/** What the kept results are called in the Inbox's Notices. */
export const RESULTS = "Results";
