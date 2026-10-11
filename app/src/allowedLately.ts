/**
 * **What was allowed here in the last day** (#1700, D-1700-7): the empty Inbox's look back past
 * what was answered in it while the window was open. Read from the grants each source keeps
 * already (`sandbox_grants`, `dispatch_grants`), so an Allow on a pane's Notice and one before a
 * relaunch are listed too. Read-only: each is revoked in Settings, where the grants are listed
 * whole. Only what was granted on this machine (a grant a teammate committed has its `by`), and
 * only what says when.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type DispatchGrant,
  type GrantLevel,
  type PlaneId,
  type SandboxGrant,
} from "./bindings";

/** How far back the list looks: a day, as the Inbox keeps its updates. */
export const LATELY_MS = 24 * 60 * 60 * 1000;

/** The most it lists. */
export const MOST_LATELY = 10;

/** One grant, as the list draws it. */
export type AllowedLately = {
  key: string;
  /** When, in ms since the epoch. */
  at: number;
  /** The chat it was allowed from, where the grant knows: a name, as data. */
  chat: string | null;
  /** What it allows, in one line: the host, folder or persona is the grant's, as data. */
  says: string;
  /** Who it is allowed for. */
  level: string;
};

/** Who a grant is for, as the list says it. */
const FOR: Readonly<Record<GrantLevel, string>> = {
  chat: "for that chat",
  you: "for you on this machine",
  project: "for everyone in this project",
};

function ofSandbox(grant: SandboxGrant): AllowedLately | undefined {
  if (grant.by !== null || grant.at === null) return undefined;
  const says =
    grant.what === "host"
      ? `Reach ${grant.target}`
      : grant.what === "write"
        ? `Write in ${grant.target}`
        : undefined;
  if (says === undefined) return undefined;
  return {
    key: `sandbox:${grant.id}`,
    at: grant.at * 1000,
    chat: grant.chat,
    says,
    level: FOR[grant.level],
  };
}

function ofDispatch(grant: DispatchGrant): AllowedLately | undefined {
  if (grant.by !== null || grant.at === null || grant.declined) return undefined;
  return {
    key: `dispatch:${grant.id}`,
    at: grant.at * 1000,
    chat: grant.chat,
    says: `${grant.asking ?? "A chat on no persona"} dispatches to ${grant.target}`,
    level: FOR[grant.level],
  };
}

/** **The grants of the last day, newest first**, from what the two sources answered. */
export function lately(
  sandbox: readonly SandboxGrant[],
  dispatch: readonly DispatchGrant[],
  now: number,
): AllowedLately[] {
  return [...sandbox.map(ofSandbox), ...dispatch.map(ofDispatch)]
    .filter((one): one is AllowedLately => one !== undefined && now - one.at < LATELY_MS)
    .sort((one, other) => other.at - one.at || one.key.localeCompare(other.key))
    .slice(0, MOST_LATELY);
}

/** The time now, in ms. */
const now = () => Date.now();

/** What `plane` allowed in the last day, read while `reading` (the Inbox is empty); nothing
 *  where a source cannot be read, which is said by its absence and never as an error here. */
export function useAllowedLately(plane: PlaneId, reading: boolean): readonly AllowedLately[] {
  const [listed, setListed] = useState<{ plane: PlaneId; list: AllowedLately[] }>();
  useEffect(() => {
    if (!reading) return;
    let gone = false;
    const list = <T>(answer: { status: "ok"; data: T } | { status: "error" }) =>
      answer.status === "ok" ? answer.data : undefined;
    void Promise.all([
      commands.sandboxGrants(plane).catch(() => ({ status: "error" as const })),
      commands.dispatchGrants(plane).catch(() => ({ status: "error" as const })),
    ]).then(([sandbox, dispatch]) => {
      if (gone) return;
      const grants = list(sandbox);
      const dispatched = list(dispatch);
      setListed({
        plane,
        list: lately(
          Array.isArray(grants) ? grants : [],
          Array.isArray(dispatched?.grants) ? dispatched.grants : [],
          now(),
        ),
      });
    });
    return () => {
      gone = true;
    };
  }, [plane, reading]);
  return listed?.plane === plane ? listed.list : [];
}
