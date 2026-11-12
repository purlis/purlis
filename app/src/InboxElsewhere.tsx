// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId } from "react";
import type { Offer } from "./actions";
import type { Shown } from "./bindings";
import { backSaid } from "./chatState";
import { KindIcon } from "./InboxRow";
import { Notice } from "./Notice";
import type { Needing, Quiet } from "./NeedsYou";
import { PersonaMark } from "./PersonaMark";
import { X } from "lucide-react";
import { Button, IconButton } from "./ui/button/Button";
import {
  MessageBody,
  MessageLinks,
  MessageMeta,
  MessageRow,
  MessageTop,
} from "./ui/message/Message";
import { ListRowSection } from "./ui/list/List";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **What waits on the person where this project's asks registry cannot see** (#1695): what the
 * ✋'s list named until it was retired, and the Inbox lists now, so retiring the list lost
 * nothing.
 *
 * - **A chat waiting in another window's project.** That window's registry reads its asks; this
 *   window hears of the chat from what that window tells the others (`windows.ts`), and the
 *   hand's number counts it. So it is an ask here too, listed after this project's own, one
 *   group per chat with **Go to chat** (that window comes forward with the chat in front) and,
 *   where its queue offers it, **Ignore**. It is answered there, where what it asks is shown.
 * - **A chat that cannot tell purlis it is waiting** (charter-app#52): a shell, or a harness
 *   without purlis's hooks, in any project of any window. Nothing has asked, so it is no ask:
 *   it is a Notice, with Go to chat, as the faint hand names it. The status line's count leaves
 *   it out (`FROM_THE_HAND`).
 */
export type Elsewhere = {
  /** The chats waiting in the projects of the window's other windows. */
  asking: readonly Needing[];
  /** Every chat that can be waiting without saying so, in every project of every window. */
  quiet: readonly Quiet[];
  /** Carries a row out in the project it is about, in whichever window holds it. */
  onPress: (plane: string, offer: Offer) => void;
  /**
   * **The projects of this window with something waiting** (#1700, 2026-10-10 21:09 item 5),
   * each with how many asks its registry holds. Each Inbox names the others, one line each:
   * the Inbox is one project's, and the ✋ opens only one of them.
   */
  waitingHere?: readonly WaitingHere[];
  /**
   * **The projects of the other windows, ask by ask** (#1921), as each window tells the others
   * its registry's asks (`windows.ts`). A project of theirs listed here is drawn ask by ask, and
   * its chats in {@link Elsewhere.asking} are not drawn again.
   */
  there?: readonly WaitingHere[];
  /** Go to project: that project in front, with its Inbox open. */
  onOpen?: (plane: string) => void;
};

/** A project with something waiting, and how many asks. */
export type WaitingHere = {
  plane: string;
  project: string;
  count: number;
  /**
   * **What waits there, ask by ask** (qw359, #1921): each ask whole, as that project's registry
   * derived it, so Asks lists it as one of its own rows and answers it in place, in that project
   * (ADR 0121 decision 4). Absent until its registry has been read: then it is one line, with Go
   * to project.
   */
  asks?: readonly Shown[];
  /** Where each of its chats is filed, by session: its workspace and persona, as its row says
   *  them beside the project's name. */
  where?: Readonly<Record<string, string>>;
};

/** Another project's asks, as Asks lists them: its project, and whether this window holds it. */
export type TheirAsks = WaitingHere & { asks: readonly Shown[]; here: boolean };

/** What the lines for this window's other projects are called. */
export const OTHER_PROJECTS = "Other projects in this window";

/**
 * **Every other open project's asks, ask by ask** (#1921): this window's other projects and the
 * other windows', each as its registry derived them. Asks lists them among its own, in one order
 * by facts, and answers each in its project.
 */
export function theirAsks(plane: string, elsewhere: Elsewhere | undefined): TheirAsks[] {
  const mine = new Set((elsewhere?.waitingHere ?? []).map((one) => one.plane));
  const told = (here: boolean) => (one: WaitingHere) =>
    one.plane !== plane && one.asks !== undefined && one.asks.length > 0
      ? [{ ...one, asks: one.asks, here }]
      : [];
  return [
    ...(elsewhere?.waitingHere ?? []).flatMap(told(true)),
    ...(elsewhere?.there ?? []).filter((one) => !mine.has(one.plane)).flatMap(told(false)),
  ];
}

/** The chats waiting in other windows' projects whose asks those windows have not told: drawn
 *  a chat at a time, as before (#1695). */
export const askingUntold = (elsewhere: Elsewhere | undefined): readonly Needing[] => {
  const told = new Set(
    (elsewhere?.there ?? []).filter((one) => one.asks !== undefined).map((one) => one.plane),
  );
  return (elsewhere?.asking ?? []).filter((one) => !told.has(one.plane));
};

/**
 * **The ✋'s count** (ADR 0121 decision 4, #5999): every open project's Blocking asks, in this
 * window and the others, never narrowed by Asks' scope. `elsewhere` is how many of them are
 * outside `plane`, the project in front: its own asks registry's for this window's other
 * projects, each other window's for its projects, as it tells them (#1921), and one for each
 * chat another window says waits where it has not told its asks. A snoozed chat's asks are in
 * none (`Asking.snoozed`).
 */
export function handCount(
  plane: string,
  here: number,
  elsewhere: Elsewhere | undefined,
): { total: number; elsewhere: number } {
  const there = elsewhereCount(plane, elsewhere);
  return { total: here + there, elsewhere: there };
}

/** How many asks wait in the open projects other than `plane`, as {@link handCount} counts. */
export function elsewhereCount(plane: string, elsewhere: Elsewhere | undefined): number {
  const mine = new Set((elsewhere?.waitingHere ?? []).map((one) => one.plane));
  const inHere = (elsewhere?.waitingHere ?? [])
    .filter((one) => one.plane !== plane)
    .reduce((sum, one) => sum + one.count, 0);
  const inThere = (elsewhere?.there ?? [])
    .filter((one) => !mine.has(one.plane) && one.plane !== plane)
    .reduce((sum, one) => sum + one.count, 0);
  return inHere + inThere + askingUntold(elsewhere).length;
}

/** What the ✋ says it counts, and how many of them wait outside the project in front. */
export function handSaid({ total, elsewhere }: { total: number; elsewhere: number }): string {
  const waits = total === 1 ? "1 thing waits on you" : `${total} things wait on you`;
  return elsewhere > 0 ? `${waits}, +${elsewhere} elsewhere` : waits;
}

/** The row that brings chat `session` of `plane` to the front: the queue's own Go, where it is
 *  offered, else the same verb. */
export function showOffer(session: number, name: string, go?: Offer): Offer {
  if (go?.available) return go;
  return {
    id: `needs.show:${session}`,
    title: `Show ${name}`,
    available: true,
    reason: "",
    does: { verb: "showChat", session },
    name,
  };
}

/** The key of a chat elsewhere, its row's. */
export const elsewhereKey = (item: { plane: string; session: number }) =>
  `elsewhere:${item.plane}#${item.session}`;

/** What the groups for chats elsewhere draw, in their order: each row, its Go to chat and its
 *  Ignore. */
export function elsewherePlaces(asking: readonly Needing[]): string[] {
  return asking.flatMap((item) => {
    const key = elsewhereKey(item);
    return [key, `${key}:go`, ...(item.ignore?.available ? [`${key}:ignore`] : [])];
  });
}

/** What a group for a chat elsewhere says it is. */
export const IN_ANOTHER_WINDOW = "In another window";

/** One chat waiting in another window's project, as an Inbox group. */
export function ElsewhereGroup({
  item,
  onPress,
}: {
  item: Needing;
  onPress: (plane: string, offer: Offer) => void;
}) {
  const id = useId();
  const key = elsewhereKey(item);
  // Every word here is what that window said: a chat's name, its reason, drawn as text.
  const says =
    item.needed ?? backSaid(item.reported ?? [], item.stoppedBelow) ?? item.why ?? "Waiting on you";
  const where = `${item.workspace} · ${item.project}`;
  return (
    <ListRowSection
      id={`group:${key}`}
      className="inbox-chat inbox-elsewhere"
      title={
        <span className="inbox-chain">
          {item.persona != null && <PersonaMark persona={item.persona} mark={item.mark} />}
          {item.name}
        </span>
      }
      named
      // Not `data-session`: a notification lands on this project's own group by that
      // (`landOnGroup`), and a chat elsewhere can have the same number in its project.
      data-elsewhere={`${item.plane}#${item.session}`}
    >
      <MessageRow
        as="row"
        id={key}
        textValue={`${item.name}: ${says}`}
        tone="waiting"
        icon={<KindIcon kind="elsewhere" />}
        className="inbox-ask"
        data-source="elsewhere"
        aria-labelledby={`${id}-says`}
        aria-describedby={`${id}-whose ${id}-where`}
        dismiss={
          item.ignore?.available ? (
            <IconButton
              icon={<X />}
              size="sm"
              variant="quiet"
              aria-label={item.ignore.title}
              tooltip={item.ignore.title}
              onPress={() => item.ignore && onPress(item.plane, item.ignore)}
            />
          ) : undefined
        }
      >
        <MessageTop title={item.name} />
        <MessageMeta>
          <span id={`${id}-whose`} className="inbox-source">
            {IN_ANOTHER_WINDOW}
          </span>
          {" · "}
          <span id={`${id}-where`} className="inbox-where">
            {where}
          </span>
        </MessageMeta>
        <MessageBody>
          <Tooltip tip={says}>
            <span id={`${id}-says`} className="ask-says inbox-clamped">
              {says}
            </span>
          </Tooltip>
        </MessageBody>
        <MessageLinks>
          <Button
            size="sm"
            variant="quiet-link"
            aria-label={`Go to chat ${item.name} · ${where}, in another window`}
            onPress={() => onPress(item.plane, showOffer(item.session, item.name, item.go))}
            className="inbox-go"
          >
            Go to chat
          </Button>
        </MessageLinks>
      </MessageRow>
    </ListRowSection>
  );
}

/** Every chat that cannot tell purlis it is waiting, each a Notice with Go to chat. */
export function QuietNotices({
  plane,
  quiet,
  onPress,
}: {
  plane: string;
  quiet: readonly Quiet[];
  onPress: (plane: string, offer: Offer) => void;
}) {
  return (
    <>
      {quiet.map((one) => (
        <Notice
          key={`${one.plane}#${one.session}`}
          cause={`chat-quiet:${one.plane}#${one.session}`}
          link={{
            label: "Go to chat",
            onPress: () => onPress(one.plane, showOffer(one.session, one.name)),
          }}
        >
          <strong>{one.name}</strong>
          {one.plane === plane ? "" : ` in ${one.project}`} can&apos;t tell purlis it&apos;s
          waiting.
        </Notice>
      ))}
    </>
  );
}
