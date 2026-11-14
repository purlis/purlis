// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { X } from "lucide-react";
import { type InboxUpdate } from "./bindings";
import { KindIcon, kindOfUpdate } from "./InboxRow";
import { toneOfUpdate } from "./messageTone";
import { type NoticesClear } from "./Notice";
import { DISMISS_ALL, DISMISS_ALL_TIP, holdOrCarryOut, type DismissHold } from "./dismissHold";
import { KIND_SAID } from "./inboxUpdates";
import { Button, ButtonRow, IconButton } from "./ui/button/Button";
import {
  MessageBody,
  MessageForm,
  MessageLinks,
  MessageMeta,
  MessageRow,
  MessageTop,
} from "./ui/message/Message";
import { List } from "./ui/list/List";
import { PanelSectionTitle } from "./ui/panel/Panel";
import { Tooltip } from "./ui/tooltip/Tooltip";

import { chainSaid, NothingDone, timeSaid, useSettledPlace } from "./inboxRowParts";
import { updateSeenKey, useSeenRow } from "./seenRows";

// **The Updates view's rows** (#1693, #5646; moved from `Inbox.tsx`): Mine's and Project's lines,
// and their Dismiss held for its Undo.

/** One update as the Inbox draws it, with what can be done with it there. */
export type UpdateRow = {
  update: InboxUpdate;
  /** Go to chat, where it is about a chat that is still there. */
  go?: () => void;
  /** A sentence of the source's own under it, before its answers. */
  more?: string;
  /** **What it opens in place to show** (#5651, ADR 0119 decision 8): a finished task's report,
   *  or why it failed, in the task's words. Read opens it. */
  report?: string;
  /** Mark it read: done once it is opened in place. */
  read?: () => void;
  /** Its source's own answers, in their order: a refused dispatch's three. */
  answers?: readonly UpdateAnswer[];
  /** Put it away: and, where its source lists it, put away there too. None on a line that
   *  holds what still waits on the person (Q274): it has no Dismiss, and Dismiss all leaves it. */
  dismiss?: () => void;
  /** What Dismiss does, as its tooltip, where it does more than put the update away. */
  dismissSays?: string;
};

/** One of an update's answers. */
export type UpdateAnswer = {
  label: string;
  /** Its accessible name: what it does, whole. */
  name: string;
  /** What it does, as its tooltip. */
  title: string;
  /** Whether it allows something: drawn as an Allow is. */
  allows?: boolean;
  /** What it does. Where it answers a sentence, the answer was refused, and the line says why
   *  (Q274), as an ask's row does. */
  press: () => Promise<string | undefined> | undefined;
};

/** The project's updates and the two acts on all of them. */
export type Updates = {
  /** Newest first; nothing before they were read. */
  rows: readonly UpdateRow[] | undefined;
  /** The line a clicked notification opens, read in place, and the ask it came with: a task
   *  that ended (#6005). Each ask opens it once. */
  opened?: { key: string; at: number };
  /** Every update listed, marked read: none is put away. `keys` are the lines the list shows
   *  (#5646): Mine's or Project's, in the scope picked. */
  onMarkAllRead: (keys: readonly string[]) => void;
  /** Every update listed, put away. No ask is touched. */
  onDismissAll: (keys: readonly string[]) => void;
  /** The teammate Notices Project lists with a Dismiss of their own, which its Dismiss all
   *  dismisses too (#5650). */
  teammates?: NoticesClear;
};

/** The key an update is held by while its Dismiss waits for its Undo. */
export const heldUpdate = (key: string) => `update:${key}`;

/**
 * **Updates as a list that holds its Dismiss draws them** (#5650): the lines held for an Undo
 * left out, each line's Dismiss and Dismiss all held for it, and Project's Dismiss all doing
 * each teammate Notice's own Dismiss with the lines'.
 */
export function heldBy(
  hold: DismissHold | undefined,
  updates: Updates,
  rows: readonly UpdateRow[] | undefined,
  teammates: NoticesClear | undefined,
): Updates {
  const done = (keys: readonly string[], out: () => void) => holdOrCarryOut(hold, keys, out);
  return {
    ...updates,
    rows: rows
      ?.filter((row) => hold?.held.has(heldUpdate(row.update.key)) !== true)
      .map(({ dismiss, ...row }) =>
        hold === undefined || dismiss === undefined
          ? { ...row, dismiss }
          : { ...row, dismiss: () => done([heldUpdate(row.update.key)], dismiss) },
      ),
    teammates,
    onDismissAll: (keys) => {
      const theirs = teammates?.take();
      done([...keys.map(heldUpdate), ...(theirs?.keys ?? [])], () => {
        if (keys.length > 0) updates.onDismissAll(keys);
        theirs?.out();
      });
    },
  };
}

