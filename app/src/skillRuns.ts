// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Skill runs** (SK-3, #2766): the run open in each chat, for the chip on its tab, which opens
 * the skill's Runs in the Skill studio (#3682).
 *
 * The core opens a run when a chat uses a skill, counts the tool calls made inside it and ends
 * it with the turn, and sends `chat-skill-run` each time a chat's open run changes. This holds
 * the latest per chat, outside React, as what the chats are doing is held (`chatDoing.ts`): a
 * chip subscribes to its own chat and is redrawn only when that chat's run changes. In memory
 * only.
 */
import { createContext, useContext, useEffect, useRef, useState } from "react";
import { useSyncExternalStoreWithSelector } from "use-sync-external-store/with-selector";
import { listen } from "./here";
import { commands, type ChatSkillRun, type OpenRun, type PlaneId } from "./bindings";

/** The open run of each chat, by session. A session that is not here has none open. */
export type OpenRuns = Readonly<Record<number, OpenRun>>;

const NONE: OpenRuns = {};

/** Applies one telling. */
export function toldRun(runs: OpenRuns, one: ChatSkillRun): OpenRuns {
  const was = runs[one.session];
  const now = one.run ?? undefined;
  if (sameRun(was, now)) return runs;
  if (now !== undefined) return { ...runs, [one.session]: now };
  return Object.fromEntries(
    Object.entries(runs).filter(([session]) => Number(session) !== one.session),
  );
}

/** Whether two open runs show the same. */
export function sameRun(one: OpenRun | undefined, other: OpenRun | undefined): boolean {
  return (
    one === other ||
    (one !== undefined &&
      other !== undefined &&
      one.id === other.id &&
      one.skill === other.skill &&
      one.name === other.name &&
      one.tool_calls === other.tool_calls)
  );
}

type Known = { readonly plane?: PlaneId; readonly runs: OpenRuns };

/** The open runs of a project's chats, held outside React. */
export type RunStore = {
  subscribe: (listener: () => void) => () => void;
  /** What is known about `plane`, and nothing at all about any other. */
  runsFor: (plane: PlaneId | undefined) => OpenRuns;
};

/** A store and the project its readers ask about. */
export type RunsOf = { readonly store: RunStore; readonly plane: PlaneId | undefined };

type HeldStore = RunStore & { change: (how: (was: Known) => Known) => void };

function runStore(): HeldStore {
  let known: Known = { runs: NONE };
  const listeners = new Set<() => void>();
  return {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
    runsFor: (plane) => (known.plane === plane ? known.runs : NONE),
    change: (how) => {
      const now = how(known);
      if (now === known) return;
      known = now;
      for (const listener of [...listeners]) listener();
    },
  };
}

/** Open runs that are `runs` and never change: for a component drawn on its own. */
export function fixedRuns(runs: OpenRuns = NONE): RunsOf {
  return { store: { subscribe: () => () => {}, runsFor: () => runs }, plane: undefined };
}

/** The open runs of the project a tab is drawn in. `PlaneView` provides it. */
export const SkillRunsHere = createContext<RunsOf>(fixedRuns());

/** The run open in chat `session`, redrawn only when it changes. */
export function useOpenRunOf(session: number | undefined): OpenRun | undefined {
  const { store, plane } = useContext(SkillRunsHere);
  const runs = () => store.runsFor(plane);
  return useSyncExternalStoreWithSelector(
    store.subscribe,
    runs,
    runs,
    (all) => (session === undefined ? undefined : all[session]),
    sameRun,
  );
}

/**
 * Keeps the open runs of ONE project's chats, starting from what the core holds. As
 * `useDoings`: listening starts at the mount, every telling is checked against the project it
 * came from, and the first answer is folded under whatever has arrived meanwhile.
 */
export function useSkillRuns(plane: PlaneId | undefined): RunsOf {
  const [store] = useState(runStore);
  const showing = useRef(plane);
  useEffect(() => {
    showing.current = plane;
  }, [plane]);

  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatSkillRun>("chat-skill-run", (event) => {
          const one = event.payload;
          if (gone || one.plane !== showing.current) return;
          store.change((was) => ({
            plane: one.plane,
            runs: toldRun(was.plane === one.plane ? was.runs : NONE, one),
          }));
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
  }, [store]);

  useEffect(() => {
    if (plane === undefined) return;
    let gone = false;
    void (async () => {
      let known: ChatSkillRun[];
      try {
        const answer = await commands.chatSkillRuns(plane);
        if (answer.status !== "ok") return;
        known = answer.data;
      } catch {
        return;
      }
      if (!gone && Array.isArray(known))
        store.change((was) => ({
          plane,
          runs: known.reduce(toldRun, was.plane === plane ? was.runs : NONE),
        }));
    })();
    return () => {
      gone = true;
    };
  }, [plane, store]);

  return { store, plane };
}
