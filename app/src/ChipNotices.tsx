// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";
import type { Said } from "./actions";
import type { Shown } from "./bindings";
import { ActionResultNotice } from "./ActionResultNotice";
import { passing, type KeptResult } from "./keptResults";
import { Notice, type NoticeAction } from "./Notice";
import { aboutOf, kindOf, needsOn, placeOf, type Chip, type Place } from "./noticePlace";
import { CARD_STOP } from "./regionCycle";
import { noticeSeenKey } from "./seenRows";
import type { SettingsLink } from "./settings/links";
import { HungPopover } from "./ui/popover/Popover";
import { mostUrgentOf, type NoticeKind } from "./ui/titlebar/TitleBarWorkspaces";

/**
 * **A workspace's notices, hung from its chip** (#3648, spec #3647): a view onto the one store of
 * results the window keeps (`keptResults.ts`), beside the Notices centre's Results, which lists
 * the same ones. So dismissing one here drops it there and the other way round: there is one
 * dismissed state, the kept list's. **A chip shows a dot, never a card that comes by itself** (ADR
 * 0121 decision 1, #6004): its notices wait behind the dot, listed in the Notices centre as New,
 * and the dot's press shows the card; a press elsewhere hides it again.
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
  /** What it says, as the Notices centre's line says it: a sentence, or one with code in it
   *  (a stale repo's branch and addresses, #3650). */
  says: ReactNode;
  /** **How urgent it is**, which the chip's dot shows (#3650): a stale repo and a layout
   *  behind are warnings (▲). News (●) where it is not said. */
  kind?: NoticeKind;
  /** Its look, as the centre's line has it: trouble once a press was refused. */
  tone?: "news" | "trouble";
  /** Its accessible name, where the sentence alone would not make a good one. */
  label?: string;
} & StandingWays;

/**
 * **A standing line's ways out**, said in the type (#3930's review): one the person can dismiss,
 * here and in the Notices centre, with whatever fixes it has; or one that goes only when it is
 * mended (a layout behind, #3650, which the centre lists with no Dismiss either), which then has
 * at least one fix.
 */
export type StandingWays =
  | {
      /** Its ways on, as the Notices centre's line offers them. */
      fixes: readonly NoticeAction[];
      /** Dismisses it, here and in the Notices centre. */
      dismiss: () => void;
    }
  | { fixes: readonly [NoticeAction, ...NoticeAction[]]; dismiss?: undefined };

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
  if ("standing" in one) one.standing.dismiss?.();
  else does.dismiss(one.id);
};

/** **The cause `one` is listed by in the Notices centre**: a standing line's own, or the cause
 *  of a result's line under Results (`ResultsGroup`), so the card and the centre say one. */
export const chipNoticeCause = (one: ChipNotice): string =>
  "standing" in one ? one.standing.cause : `result:${one.said.from}:${one.id}`;

/**
 * **The places holding something New** (ADR 0121 decision 10, #6004): each chip, or
 * {@link AT_STATUS}, with a notice on it the person has not seen, by the key its line in the
 * Notices centre is seen by. A line under way ({@link PASSING}) is in no list, so it is never New.
 */
export function freshOn(
  on: ChipNotices["on"],
  isNew: (key: string) => boolean,
): ReadonlySet<string> {
  const fresh = new Set<string>();
  for (const [place, notices] of on)
    if (notices.some((one) => one.id !== PASSING && isNew(noticeSeenKey(chipNoticeCause(one)))))
      fresh.add(place);
  return fresh;
}

/** What the title bar and the status bar draw of the notices hung from them. */
export type ChipNotices = {
  /** The notices hung from each chip, and from the status bar's Notices item ({@link AT_STATUS},
   *  #3653), newest first; one with none is not listed. */
  on: ReadonlyMap<string, readonly ChipNotice[]>;
  /** The chip whose card is up, or {@link AT_STATUS}, if one is. */
  open: string | undefined;
  /** **The places with something New on them** ({@link freshOn}, #6004): each draws a "new"
   *  pip until it is seen. None where the seen record is not read. */
  fresh?: ReadonlySet<string>;
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
  /** **The project in front** (R-333): a control a result's words name is gone to in its
   *  Notices view, from the card the result is first seen on. */
  plane?: string;
};

/** No asks: one list, so the chips' needs are worked out again only when the asks move. */
const NO_ASKS: readonly Shown[] = [];

/** Where `said` is drawn, given the title bar's `chips`. */
export const placeOfSaid = (said: Said, chips: readonly Chip[]): Place =>
  placeOf(aboutOf(said), chips);

