// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type PlaneId, type StaleClone, type StaleFix } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { GIT, usePlaneChanged } from "./planeChanged";
import { useWindowFocused } from "./windowFocused";

/** The family of a stale repo's cause, whose dismissal is kept until the repo is fixed. */
export const STALE_CLONE = "stale-clone";

/** What a stale repo's Notice is keyed by: its workspace and its name. */
export const staleCloneCause = (stale: Pick<StaleClone, "workspace" | "repo">) =>
  `${STALE_CLONE}:${stale.workspace}/${stale.repo}`;

/**
 * **A workspace's repo left on a chat's branch, or on a moved remote** (#1985): one Notice per
 * repo in the Inbox, with its Fix.
 *
 * - **On a chat's branch**: the repo is on a branch purlis cut for a chat or a task, not on its
 *   default branch. *Switch to `<default>`* switches it back, only where its tree holds no change
 *   and every commit of the branch is on the remote or another branch; the branch is kept.
 * - **On a moved remote**: the forge answered a fetch with the repo's new address. *Use the new
 *   address* sets `origin` to it.
 *
 * A Fix that would lose work does nothing and says what blocks it, on the Notice. The list is
 * read again when the window comes back into focus and when the project's git standing moves,
 * so a Notice goes when its repo is fixed, from here or by hand.
 */
export function StaleCloneNotices({
  plane,
  dismissed,
  dismiss,
  settle,
}: {
  plane: PlaneId;
  /** The causes dismissed in this project, and the ways to change them (`useDismissals`). */
  dismissed: ReadonlySet<string>;
  dismiss: (cause: string) => void;
  settle: (family: string, present: readonly string[]) => void;
}) {
  const [stale, setStale] = useState<readonly StaleClone[]>([]);
  /** What the last press said, by the Notice it was pressed on. */
  const [said, setSaid] = useState<Readonly<Record<string, string>>>({});
  const [busy, setBusy] = useState<string>();
  const [asked, setAsked] = useState(0);
  const focused = useWindowFocused();
  const moved = usePlaneChanged([plane], GIT);

  useEffect(() => {
    let live = true;
    void commands
      .staleClones(plane)
      .then((answer) => {
        if (live && answer.status === "ok" && Array.isArray(answer.data)) {
          setStale(answer.data);
          // A dismissal stands until its repo is no longer stale, and a return shows again.
          settle(STALE_CLONE, answer.data.map(staleCloneCause));
        }
      })
      // A project whose repos cannot be read says nothing here.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, focused, moved, asked, settle]);

  const press = useCallback(
    (one: StaleClone, fix: StaleFix) => {
      const cause = staleCloneCause(one);
      if (busy !== undefined) return;
      setBusy(cause);
      void commands
        .fixStaleClone(plane, one.workspace, one.repo, fix)
        .then((answer) => {
          const words = answer.status === "ok" ? undefined : answer.error;
          setSaid((was) => {
            const rest = Object.fromEntries(Object.entries(was).filter(([key]) => key !== cause));
            return words === undefined ? rest : { ...rest, [cause]: words };
          });
        })
        .catch((err: unknown) => setSaid((was) => ({ ...was, [cause]: String(err) })))
        .finally(() => {
          setBusy(undefined);
          setAsked((n) => n + 1);
        });
    },
    [plane, busy],
  );

  return (
    <>
      {stale
        .filter((one) => !dismissed.has(staleCloneCause(one)))
        .map((one) => {
          const cause = staleCloneCause(one);
          const fixes: NoticeAction[] = [];
          if (one.branch !== null)
            fixes.push({
              label: `Switch to ${one.branch.default}`,
              busy: busy === cause,
              onPress: () => press(one, "branch"),
            });
          if (one.remote !== null)
            fixes.push({
              label: "Use the new address",
              busy: busy === cause,
              onPress: () => press(one, "remote"),
            });
          const [first, ...rest] = fixes;
          if (first === undefined) return null;
          const blocked = said[cause];
          return (
            <Notice
              key={cause}
              cause={cause}
              tone={blocked === undefined ? "news" : "trouble"}
              label={`${one.repo} in ${one.workspace}`}
              fixes={[first, ...rest]}
              onDismiss={() => dismiss(cause)}
            >
              <p>
                {one.branch !== null && (
                  <>
                    <code>{one.repo}</code> in {one.workspace} is on{" "}
                    <code>{one.branch.branch}</code>, a branch purlis cut for a chat, and not on{" "}
                    <code>{one.branch.default}</code>. A chat that starts in this repo works on that
                    branch.{" "}
                  </>
                )}
                {one.remote !== null && (
                  <>
                    <code>{one.repo}</code> in {one.workspace} fetches from{" "}
                    <code>{one.remote.origin}</code>, and the forge says it moved to{" "}
                    <code>{one.remote.now}</code>. Its fetches and pushes reach it only while the
                    forge redirects the old address.
                  </>
                )}
                {blocked !== undefined && ` ${blocked}`}
              </p>
            </Notice>
          );
        })}
    </>
  );
}
