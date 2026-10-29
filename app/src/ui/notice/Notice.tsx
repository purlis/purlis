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
 *   Notices as washed cards with their mark and no edge (`paneFrame.css`, #3142).
 * - **The Inbox's list** (`NoticeListBox`, `data-scope="notice-list"`) is one box of rows with a
 *   separator between them; **a pane's row** (`NoticeRow`, `NoticeStack`,
 *   `data-scope="notice-row"`) stands above the terminal and takes the height it leaves. Both are
 *   layout, drawn here and themed through the Notices they hold.
 */
import { CircleAlert, CircleCheck, CircleQuestionMark } from "lucide-react";
import type { ComponentPropsWithRef, ReactNode, Ref } from "react";

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
  layout,
  children,
}: {
  /** **`card`** (#3344, the sidebar-views prototype's Q3 A): its mark at the start, its title
   *  with its × at the top right, its sentence, its ways on a row of their own and what a way
   *  out opened inside it. Without, the line. */
  layout?: "card";
  tone: NoticeTone;
  placement: NoticePlacement;
  /** A way out opened something under the line: in a pane the box takes a sentence's width. */
  opened: boolean;
  /** It has something to do about it (a fix or a link): what a pane's row stands first. */
  asks: boolean;
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
      data-layout={layout}
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
}: {
  children: ReactNode;
  live?: boolean;
  /** What a card is about, stably, for a test to name it by (`src/Notice.tsx`). */
  cause?: string;
}) {
  return (
    <div
      data-scope="notice"
      data-part="message"
      role={live === true ? "status" : undefined}
      data-cause={cause}
    >
      {children}
    </div>
  );
}

/** **A Notice's mark** (#3344): a check for news, an alert for trouble, a question for what needs
 *  a decision (#3678); decoration, its tone said in words inside the message. A card draws it at
 *  its start, and an attention line before its words, so attention is never told by its colour
 *  alone. */
export function NoticeMark({ tone }: { tone: NoticeTone }) {
  const Mark =
    tone === "trouble" ? CircleAlert : tone === "attention" ? CircleQuestionMark : CircleCheck;
  return (
    <span data-scope="notice" data-part="mark" aria-hidden="true">
      <Mark />
    </span>
  );
}

/** **A card's title** (#3344): what happened, before its one sentence. */
export function NoticeTitle({ children }: { children: ReactNode }) {
  return (
    <strong data-scope="notice" data-part="title">
      {children}
    </strong>
  );
}

/** **Whose it is**, for a chat that is not on screen: the first words of what it says. */
export function NoticeWhose({ children }: { children: ReactNode }) {
  return (
    <strong data-scope="notice" data-part="whose">
      {children}
    </strong>
  );
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

/** **The Notices of a pane's row, one under another**: two stand, the rest wait behind +N more. */
export function NoticeStack({
  children,
  ...rest
}: Omit<ComponentPropsWithRef<"div">, "style" | "className"> & { children: ReactNode }) {
  return (
    <div {...rest} data-scope="notice-row" data-part="stack">
      {children}
    </div>
  );
}

/** **What a pane row's "+N more" says**, open or closed (#3142): both are drawn in one cell, the
 *  one not said hidden, so the button is as wide as the longer whichever it says. */
export function NoticeMoreSays({
  open,
  closed,
  opened,
}: {
  open: boolean;
  closed: string;
  opened: string;
}) {
  return (
    <span data-scope="notice-row" data-part="says">
      <span aria-hidden={open || undefined} data-off={open || undefined}>
        {closed}
      </span>
      <span aria-hidden={!open || undefined} data-off={!open || undefined}>
        {opened}
      </span>
    </span>
  );
}
