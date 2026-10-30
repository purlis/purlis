// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { PoolProfileRow, PoolProfiles as PoolProfilesAnswered } from "../bindings";
import {
  ASK_AGAIN_WHILE_FETCHING_MS,
  addProfileToPool,
  askProfiles,
  removeProfileFromPool,
  signProfileOutOfPool,
  usePools,
} from "../pools";
import { Button } from "../ui/button/Button";
import { Menu, MenuItem, MenuTrigger } from "../ui/menu/Menu";
import { askProfileSignIn } from "./links";
import { PoolSignIn, type SignInStanding } from "./PoolSignIn";

/** The harnesses whose sign-in purlis drives in place, with no terminal (#4042). */
const DRIVEN = new Set(["claude"]);

/** The harnesses with a sign-out of their own that purlis runs: opencode's asks in a terminal. */
const SIGNS_OUT = new Set(["claude", "codex"]);

/** What a profile's line says of it while its sign-in strip is open (spec #4023 story 29). */
const STANDING_SAID: Record<SignInStanding, string> = {
  "signing-in": "Signing in…",
  "in-the-terminal": "Needs you in the terminal",
  failed: "Sign-in failed",
  "signed-in": "Signed in",
};

/** What a profile's row on a pool says of it (#2049). */
export function standingOnPool(pool: string, row: PoolProfileRow): string {
  if (row.not_kept !== null) return `Not on pool ${pool}: ${row.not_kept}.`;
  switch (row.standing) {
    case "signed-in":
      return `Signed in on pool ${pool}.`;
    case "signed-out":
      return `Not signed in on pool ${pool}: a chat on it does not start there until it is.`;
    case "fetching":
      return `Fetching ${row.harness} for this runner…`;
    default:
      return `${row.harness} cannot say whether it is signed in on pool ${pool}.`;
  }
}

/**
 * **The profiles on pool `pool`'s runner** (#2049, #4047): only the ones chosen for it, or,
 * where none were, the profile a new chat in the open project `plane` gets; each kept there, in
 * a folder of the runner's own with nothing of this machine's in it, and whether it is signed
 * in there. Add a profile… adds another of this machine's, and Remove, asked first, takes one
 * off, its sign-in there with it; neither is offered while a sign-in runs on the pool.
 * Sign in on a Claude Code profile opens its line in place into a sign-in strip that drives the
 * harness's own login with no terminal ({@link PoolSignIn}, #4042). Another harness's opens a
 * shell tab in the project `plane`'s window, where the core types the runner's own sign-in, which
 * the person drives; with no project open, it says how to sign in from a terminal instead.
 * Nothing of this machine's sign-in goes to the runner. Each line says where it stands, and a
 * signed-in one offers Sign out, asked first, which runs the harness's own there (#4043).
 */
