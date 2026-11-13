// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The notice family** (#2912, spec #2897, ADR 0104): how a Notice is drawn. What a Notice is,
 * its ways out and where it stands are `src/Notice.tsx`'s (ADR 0037's amendment, NO-1): it draws
 * through these parts, and no screen draws a Notice's box, its list or its pane's row by hand
 * (`purlis/notice-family`).
 *
 * - **Drawn by its parts.** A Notice's box is `data-scope="notice" data-part="root"`, its
 *   `data-variant` its tone (`news`, `trouble`, `attention`) and its `data-placement` where it
 *   stands: `pane`
 *   (its pane's row, above the terminal), `inbox` (a row of the Inbox's list) or `alone` (on a
 *   page). Inside it are its line (the ways out's row, `ButtonRow`), what it says (`message`),
 *   whose it is (`whose`) and what a way out opened (`under`). `notice.css` draws the family
 *   default and a theme's parts reach the same attributes (ADR 0103).
 * - **A Notice is a consent surface.** It holds Allow beside Keep blocked, so its fill is one of
 *   the grounds an answer that grants is held apart from the primary on, and its edge's width is
 *   no theme's (`purlis_core::theme::parts`): Allow keeps its own look on it, and purlis speaking
 *   wherever the window draws its edge keeps its waiting edge. A chat pane's band draws its
 *   Notices as compact rows (`layout="row"`, `paneFrame.css`, #5215), a consent's waiting edge held.
 * - **The Inbox's list** (`NoticeListBox`, `data-scope="notice-list"`) is, in the Notices view,
 *   a column of the message family's rows (#5649; cards in the Inbox card's style before, #5217),
 *   and elsewhere one box of
 *   rows with a separator between them; **a pane's row** (`NoticeRow`, `NoticeStack`,
 *   `data-scope="notice-row"`) stands above the terminal and takes the height it leaves. Both are
 *   layout, drawn here and themed through the Notices they hold.
 */
import {
  ChevronDown,
  CircleAlert,
  CircleCheck,
  CircleQuestionMark,
  Info,
  LoaderCircle,
  Shield,
  SquareTerminal,
} from "lucide-react";
import {
  useLayoutEffect,
  type ComponentPropsWithRef,
  type ReactNode,
  type Ref,
  type RefObject,
} from "react";
import { Button } from "../button/Button";
import { useObjectRef } from "react-aria";
import { useScrollEdges } from "./scrollEdges";

/** The family's variants, in the order the manifest names them. */
export const NOTICE_VARIANTS = ["news", "trouble", "attention"] as const;
export type NoticeTone = (typeof NOTICE_VARIANTS)[number];

/** Where a Notice stands: its pane's row, a row of the Inbox's list, or alone on a page. */
export type NoticePlacement = "pane" | "inbox" | "alone";

/** **A Notice's box**: its line and what a way out opened under it. */
export function NoticeBox({
  tone,
  placement,
  opened,
  asks,
  waits = false,
  consent = false,
  layout,
  whole = false,
  what,
  children,
}: {
  /** **`card`** (#3344, the sidebar-views prototype's Q3 A): its mark at the start, its title
   *  with its × at the top right, its sentence, its ways on a row of their own and what a way
   *  out opened inside it. Without, the line. **`head`** (#5217): the Notices centre's, a
   *  one-line head over its sentence and its ways. **`row`** (#5215): one compact row of a chat
   *  pane's band. */
  layout?: "card" | "head" | "row";
  /** **A row's message, opened whole** (#5215): its ways are pinned at its foot. */
  whole?: boolean;
  /** **What it is about, in a word or two**, for the folded band's line to name (#5215). */
  what?: string;
  /** **It waits on the person's consent** (ADR 0097 §6): in the Notices centre (#5217) and as a
   *  pane's row (#5215) it keeps its full waiting edge, where any other Notice's outline is softer. */
  consent?: boolean;
  tone: NoticeTone;
  placement: NoticePlacement;
  /** A way out opened something under the line: in a pane the box takes a sentence's width. */
  opened: boolean;
  /** It has something to do about it (a fix or a link). */
  asks: boolean;
  /** **An ask, in a pane** (#4754): it stays when the pane's notices are minimised, and its
   *  bell never counts it. */
  waits?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      data-scope="notice"
      data-part="root"
      data-variant={tone}
      data-placement={placement}
      data-opened={opened ? "" : undefined}
      data-asks={asks ? "" : undefined}
      data-waits={waits ? "" : undefined}
      data-consent={consent ? "" : undefined}
      data-layout={layout}
      data-whole={whole ? "" : undefined}
      data-what={what}
    >
      {children}
    </div>
  );
}

/** **What a Notice says**: its sentence, and anything it says before its ways out. A card's is
 *  its live region (`live`), unless it is a copy of one already said. */
export function NoticeMessage({
  children,
  live,
  cause,
  ref,
}: {
  children: ReactNode;
  live?: boolean;
  /** What a card is about, stably, for a test to name it by (`src/Notice.tsx`). */
  cause?: string;
  /** Where a row measures whether its one line cuts it (#5215). */
  ref?: Ref<HTMLDivElement>;
}) {
  return (
    <div
      ref={ref}
      data-scope="notice"
      data-part="message"
      role={live === true ? "status" : undefined}
      data-cause={cause}
    >
      {children}
    </div>
  );
}

/** The icons only a row of a pane's band draws (#5215). */
export type NoticeMarkKind = "note" | "prompt" | "consent";

const NOTICE_MARKS = { note: Info, prompt: SquareTerminal, consent: Shield } as const;

/** **A Notice's mark** (#3344): a check for news, an alert for trouble, a question for what needs
 *  a decision (#3678); decoration, its tone said in words inside the message. A card draws it at
 *  its start, and an attention line before its words, so attention is never told by its colour
 *  alone. */
export function NoticeMark({
  tone,
  running = false,
  kind,
}: {
  tone: NoticeTone;
  running?: boolean;
  /** **What a row of a pane's band says it is, by its icon** (#5215): an information line, a
   *  prompt in the terminal, or a consent. Without, its tone's. */
  kind?: NoticeMarkKind;
}) {
  const Mark =
    kind === undefined
      ? tone === "trouble"
        ? CircleAlert
        : tone === "attention"
          ? CircleQuestionMark
          : CircleCheck
      : NOTICE_MARKS[kind];
  return (
    <span data-scope="notice" data-part="mark" aria-hidden="true">
      {running ? <LoaderCircle className="spinning" /> : <Mark />}
    </span>
  );
}

/** **A Notices view row's live words** (#5217, #5649): the whole row, its ways and its × with it,
 *  so a test and a screen reader find them as they do a line's, and never what a way out opened. */
export function NoticeSaid({
  children,
  label,
  cause,
  live,
}: {
  children: ReactNode;
  label?: string;
  cause: string;
  /** Off for a copy of a Notice already said (a result kept under Results). */
  live: boolean;
}) {
  return (
    <div
      data-scope="notice-card"
      data-part="said"
      role={live ? "status" : undefined}
      aria-label={live ? label : undefined}
      data-cause={cause}
    >
      {children}
    </div>
  );
}

/** **A card's title** (#3344): what happened, before its one sentence. */
export function NoticeTitle({ children }: { children: ReactNode }) {
  return (
    <strong data-scope="notice" data-part="title" className="ui-text-body ui-text-strong">
      {children}
    </strong>
  );
}

/** **Whose it is**, for a chat that is not on screen: the first words of what it says. */
export function NoticeWhose({ children }: { children: ReactNode }) {
  return (
    <strong data-scope="notice" data-part="whose" className="ui-text-strong">
      {children}
    </strong>
  );
}

/** **A row's words** (#5215): what it says on one line, and its sub-line muted beneath. */
export function NoticeWords({ children }: { children: ReactNode }) {
  return (
    <div data-scope="notice" data-part="words">
      {children}
    </div>
  );
}

/** **A row's sub-line** (#5215): what kind of notice it is, or who asks, and its age. */
export function NoticeSub({ children }: { children: ReactNode }) {
  return (
    <span data-scope="notice" data-part="sub" className="ui-text-meta">
      {children}
    </span>
  );
}

/** **A row's ways out** (#5215): its own buttons, its toggle, then its ×, in that order. A message
 *  opened whole pins them at its foot. */
export function NoticeWays({ children, ref }: { children: ReactNode; ref?: Ref<HTMLDivElement> }) {
  return (
    <div ref={ref} data-scope="notice" data-part="ways">
      {children}
    </div>
  );
}

/** **The room one of a row's icon ways takes** (#5215): its toggle, kept where a way out must not
 *  move for it, or the × a note with no other home has none of (#5311). */
export function NoticeWaySlot() {
  return <span data-scope="notice" data-part="way-slot" aria-hidden="true" />;
}

/** **What a way out opened**, under the line and outside its live region. */
export function NoticeUnder({ children }: { children: ReactNode }) {
  return (
    <div data-scope="notice" data-part="under">
      {children}
    </div>
  );
}

/** Where the Inbox's list is drawn: in the Inbox (`inbox`), the window's own lines at the top of
 *  an Inbox (`lines`), under the title bar until an Inbox is drawn (`band`), or at the top of the
 *  page a window with no project shows (`page`). */
export type NoticeListAt = "inbox" | "lines" | "band" | "page";

/** **The Inbox's list of Notices**: one box, a row per Notice, which the list moves itself. */
export function NoticeListBox({
  at,
  hidden,
  ref,
  className,
}: {
  at: NoticeListAt;
  hidden: boolean;
  ref: Ref<HTMLDivElement>;
  /** Its place in its view. */
  className?: string;
}) {
  return (
    <div
      ref={ref}
      data-scope="notice-list"
      data-part="root"
      data-at={at}
      hidden={hidden}
      className={className}
    />
  );
}

/** **A pane's row of Notices**, above its terminal; none drawn while it holds none. */
export function NoticeRow({
  empty,
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style" | "className"> & {
  empty: boolean;
  children: ReactNode;
}) {
  return (
    <div {...rest} data-scope="notice-row" data-part="root" data-empty={empty ? "" : undefined}>
      {children}
    </div>
  );
}

/** **The Notices of a pane's row, one under another**: the asks, and the notes unless the band
 *  is minimised (#4754). Past its room it scrolls, and marks the edge it has more past for its fade (#3168). */
export function NoticeStack({
  children,
  ref,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style" | "className"> & { children: ReactNode }) {
  const own = useObjectRef(ref);
  useScrollEdges(own);
  return (
    <div {...rest} ref={own} data-scope="notice-row" data-part="stack">
      {children}
    </div>
  );
}

/** **The folded band's one line** (#5215): one button, what waits at its start and Show at its
 *  far end, where a dot before it would stand alone, so only a reader hears that. */
export function NoticeRowFolded({
  says,
  show,
  onPress,
  ref,
}: {
  says: string;
  show: string;
  onPress: () => void;
  ref?: Ref<HTMLButtonElement>;
}) {
  return (
    <div data-scope="notice-row" data-part="folded">
      <Button ref={ref} size="sm" variant="ghost" onPress={onPress} data-way="show">
        <span data-scope="notice-row" data-part="folded-says">
          {says}
        </span>
        <span className="sr-only"> · </span>
        <span data-scope="notice-row" data-part="folded-show">
          {show}
          <ChevronDown aria-hidden="true" />
        </span>
      </Button>
    </div>
  );
}

/**
 * **What a pinned foot covers is kept clear of the keyboard** (#5215, WCAG 2.4.11): while `pinned`,
 * the stack it scrolls in stops a focused control above it, however tall the foot is.
 */
export function usePinnedRoom(foot: RefObject<HTMLElement | null>, pinned: boolean): void {
  useLayoutEffect(() => {
    const at = foot.current;
    const stack = at?.closest<HTMLElement>('[data-scope="notice-row"][data-part="stack"]');
    if (!pinned || !at || !stack) return;
    const say = () => stack.style.setProperty("scroll-padding-block-end", `${at.offsetHeight}px`);
    say();
    const watch = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(say);
    watch?.observe(at);
    return () => {
      watch?.disconnect();
      stack.style.removeProperty("scroll-padding-block-end");
    };
  }, [foot, pinned]);
}

/** **The open band's head** (#4754, #5215): what it holds, then its ways (Dismiss all, the fold),
 *  above its rows. */
export function NoticeRowHead({ says, children }: { says: string; children: ReactNode }) {
  return (
    <div data-scope="notice-row" data-part="head">
      <span data-scope="notice-row" data-part="says">
        {says}
      </span>
      {children}
    </div>
  );
}
