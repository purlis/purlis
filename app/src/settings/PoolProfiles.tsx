// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import type { PoolProfileRow } from "../bindings";
import { askProfiles, usePools } from "../pools";
import { Button } from "../ui/button/Button";
import { askProfileSignIn } from "./links";

/** What a profile's row on a pool says of it (#2049). */
export function standingOnPool(pool: string, row: PoolProfileRow): string {
  if (row.not_kept !== null) return `Not on pool ${pool}: ${row.not_kept}.`;
  switch (row.standing) {
    case "signed-in":
      return `Signed in on pool ${pool}.`;
    case "signed-out":
      return `Not signed in on pool ${pool}: a chat on it does not start there until it is.`;
    default:
      return `${row.harness} cannot say whether it is signed in on pool ${pool}.`;
  }
}

/**
 * **This machine's profiles on pool `pool`'s runner** (#2049): each kept there, in a folder of
 * the runner's own with nothing of this machine's in it, and whether it is signed in there. Sign
 * in opens a shell tab in the project `plane`'s window, where the core types the runner's own
 * sign-in, which the person drives; nothing of this machine's sign-in goes to the runner. With
 * no project open, it says how to sign in from a terminal instead.
 */
export function PoolProfiles({ pool, plane }: { pool: string; plane: string | undefined }) {
  const now = usePools();
  const rows = now.profiles[pool];
  useEffect(() => {
    if (rows === undefined) void askProfiles(pool);
  }, [pool, rows]);
  if (rows === undefined || rows === "asking") {
    return <p className="ui-setting-help">Asking the runner about this machine&apos;s profiles…</p>;
  }
  if (!Array.isArray(rows)) {
    return (
      <p className="ui-setting-help" role="alert">
        {rows.trouble}
      </p>
    );
  }
  if (rows.length === 0) {
    return <p className="ui-setting-help">This machine keeps no profile of its own yet.</p>;
  }
  const signIn = (row: PoolProfileRow, provider?: string) => {
    if (plane !== undefined) askProfileSignIn(plane, row.name, provider, pool);
  };
  return (
    <>
      <ul className="ui-machine-list" aria-label={`Profiles on pool ${pool}`}>
        {rows.map((row) => (
          <li key={row.name} className="ui-machine-entry">
            <span className="ui-machine-name">{row.name}</span>
            <span className="ui-setting-help">{standingOnPool(pool, row)}</span>
            {row.not_kept === null &&
              row.standing !== "signed-in" &&
              (plane === undefined ? (
                <span className="ui-setting-help">
                  Open a project to sign it in from its window.
                </span>
              ) : row.plans.length > 0 ? (
                row.plans.map((plan) => (
                  <Button
                    key={plan.id}
                    aria-label={`Sign ${row.name} in to ${plan.title} on pool ${pool}`}
                    onPress={() => signIn(row, plan.id)}
                    className="ui-setting-reset"
                  >
                    {`Sign in to ${plan.title}`}
                  </Button>
                ))
              ) : (
                <Button
                  aria-label={`Sign ${row.name} in on pool ${pool}`}
                  onPress={() => signIn(row)}
                  className="ui-setting-reset"
                >
                  Sign in
                </Button>
              ))}
          </li>
        ))}
      </ul>
      <Button size="sm" onPress={() => void askProfiles(pool)}>
        Ask again
      </Button>
    </>
  );
}
