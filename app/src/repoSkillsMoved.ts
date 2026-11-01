// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import type { BranchChanged, PlaneId } from "./bindings";
import { branchKey, type BranchRef } from "./branchStatus";
import { answeredBy } from "./branchFolders";
import { listenToBranches, newestListen } from "./branchWatchSet";
import { listen } from "./here";

/**
 * **The repo skills of these checkouts moved** (#4377): the plane's watch stays out of clones, so
 * the Skills view listens to the checkout its chat works in on the branch watch, and the core says
 * `repo-skills-moved` only for a move in a folder a harness reads repo skills from, so an agent's
 * other writes read nothing.
 *
 * Calls `heard` for each such move, and once when a checkout's watch holds, since a change before
 * that was told by nothing (`branchStatus.ts`, #1427). Listens only while the view is drawn.
 */
export function useRepoSkillsMoved(
  plane: PlaneId,
  workspace: string | undefined,
  checkouts: readonly BranchRef[],
  heard: () => void,
) {
  const latest = useRef(heard);
  useEffect(() => {
    latest.current = heard;
  });
  const [owner] = useState(() => Symbol("skills view"));
  const keys = checkouts.map(branchKey).join("\0");
  const mine = useRef(checkouts);
  useEffect(() => {
    mine.current = checkouts;
  });

  useEffect(() => {
    let gone = false;
    const wanted =
      workspace === undefined ? [] : mine.current.map((one) => ({ plane, workspace, ...one }));
    void listenToBranches(owner, wanted);
    void answeredBy(newestListen).then(() => {
      if (!gone && wanted.length > 0) latest.current();
    });
    return () => {
      gone = true;
    };
  }, [owner, plane, workspace, keys]);

  useEffect(() => () => void listenToBranches(owner, []), [owner]);

  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<BranchChanged>("repo-skills-moved", (event) => {
          if (gone) return;
          const told = event.payload.branches.some(
            (one) =>
              one.plane === plane &&
              one.workspace === workspace &&
              mine.current.some((ref) => branchKey(ref) === branchKey(one)),
          );
          if (told) latest.current();
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
  }, [plane, workspace]);
}
