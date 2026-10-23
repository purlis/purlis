// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";

import type { Offer } from "./actions";
import { commands, type PlaneId, type PlaneUpdated } from "./bindings";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** The files each chat started on that have changed since, by session. */
export type PlaneUpdates = Readonly<Record<number, readonly string[]>>;

/**
 * Which of this project's chats are running on instructions the plane has changed since they
 * started (charter#369).
 *
 * A chat reads `CLAUDE.md`, its harness settings and sub-agents, and its persona's charter
 * once, when it starts. The Python charter said "control plane updated" in the transcript on
 * the next prompt; the ruling on #369 moved it here, onto the chat's tab, so the operator —
 * who is the one who can start it fresh — is the one told.
 *
 * Asked at the mount and whenever the core says a change on disk concerns the instructions
 * (`instructionsChanges`, from `usePlaneChanged(…, INSTRUCTIONS)`, FD-10d), which is when the
 * answer can move. A window that cannot ask marks nothing.
 */
export function usePlaneUpdated(plane: PlaneId, instructionsChanges: number): PlaneUpdates {
  const [updates, setUpdates] = useState<PlaneUpdates>({});
  useEffect(() => {
    let gone = false;
    void planeUpdatedAt(plane, instructionsChanges).then((all) => {
      if (!gone)
        setUpdates(
          Object.fromEntries(
            all.filter((one) => one.files.length > 0).map((one) => [one.session, one.files]),
          ),
        );
    });
    return () => {
      gone = true;
    };
  }, [plane, instructionsChanges]);
  return updates;
}

/** The last ask of `chats_plane_updated`, by project, and the change it was asked at. */
const asked = new Map<PlaneId, { at: number; answer: Promise<readonly PlaneUpdated[]> }>();

/**
 * **What `chats_plane_updated` says of `plane` at its `at`th change**, asked once for the
 * readers that change draws at once: the tab marks here and each chat pane's skills-changed offer
 * (`SkillsChangedNotice`, #2770) read one answer, so a pane adds no ask of its own. Nothing,
 * where it cannot be asked.
 */
export function planeUpdatedAt(plane: PlaneId, at: number): Promise<readonly PlaneUpdated[]> {
  const held = asked.get(plane);
  if (held !== undefined && held.at === at) return held.answer;
  const answer: Promise<readonly PlaneUpdated[]> = commands
    .chatsPlaneUpdated(plane)
    .then((said) => (said.status === "ok" ? (said.data ?? []) : []))
    .catch(() => [])
    .finally(() => {
      // Shared while it is in flight, by the readers one change draws at once; a later reader
      // asks again, so no answer outlives what it was about.
      if (asked.get(plane)?.answer === answer) asked.delete(plane);
    });
  asked.set(plane, { at, answer });
  return answer;
}

/**
 * The mark on a chat's tab when the plane's instructions changed after it started.
 *
 * **Not a needs-you item.** Nothing is waiting on the operator: the chat works, on what it
 * read. So it is a quiet mark beside the pin, not a red count, and it adds nothing to the
 * queue. What it says is what the operator can do about it — start the chat fresh — and which
 * files moved, so they can judge whether that is worth it.
 */
export function PlaneUpdatedMark({ files }: { files?: readonly string[] }) {
  if (!files || files.length === 0) return null;
  return (
    <Tooltip
      tip={`Project updated since this chat started: ${files.join(", ")}. It runs on what it read at its start until it is started fresh.`}
    >
      <span
        className="plane-updated"
        role="img"
        aria-label="project updated since this chat started"
      >
        <RefreshCw />
      </span>
    </Tooltip>
  );
}

/**
 * **The mark on the strip, as the way out it names** (NO-3): the same mark, drawn as a button
 * beside the tab rather than inside it — a button cannot sit in the tab's own — that presses the
 * catalogue's `tab.fresh:<id>`, the row the palette and the tab's menu list. So its accessible
 * name is the row's words, and the question it asks is the row's (`PlaneView`'s `ChatAsk`).
 *
 * **Not a Tab stop**, for `Closer`'s reason (charter-app#189): the strip is one stop, and a
 * keyboard reaches the same row from the palette or the tab's menu. So the tab is **described
 * by** it (`id`, #1246): a screen reader on the tab hears that the plane was updated, which a
 * mark outside the tab would otherwise never tell it.
 */
export function FreshMark({
  id,
  offer,
  files,
  onPress,
}: {
  /** What the tab's `aria-describedby` names. */
  id: string;
  offer?: Offer;
  files?: readonly string[];
  onPress: (offer: Offer) => void;
}) {
  if (!offer || !files || !freshMarkShown(offer, files)) return null;
  return (
    <Tooltip
      tip={`Project updated since this chat started: ${files.join(", ")}. Press to start it fresh on what is there now.`}
    >
      <button
        id={id}
        type="button"
        className="plane-updated fresh-mark"
        tabIndex={-1}
        aria-label={`${offer.title} — project updated since this chat started`}
        onClick={() => onPress(offer)}
      >
        <RefreshCw />
      </button>
    </Tooltip>
  );
}

/** Whether a {@link FreshMark} is drawn: there is a Start fresh row, and files that moved. The
 *  tab asks the same, so it is never described by a mark that is not there. */
export function freshMarkShown(
  offer: Offer | undefined,
  files: readonly string[] | undefined,
): boolean {
  return offer !== undefined && files !== undefined && files.length > 0;
}
