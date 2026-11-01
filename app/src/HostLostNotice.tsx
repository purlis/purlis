// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useContext, useEffect, useState } from "react";
import { commands, type LostChat, type PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice, NoticeOf } from "./Notice";

/** The event the core sends when a chat's program ended with its session host. */
export const LOST = "chat-lost-with-the-host";

/**
 * **A chat whose program ended with its session host, on its pane** (#2007, ADR 0068 §3).
 *
 * The session host the chat ran in ended unexpectedly, and the chat's program ended with it.
 * The core keeps how that left the chat's run (`chat-lost-with-the-host`), and this says it:
 * **failed**, in the middle of a turn that is lost, or **hibernated**, idle at a turn's end
 * with nothing lost. A chat whose host was a pool's runner, gone for good, says that instead:
 * its conversation went with the runner's home, so nothing can be resumed (#4350). It offers
 *
 * - **Resume**: the chat's restart on its conversation (`onRestart`), one click, where it has a
 *   conversation to resume;
 * - **Dismiss**: puts it away.
 */
export function HostLostNotice({
  plane,
  session,
  onRestart,
}: {
  plane: PlaneId;
  session: number;
  /** Asks for this chat's restart on its conversation: the pane's Restart chat. */
  onRestart: () => void;
}) {
  const { lost, read } = useLost(plane, session);
  const offScreen = useContext(NoticeOf) !== null;
  if (lost === null) return null;
  const chat = offScreen ? "that chat" : "this chat";

  const putAway = () => {
    void commands
      .putAwayLostChat(plane, session)
      .catch(() => {})
      .finally(read);
  };

  const said = lost.with_the_runner
    ? `${offScreen ? "That chat" : "This chat"} ended with its pool's runner, which this machine no longer keeps, and its conversation was on that runner.`
    : lost.left_in === "hibernated"
      ? `purlis's session host stopped unexpectedly while ${chat} was waiting for you, so nothing was lost.`
      : `purlis's session host stopped unexpectedly in the middle of ${chat}'s turn, so that turn was lost.`;
  const next = lost.resumable
    ? " Resume starts it again on its conversation."
    : lost.with_the_runner
      ? " Start a new chat to go on."
      : " It named no conversation to resume, so start a new chat to go on.";

  return (
    <Notice
      cause={`chat-lost-with-the-host:${session}`}
      at="pane"
      tone="trouble"
      label={lost.with_the_runner ? "Ended with its runner" : "Session host stopped"}
      fixes={
        lost.resumable
          ? [
              {
                label: "Resume",
                onPress: () => {
                  onRestart();
                  putAway();
                },
              },
            ]
          : undefined
      }
      onDismiss={putAway}
    >
      {said}
      {next}
    </Notice>
  );
}

/**
 * Whether chat `session`'s program ended with its session host, read when the pane mounts and
 * each time the core says one did, and `read` to read it again after a press. What cannot be
 * read shows nothing.
 */
function useLost(plane: PlaneId, session: number): { lost: LostChat | null; read: () => void } {
  const [lost, setLost] = useState<LostChat | null>(null);
  const read = useCallback(() => {
    void commands
      .chatLostWithTheHost(plane, session)
      .then((answer) => {
        if (answer.status === "ok")
          setLost(
            answer.data?.left_in === "failed" || answer.data?.left_in === "hibernated"
              ? answer.data
              : null,
          );
      })
      .catch(() => {});
  }, [plane, session]);
  useEffect(() => {
    read();
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<LostChat>(LOST, (event) => {
          if (event.payload.plane !== plane || event.payload.session !== session) return;
          read();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, session, read]);
  return { lost, read };
}
