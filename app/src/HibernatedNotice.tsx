// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useContext, useEffect, useState, useSyncExternalStore } from "react";
import { commands, type ChatHibernation, type PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice, NoticeOf } from "./Notice";

/** The event the core sends when a chat is hibernated or woken. */
export const HIBERNATION_CHANGED = "chat-hibernation-changed";

/**
 * **A chat the person hibernated, on its pane** (SC-4a, #7630, spec #1391): its program ended at
 * a turn boundary and its engine was dropped, and its conversation is the harness's, kept.
 * A plain band, as {@link EndedBand} is, until Hibernated is a chat state of its own (SC-4i,
 * #7638). It offers **Resume**: the chat's restart on its conversation (`onResume`), one press,
 * which begins a new `wake` run. No screen is kept yet (SC-4b, #7631), and the band says so.
 */
export function HibernatedNotice({
  plane,
  session,
  onResume,
}: {
  plane: PlaneId;
  session: number;
  /** Asks for this chat's restart on its conversation: the pane's Restart chat. */
  onResume: () => void;
}) {
  const hibernated = useHibernated(plane, session);
  if (hibernated === null) return null;
  return <HibernatedBand session={session} onResume={onResume} />;
}

/** The band itself, for a chat known to be hibernated: what its story draws. */
export function HibernatedBand({ session, onResume }: { session: number; onResume: () => void }) {
  const offScreen = useContext(NoticeOf) !== null;
  const chat = offScreen ? "That chat" : "This chat";
  return (
    <Notice
      cause={`chat-hibernated:${session}`}
      at="pane"
      label="Hibernated"
      fixes={[{ label: "Resume", onPress: onResume }]}
    >
      {chat} is hibernated: its program ended and its conversation is kept. purlis kept no copy of
      its screen. Resume starts it again on its conversation.
    </Notice>
  );
}

/**
 * How chat `session` is hibernated, read when the pane mounts and each time the core says a
 * chat's hibernation moved. What cannot be read shows nothing.
 */
function useHibernated(plane: PlaneId, session: number): ChatHibernation | null {
  const [hibernated, setHibernated] = useState<ChatHibernation | null>(null);
  const read = useCallback(() => {
    void commands
      .chatHibernated(plane, session)
      .then((answer) => {
        if (answer.status === "ok") setHibernated(answer.data);
      })
      .catch(() => {});
  }, [plane, session]);
  useEffect(() => {
    read();
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<{ plane: PlaneId; session: number }>(
          HIBERNATION_CHANGED,
          (event) => {
            if (event.payload.plane !== plane) return;
            read();
          },
        );
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
  return hibernated;
}

/** Why a chat was not hibernated, by plane and session: what the core said when it refused. */
const refusals = new Map<string, string>();
const listeners = new Set<() => void>();

const keyOf = (plane: PlaneId, session: number) => `${plane}:${session}`;

/** The core refused to hibernate chat `session`, saying `why`: said on its pane until put away. */
export function hibernateRefused(plane: PlaneId, session: number, why: string) {
  refusals.set(keyOf(plane, session), why);
  for (const tell of listeners) tell();
}

function subscribe(tell: () => void) {
  listeners.add(tell);
  return () => {
    listeners.delete(tell);
  };
}

/**
 * **Why a chat was not hibernated, on its pane** (SC-4a, #7630): the core's sentence, which says
 * what happened and what to do. Put away with Dismiss.
 */
export function HibernateRefusedNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const key = keyOf(plane, session);
  const why = useSyncExternalStore(
    subscribe,
    () => refusals.get(key),
    () => undefined,
  );
  if (why === undefined) return null;
  return (
    <Notice
      cause={`chat-not-hibernated:${session}`}
      at="pane"
      tone="trouble"
      label="Not hibernated"
      onDismiss={() => {
        refusals.delete(key);
        for (const tell of listeners) tell();
      }}
    >
      {why}
    </Notice>
  );
}
