// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
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
import { passing, type KeptResult } from "./keptResults";
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

/**
 * **The status bar's Notices item, as a key of {@link ChipNotices}** (#3653): what a project's or
 * the app's result hangs from, as a workspace's hangs from its chip. A key no workspace can have
 * (a workspace's name has no `/`), as `OUTSIDE` is the plane root's.
 */
export const AT_STATUS = "status/bar/notices";

/**
 * **The id of a line about something still under way** (`passing` in `keptResults.ts`): "Cloning
 * 2 repo(s)…", or a command the core has not answered yet. It is never kept, so it is in no list
 * of the Notices centre; while it is the newest thing said it stands on its card, and × or Esc
 * puts it away until the next thing is said, as the notice at the top of the content did.
 */
export const PASSING = -1;

/** What the title bar and the status bar draw of the notices hung from them. */
export type ChipNotices = {
  /** The notices hung from each chip, and from the status bar's Notices item ({@link AT_STATUS}),
   *  newest first; one with none is not listed. */
  on: ReadonlyMap<string, readonly ChipNotice[]>;
  /** The chip whose card is up, or {@link AT_STATUS}, if one is. */
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
  /** The line under way put away by its ×, until the next thing is said. */
  const [putAway, setPutAway] = useState<Said>();
  const under = said !== undefined && passing(said) && said !== putAway ? said : undefined;
  // **The status bar's notices** (#3653), worked out apart from the chips': a project's or the
  // app's result is placed on the status bar whatever chips are drawn, so a chip's count
  // changing (a chat moving) hands the status bar the same list, and its project's view is not
  // drawn again for it.
  const status = useMemo(() => {
    const at: ChipNotice[] = [];
    const onStatus = (one: Said) => "project" in aboutOf(one);
    // A line under way first: it is the newest thing said.
    if (under !== undefined && onStatus(under)) at.push({ id: PASSING, said: under, kind: "info" });
    for (const one of kept)
      // The window's own, said with no project in front, are listed in every project's, as the
      // Notices centre lists them (`resultsFor`).
      if ((one.plane === plane || one.plane === undefined) && onStatus(one.said))
        at.push({ id: one.id, said: one.said, kind: "info" });
    return at.length === 0 ? undefined : at;
  }, [kept, plane, under]);
  const on = useMemo(() => {
    const by = new Map<string, ChipNotice[]>();
    const hang = (place: Place, notice: ChipNotice) => {
      if (place.at === "chip")
        by.set(place.workspace, [...(by.get(place.workspace) ?? []), notice]);
    };
    if (under !== undefined)
      hang(placeOfSaid(under, chips), { id: PASSING, said: under, kind: "info" });
    for (const one of kept) {
      if (one.plane !== plane) continue;
      hang(placeOfSaid(one.said, chips), { id: one.id, said: one.said, kind: "info" });
    }
    if (status !== undefined) by.set(AT_STATUS, status);
    return by;
  }, [kept, plane, chips, under, status]);
  // **Which card is up, and in which project** (#3648's review): a card is up only in the
  // project whose result brought it up, and a project brought to the front brings none up.
  const [up, setUp] = useState<{
    open: string | undefined;
    on: string | undefined;
    seen: Said | undefined;
    in: string | undefined;
    /** The card's chip has been drawn since it was wanted. */
    chipped: boolean;
  }>({ open: undefined, on: plane, seen: said, in: plane, chipped: false });
  let open = up.open;
  let openIn = up.on;
  let chipped = up.chipped;
  if (said !== up.seen || plane !== up.in) {
    if (said !== up.seen && (plane === up.in || up.in === undefined)) {
      // A new result in the project in front. By what it is about, not by the chips drawn now:
      // a workspace just made is said before the title bar has heard of its chip, and its card
      // comes up as the chip does. A result about no workspace takes the card down.
      // A project's or the app's brings the status bar's card up (#3653).
      const about = said === undefined ? undefined : aboutOf(said);
      open =
        about === undefined
          ? undefined
          : "workspace" in about
            ? about.workspace
            : "project" in about
              ? AT_STATUS
              : undefined;
      openIn = plane;
      chipped = false;
    } else if (up.in === undefined) {
      // **The window's first project to the front** (#3653): what the window said with none in
      // front (a repo taken in from the opener) stays up on it, as it did on the opener's page.
      openIn = plane;
    }
    // Another project in front: what it said while it was behind stays behind its dots.
  }
  // **A card wanted on a chip that has gone is no longer wanted** (#3648's review): once its
  // chip was drawn and then is not (unpinned, and another workspace in front), its notice is
  // behind its dot when the chip comes back, never a card coming up unasked.
  // The status bar's Notices item is always drawn.
  const drawn =
    open !== undefined && (open === AT_STATUS || chips.some((one) => one.workspace === open));
  if (open !== undefined && openIn === plane) {
    if (drawn) chipped = true;
    else if (chipped) {
      open = undefined;
      chipped = false;
    }
  }
  if (
    open !== up.open ||
    openIn !== up.on ||
    said !== up.seen ||
    plane !== up.in ||
    chipped !== up.chipped
  )
    setUp({ open, on: openIn, seen: said, in: plane, chipped });
  const shown = openIn === plane ? open : undefined;
  // A card with nothing left on it is down.
  const cardUp = shown !== undefined && on.has(shown) ? shown : undefined;
  const show = useCallback(
    (workspace: string) => setUp((was) => ({ ...was, open: workspace, on: plane, chipped: true })),
    [plane],
  );
  const hide = useCallback(() => setUp((was) => ({ ...was, open: undefined })), []);
  const dismiss = useCallback(
    (id: number) => (id === PASSING ? setPutAway(said) : drop(id)),
    [drop, said],
  );
  // One object while nothing in it changes, so a project's view handed it is not drawn again.
  return useMemo(
    () => ({ on, open: cardUp, show, hide, dismiss, onSettings }),
    [on, cardUp, show, hide, dismiss, onSettings],
  );
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
 * The status bar's card is the same card, hung above its Notices item (#3653).
 */
export function ChipCard({
  workspace,
  named,
  notices,
  anchor,
  does,
  takesKeyboard = false,
  onKeyboardTaken,
  clip,
  side = "bottom",
}: {
  /** Where it hangs from what it is about: under a title-bar chip, above the status bar's
   *  Notices item, pointing down at it (#3653). */
  side?: "bottom" | "top";
  /** What clips the chip it hangs from (the strip): out of its sight, the card is hidden. */
  clip?: RefObject<Element | null>;
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
  // **The keyboard back on the chip** (#3648's review, WCAG 2.4.3): a card the keyboard was in
  // that goes (its last notice dismissed) hands it to the chip it hung from, which is still
  // drawn, unless something else took it as it went. The dot it may have come from is gone.
  const chip = useRef<Element | null>(null);
  const clipped = useRef<Element | null>(null);
  useLayoutEffect(() => {
    chip.current = anchor.current;
    clipped.current = clip?.current ?? null;
  });
  useLayoutEffect(
    () => () => {
      const had = region.current?.contains(document.activeElement) ?? false;
      const to = chip.current;
      const by = clipped.current;
      if (!had || !(to instanceof HTMLElement)) return;
      queueMicrotask(() => {
        const at = document.activeElement;
        if ((at !== null && at !== document.body) || !to.isConnected) return;
        // A chip out of its strip's sight is brought back into it, so the keyboard's focus is
        // never on what cannot be seen (WCAG 2.4.11); one in sight stays where it is.
        const box = to.getBoundingClientRect();
        const seen = by?.getBoundingClientRect();
        const hidden = seen !== undefined && (box.right <= seen.left || box.left >= seen.right);
        to.focus({ preventScroll: true });
        if (hidden && typeof to.scrollIntoView === "function")
          to.scrollIntoView({ inline: "nearest", block: "nearest" });
      });
    },
    [],
  );
  const escape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    event.preventDefault();
    for (const one of notices) does.dismiss(one.id);
  };
  return (
    <HungPopover
      triggerRef={anchor}
      clipRef={clip}
      side={side}
      isOpen
      onHide={does.hide}
      className="chip-card"
    >
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
