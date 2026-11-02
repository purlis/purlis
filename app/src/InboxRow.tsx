// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  AppWindow,
  Info,
  MessageCircleQuestion,
  ShieldQuestion,
  SquareTerminal,
  TriangleAlert,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { Shown, UpdateKind } from "./bindings";
import { agoSaid } from "./inboxRules";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **What an Inbox row is made of** (qw359): the parts every row of the Inbox shares, so an ask,
 * a chat in another window, another project's ask and an update read alike.
 *
 * A row has three ranks of text, each a text role (`primitives/text.css`, #1920) worn beside
 * the class that places it in the row:
 *
 * | Rank      | Says                                   | Placed by         | Wears                         |
 * | --------- | -------------------------------------- | ----------------- | ----------------------------- |
 * | primary   | what needs the person: the ask's words | `inbox-primary`   | `ui-text-body ui-text-strong` |
 * | secondary | where: project · workspace · persona   | `inbox-secondary` | `ui-text-secondary`           |
 * | meta      | its kind and how long it has waited    | `inbox-meta`      | `ui-text-meta`                |
 *
 * The primary line is the strongest and may be cut to two lines (`inbox-clamped`), with the
 * whole of it in its tooltip and still in the page for a screen reader. **The words of an ask
 * that offers an answer that allows are never cut** (#1146): what an Allow lets run is read
 * whole before it is pressed.
 *
 * A row's kind is its mark and its words together ({@link KindMark} beside the meta), never a
 * colour alone.
 */

/** What kind of thing a row is: what its mark is and which tone it wears. */
export type RowKind = "decision" | "reply" | "terminal" | "elsewhere" | "update" | "trouble";

/** The kind of an ask, by its source: a decision is anything answered by allowing or not. */
export const kindOfAsk = (source: Shown["source"]): RowKind =>
  source === "question" || source === "task-question"
    ? "reply"
    : source === "terminal"
      ? "terminal"
      : "decision";

/** The updates that say something went wrong, marked as trouble. */
const TROUBLE: ReadonlySet<UpdateKind> = new Set<UpdateKind>([
  "task-failed",
  "refused-away",
  "smart-close",
  "report-undelivered",
  "commit-refused",
  "question-failed",
]);

/** The kind of an update: trouble where it says something came to nothing. */
export const kindOfUpdate = (kind: UpdateKind): RowKind =>
  TROUBLE.has(kind) ? "trouble" : "update";

const MARKS: Record<RowKind, typeof Info> = {
  decision: ShieldQuestion,
  reply: MessageCircleQuestion,
  terminal: SquareTerminal,
  elsewhere: AppWindow,
  update: Info,
  trouble: TriangleAlert,
};

/**
 * **A row's kind, as a mark**: an icon in the kind's tone (`data-kind`). Decoration: the row's
 * meta says the kind in words, so the mark is hidden from a screen reader and nothing is told
 * by its colour or its shape alone.
 */
export function KindMark({ kind }: { kind: RowKind }) {
  const Mark = MARKS[kind];
  return (
    <span className="inbox-kind" data-kind={kind} aria-hidden="true">
      <Mark />
    </span>
  );
}

/** How often a drawn age is read again. */
const AGE_READ_MS = 60_000;

/** The time as of the last reading: at the first drawing, then again each minute, so an age
 *  drawn from it is the same however often its row is drawn in between. */
export function useNow(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const again = setInterval(() => setNow(Date.now()), AGE_READ_MS);
    return () => clearInterval(again);
  }, []);
  return now;
}

/**
 * **How long a row has waited, or when it happened**: `5m`, as `agoSaid` says it, with the time
 * itself in its tooltip. Nothing where the time is not known.
 */
export function RowAge({
  when,
  now,
  since,
  lead = false,
}: {
  when: number;
  now: number;
  /** What its tooltip says before the time: "Waiting since". */
  since: string;
  /** Whether it follows other words of the meta, and is set off from them. */
  lead?: boolean;
}) {
  const said = agoSaid(when, now);
  if (said === undefined) return null;
  const at = new Date(when);
  return (
    <>
      {lead && <span className="inbox-meta-dot">{" · "}</span>}
      <Tooltip
        tip={`${since} ${at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`}
      >
        <time className="inbox-age" dateTime={at.toISOString()}>
          {said}
        </time>
      </Tooltip>
    </>
  );
}
