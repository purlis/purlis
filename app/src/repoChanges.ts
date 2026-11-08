// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { commands, type BranchChanged, type PlaneId, type RepoChanged } from "./bindings";
import { answeredBy, heldWithin } from "./branchFolders";
import { listenToBranches, newestListen } from "./branchWatchSet";
import { listen } from "./here";

/**
 * **What each closed repo row says it changed, in one ask** (#3489): the explorer's rows for the
 * repos of a workspace (#3343), each with its own folder's mark and how many files it changed.
 *
 * Each row used to read its repo's whole status (`branch_status`, `branchStatus.ts`), on every
 * refresh: twenty status reads for a workspace of twenty repos. They are one `repos_changed` now,
 * whose answer the core keeps per clone on the store the save standings share
 * (`purlis_core::files::repo_changes`): an ask with nothing moved reads no status at all, and one
 * after a write reads only the repo it was in.
 *
 * **Asked again when one of them moves.** Each repo's own folder is listened to on the one watch
 * per repo (`branchwatch.rs`), as an open branch is, and a `branch-changed` naming any of them asks
 * for them all again: one ask, which the core answers from what it kept for every repo but the
 * one that moved. Watched first, then asked, as a branch's status is: the first ask waits for the
 * watch to hold, or `WATCH_WAIT_MS`, and asks once more when it holds if it did not in time. One
 * ask runs at a time; a move during it asks for exactly one more after it.
 *
 * @param repos The repos whose closed rows are drawn, in this workspace.
 */
export function useRepoChanges(
  plane: PlaneId | undefined,
  workspace: string | undefined,
  repos: readonly string[],
): ReadonlyMap<string, RepoChanged> {
  const [held, setHeld] = useState<{ key?: string; changed: ReadonlyMap<string, RepoChanged> }>({
    changed: EMPTY,
  });
  const names = repos.join("\0");
  const key = plane === undefined || workspace === undefined ? undefined : `${workspace}\0${names}`;
  /** The ask: whether one is running, and whether another is wanted after it. */
  const flight = useRef<{ key: string; again: boolean } | undefined>(undefined);
  const ask = useRef<() => void>(() => {});
  /** The rows' key now: an answer for an older one is dropped. */
  const asked = useRef(key);

  useEffect(() => {
    asked.current = key;
    ask.current = () => {
      if (plane === undefined || workspace === undefined || key === undefined || names === "")
        return;
      if (flight.current?.key === key) {
        flight.current.again = true;
        return;
      }
      const now = { key, again: false };
      flight.current = now;
      const told = (changed?: RepoChanged[]) => {
        if (flight.current === now) flight.current = undefined;
        if (changed !== undefined && asked.current === key)
          setHeld({ key, changed: new Map(changed.map((one) => [one.repo, one])) });
        if (now.again && asked.current === key) ask.current();
      };
      void commands
        .reposChanged(plane, workspace, names.split("\0"))
        .then((said) => told(said.status === "ok" && Array.isArray(said.data) ? said.data : []))
        .catch(() => told());
    };
  });

  const [owner] = useState(() => Symbol("closed repo rows"));
  useEffect(() => {
    const wanted =
      plane === undefined || workspace === undefined || names === ""
        ? []
        : names.split("\0").map((repo) => ({ plane, workspace, repo, piece: null }));
    void listenToBranches(owner, wanted);
    if (wanted.length === 0) return;
    let gone = false;
    const latest = newestListen;
    void heldWithin(latest).then((inTime) => {
      if (gone) return;
      ask.current();
      // Asked before its watch held: asked again once it holds, so a change in between counts.
      if (!inTime) void answeredBy(latest).then(() => !gone && ask.current());
    });
    return () => {
      gone = true;
    };
  }, [names, owner, plane, workspace]);

  // Nothing listened to for them once the rows are gone.
  useEffect(() => () => void listenToBranches(owner, []), [owner]);

  // Something moved in one of them: they are asked for again, in one ask.
  useEffect(() => {
    if (plane === undefined || workspace === undefined || names === "") return;
    const mine = new Set(names.split("\0"));
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<BranchChanged>("branch-changed", (event) => {
          if (gone) return;
          const moved = event.payload.branches.some(
            (one) =>
              one.plane === plane &&
              one.workspace === workspace &&
              one.piece === null &&
              mine.has(one.repo),
          );
          if (moved) ask.current();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [names, plane, workspace]);

  return held.key === key ? held.changed : EMPTY;
}

const EMPTY: ReadonlyMap<string, RepoChanged> = new Map();
