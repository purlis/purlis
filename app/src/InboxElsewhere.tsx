// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId } from "react";
import * as RovingFocusGroup from "@radix-ui/react-roving-focus";
import type { Offer } from "./actions";
import { backSaid } from "./chatState";
import { Notice } from "./Notice";
import type { Needing, Quiet } from "./NeedsYou";
import { PersonaMark } from "./PersonaMark";

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
  /** Go to project: that project in front, with its Inbox open. */
  onOpen?: (plane: string) => void;
};

/** A project of this window with something waiting, and how many asks. */
export type WaitingHere = { plane: string; project: string; count: number };

/** What the lines for this window's other projects are called. */
export const OTHER_PROJECTS = "Other projects in this window";

/** The key of a line for another project of this window, for its stops. */
export const hereKey = (plane: string) => `here:${plane}`;

/** The projects other than `plane` with something waiting, as their lines draw them. */
export const othersWaiting = (plane: string, elsewhere: Elsewhere | undefined) =>
  (elsewhere?.waitingHere ?? []).filter((one) => one.plane !== plane && one.count > 0);

/** The stops of those lines, in the order they are drawn. */
export const hereStops = (others: readonly WaitingHere[]) =>
  others.flatMap((one) => [hereKey(one.plane), `${hereKey(one.plane)}:go`]);

/**
 * **One line for each other project of this window that has something waiting** (#1700): its
 * name, how many asks wait there, and Go to project. It answers nothing: what waits is read and
 * answered in that project's own Inbox, where it is drawn whole.
 */
export function OtherProjects({
  others,
  onOpen,
}: {
  others: readonly WaitingHere[];
  onOpen: (plane: string) => void;
}) {
  return (
    <section className="inbox-chat inbox-elsewhere" aria-label={OTHER_PROJECTS}>
      <h3 className="inbox-chain">{OTHER_PROJECTS}</h3>
      <ul>
        {others.map((one) => (
          <RovingFocusGroup.Item asChild tabStopId={hereKey(one.plane)} key={one.plane}>
            <li className="inbox-ask" data-source="here">
              <p className="inbox-says">
                {/* A project's name is data: drawn as text. */}
                <span className="ask-says">{`${one.project} · ${one.count} waiting on you`}</span>
              </p>
              <RovingFocusGroup.Item asChild tabStopId={`${hereKey(one.plane)}:go`}>
                <button
                  type="button"
                  className="inbox-go"
                  aria-label={`Go to project ${one.project}`}
                  onClick={() => onOpen(one.plane)}
                >
                  Go to project
                </button>
              </RovingFocusGroup.Item>
            </li>
          </RovingFocusGroup.Item>
        ))}
      </ul>
    </section>
  );
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

/** The key of a chat elsewhere, for its stops. */
export const elsewhereKey = (item: { plane: string; session: number }) =>
  `elsewhere:${item.plane}#${item.session}`;

/** The stops of the groups for chats elsewhere, in the order they are drawn. */
export function elsewhereStops(asking: readonly Needing[]): string[] {
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
    <section
      className="inbox-chat inbox-elsewhere"
      aria-label={`${item.name} · ${where}`}
      // Not `data-session`: a notification lands on this project's own group by that
      // (`landOnGroup`), and a chat elsewhere can have the same number in its project.
      data-elsewhere={`${item.plane}#${item.session}`}
    >
      <h3 className="inbox-chain">
        {item.persona != null && <PersonaMark persona={item.persona} mark={item.mark} />}
        {item.name}
      </h3>
      <ul>
        <RovingFocusGroup.Item asChild tabStopId={key}>
          <li
            className="inbox-ask"
            data-source="elsewhere"
            aria-labelledby={`${id}-says`}
            aria-describedby={`${id}-whose`}
          >
            <p className="inbox-says">
              <span id={`${id}-whose`} className="inbox-source">
                {IN_ANOTHER_WINDOW}
              </span>
              <span id={`${id}-says`} className="ask-says">
                {says}
              </span>
              <span className="inbox-where">{where}</span>
            </p>
            <RovingFocusGroup.Item asChild tabStopId={`${key}:go`}>
              <button
                type="button"
                className="inbox-go"
                aria-label={`Go to chat ${item.name} · ${where}, in another window`}
                onClick={() => onPress(item.plane, showOffer(item.session, item.name, item.go))}
              >
                Go to chat
              </button>
            </RovingFocusGroup.Item>
            {item.ignore?.available && (
              <RovingFocusGroup.Item asChild tabStopId={`${key}:ignore`}>
                <button
                  type="button"
                  className="inbox-go"
                  aria-label={item.ignore.title}
                  title={item.ignore.title}
                  onClick={() => item.ignore && onPress(item.plane, item.ignore)}
                >
                  Ignore
                </button>
              </RovingFocusGroup.Item>
            )}
          </li>
        </RovingFocusGroup.Item>
      </ul>
    </section>
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
