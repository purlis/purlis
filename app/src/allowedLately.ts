// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What was allowed here in the last day** (#1700, D-1700-7): the empty Inbox's look back past
 * what was answered in it while the window was open. Read from the grants each source keeps
 * already (`sandbox_grants`, `dispatch_grants`), so an Allow on a pane's Notice and one before a
 * relaunch are listed too. Read-only: each is revoked in Settings, where the grants are listed
 * whole. Only what was granted on this machine (a grant a teammate committed has its `by`), and
 * only what says when. Every name in it is drawn safely (`sandboxBlocks.inertly`, I-1): a chat
 * named the folder or host it was allowed.
 *
 * **And the person's Keep blocked answers** (#1700): a no grants nothing, so no grant lists it;
 * the network record keeps each one (`sandbox_kept_blocked`), and it is listed here beside the
 * Allows, so the look back shows both answers a host ask has.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type DispatchGrant,
  type GrantLevel,
  type KeptBlocked,
  type PlaneId,
  type SandboxGrant,
} from "./bindings";
import { inertly } from "./sandboxBlocks";

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
  once: "once, for that chat",
  chat: "for that chat",
  you: "for you on this machine",
  machine: "for every project on this machine",
  project: "for everyone in this project",
};

function ofSandbox(grant: SandboxGrant): AllowedLately | undefined {
  if (grant.by !== null || grant.at === null) return undefined;
  const says =
    grant.what === "host"
      ? `Reach ${inertly(grant.target)}`
      : grant.what === "write"
        ? `Write in ${inertly(grant.target)}`
        : grant.what === "socket"
          ? `Connect to ${inertly(grant.target)}`
          : grant.what === "service"
            ? `Reach the service ${inertly(grant.target)}`
            : grant.what === "port"
              ? `Connect to the service on this Mac at port ${inertly(grant.target)}`
              : grant.what === "listen"
                ? `Listen on port ${inertly(grant.target)} of this Mac`
                : undefined;
  if (says === undefined) return undefined;
  return {
    key: `sandbox:${grant.id}`,
    at: grant.at * 1000,
    chat: grant.chat === null ? null : inertly(grant.chat),
    says,
    level: FOR[grant.level],
  };
}

function ofDispatch(grant: DispatchGrant): AllowedLately | undefined {
  if (grant.by !== null || grant.at === null || grant.declined) return undefined;
  return {
    key: `dispatch:${grant.id}`,
    at: grant.at * 1000,
    chat: grant.chat === null ? null : inertly(grant.chat),
    says: `${grant.asking === null ? "A chat on no persona" : inertly(grant.asking)} dispatches to ${inertly(grant.target)}`,
    level: FOR[grant.level],
  };
}

function ofKept(kept: KeptBlocked): AllowedLately {
  return {
    key: `kept:${kept.at}:${kept.target}:${kept.chat ?? ""}`,
    at: kept.at * 1000,
    chat: kept.chat === null ? null : inertly(kept.chat),
    says: `Kept ${inertly(kept.target)} blocked`,
    level: FOR.chat,
  };
}

/** **The answers of the last day, newest first**, from what the sources answered: the grants,
 *  and the Keep blocked answers the network record keeps. */
export function lately(
  sandbox: readonly SandboxGrant[],
  dispatch: readonly DispatchGrant[],
  now: number,
  kept: readonly KeptBlocked[] = [],
): AllowedLately[] {
  return [...sandbox.map(ofSandbox), ...dispatch.map(ofDispatch), ...kept.map(ofKept)]
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
    void Promise.all([
      commands.sandboxGrants(plane).catch(() => ({ status: "error" as const })),
      commands.dispatchGrants(plane).catch(() => ({ status: "error" as const })),
      commands.sandboxKeptBlocked(plane).catch(() => ({ status: "error" as const })),
    ]).then(([sandbox, dispatch, keptBlocked]) => {
      if (gone) return;
      const grants = sandbox.status === "ok" ? sandbox.data : undefined;
      const dispatched = dispatch.status === "ok" ? dispatch.data : undefined;
      const kept = keptBlocked.status === "ok" ? keptBlocked.data : undefined;
      setListed({
        plane,
        list: lately(
          Array.isArray(grants) ? grants : [],
          Array.isArray(dispatched?.grants) ? dispatched.grants : [],
          now(),
          Array.isArray(kept) ? kept : [],
        ),
      });
    });
    return () => {
      gone = true;
    };
  }, [plane, reading]);
  return listed?.plane === plane ? listed.list : [];
}
