// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useContext, useEffect, useState, useSyncExternalStore } from "react";
import { commands, type ChatHibernation, type HandedBack, type PlaneId } from "./bindings";
import { listen } from "./here";
import { Notice, NoticeOf } from "./Notice";
import { HIBERNATION_CHANGED } from "./chatState";

/** The event the core sends when a chat is hibernated or woken (`chatState.ts` names it). */
export { HIBERNATION_CHANGED };

/**
 * **A chat the person hibernated, on its pane** (SC-4a, #7630, spec #1391): its program ended at
 * a turn boundary and its engine was dropped, and its conversation is the harness's, kept.
 * A plain band, as {@link EndedBand} is, until Hibernated is a chat state of its own (SC-4i,
 * #7638). It offers **Resume**: the chat's restart on its conversation (`onResume`), one press,
 * which begins a new `wake` run. What it showed is drawn read-only under the band where it was
 * kept (SC-4b, #7631; `HibernatedScreen`), and the pane says so where it was not.
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
  return (
    <HibernatedBand
      session={session}
      onResume={onResume}
      unsent={hibernated.unsent ?? undefined}
      notWoken={hibernated.not_woken ?? undefined}
    />
  );
}

/**
 * The band itself, for a chat known to be hibernated: what its story draws. `unsent` is what
 * its pane typed once its program was ending, which no program read (SC-4d, #7633): shown
 * here until the chat wakes, when the woken run reads it (SC-4j, #7639). `notWoken` is the
 * core's word on why typing in it did not wake it.
 */
export function HibernatedBand({
  session,
  onResume,
  unsent,
  notWoken,
}: {
  session: number;
  onResume: () => void;
  unsent?: string;
  notWoken?: string;
}) {
  const offScreen = useContext(NoticeOf) !== null;
  const chat = offScreen ? "That chat" : "This chat";
  const typed = unsent === undefined ? "" : shown(unsent);
  return (
    <Notice
      cause={`chat-hibernated:${session}`}
      at="pane"
      label="Hibernated"
      fixes={[{ label: "Resume", onPress: onResume }]}
    >
      {chat} is hibernated: its program ended and its conversation is kept. Resume starts it again
      on its conversation.
      {typed !== "" && (
        <>
          {" "}
          What you typed since was not sent, and is kept until it wakes: “
          <span className="whitespace-pre-wrap">{typed}</span>”.
        </>
      )}
      {notWoken !== undefined && <> purlis could not wake it: {sentence(notWoken)}</>}
    </Notice>
  );
}

/** The core's reason as the end of a sentence: with its full stop, written once. */
function sentence(why: string): string {
  const said = why.trim();
  return /[.!?]$/.test(said) ? said : `${said}.`;
}

/**
 * What a pane typed, as the window shows it: each control character (an Enter, a Tab) a space,
 * so it reads as the words typed, and nothing drawn as a terminal would draw it.
 */
function shown(typed: string): string {
  // eslint-disable-next-line no-control-regex
  return typed.replace(/[\u0000-\u001f\u007f]/g, " ").trim();
}

/**
 * How chat `session` is hibernated, read when the pane mounts and each time the core says a
 * chat's hibernation moved. What cannot be read shows nothing.
 */
export function useHibernated(plane: PlaneId, session: number): ChatHibernation | null {
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

/** What woken runs never read, handed back by the core, by plane and session (SC-4j, #7639). */
const handedBack = new Map<string, HandedBack>();

/**
 * **What chat `session`'s pane typed that its woken run never read** (SC-4j, #7639): its run
 * ended before it was ready, or typing it in was refused. The core hands it over once, read when
 * the pane mounts and each time it says a chat's hibernation moved; the pane keeps it, unsent,
 * until it is put away.
 */
export function UnsentNotice({ plane, session }: { plane: PlaneId; session: number }) {
  const key = keyOf(plane, session);
  useEffect(() => {
    const take = () => {
      void commands
        .chatUnsentHandedBack(plane, session)
        .then((answer) => {
          if (answer.status !== "ok" || answer.data === null) return;
          handedBack.set(key, answer.data);
          for (const tell of listeners) tell();
        })
        .catch(() => {});
    };
    take();
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<{ plane: PlaneId; session: number }>(
          HIBERNATION_CHANGED,
          (event) => {
            if (event.payload.plane === plane && event.payload.session === session) take();
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
  }, [plane, session, key]);
  const back = useSyncExternalStore(
    subscribe,
    () => handedBack.get(key),
    () => undefined,
  );
  const offScreen = useContext(NoticeOf) !== null;
  if (back === undefined || shown(back.text) === "") return null;
  const chat = offScreen ? "That chat" : "This chat";
  return (
    <Notice
      cause={`chat-unsent:${session}`}
      at="pane"
      label="Not sent"
      onDismiss={() => {
        handedBack.delete(key);
        for (const tell of listeners) tell();
      }}
    >
      {chat} was woken, but what you typed while it was hibernated was not sent: “
      <span className="whitespace-pre-wrap">{shown(back.text)}</span>”. {sentence(back.why)} Type it
      again if you still want it.
    </Notice>
  );
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
