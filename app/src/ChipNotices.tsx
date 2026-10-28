// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";
import type { Said } from "./actions";
import { ActionResultNotice } from "./ActionResultNotice";
import type { KeptResult } from "./keptResults";
import { aboutOf, placeOf, type Chip, type Place } from "./noticePlace";
import { CARD_STOP } from "./regionCycle";
import type { SettingsLink } from "./settings/links";
import { HungPopover } from "./ui/popover/Popover";
import type { NoticeKind } from "./ui/titlebar/TitleBarWorkspaces";

/**
 * **A workspace's notices, hung from its chip** (#3648, spec #3647): a view onto the one store of
 * results the window keeps (`keptResults.ts`), beside the Notices centre's Results, which lists
 * the same ones. So dismissing one here drops it there and the other way round: there is one
 * dismissed state, the kept list's. The card is up while a new notice comes to its chip, until a
 * press elsewhere hides it; a hidden one leaves the chip's dot, whose press shows it again.
 */

/** A notice on a chip: a result the window keeps, by its place in that list. */
export type ChipNotice = { id: number; said: Said; kind: NoticeKind };

/** What the title bar draws of the notices about its chips' workspaces. */
export type ChipNotices = {
  /** The notices hung from each chip, newest first; a chip with none is not listed. */
  on: ReadonlyMap<string, readonly ChipNotice[]>;
  /** The chip whose card is up, if one is. */
  open: string | undefined;
  /** Shows `workspace`'s card: its dot's press. */
  show: (workspace: string) => void;
  /** Hides the card up, dismissing nothing: a press anywhere else. */
  hide: () => void;
  /** Dismisses one notice, here and in the Notices centre. */
  dismiss: (id: number) => void;
  /** A refusal's way to the setting that puts it right. */
  onSettings: (link: SettingsLink) => void;
};

/** Where `said` is drawn, given the title bar's `chips`. */
export const placeOfSaid = (said: Said, chips: readonly Chip[]): Place =>
  placeOf(aboutOf(said), chips);

/**
 * **The chips' notices**, worked out from what the window keeps: each result kept in `plane`
 * whose place is a chip's card, newest first, and which card is up. A new result placed on a
 * chip brings its card up, worked out during the render as `useKeptResults` keeps it, so no
 * frame draws it hidden first.
 */
export function useChipNotices({
  said,
  kept,
  plane,
  chips,
  drop,
  onSettings,
}: {
  /** The newest result said, which brings its chip's card up when it is placed on one. */
  said: Said | undefined;
  kept: readonly KeptResult[];
  plane: string | undefined;
  chips: readonly Chip[];
  drop: (id: number) => void;
  onSettings: (link: SettingsLink) => void;
}): ChipNotices {
  const on = useMemo(() => {
    const by = new Map<string, ChipNotice[]>();
    for (const one of kept) {
      if (one.plane !== plane) continue;
      const place = placeOfSaid(one.said, chips);
      if (place.at !== "chip") continue;
      by.set(place.workspace, [
        ...(by.get(place.workspace) ?? []),
        { id: one.id, said: one.said, kind: "info" },
      ]);
    }
    return by;
  }, [kept, plane, chips]);
  // **Which card is up, and in which project** (#3648's review): a card is up only in the
  // project whose result brought it up, and a project brought to the front brings none up.
  const [up, setUp] = useState<{
    open: string | undefined;
    on: string | undefined;
    seen: Said | undefined;
    in: string | undefined;
  }>({ open: undefined, on: plane, seen: said, in: plane });
  let open = up.open;
  let openIn = up.on;
  if (said !== up.seen || plane !== up.in) {
    if (plane === up.in) {
      // A new result in the project in front. By what it is about, not by the chips drawn now:
      // a workspace just made is said before the title bar has heard of its chip, and its card
      // comes up as the chip does. A result about no workspace takes the card down.
      const about = said === undefined ? undefined : aboutOf(said);
      open = about !== undefined && "workspace" in about ? about.workspace : undefined;
      openIn = plane;
    }
    // Another project in front: what it said while it was behind stays behind its dots.
    setUp({ open, on: openIn, seen: said, in: plane });
  }
  const shown = openIn === plane ? open : undefined;
  return {
    on,
    // A card with nothing left on it is down.
    open: shown !== undefined && on.has(shown) ? shown : undefined,
    show: (workspace) => setUp((was) => ({ ...was, open: workspace, on: plane })),
    hide: () => setUp((was) => ({ ...was, open: undefined })),
    dismiss: drop,
    onSettings,
  };
}

/** How urgent each kind is: the dot shows the most urgent of a chip's notices (spec #3647). */
const URGENCY: Record<NoticeKind, number> = { info: 1 };

/** The most urgent kind among `notices`: what a chip's dot shows. News is the least urgent. */
export const mostUrgent = (notices: readonly ChipNotice[]): NoticeKind =>
  notices.reduce<NoticeKind>(
    (top, one) => (URGENCY[one.kind] > URGENCY[top] ? one.kind : top),
    "info",
  );

/**
 * **A chip's card** (#3648): the popover family's notice surface hung from the chip, centred
 * under it with its point, holding the notices about the workspace, newest first, each as the
 * card Notice of what an action answered (its ×, its ways and its Details in place). It takes no
 * keyboard; F6 and a press reach it. Esc while the keyboard is in it dismisses everything on it.
 */
export function ChipCard({
  workspace,
  named,
  notices,
  anchor,
  does,
  takesKeyboard = false,
  onKeyboardTaken,
}: {
  /** Brought back by a press of its dot: the keyboard goes into it as it comes, which no notice
   *  coming of itself ever does. */
  takesKeyboard?: boolean;
  onKeyboardTaken?: () => void;
  workspace: string;
  /** What the workspace is called: its name, or the plane root's (`OUTSIDE_TITLE`). */
  named: string;
  notices: readonly ChipNotice[];
  /** The chip it hangs from. */
  anchor: RefObject<Element | null>;
  does: ChipNotices;
}) {
  const region = useRef<HTMLDivElement>(null);
  const handed = useRef({ takesKeyboard, onKeyboardTaken });
  useLayoutEffect(() => {
    handed.current = { takesKeyboard, onKeyboardTaken };
  });
  // Once React Aria has placed it (it is drawn hidden until then), a frame or a few later.
  useEffect(() => {
    if (!takesKeyboard) return;
    let frames = 0;
    let asked = 0;
    const land = () => {
      const at = region.current;
      at?.focus({ preventScroll: true });
      if ((at !== null && document.activeElement === at) || ++frames > 10) {
        handed.current.onKeyboardTaken?.();
        return;
      }
      asked = requestAnimationFrame(land);
    };
    asked = requestAnimationFrame(land);
    return () => cancelAnimationFrame(asked);
  }, [takesKeyboard]);
  const escape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    event.preventDefault();
    for (const one of notices) does.dismiss(one.id);
  };
  return (
    <HungPopover triggerRef={anchor} isOpen onHide={does.hide} className="chip-card">
      <div
        ref={region}
        role="region"
        aria-label={`Notices for ${named}`}
        tabIndex={-1}
        data-chip={workspace}
        {...{ [CARD_STOP]: "" }}
        onKeyDown={escape}
      >
        {notices.map((one) => (
          <ActionResultNotice
            key={one.id}
            cause={`result:${one.said.from}:${one.id}`}
            said={one.said}
            onClose={() => does.dismiss(one.id)}
            onSettings={does.onSettings}
            escape={false}
          />
        ))}
      </div>
    </HungPopover>
  );
}
