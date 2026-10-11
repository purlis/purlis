// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act } from "@testing-library/react";
import type { PlaneId } from "./bindings";
import { nothingKnown, type Chats, type ChatStates } from "./chatState";

/**
 * A project's chats for a test, and a way to start and close one: what `ChatsHere` gives a
 * component in the window, told by hand instead of by `chat-moved`.
 */
export function startingChats(plane: PlaneId): {
  chats: Chats;
  start: (session: number) => void;
  close: (session: number) => void;
} {
  let states: ChatStates = nothingKnown;
  const listeners = new Set<() => void>();
  const tell = (now: ChatStates) => {
    states = now;
    act(() => {
      for (const listener of [...listeners]) listener();
    });
  };
  return {
    chats: {
      store: {
        subscribe: (listener) => {
          listeners.add(listener);
          return () => void listeners.delete(listener);
        },
        statesFor: (asked) => (asked === plane ? states : nothingKnown),
      },
      plane,
    },
    start: (session) =>
      tell({ ...states, bySession: { ...states.bySession, [session]: "running" } }),
    close: (session) =>
      tell({
        ...states,
        bySession: Object.fromEntries(
          Object.entries(states.bySession).filter(([one]) => Number(one) !== session),
        ),
      }),
  };
}
