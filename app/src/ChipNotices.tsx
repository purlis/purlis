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
import type { Shown } from "./bindings";
import { ActionResultNotice } from "./ActionResultNotice";
import { passing, trackOf, type KeptResult } from "./keptResults";
import { Notice, type NoticeAction } from "./Notice";
import { aboutOf, kindOf, needsOn, placeOf, type Chip, type Place } from "./noticePlace";
import { CARD_STOP } from "./regionCycle";
import type { SettingsLink } from "./settings/links";
import { HungPopover } from "./ui/popover/Popover";
import { mostUrgentOf, type NoticeKind } from "./ui/titlebar/TitleBarWorkspaces";

/**
 * **A workspace's notices, hung from its chip** (#3648, spec #3647): a view onto the one store of
 * results the window keeps (`keptResults.ts`), beside the Notices centre's Results, which lists
 * the same ones. So dismissing one here drops it there and the other way round: there is one
 * dismissed state, the kept list's. The card is up while a new notice comes to its chip, until a
 * press elsewhere hides it; a hidden one leaves the chip's dot, whose press shows it again.
 */

/**
 * **A standing line about one workspace** (#3651): a Notice the window draws while something is
 * true, whose dismissal is the per-project record's (`dismissals.ts`), keyed by its `cause`,
 * not a result the window keeps. "X is gone, kept dormant" is one. It is listed in the Notices
 * centre by its own source as it always was, and hung from its workspace's chip here too, with
 * the same ways: the two read and write one dismissed state.
 */
export type StandingNotice = {
  /** The workspace it is about, whose chip it hangs from. */
  workspace: string;
  /** What it is about, stably: its dismissal's key (`pin-dormant:delta`). */
  cause: string;
  /** Its one sentence. */
  says: string;
  /** Its ways on, as the Notices centre's line offers them. */
  fixes: readonly NoticeAction[];
  /** Dismisses it, here and in the Notices centre. */
  dismiss: () => void;
};

/** **The line of a pin whose workspace is gone** (V91c as amended, NO-1): its cause, the
 *  dismissal record's key, and its words, one for the Notices centre and the dormant chip. */
export const dormantCause = (workspace: string) => `pin-dormant:${workspace}`;
export const dormantSays = (workspace: string) =>
  `${workspace} is gone, kept dormant: its pin comes back in its place when the workspace does.`;

/**
 * **The status bar's Notices item, as a key of {@link ChipNotices}** (#3653): what a project's or
 * the app's result hangs from, as a workspace's hangs from its chip. A key no workspace can have
 * (a workspace's name has no `/`), as `OUTSIDE` is the plane root's.
 */
export const AT_STATUS = "status/bar/notices";

/**
 * **The id of a line about something still under way** (`passing` in `keptResults.ts`): a
 * command the core has not answered yet, or a progress line on no track. It is never kept, so it
 * is in no list of the Notices centre; while it is the newest thing said it stands on its card,
 * and × or Esc puts it away until the next thing is said, as the notice at the top of the
 * content did (#3653).
 */
export const PASSING = -1;

/** A notice on a chip: a result the window keeps, by its place in that list, or a standing
 *  line, by its cause (#3651). */
export type ChipNotice =
  | { id: number; said: Said; kind: NoticeKind }
  | { id: string; standing: StandingNotice; kind: NoticeKind };

/** Dismisses `one`, wherever it is kept: the kept results, or the per-project record. */
const dismissOne = (one: ChipNotice, does: Pick<ChipNotices, "dismiss">) => {
  if ("standing" in one) one.standing.dismiss();
  else does.dismiss(one.id);
};

/** What the title bar and the status bar draw of the notices hung from them. */
export type ChipNotices = {
  /** The notices hung from each chip, and from the status bar's Notices item ({@link AT_STATUS},
   *  #3653), newest first; one with none is not listed. */
  on: ReadonlyMap<string, readonly ChipNotice[]>;
  /** The chip whose card is up, or {@link AT_STATUS}, if one is. */
  open: string | undefined;
  /** **The chips a chat needs the person on** (#3652): each workspace a chat asks to have
   *  removed, by the asking chat's session. Its ✋ dot stays until the registry no longer lists
   *  the question, whatever card is up. */
  needs?: ReadonlyMap<string, number>;
  /** Brings chat `session` forward, where its question is answered on its band (#3652). */
  bring?: (session: number) => void;
  /** Shows `workspace`'s card: its dot's press, or a dormant chip's, which brings back a
   *  standing line dismissed before (#3651). */
  show: (workspace: string) => void;
  /** Hides the card up, dismissing nothing: a press anywhere else. */
  hide: () => void;
  /** Dismisses one notice, here and in the Notices centre. */
  dismiss: (id: number) => void;
  /** A refusal's way to the setting that puts it right. */
  onSettings: (link: SettingsLink) => void;
};