export function PoolProfiles({ pool, plane }: { pool: string; plane: string | undefined }) {
  const now = usePools();
  const asked = now.profiles[pool];
  const [signing, setSigning] = useState<string>();
  // The rows a sign-in was opened from: asked again meanwhile, they stay drawn, so its strip stays
  // and its login with it.
  const [heldRows, setHeldRows] = useState<PoolProfilesAnswered>();
  // The profile whose Remove was pressed, asked about before it goes; and what a change said.
  const [removing, setRemoving] = useState<string | undefined>(undefined);
  const [said, setSaid] = useState<string | undefined>(undefined);
  // What the open strip's line says, and the lines whose last sign-in failed.
  const [strip, setStrip] = useState<SignInStanding>("signing-in");
  const [failed, setFailed] = useState<ReadonlySet<string>>(new Set());
  // The profile whose Sign out was pressed, asked about first, then running; and what it said.
  const [out, setOut] = useState<{ name: string; running: boolean }>();
  const [outSaid, setOutSaid] = useState<{ text: string; trouble: boolean }>();
  useEffect(() => {
    if (asked === undefined) void askProfiles(pool, plane);
  }, [pool, plane, asked]);
  // A runner fetching a profile's harness answers once it is fetched: asked again until then,
  // keeping the rows shown (#4008).
  const fetching =
    typeof asked === "object" &&
    "kept" in asked &&
    asked.kept.some((row) => row.standing === "fetching");
  useEffect(() => {
    if (!fetching) return;
    const timer = setTimeout(
      () => void askProfiles(pool, plane, { quietly: true }),
      ASK_AGAIN_WHILE_FETCHING_MS,
    );
    return () => clearTimeout(timer);
  }, [pool, plane, asked, fetching]);
  const rows = signing !== undefined && heldRows !== undefined ? heldRows : asked;
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
  const ended = (signedIn: boolean) => {
    if (signing !== undefined) {
      const name = signing;
      setFailed((before) => {
        const after = new Set(before);
        if (strip === "failed") after.add(name);
        else after.delete(name);
        return after;
      });
    }
    setSigning(undefined);
    setHeldRows(undefined);
    if (signedIn) void askProfiles(pool, plane);
  };
  const add = async (name: string) => {
    setSaid(await addProfileToPool(pool, name, plane));
  };
  const remove = async (name: string) => {
    setRemoving(undefined);
    setSaid(await removeProfileFromPool(pool, name, plane));
  };
  const signOut = async (name: string) => {
    setOut({ name, running: true });
    setOutSaid(undefined);
    const why = await signProfileOutOfPool(pool, name, plane);
    setOut(undefined);
    setOutSaid(
      why === undefined
        ? { text: `${name} is signed out on pool ${pool}.`, trouble: false }
        : { text: why, trouble: true },
    );
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
            <span className="ui-setting-help">
              {signing === row.name
                ? STANDING_SAID[strip]
                : out?.name === row.name && out.running
                  ? `Signing out on pool ${pool}…`
                  : failed.has(row.name) && row.standing !== "signed-in"
                    ? STANDING_SAID.failed
                    : standingOnPool(pool, row)}
            </span>
            {signing === row.name ? (
              <PoolSignIn
                pool={pool}
                profile={row.name}
                harness={row.harness}
                onEnd={ended}
                onStanding={setStrip}
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
                    setRemoving(undefined);
                    setOut(undefined);
                    setOutSaid(undefined);
                    setStrip("signing-in");
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
            {signing === undefined &&
              row.not_kept === null &&
              row.standing === "signed-in" &&
              removing !== row.name &&
              (!SIGNS_OUT.has(row.kind) ? (
                <span className="ui-setting-help">
                  {`${row.harness} signs out only in its own terminal.`}
                </span>
              ) : out?.name === row.name && !out.running ? (
                <>
                  <span className="ui-setting-help">
                    {`Sign ${row.name} out on pool ${pool}? A chat with it does not start there until it signs in again.`}
                  </span>
                  <Button
                    variant="danger"
                    aria-label={`Yes, sign ${row.name} out on pool ${pool}`}
                    onPress={() => void signOut(row.name)}
                    className="ui-setting-reset"
                  >
                    Sign out
                  </Button>
                  <Button
                    aria-label={`Stay signed in as ${row.name} on pool ${pool}`}
                    onPress={() => setOut(undefined)}
                    className="ui-setting-reset"
                  >
                    Keep
                  </Button>
                </>
              ) : (
                <Button
                  variant="ghost"
                  aria-label={`Sign ${row.name} out on pool ${pool}`}
                  isDisabled={out?.running === true}
                  onPress={() => {
                    setRemoving(undefined);
                    setOut({ name: row.name, running: false });
                  }}
                  className="ui-setting-reset"
                >
                  Sign out
                </Button>
              ))}
            {signing === undefined &&
              out?.name !== row.name &&
              (removing === row.name ? (
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
              ))}
          </li>
        ))}
      </ul>
      {told}
      {outSaid !== undefined && (
        <p className="ui-setting-help" role={outSaid.trouble ? "alert" : "status"}>
          {outSaid.text}
        </p>
      )}
      {signing === undefined && adding}
      {signing === undefined && again}
    </>
  );
}
