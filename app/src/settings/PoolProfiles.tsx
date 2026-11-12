// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type ReactNode } from "react";
import type { PoolProfileRow, PoolProfiles as PoolProfilesAnswered } from "../bindings";
import {
  ASK_AGAIN_WHILE_FETCHING_MS,
  addProfileToPool,
  askProfiles,
  removeProfileFromPool,
  signProfileOutOfPool,
  usePools,
} from "../pools";
import { OfferToRaise } from "../RaiseLimit";
import { Button } from "../ui/button/Button";
import { List, ListRow } from "../ui/list/List";
import { RowBelow, RowDescription, RowLabel, RowTrail } from "../ui/list/Row";
import { Menu, MenuItem, MenuTrigger } from "../ui/menu/Menu";
import { askProfileSignIn, askProfileSignOut } from "./links";
import { PoolSignIn, approvesIn, drivenPlan, type SignInStanding } from "./PoolSignIn";

/** The harnesses whose sign-in purlis drives in place, with no terminal (#4042, #4044). */
export const DRIVEN = new Set(["claude", "codex"]);

/** The harnesses with a sign-out of their own that purlis runs: opencode's asks in a terminal,
 *  which its line opens (#4124). */
const SIGNS_OUT = new Set(["claude", "codex"]);

/** What a terminal sign-out reaches (#4274): the running runner, never one still draining. */
const SIGNS_OUT_IN_A_TERMINAL_ON =
  "This signs out on the running runner. A draining runner keeps its sign-in until it is removed, and that sign-in does not move to the new runner.";

/** What a profile's line says of it while its sign-in strip is open (spec #4023 story 29). */
const STANDING_SAID: Record<SignInStanding, string> = {
  "signing-in": "Signing in…",
  "in-the-terminal": "Needs you in the terminal",
  failed: "Sign-in failed",
  "signed-in": "Signed in",
};

