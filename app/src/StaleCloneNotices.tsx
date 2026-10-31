// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import { commands, type PlaneId, type StaleClone, type StaleFix } from "./bindings";
import type { StandingNotice } from "./ChipNotices";
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
 *   own goes with it where every one is its old address, and the Notice says pushes move
 *   too; where any is another, every one is left, and the Notice names them (#2117).
 *
 * A Fix that would lose work does nothing and says what blocks it, on the Notice. The list is
 * read again when the window comes back into focus and when the project's git standing moves,
 * so a Notice goes when its repo is fixed, from here or by hand.
 *
 * **Read once, drawn twice** (#3650): each line also hangs from its workspace's chip in the title
 * bar (`ChipNotices.tsx`), with the same fixes, the same busy and refused state, and the one
 * dismissal record (`dismissals.ts`), kept until the finding changes.
 */
export function useStaleClones({
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
}): StaleClones {
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

  const lines = useMemo(
    () =>
      stale
        .filter((one) => !dismissed.has(staleCloneCause(one)))
        .flatMap((one): StaleLine[] => {
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
          if (first === undefined) return [];
          const blocked = said[repo];
          return [
            {
              workspace: one.workspace,
              cause,
              says: <StaleSays one={one} blocked={blocked} />,
              fixes: [first, ...rest],
              // A repo another chat would work on by mistake: a warning (▲), not news (#3650).
              kind: "warning",
              tone: blocked === undefined ? "news" : "trouble",
              label: `${one.repo} in ${one.workspace}`,
              dismiss: () => dismiss(cause),
            },
          ];
        }),
    [stale, dismissed, busy, said, press, dismiss],
  );
  return { lines, unread, unreadShown: unread.length > 0 && !dismissed.has(UNREAD), dismiss };
}

/** **What the project's stale repos are**, read once and drawn in two places (#3650): the Inbox's
 *  Notices, and each workspace's chip in the title bar, with the same ways and one dismissal. */
export type StaleClones = {
  /** One line per stale repo not dismissed, with its fixes. */
  lines: readonly StaleLine[];
  /** What could not be read, said in the Inbox alone. */
  unread: readonly string[];
  unreadShown: boolean;
  dismiss: (cause: string) => void;
};

/** **A stale repo's line**: always one fix at least, and always its Dismiss. */
export type StaleLine = StandingNotice & {
  fixes: readonly [NoticeAction, ...NoticeAction[]];
  dismiss: () => void;
};

/**
 * **A stale repo's words**: off its default branch, on a moved remote, or both, with what the last
 * press answered where it was refused.
 */
function StaleSays({ one, blocked }: { one: StaleClone; blocked: string | undefined }) {
  return (
    <p>
      {one.branch !== null && (
        <>
          <code>{one.repo}</code> in {one.workspace} is on <code>{one.branch.branch}</code>, not{" "}
          <code>{one.branch.default}</code>. A chat that starts in this repo works on{" "}
          <code>{one.branch.branch}</code>.{" "}
        </>
      )}
      {one.remote !== null && (
        <>
          <code>{one.repo}</code> in {one.workspace} fetches from <code>{one.remote.origin}</code>,
          and the forge says it moved to <code>{one.remote.now}</code>. Its fetches and pushes reach
          it only while the forge redirects the old address.
          {one.remote.pushes_follow && " Pushes move to the new address too."}
          <KeptPushes places={one.remote.pushes_kept} />
        </>
      )}
      {blocked !== undefined && ` ${blocked}`}
    </p>
  );
}

/**
 * **The project's stale repos in the Inbox** (#1985): what {@link useStaleClones} read, one
 * Notice per repo, and a line for what could not be read.
 */
export function StaleCloneNotices({ clones }: { clones: StaleClones }) {
  return (
    <>
      {clones.unreadShown && (
        <Notice cause={UNREAD} tone="trouble" onDismiss={() => clones.dismiss(UNREAD)}>
          <p>
            purlis could not check some repos for a stale branch or address:{" "}
            {clones.unread.join(" ")}
          </p>
        </Notice>
      )}
      {clones.lines.map((line) => (
        <Notice
          key={line.cause}
          cause={line.cause}
          tone={line.tone}
          label={line.label}
          fixes={line.fixes}
          onDismiss={line.dismiss}
        >
          {line.says}
        </Notice>
      ))}
    </>
  );
}

/**
 * Where the repo pushes that *Use the new address* leaves as it is (#2117): its push addresses,
 * where any is another than `origin`'s old one. Nothing where there is none.
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