/** **What the updates' section draws, in its order** (#1693, #3123): its Mark all read and
 *  Dismiss all, then each line, its answers, its Read and Go to chat, and its Dismiss. A line
 *  stands under what is drawn above it, and moves when that changes (`useSettledPlace`). */
export function updatePlaces(updates: Updates | undefined): string[] {
  const rows = updates?.rows ?? [];
  if (rows.length === 0 && updates?.teammates === undefined) return [];
  return [
    READ_ALL,
    DISMISS_EVERY,
    ...rows.flatMap(({ update, go, report, answers = [], dismiss }) => {
      const key = `update:${update.key}`;
      return [
        key,
        ...answers.map((_, at) => `${key}:${at}`),
        ...(report === undefined ? [] : [`${key}:read`]),
        ...(go === undefined ? [] : [`${key}:go`]),
        ...(dismiss === undefined ? [] : [`${key}:dismiss`]),
      ];
    }),
  ];
}

const READ_ALL = "inbox:read-all";
const DISMISS_EVERY = "inbox:dismiss-all";

/** What the updates' section is called. */
export const UPDATES = "Updates";

/** The updates, after the asks: newest first, each with Dismiss (#1693). `children` are the
 *  teammate Notices Project lists after its lines (#5646), which its Dismiss all takes too. */
export function UpdateList({
  updates,
  places,
  children,
}: {
  updates: Updates;
  /** What the Inbox draws, in its order (`updatePlaces`): what each line stands under. */
  places: readonly string[];
  children?: ReactNode;
}) {
  const rows = updates.rows ?? [];
  const unread = rows.some(({ update }) => !update.read);
  // The teammate Notices stay in one place whatever is drawn above them, so none is drawn again.
  return (
    <>
      {(rows.length > 0 || updates.teammates !== undefined) && (
        <section className="inbox-updates" aria-label={UPDATES}>
          <div className="inbox-updates-head">
            <PanelSectionTitle level={3} className="inbox-chain">
              {UPDATES}
            </PanelSectionTitle>
            <Button
              size="sm"
              variant="ghost"
              isDisabled={!unread}
              tooltip="Mark every update read. Nothing that waits on you is answered."
              onPress={() => updates.onMarkAllRead(rows.map(({ update }) => update.key))}
              className="inbox-go"
            >
              Mark all read
            </Button>
            <Button
              size="sm"
              variant="ghost"
              tooltip={DISMISS_ALL_TIP}
              // A line with no Dismiss of its own is not Dismiss all's either (Q274).
              onPress={() =>
                updates.onDismissAll(
                  rows.filter((row) => row.dismiss !== undefined).map(({ update }) => update.key),
                )
              }
              className="inbox-go"
            >
              {DISMISS_ALL}
            </Button>
          </div>
          {rows.length > 0 && (
            // **The list family's** (#3123, ruling B): ↑ and ↓ between the lines, ← and →
            // through a line's own links and answers, and typing finds none.
            <List aria-label={UPDATES} className="inbox-rows" disallowTypeAhead>
              {rows.map((row) => (
                <UpdateItem
                  key={row.update.key}
                  row={row}
                  opened={updates.opened?.key === row.update.key ? updates.opened.at : undefined}
                  above={places.slice(0, places.indexOf(`update:${row.update.key}`)).join("\n")}
                />
              ))}
            </List>
          )}
        </section>
      )}
      {children}
    </>
  );
}