/** What a cluster pool on a fresh claim says of a sign-in there (#3819, spec #4023 Q6). */
export const lastsOnlyForThisClaim = (pool: string) =>
  `Pool ${pool}'s runner keeps nothing once it ends, so a sign-in there lasts only as long as the runner.`;

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
 * Sign in on a Claude Code or Codex profile, or an opencode profile's Copilot or ChatGPT plan,
 * opens a sign-in strip under the list, its line saying where it stands, that drives the harness's own login with no
 * terminal ({@link PoolSignIn}, #4042, #4044, #4045). Another opens a shell tab in the project
 * `plane`'s window, where the core types the runner's own sign-in, which the person drives; with
 * no project open, it says how to sign in from a terminal instead.
 * Nothing of this machine's sign-in goes to the runner. Each line says where it stands, and a
 * signed-in one offers Sign out, asked first, which runs the harness's own there (#4043).
 */
export function PoolProfiles({ pool, plane }: { pool: string; plane: string | undefined }) {
  const now = usePools();
  const asked = now.profiles[pool];
  const [signing, setSigning] = useState<string>();
  // The plan it signs in to, for a harness that serves several.
  const [plan, setPlan] = useState<string>();
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
  const signIn = (row: PoolProfileRow, provider?: string, device?: boolean) => {
    if (plane !== undefined) askProfileSignIn(plane, row.name, provider, pool, device);
  };
  const open = (row: PoolProfileRow, provider?: string) => {
    setRemoving(undefined);
    setOut(undefined);
    setOutSaid(undefined);
    setStrip("signing-in");
    setHeldRows(rows);
    setPlan(provider);
    setSigning(row.name);
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
    const said = await signProfileOutOfPool(pool, name, plane);
    setOut(undefined);
    setOutSaid(
      "failed" in said
        ? { text: said.failed, trouble: true }
        : {
            text: [`${name} is signed out on pool ${pool}.`, said.draining]
              .filter(Boolean)
              .join(" "),
            trouble: false,
          },
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
  // Said above its profiles, so it stays in sight while a sign-in's strip is open.
  const lasts = rows.fresh_claim ? (
    <p className="ui-setting-help">{lastsOnlyForThisClaim(pool)}</p>
  ) : null;
  const told =
    said === undefined ? null : (
      <p className="ui-setting-help" role="status">
        {said}
      </p>
    );
  if (rows.kept.length === 0) {
    return (
      <>
        {lasts}
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
  /** What a row offers, at its end, and what it says under its words (a question asked, or
   *  why it offers nothing here). */
  const offers = (row: PoolProfileRow): { trail: ReactNode; below: ReactNode } => {
    let trail: ReactNode = null;
    let below: ReactNode = null;
    // A sign-in's strip, a limit's form: drawn under the list, which they would not fit in.
    if (signing === row.name) return { trail, below };
    if (row.not_kept === null && row.standing !== "signed-in") {
      if (DRIVEN.has(row.kind))
        trail = (
          <Button
            size="sm"
            aria-label={`Sign ${row.name} in on pool ${pool}`}
            isDisabled={out?.running === true}
            onPress={() => open(row)}
          >
            Sign in
          </Button>
        );
      else if (plane === undefined && !row.plans.some((one) => drivenPlan(row.kind, one.id)))
        below = "Open a project to sign it in from its window.";
      else if (row.plans.length > 0) {
        trail = row.plans
          .filter((one) => plane !== undefined || drivenPlan(row.kind, one.id))
          .map((one) => (
            <Button
              key={one.id}
              size="sm"
              aria-label={`Sign ${row.name} in to ${one.title} on pool ${pool}`}
              isDisabled={out?.running === true}
              onPress={() =>
                drivenPlan(row.kind, one.id) ? open(row, one.id) : signIn(row, one.id)
              }
            >
              {`Sign in to ${one.title}`}
            </Button>
          ));
        if (plane === undefined)
          below = "Open a project to sign its other plans in from its window.";
      } else
        trail = (
          <Button
            size="sm"
            aria-label={`Sign ${row.name} in on pool ${pool}`}
            isDisabled={out?.running === true}
            onPress={() => signIn(row)}
          >
            Sign in
          </Button>
        );
    }
    // Neither Sign out nor Remove is offered on any row while a sign-in runs on the pool.
    if (signing !== undefined) return { trail, below };
    if (row.not_kept === null && row.standing === "signed-in" && removing !== row.name) {
      if (!SIGNS_OUT.has(row.kind)) {
        if (row.kind !== "opencode") below = `${row.harness} signs out only in its own terminal.`;
        else if (plane === undefined)
          below = `${row.harness} signs out in a terminal: open a project to sign ${row.name} out here.`;
        else {
          below = SIGNS_OUT_IN_A_TERMINAL_ON;
          trail = (
            <Button
              variant="ghost"
              size="sm"
              aria-label={`Sign out ${row.name} on pool ${pool} in a terminal`}
              onPress={() => askProfileSignOut(plane, row.name, pool)}
            >
              Sign out in a terminal
            </Button>
          );
        }
      } else if (out?.name === row.name && !out.running) {
        below = `Sign ${row.name} out on pool ${pool}? A chat with it does not start there until it signs in again.`;
        trail = (
          <>
            <Button
              variant="danger"
              size="sm"
              aria-label={`Yes, sign out ${row.name} on pool ${pool}`}
              onPress={() => void signOut(row.name)}
            >
              Sign out
            </Button>
            <Button
              size="sm"
              aria-label={`Keep ${row.name} signed in on pool ${pool}`}
              onPress={() => setOut(undefined)}
            >
              Keep
            </Button>
          </>
        );
      } else
        trail = (
          <Button
            variant="ghost"
            size="sm"
            aria-label={`Sign out ${row.name} on pool ${pool}`}
            isDisabled={out?.running === true}
            onPress={() => {
              setRemoving(undefined);
              setOut({ name: row.name, running: false });
            }}
          >
            Sign out
          </Button>
        );
    }
    if (out?.name !== row.name) {
      if (removing === row.name) {
        below = `Remove ${row.name} from pool ${pool}? Its runner forgets it, and its sign-in there goes with it.`;
        trail = (
          <>
            {trail}
            <Button
              variant="danger"
              size="sm"
              aria-label={`Yes, remove ${row.name} from pool ${pool}`}
              onPress={() => void remove(row.name)}
            >
              Remove
            </Button>
            <Button
              size="sm"
              aria-label={`Keep ${row.name} on pool ${pool}`}
              onPress={() => setRemoving(undefined)}
            >
              Keep
            </Button>
          </>
        );
      } else
        trail = (
          <>
            {trail}
            <Button
              variant="ghost"
              size="sm"
              aria-label={`Remove ${row.name} from pool ${pool}`}
              isDisabled={out?.running === true}
              onPress={() => setRemoving(row.name)}
            >
              Remove
            </Button>
          </>
        );
    }
    return { trail, below };
  };
  const signingRow = rows.kept.find((row) => row.name === signing);
  return (
    <>
      {lasts}
      {/* **The profiles, as rows of the list family** (#6301): a row a profile, its name, where
          it stands on the pool as its line, and what it offers at its end, which → reaches; a
          question asked about it under its words. */}
      <List aria-label={`Profiles on pool ${pool}`} className="pool-profiles" disallowTypeAhead>
        {rows.kept.map((row) => {
          const { trail, below } = offers(row);
          return (
            <ListRow key={row.name} id={row.name} textValue={row.name}>
              <RowLabel>{row.name}</RowLabel>
              <RowDescription>
                {signing === row.name
                  ? STANDING_SAID[strip]
                  : out?.name === row.name && out.running
                    ? `Signing out on pool ${pool}…`
                    : failed.has(row.name) && row.standing !== "signed-in"
                      ? STANDING_SAID.failed
                      : standingOnPool(pool, row)}
              </RowDescription>
              {trail !== null && <RowTrail>{trail}</RowTrail>}
              {below !== null && <RowBelow>{below}</RowBelow>}
            </ListRow>
          );
        })}
      </List>
      {/* Where a limit kept a profile off the runner, its form, which asks again (#4397). */}
      {rows.kept
        .filter((row) => row.not_kept !== null && row.limit !== null && signing !== row.name)
        .map((row) =>
          row.limit === null ? null : (
            <OfferToRaise
              key={`${row.name}:${row.not_kept}`}
              offer={row.limit}
              action="Save to the pool and ask again"
              onRaised={() => void askProfiles(pool, plane)}
            />
          ),
        )}
      {signingRow !== undefined && (
        <PoolSignIn
          pool={pool}
          profile={signingRow.name}
          kind={signingRow.kind}
          harness={signingRow.harness}
          provider={plan}
          onEnd={ended}
          onStanding={setStrip}
          onTerminal={
            plane === undefined
              ? undefined
              : () => {
                  ended(false);
                  // The harness's browser flow in a terminal has no way back to the runner, so
                  // a login approved in the browser runs its device flow there.
                  signIn(signingRow, plan, approvesIn(signingRow.kind, plan) || undefined);
                }
          }
        />
      )}
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