/** No asks: one list, so the chips' needs are worked out again only when the asks move. */
const NO_ASKS: readonly Shown[] = [];

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
  standing = NO_STANDING,
  bringBack,
  asks = NO_ASKS,
  bring = () => {},
}: {
  /** The standing lines about one workspace each (#3651), placed as a result about it is. */
  standing?: readonly StandingNotice[];
  /** Brings back what was dismissed about `workspace` before its card is shown, where a press
   *  asks for it again (a dormant chip's). */
  bringBack?: (workspace: string) => void;
  /** What waits on the person in the project in front, as the registry lists it (#3652). */
  asks?: readonly Shown[];
  /** Brings a chat of the project in front forward. */
  bring?: (session: number) => void;
  /** The newest result said, which brings its chip's card up when it is placed on one. */
  said: Said | undefined;
  kept: readonly KeptResult[];
  plane: string | undefined;
  chips: readonly Chip[];
  drop: (id: number) => void;
  onSettings: (link: SettingsLink) => void;
}): ChipNotices {
  /** The line under way put away by its ×, until the next thing is said (#3653). */
  const [putAway, setPutAway] = useState<Said>();
  const under = said !== undefined && passing(said) && said !== putAway ? said : undefined;
  // **The status bar's notices** (#3653), worked out apart from the chips': a project's or the
  // app's result is placed on the status bar whatever chips are drawn, so a chip changing (a
  // chat moving) hands the status bar the same list, and its project's view is not drawn again
  // for it. The window's own, said with no project in front, are listed in every project's, as
  // the Notices centre lists them (`resultsFor`).
  const status = useMemo(() => {
    const onStatus = (one: Said) => "project" in aboutOf(one);
    const at: ChipNotice[] = [];
    for (const one of kept)
      if ((one.plane === plane || one.plane === undefined) && onStatus(one.said))
        at.push({ id: one.id, said: one.said, kind: kindOf(one.said) });
    // Newest first, by the order each was first kept (#3649).
    at.sort((a, b) => (typeof a.id === "number" && typeof b.id === "number" ? b.id - a.id : 0));
    // A line under way first: it is the newest thing said.
    if (under !== undefined && onStatus(under))
      at.unshift({ id: PASSING, said: under, kind: kindOf(under) });
    return at.length === 0 ? undefined : at;
  }, [kept, plane, under]);
  const on = useMemo(() => {
    const by = new Map<string, ChipNotice[]>();
    /** The workspaces whose chip is dormant: gone, so nothing is offered to do in them. */
    const dormant = new Set(
      chips.filter((one) => one.look === "dormant").map((one) => one.workspace),
    );
    for (const one of kept) {
      if (one.plane !== plane) continue;
      const place = placeOfSaid(one.said, chips);
      if (place.at !== "chip") continue;
      by.set(place.workspace, [
        ...(by.get(place.workspace) ?? []),
        { id: one.id, said: onDormant(one.said, dormant), kind: kindOf(one.said) },
      ]);
    }
    // Newest first, by the order each was first kept, whatever order they are handed in: a line
    // updated in place keeps its first place (#3649). Results alone have such an order.
    for (const list of by.values())
      list.sort((a, b) => (typeof a.id === "number" && typeof b.id === "number" ? b.id - a.id : 0));
    // A standing line is older than any result this run said: after them, once they are sorted
    // (#3651).
    for (const line of standing) {
      const place = placeOf({ workspace: line.workspace }, chips);
      if (place.at !== "chip") continue;
      by.set(place.workspace, [
        ...(by.get(place.workspace) ?? []),
        // A standing line's kind is said, never worked out: it is no result (#3651).
        { id: line.cause, standing: line, kind: "info" },
      ]);
    }
    if (status !== undefined) by.set(AT_STATUS, status);
    return by;
  }, [kept, plane, chips, standing, status]);
  const needs = useMemo(() => needsOn(asks, chips), [asks, chips]);
  // **Which card is up, and in which project** (#3648's review): a card is up only in the
  // project whose result brought it up, and a project brought to the front brings none up.
  const [up, setUp] = useState<{
    open: string | undefined;
    on: string | undefined;
    seen: Said | undefined;
    in: string | undefined;
    /** The card's chip has been drawn since it was wanted. */
    chipped: boolean;
    /** Every track a line was said on, by project (`trackOf`). */
    tracks: ReadonlySet<string>;
  }>(() => ({
    open: undefined,
    on: plane,
    seen: said,
    in: plane,
    chipped: false,
    tracks: said?.track === undefined ? new Set() : new Set([trackOf(said, plane)]),
  }));
  let open = up.open;
  let openIn = up.on;
  let chipped = up.chipped;
  let tracks = up.tracks;
  // **A line on a track already said on** (#3649's review): a clone's count going on, or its
  // success, updates its notice in place and leaves the cards as the person left them, up or
  // hidden behind a dot, whatever was said between its lines, and when its notice was
  // dismissed. Decided by the track, never by the last thing said. A failure on it brings its
  // card up, since it asks for the person.
  const fresh = said !== undefined && said !== up.seen && plane === up.in ? said : undefined;
  const track = fresh?.track === undefined ? undefined : trackOf(fresh, plane);
  const follows = track !== undefined && tracks.has(track) && fresh?.refused !== true;
  if (track !== undefined && !tracks.has(track)) tracks = new Set([...tracks, track]);
  if (said !== up.seen || plane !== up.in) {
    // Nothing said is no new result: the window lets its newest result go as it moves to
    // another chat (a ✋'s press does, #3652), and the card up then stays up.
    if ((plane === up.in || up.in === undefined) && !follows && said !== undefined) {
      // A new result in the project in front. By what it is about, not by the chips drawn now:
      // a workspace just made is said before the title bar has heard of its chip, and its card
      // comes up as the chip does. A result about no workspace takes the card down.
      // A project's or the app's brings the status bar's card up (#3653). A result said with no
      // project in front (a repo taken in from the opener) stays up on the first project that
      // comes.
      const about = aboutOf(said);
      open = "workspace" in about ? about.workspace : "project" in about ? AT_STATUS : undefined;
      openIn = plane;
      chipped = false;
    } else if (up.in === undefined && plane !== undefined) {
      // The window's first project to the front: what the window said with none stays up on it.
      openIn = plane;
    }
    // Another project in front: what it said while it was behind stays behind its dots.
  }
  // **A card wanted on a chip that has gone is no longer wanted** (#3648's review): once its
  // chip was drawn and then is not (unpinned, and another workspace in front), its notice is
  // behind its dot when the chip comes back, never a card coming up unasked. Nor is a card on a
  // drawn chip with nothing left on it (#3649's review): its notices dismissed, a later line kept
  // on it again stays behind its dot.
  // The status bar's Notices item is always drawn.
  const drawn =
    open !== undefined && (open === AT_STATUS || chips.some((one) => one.workspace === open));
  if (open !== undefined && openIn === plane) {
    if (drawn && !on.has(open)) {
      open = undefined;
      chipped = false;
    } else if (drawn) chipped = true;
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
    chipped !== up.chipped ||
    tracks !== up.tracks
  )
    setUp({ open, on: openIn, seen: said, in: plane, chipped, tracks });
  const shown = openIn === plane ? open : undefined;
  // A card with nothing left on it is down.
  const cardUp = shown !== undefined && on.has(shown) ? shown : undefined;
  const lastBringBack = useRef(bringBack);
  useLayoutEffect(() => {
    lastBringBack.current = bringBack;
  });
  const show = useCallback(
    (workspace: string) => {
      lastBringBack.current?.(workspace);
      setUp((was) => ({ ...was, open: workspace, on: plane, chipped: true }));
    },
    [plane],
  );
  const hide = useCallback(() => setUp((was) => ({ ...was, open: undefined })), []);
  // A line under way is put away until the next thing is said; a result kept is dropped.
  const dismiss = useCallback(
    (id: number) => (id === PASSING ? setPutAway(said) : drop(id)),
    [drop, said],
  );
  // One object while nothing in it changes, so a project's view handed it is not drawn again.
  return useMemo(
    () => ({ on, open: cardUp, needs, bring, show, hide, dismiss, onSettings }),
    [on, cardUp, needs, bring, show, hide, dismiss, onSettings],
  );
}