/**
 * **The chips' notices**, worked out from what the window keeps: each result kept in `plane`
 * whose place is a chip's card, newest first, and which card is up: only the one a dot's press
 * asked for (#6004).
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
  /** The newest result said: a line under way stands on the status bar's card while it is. */
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
        { id: line.cause, standing: line, kind: line.kind ?? "info" },
      ]);
    }
    if (status !== undefined) by.set(AT_STATUS, status);
    return by;
  }, [kept, plane, chips, standing, status]);
  const needs = useMemo(() => needsOn(asks, chips), [asks, chips]);
  // **Which card is up, and in which project** (#3648's review): only one the person asked for,
  // by its dot's press (`show`), and only in the project it was asked in. **Nothing said brings
  // a card up by itself** (ADR 0121 decision 1, #6004): a new result, a failure, a project's
  // result and a line under way each stand behind a dot, listed in the Notices centre as New.
  const [up, setUp] = useState<{ open: string | undefined; on: string | undefined }>({
    open: undefined,
    on: undefined,
  });
  // **A card on a chip that has gone, or with nothing left on it, is no longer wanted** (#3648's
  // review, #3649's review): its chip back, or a line kept on it again, stays behind its dot.
  // The status bar's Notices item is always drawn.
  const asked = up.on === plane ? up.open : undefined;
  if (
    asked !== undefined &&
    (!on.has(asked) || (asked !== AT_STATUS && !chips.some((one) => one.workspace === asked)))
  )
    setUp({ open: undefined, on: undefined });
  const cardUp = asked !== undefined && on.has(asked) ? asked : undefined;
  const lastBringBack = useRef(bringBack);
  useLayoutEffect(() => {
    lastBringBack.current = bringBack;
  });
  const show = useCallback(
    (workspace: string) => {
      lastBringBack.current?.(workspace);
      setUp({ open: workspace, on: plane });
    },
    [plane],
  );
  const hide = useCallback(() => setUp({ open: undefined, on: undefined }), []);
  // A line under way is put away until the next thing is said; a result kept is dropped.
  const dismiss = useCallback(
    (id: number) => (id === PASSING ? setPutAway(said) : drop(id)),
    [drop, said],
  );
  // One object while nothing in it changes, so a project's view handed it is not drawn again.
  return useMemo(
    () => ({ on, open: cardUp, needs, bring, show, hide, dismiss, onSettings, plane }),
    [on, cardUp, needs, bring, show, hide, dismiss, onSettings, plane],
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
  // At once where it can take it, as a window's frames can stall for seconds on a busy machine
  // (#4026); else once React Aria has placed it (it is drawn hidden until then), a frame or a few
  // later. Never taken back to the card's edge once the keyboard is in it already (#3896): Tab in
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
    land();
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
  // **The keyboard stays in the card when the line it was on goes** (#3930's review, WCAG 2.4.3):
  // a stale repo switched, a layout updated, one line of several dismissed. The card stays up for
  // the rest, so its own hand-off (above) does not run; the region takes the keyboard instead of
  // the page. A browser may say nothing as a focused element is removed, so every render looks,
  // as well as the blur.
  const inside = useRef<HTMLElement | null>(null);
  const keep = () => {
    const had = inside.current;
    if (had === null || had.isConnected) return;
    inside.current = null;
    const at = document.activeElement;
    if (at !== null && at !== document.body) return;
    region.current?.focus({ preventScroll: true });
  };
  useLayoutEffect(keep);
  const onFocus = (event: FocusEvent) => {
    inside.current = event.target as HTMLElement;
  };
  const onBlur = (event: FocusEvent) => {
    // To somewhere that takes it: nothing to hand back.
    if (event.relatedTarget !== null) {
      inside.current = null;
      return;
    }
    queueMicrotask(keep);
  };
  const escape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    event.preventDefault();
    for (const one of notices) dismissOne(one, does);
    // **What cannot be dismissed goes behind its dot** (#3930's review): a layout behind stays
    // until it is mended, so Esc hides the card rather than doing nothing, and the keyboard
    // goes back to the chip (D-3648-21).
    if (notices.some((one) => "standing" in one && one.standing.dismiss === undefined)) does.hide();
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
        onFocus={onFocus}
        onBlur={onBlur}
      >
        {/* **Stacked, newest first** (#3649): one card per chip, and a line that says so while
        it holds more than one. */}
        {notices.length > 1 && (
          <p className="chip-card-count">{notices.length} notices, newest first</p>
        )}
        {notices.map((one) =>
          "standing" in one ? (
            <StandingLine
              key={one.id}
              line={one.standing}
              onDismiss={() => dismissOne(one, does)}
            />
          ) : (
            <ActionResultNotice
              key={one.id}
              cause={chipNoticeCause(one)}
              said={one.said}
              onClose={() => dismissOne(one, does)}
              onSettings={does.onSettings}
              // Its Go to, where it is first seen (R-333): in the project in front's Notices view.
              project={does.plane}
              escape={false}
              // Said once as it was said, by the window's own line (`SaidAloud`, #6004).
              quiet
              // A way that goes from under the keyboard hands it to the card (#3649's review).
              refocus={() => region.current}
            />
          ),
        )}
      </div>
    </HungPopover>
  );
}

/**
 * **One standing line on a card** (#3651, #3650): the Notice the Notices centre draws for it, with
 * its fixes and, where it has one, its Dismiss. Its ways out are held by its type
 * ({@link StandingWays}), never by a cast.
 */
function StandingLine({ line, onDismiss }: { line: StandingNotice; onDismiss: () => void }) {
  if (line.dismiss === undefined)
    return (
      // Goes only when it is mended: its fixes are its ways out (#3650).
      <Notice cause={line.cause} tone={line.tone} label={line.label} fixes={line.fixes}>
        {line.says}
      </Notice>
    );
  const [first, ...rest] = line.fixes;
  return (
    <Notice
      cause={line.cause}
      tone={line.tone}
      label={line.label}
      fixes={first === undefined ? undefined : [first, ...rest]}
      onDismiss={onDismiss}
    >
      {line.says}
    </Notice>
  );
}
