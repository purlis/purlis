// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PoolProfileRow } from "../bindings";
import { addProfileToPool, askProfiles, removeProfileFromPool, usePools } from "../pools";
import { Button } from "../ui/button/Button";
import { Menu, MenuItem, MenuTrigger } from "../ui/menu/Menu";
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
 * **The profiles on pool `pool`'s runner** (#2049, #4047): only the ones chosen for it, or,
 * where none were, the profile a new chat in the open project `plane` gets; each kept there, in
 * a folder of the runner's own with nothing of this machine's in it, and whether it is signed
 * in there. Add a profile… adds another of this machine's, and Remove, asked first, takes one
 * off, its sign-in there with it. Sign in opens a shell tab in the project `plane`'s window,
 * where the core types the runner's own sign-in, which the person drives; nothing of this
 * machine's sign-in goes to the runner. With no project open, it says how to sign in from a
 * terminal instead.
 */
export function PoolProfiles({ pool, plane }: { pool: string; plane: string | undefined }) {
  const now = usePools();
  const rows = now.profiles[pool];
  // The profile whose Remove was pressed, asked about before it goes; and what a change said.
  const [removing, setRemoving] = useState<string | undefined>(undefined);
  const [said, setSaid] = useState<string | undefined>(undefined);
  useEffect(() => {
    if (rows === undefined) void askProfiles(pool, plane);
  }, [pool, plane, rows]);
  if (rows === undefined || rows === "asking") {
    return <p className="ui-setting-help">Asking the runner about this machine&apos;s profiles…</p>;
  }
  const again = (
    <Button size="sm" onPress={() => void askProfiles(pool, plane)}>
      Ask again
    </Button>
  );
  if (!("kept" in rows)) {
    return (
      <>
        <p className="ui-setting-help" role="alert">
          {rows.trouble}
        </p>
        {again}
      </>
    );
  }
  const signIn = (row: PoolProfileRow, provider?: string) => {
    if (plane !== undefined) askProfileSignIn(plane, row.name, provider, pool);
  };
  const add = async (name: string) => {
    setSaid(await addProfileToPool(pool, name, plane));
  };
  const remove = async (name: string) => {
    setRemoving(undefined);
    setSaid(await removeProfileFromPool(pool, name, plane));
  };
  const adding =
    rows.addable.length === 0 ? null : (
      <MenuTrigger>
        <Button size="sm">Add a profile…</Button>
        <Menu aria-label="Add a profile…">
          {rows.addable.map((one) => (
            <MenuItem
              key={one.name}
              id={one.name}
              textValue={`${one.name}, ${one.harness}`}
              onAction={() => void add(one.name)}
            >
              {`${one.name} (${one.harness})`}
            </MenuItem>
          ))}
        </Menu>
      </MenuTrigger>
    );
  const told =
    said === undefined ? null : (
      <p className="ui-setting-help" role="status">
        {said}
      </p>
    );
  if (rows.kept.length === 0) {
    return (
      <>
        <p className="ui-setting-help">
          {rows.addable.length === 0
            ? "This machine keeps no profile of its own yet."
            : `No profile is on pool ${pool}: its chats start on the profiles its runner has of its own.`}
        </p>
        {told}
        {adding}
      </>
    );
  }
  return (
    <>
      <ul className="ui-machine-list" aria-label={`Profiles on pool ${pool}`}>
        {rows.kept.map((row) => (
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
            {removing === row.name ? (
              <>
                <span className="ui-setting-help">
                  {`Remove ${row.name} from pool ${pool}? Its runner forgets it, and its sign-in there goes with it.`}
                </span>
                <Button
                  variant="danger"
                  aria-label={`Yes, remove ${row.name} from pool ${pool}`}
                  onPress={() => void remove(row.name)}
                  className="ui-setting-reset"
                >
                  Remove
                </Button>
                <Button
                  aria-label={`Keep ${row.name} on pool ${pool}`}
                  onPress={() => setRemoving(undefined)}
                  className="ui-setting-reset"
                >
                  Keep
                </Button>
              </>
            ) : (
              <Button
                variant="ghost"
                aria-label={`Remove ${row.name} from pool ${pool}`}
                onPress={() => setRemoving(row.name)}
                className="ui-setting-reset"
              >
                Remove
              </Button>
            )}
          </li>
        ))}
      </ul>
      {told}
      {adding}
      {again}
    </>
  );
}