/**
 * **What a result says on a dormant chip** (#3651's review): its words and its Details, but no
 * way to act in a workspace the project no longer has (an "Add a vision" on a workspace made and
 * gone since, ADR 0034's hazard). Elsewhere, as it was said.
 */
function onDormant(said: Said, dormant: ReadonlySet<string>): Said {
  if (said.workspace === undefined || !dormant.has(said.workspace)) return said;
  if (said.ways === undefined || said.ways.length === 0) return said;
  return { ...said, ways: [] };
}

/** No standing lines, one value for every render. */
const NO_STANDING: readonly StandingNotice[] = [];

/** The most urgent kind among `notices`: what a chip's dot shows (spec #3647). */
export const mostUrgent = (notices: readonly ChipNotice[]): NoticeKind =>
  mostUrgentOf(notices.map((one) => one.kind));

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
  fallback,
  side = "bottom",
}: {
  /** Where it hangs from what it is about: under a title-bar chip, above the status bar's
   *  Notices item, pointing down at it (#3653). */
  side?: "bottom" | "top";
  /** **Where the keyboard goes when the chip it hung from has gone with it** (#3651's review):
   *  a dormant chip's Forget takes the chip and the card at once. The strip's stop then: the
   *  chosen chip, else the house, else the ⌄, as a switcher closing hands it back. */
  fallback?: () => HTMLElement | null;
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
  // Once React Aria has placed it (it is drawn hidden until then), a frame or a few later; and
  // never taken back to the card's edge once the keyboard is in it already (#3896): Tab in
  // those frames goes on into it, as it would a moment later.
  useEffect(() => {
    if (!takesKeyboard) return;
    let frames = 0;
    let asked = 0;
    const land = () => {
      const at = region.current;
      if (at !== null && !at.contains(document.activeElement)) at.focus({ preventScroll: true });
      if ((at !== null && at.contains(document.activeElement)) || ++frames > 10) {
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
  const instead = useRef(fallback);
  useLayoutEffect(() => {
    chip.current = anchor.current;
    clipped.current = clip?.current ?? null;
    instead.current = fallback;
  });
  useLayoutEffect(
    () => () => {
      const had = region.current?.contains(document.activeElement) ?? false;
      const was = chip.current;
      const by = clipped.current;
      const otherwise = instead.current;
      if (!had) return;
      queueMicrotask(() => {
        const at = document.activeElement;
        if (at !== null && at !== document.body) return;
        // The chip, while it is still drawn; else, gone with its card (a dormant pin forgotten),
        // the strip's stop, never the page (WCAG 2.4.3).
        const to = was instanceof HTMLElement && was.isConnected ? was : (otherwise?.() ?? null);
        if (to === null || !to.isConnected) return;
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
    for (const one of notices) dismissOne(one, does);
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
        {/* **Stacked, newest first** (#3649): one card per chip, and a line that says so while
        it holds more than one. */}
        {notices.length > 1 && (
          <p className="chip-card-count">{notices.length} notices, newest first</p>
        )}
        {notices.map((one) =>
          "standing" in one ? (
            <Notice
              key={one.id}
              cause={one.standing.cause}
              fixes={
                one.standing.fixes.length === 0
                  ? undefined
                  : (one.standing.fixes as [NoticeAction, ...NoticeAction[]])
              }
              onDismiss={() => dismissOne(one, does)}
            >
              {one.standing.says}
            </Notice>
          ) : (
            <ActionResultNotice
              key={one.id}
              cause={`result:${one.said.from}:${one.id}`}
              said={one.said}
              onClose={() => dismissOne(one, does)}
              onSettings={does.onSettings}
              escape={false}
              // A way that goes from under the keyboard hands it to the card (#3649's review).
              refocus={() => region.current}
            />
          ),
        )}
      </div>
    </HungPopover>
  );
}
