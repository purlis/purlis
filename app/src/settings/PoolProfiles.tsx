// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PoolProfileRow } from "../bindings";
import { askProfiles, usePools } from "../pools";
import { Button } from "../ui/button/Button";
import { askProfileSignIn } from "./links";
import { PoolSignIn } from "./PoolSignIn";

/** The harnesses whose sign-in purlis drives in place, with no terminal (#4042). */
const DRIVEN = new Set(["claude"]);

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
 * the runner's own with nothing of this machine's in it, and whether it is signed in there.
 * Sign in on a Claude Code profile opens its line in place into a sign-in strip that drives the
 * harness's own login with no terminal ({@link PoolSignIn}, #4042). Another harness's opens a
 * shell tab in the project `plane`'s window, where the core types the runner's own sign-in, which
 * the person drives; with no project open, it says how to sign in from a terminal instead.
 * Nothing of this machine's sign-in goes to the runner.
 */
export function PoolProfiles({ pool, plane }: { pool: string; plane: string | undefined }) {
  const now = usePools();
  const asked = now.profiles[pool];
  const [signing, setSigning] = useState<string>();
  // The rows a sign-in was opened from: asked again meanwhile, they stay drawn, so its strip stays
  // and its login with it.
  const [heldRows, setHeldRows] = useState<PoolProfileRow[]>();
  useEffect(() => {
    if (asked === undefined) void askProfiles(pool);
  }, [pool, asked]);
  const rows = signing !== undefined && heldRows !== undefined ? heldRows : asked;
  if (rows === undefined || rows === "asking") {
    return <p className="ui-setting-help">Asking the runner about this machine&apos;s profiles…</p>;
  }
  const again = (
    <Button size="sm" onPress={() => void askProfiles(pool)}>
      Ask again
    </Button>
  );
  if (!Array.isArray(rows)) {
    return (
      <>
        <p className="ui-setting-help" role="alert">
          {rows.trouble}
        </p>
        {again}
      </>
    );
  }
  if (rows.length === 0) {
    return <p className="ui-setting-help">This machine keeps no profile of its own yet.</p>;
  }
  const signIn = (row: PoolProfileRow, provider?: string) => {
    if (plane !== undefined) askProfileSignIn(plane, row.name, provider, pool);
  };
  const ended = (signedIn: boolean) => {
    setSigning(undefined);
    setHeldRows(undefined);
    if (signedIn) void askProfiles(pool);
  };
  return (
    <>
      <ul className="ui-machine-list" aria-label={`Profiles on pool ${pool}`}>
        {rows.map((row) => (
          <li key={row.name} className="ui-machine-entry">
            <span className="ui-machine-name">{row.name}</span>
            <span className="ui-setting-help">
              {signing === row.name ? `Signing in on pool ${pool}…` : standingOnPool(pool, row)}
            </span>
            {signing === row.name ? (
              <PoolSignIn
                pool={pool}
                profile={row.name}
                harness={row.harness}
                onEnd={ended}
                onTerminal={
                  plane === undefined
                    ? undefined
                    : () => {
                        ended(false);
                        signIn(row);
                      }
                }
              />
            ) : (
              row.not_kept === null &&
              row.standing !== "signed-in" &&
              (DRIVEN.has(row.kind) ? (
                <Button
                  aria-label={`Sign ${row.name} in on pool ${pool}`}
                  onPress={() => {
                    setHeldRows(rows);
                    setSigning(row.name);
                  }}
                  className="ui-setting-reset"
                >
                  Sign in
                </Button>
              ) : plane === undefined ? (
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
              ))
            )}
          </li>
        ))}
      </ul>
      {signing === undefined && again}
    </>
  );
}
