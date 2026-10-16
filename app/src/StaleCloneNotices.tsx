// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useCallback, useEffect, useState } from "react";
import { commands, type PlaneId, type StaleClone, type StaleFix } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { GIT, usePlaneChanged } from "./planeChanged";
import { useWindowFocused } from "./windowFocused";

/** The family of a stale repo's cause, whose dismissal is kept until the repo is fixed. */
export const STALE_CLONE = "stale-clone";

/** The cause of the line naming what could not be read. Not kept: it is said again next run. */
const UNREAD = "stale-clone-unread";

/** Which repo a stale repo's Notice is about: its workspace and its name. */
const repoOf = (stale: Pick<StaleClone, "workspace" | "repo">) =>
  `${stale.workspace}/${stale.repo}`;

/**
 * What a stale repo's Notice is keyed by: its repo and each finding it holds, the branch it is
 * on and the address the forge named. A dismissal hides those findings: a long-lived branch is
 * dismissed once, and a later, different finding about the same repo shows again.
 */
export const staleCloneCause = (stale: StaleClone) =>
  [
    `${STALE_CLONE}:${repoOf(stale)}`,
    ...(stale.branch !== null ? [`branch:${stale.branch.branch}`] : []),
    ...(stale.remote !== null ? [`remote:${stale.remote.now}`] : []),
  ].join("|");

/**
 * **A workspace's repo left off its default branch, or on a moved remote** (#1985): one Notice
 * per repo in the Inbox, with its Fix.
 *
 * - **Off its default branch**: the repo itself (never a branch folder) is on another branch.
 *   *Switch to `<default>`* switches it back, only where its tree holds no change, every commit
 *   of the branch is on the remote or another branch, and no chat standing in it is mid-turn;
 *   the branch is kept.
 * - **On a moved remote**: the forge answered a fetch with the repo's new address. *Use the new
 *   address* sets `origin` to exactly the address the Notice shows. A push address of `origin`'s
 *   own goes with it where every one names the old place; where any names another, every one is
 *   left, and the Notice names them (#2117).
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
  const [unread, setUnread] = useState<readonly string[]>([]);
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
        if (live && answer.status === "ok" && Array.isArray(answer.data?.stale)) {
          setStale(answer.data.stale);
          setUnread(answer.data.unread);
          // A dismissal stands while its findings do, and a return shows again.
          settle(STALE_CLONE, answer.data.stale.map(staleCloneCause));
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
      const cause = repoOf(one);
      if (busy !== undefined) return;
      setBusy(cause);
      // The remote fix sets the address shown here, and nothing else.
      const shown = fix === "remote" ? (one.remote?.now ?? null) : null;
      void commands
        .fixStaleClone(plane, one.workspace, one.repo, fix, shown)
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
      {unread.length > 0 && !dismissed.has(UNREAD) && (
        <Notice cause={UNREAD} tone="trouble" onDismiss={() => dismiss(UNREAD)}>
          <p>purlis could not check some repos for a stale branch or address: {unread.join(" ")}</p>
        </Notice>
      )}
      {stale
        .filter((one) => !dismissed.has(staleCloneCause(one)))
        .map((one) => {
          const cause = staleCloneCause(one);
          const repo = repoOf(one);
          const fixes: NoticeAction[] = [];
          if (one.branch !== null)
            fixes.push({
              label: `Switch to ${one.branch.default}`,
              busy: busy === repo,
              onPress: () => press(one, "branch"),
            });
          if (one.remote !== null)
            fixes.push({
              label: "Use the new address",
              busy: busy === repo,
              onPress: () => press(one, "remote"),
            });
          const [first, ...rest] = fixes;
          if (first === undefined) return null;
          const blocked = said[repo];
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
                    <code>{one.branch.branch}</code>, not <code>{one.branch.default}</code>. A chat
                    that starts in this repo works on <code>{one.branch.branch}</code>.{" "}
                  </>
                )}
                {one.remote !== null && (
                  <>
                    <code>{one.repo}</code> in {one.workspace} fetches from{" "}
                    <code>{one.remote.origin}</code>, and the forge says it moved to{" "}
                    <code>{one.remote.now}</code>. Its fetches and pushes reach it only while the
                    forge redirects the old address.
                    <KeptPushes places={one.remote.pushes_kept} />
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

/**
 * Where the repo pushes that *Use the new address* leaves as it is (#2117): its push addresses,
 * where any names another place than `origin`. Nothing where there is none.
 */
function KeptPushes({ places }: { places: readonly string[] }) {
  if (places.length === 0) return null;
  const left = places.length === 1 ? "it is" : "they are";
  return (
    <>
      {" "}
      It pushes to <Places places={places} />, which Use the new address leaves as {left}.
    </>
  );
}

/** `places` as code, in a sentence: `a`, `a and b`, `a, b and c`. */
function Places({ places }: { places: readonly string[] }) {
  return places.map((place, at) => (
    <Fragment key={place}>
      {at === 0 ? "" : at === places.length - 1 ? " and " : ", "}
      <code>{place}</code>
    </Fragment>
  ));
}