function UpdateItem({
  row,
  above,
  opened,
}: {
  row: UpdateRow;
  above: string;
  /** The ask of a clicked notification that opens this line in place, where one did (#6005). */
  opened?: number;
}) {
  const { update, go, more, report, read, answers = [], dismiss, dismissSays } = row;
  const id = useId();
  const [said, setSaid] = useState<string>();
  /** Its report is open in place (#5651). */
  const [open, setOpen] = useState(false);
  const toggle = () => {
    // Opening it is reading it: it is marked read, and the bell drops it.
    if (!open && !update.read) read?.();
    setOpen(!open);
  };
  /** **A clicked notification opens it** (#6005): as Read does, once for each click. */
  const openedFor = useRef<number>(undefined);
  useEffect(() => {
    if (opened === undefined || openedFor.current === opened || report === undefined) return;
    openedFor.current = opened;
    if (!update.read) read?.();
    setOpen(true);
  }, [opened, report, read, update.read]);
  /**
   * **When this update was last drawn where it stands**: at its first drawing, and whenever
   * what stands above it changes, which moves it. Set in the very render that draws it, as a
   * dispatch's Notice sets its own (`TaskBlocksNotice`), so no press lands on a place whose
   * time is not known yet.
   */
  // What the row offers is part of where it stands: an answer whose words or tooltip changed
  // under the pointer is a new place, though nothing above it moved.
  const here = [above, more ?? "", ...answers.map(({ name, title }) => `${name}\t${title}`)].join(
    "\n",
  );
  const { drawn, tooSoon, readPlaces } = useSettledPlace<HTMLElement>(here);
  // **New until seen** (#6002): a line marked read is not New either.
  const [unseen, seenDrawnNow] = useSeenRow(updateSeenKey(update.key), drawn);
  const isNew = unseen && !update.read;
  /** An answer that allows something, pressed too soon after the update was drawn or moved,
   *  is not sent: what is under the pointer may not be what the person read. */
  const press = (answer: UpdateAnswer) => {
    if (answer.allows && tooSoon()) {
      setSaid(
        "This update was drawn or moved just now, so nothing was allowed. Read it and press again.",
      );
      return;
    }
    setSaid(undefined);
    const refused = answer.press();
    if (refused !== undefined)
      void refused.then((why) => {
        if (why !== undefined) setSaid(why);
      });
  };
  const key = `update:${update.key}`;
  const who = chainSaid(update.chain);
  const about = who === "" ? update.says : `${who}: ${update.says}`;
  const kind = kindOfUpdate(update.kind);
  // **A line of Mine or Project** (#5649, ADR 0119 decision 6), a row of the list family's
  // (#3123): its kind's glyph in its state's tint; whose it is, or what happened where it is no
  // chat's, with its time at the right, the × in its place under the pointer; its kind; what
  // happened; Go to chat.
  return (
    <MessageRow
      as="row"
      id={key}
      textValue={about}
      tone={toneOfUpdate(update.kind)}
      icon={<KindIcon kind={kind} />}
      read={update.read}
      ref={drawn}
      onDrawn={() => {
        readPlaces();
        seenDrawnNow();
      }}
      isNew={isNew}
      className="inbox-update"
      data-kind={update.kind}
      aria-label={about}
      aria-describedby={`${id}-kind`}
      dismiss={
        dismiss === undefined ? undefined : (
          <IconButton
            icon={<X />}
            size="sm"
            variant="quiet"
            aria-label={`Dismiss: ${about}`}
            tooltip={dismissSays ?? "Put this update away"}
            onPress={dismiss}
          />
        )
      }
    >
      {/* Every word here is its source's, drawn as text and never as a control. An update
            whose answer allows is read whole, as an ask is (#1146). */}
      <MessageTop
        title={who === "" ? <span className="ask-says">{update.says}</span> : who}
        time={
          <time dateTime={new Date(update.at * 1000).toISOString()}>
            {timeSaid(update.at * 1000)}
          </time>
        }
      />
      <MessageMeta>
        <span id={`${id}-kind`} className="inbox-source">
          {KIND_SAID[update.kind]}
          {!update.read && <span className="inbox-unread"> · new</span>}
        </span>
      </MessageMeta>
      {(who !== "" || more !== undefined || (open && report !== undefined)) && (
        <MessageBody id={`${id}-body`}>
          {who !== "" && (
            <Tooltip tip={about}>
              <span
                className={`ask-says${answers.some((one) => one.allows) ? "" : " inbox-clamped"}`}
              >
                {update.says}
              </span>
            </Tooltip>
          )}
          {more !== undefined && <p className="inbox-more">{more}</p>}
          {/* Its report, the task's own words, drawn as text (#5651). */}
          {open && report !== undefined && <p className="inbox-report">{report}</p>}
        </MessageBody>
      )}
      {(report !== undefined || go !== undefined) && (
        <MessageLinks>
          {/* **Read opens it in place** (#5651, ADR 0119 decision 8), and reads Hide while it
                is open; Go to chat still goes to the chat. */}
          {report !== undefined && (
            <Button
              size="sm"
              variant="quiet-link"
              data-lead=""
              aria-label={`${open ? "Hide" : "Read"} ${about}`}
              aria-expanded={open}
              aria-controls={open ? `${id}-body` : undefined}
              onPress={toggle}
            >
              {open ? "Hide" : "Read"}
            </Button>
          )}
          {go !== undefined && (
            <Button
              size="sm"
              variant="quiet-link"
              aria-label={`Go to chat ${who}`}
              onPress={go}
              className="inbox-go"
            >
              Go to chat
            </Button>
          )}
        </MessageLinks>
      )}
      {answers.length > 0 && (
        <MessageForm>
          <ButtonRow
            className="inbox-answers"
            role="group"
            aria-label={`Answers to ${update.says}`}
          >
            {answers.map((answer) => (
              <Button
                key={answer.label}
                // An answer that allows is a grant, drawn as every grant is (ADR 0097 §6).
                variant={answer.allows ? "grants" : "secondary"}
                className={answer.allows ? "inbox-answer allows" : "inbox-answer"}
                aria-label={answer.name}
                tooltip={answer.title}
                onPress={() => press(answer)}
              >
                {answer.label}
              </Button>
            ))}
          </ButtonRow>
        </MessageForm>
      )}
      <NothingDone said={said} />
    </MessageRow>
  );
}
