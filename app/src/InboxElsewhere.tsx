// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Offer } from "./actions";
import type { Shown } from "./bindings";
import { Notice } from "./Notice";
import type { Needing, Quiet } from "./NeedsYou";

/**
 * **What waits on the person where this project's asks registry cannot see** (#1695): what the
 * ✋'s list named until it was retired, and the Inbox lists now, so retiring the list lost
 * nothing.
 *
 * - **A chat waiting in another window's project.** That window's registry reads its asks; this
 *   window hears of the chat from what that window tells the others (`windows.ts`), and the
 *   Dock badge counts it. Asks never lists it (ADR 0121, amended 2026-11-12): it is answered in
 *   that project's own Asks, which its notification brings forward.
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
};

/** A project with something waiting, and how many asks. */
export type WaitingHere = {
  plane: string;
  project: string;
  count: number;
  /**
   * **What waits there, ask by ask** (qw359, #1921): each ask whole, as that project's registry
   * derived it. Absent until its registry has been read. Asks lists none of it: another
   * project's asks are answered in its own Asks (ADR 0121, amended 2026-11-12).
   */
  asks?: readonly Shown[];
  /** Where each of its chats is filed, by session: its workspace and persona, as its row says
   *  them beside the project's name. */
  where?: Readonly<Record<string, string>>;
};

/** The chats waiting in other windows' projects whose asks those windows have not told: drawn
 *  a chat at a time, as before (#1695). */
export const askingUntold = (elsewhere: Elsewhere | undefined): readonly Needing[] => {
  const told = new Set(
    (elsewhere?.there ?? []).filter((one) => one.asks !== undefined).map((one) => one.plane),
  );
  return (elsewhere?.asking ?? []).filter((one) => !told.has(one.plane));
};

/**
 * **How many asks wait in the open projects other than `plane`** (#5999, #1921): its own asks
 * registry's for this window's other projects, each other window's for its projects, as it tells
 * them, and one for each chat another window says waits where it has not told its asks. A
 * snoozed chat's asks are in none (`Asking.snoozed`). The ✋ never counts them (#6341, ADR 0121
 * amended 2026-11-12, decision 1); a new one among them is still said as it arrives.
 */
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

/** What the ✋ says it counts: the project in front's asks (#6341). */
export const handSaid = (count: number): string =>
  count === 1 ? "1 thing waits on you" : `${count} things wait on you`;

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
