// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useLayoutEffect, useRef } from "react";
import type { Shown } from "./bindings";
import { movedAt, useChatsHere, useChatsSelect, waitsOnItsPrompt } from "./chatState";
import { hideForNow, useHiddenForNow } from "./dismissals";
import { Notice, NoticeOf } from "./Notice";

/**
 * **A chat that is not on screen is stopped on a prompt** (reported 2026-10-09): its harness
 * asked the person's permission for a tool call, or asked them something, in a terminal nobody
 * is looking at, and everything it does waits on that answer. Said on the pane of the tab it
 * lives in — for a task, its session's tab — naming whose it is by its whole path (`NoticeOf`),
 * with **Show the task**, which switches the tab to it.
 *
 * - **Only for a chat that is not on screen.** On screen, the prompt is in its own pane, and a
 *   second line about it would say it twice. So nothing is drawn where no `NoticeOf` is given.
 * - **From two sources, one Notice.** While purlis holds a permission prompt on the chat's hook
 *   (about a minute, HP-6), the Notice says what it asks: the ask's line as the core summed it
 *   up (`asking::shown`: one line, every credential shape masked), drawn as text and nothing
 *   else, and "and N more" past the first. Past that hold, the harness's own prompt decides in
 *   the chat's pane, and the board still says the chat is stopped on it (`Moved.asking`,
 *   #1601): the Notice stays, without the prompt's words, which only the hold carried.
 * - **It answers nothing.** The person answers in the chat's own pane, where the harness shows
 *   the prompt in full, or from the title bar's needs-you list: purlis never approves a
 *   harness's prompt in the person's place.
 * - **It goes when the prompt does**: answered in the window, got past in the chat's pane (a
 *   tool of its own began after the prompt and one came back), the turn that asked ended, its
 *   next prompt, or the chat ended.
 * - **Dismiss puts it away for this wait** (qw386): hidden in memory for this run, never
 *   answered, so the needs-you list and the tab's hand still say the chat waits. Each line is
 *   keyed by what it is about: a held prompt by its ask, and the line of a chat stopped on its
 *   harness's own prompt by the moment the core says the chat last moved, so a new prompt or
 *   a new wait is a new line and shows, whether or not this pane was drawn while the old one
 *   ended. A prompt put away while purlis held it keeps the stopped line of the same wait away
 *   once the hold ends, where this pane saw it end; where it did not, the line shows.
 */
/** What Dismiss does here, as its tooltip: it hides, and answers nothing. */
const DISMISS_SAYS =
  "Hides this line. The chat still waits on you, and the needs-you list still shows it.";

export function TaskPromptNotice({
  plane,
  session,
  asks,
  registry,
}: {
  /** The project the chat is in: what a Dismiss is kept under. */
  plane: string;
  session: number;
  /** The permission prompts this project's chats hold open on their hooks (HP-6), as the
   *  window last heard them (`permissionAsks.usePermissionAsks`). */
  asks: readonly Shown[];
  /**
   * **This chat's permission and terminal asks its pane draws** (#1695), from the registry,
   * once it was read: the Notice is drawn from them alone, so an answer in the Inbox clears it.
   * Before the first read, from the hook's asks and the board, as it always was.
   */
  registry?: readonly Shown[];
}) {
  const of = useContext(NoticeOf);
  const chats = useChatsHere();
  const waitsOnIt = useChatsSelect(chats, (states) => waitsOnItsPrompt(states, session));
  const moved = useChatsSelect(chats, (states) => movedAt(states, session));
  const stopped =
    registry === undefined
      ? waitsOnIt
      : registry.some((one) => one.session === session && one.source === "terminal");
  const held = (registry ?? asks).filter(
    (one) => one.session === session && one.source === "permission",
  );
  const first = held[0];
  const waiting = first !== undefined || stopped;
  const each = `task-permission:${session}:`;
  const asked = `${each}asked:${moved}`;
  const cause = first === undefined ? asked : `${each}${first.ask}`;
  /** Where a held prompt was put away: the moment the chat had last moved then. The stopped
   *  line of the first move after it is the same wait's. Kept with this pane alone, so a pane
   *  drawn again later never takes a later wait for it. */
  const carried = useRef<number | null>(null);
  useLayoutEffect(() => {
    if (carried.current === null) return;
    if (!waiting) carried.current = null;
    else if (first === undefined && moved !== carried.current) {
      hideForNow(plane, asked);
      carried.current = null;
    }
  }, [waiting, first, moved, plane, asked]);
  const hidden = useHiddenForNow(plane, cause);
  if (of === null || !waiting || hidden) return null;
  // Put away for this wait: its prompt, and the line that stands for the wait once purlis's
  // hold of the prompt ends.
  const onDismiss = () => {
    hideForNow(plane, cause);
    hideForNow(plane, asked);
    if (first !== undefined) carried.current = moved;
  };
  // Which chat it is comes from the core's lineage (`NoticeOf.task`), never from its name.
  const fixes = [
    { label: of.task === true ? "Show the task" : "Show it", onPress: of.onGo },
  ] as const;
  return (
    // The Notice says whose it is in its own sentence and has its own way to the chat, so the
    // shared "whose:" and "Go to it" are not drawn a second time.
    <NoticeOf.Provider value={null}>
      {first === undefined ? (
        <Notice
          cause={cause}
          at="pane"
          tone="trouble"
          label={`${of.whose} is waiting on you`}
          fixes={fixes}
          onDismiss={onDismiss}
          dismissSays={DISMISS_SAYS}
        >
          {of.whose} is stopped on a prompt and waiting on you. Answer it in its own pane.
        </Notice>
      ) : (
        <Notice
          cause={cause}
          at="pane"
          tone="trouble"
          label={`${of.whose} is waiting on you for a permission`}
          fixes={fixes}
          onDismiss={onDismiss}
          dismissSays={DISMISS_SAYS}
        >
          {of.whose} is waiting on you for a permission: “
          <span className="ask-says">{first.says}</span>”
          {held.length > 1 && ` and ${held.length - 1} more`}. Answer it in its own pane.
        </Notice>
      )}
    </NoticeOf.Provider>
  );
}
