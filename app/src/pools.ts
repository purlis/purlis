// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import {
  commands,
  type PoolAdding,
  type PoolLink,
  type PoolProfileRow,
  type PoolVersion,
  type PoolsShown,
} from "./bindings";

/**
 * **This machine's Docker pools, as the window holds them** (#2679): one read, shared by the
 * Pools settings group, the palette and the Add a Docker pool dialog, and read again after every
 * add and removal, so what is drawn is what the machine keeps. Nothing about pools shows while
 * none is kept: the group is offered only once one is ({@link anyPoolKept}), and the palette's
 * Add a Docker pool… is the way to the first.
 *
 * **Each pool's runner is asked once whether it answers with the key pinned for it**
 * ({@link askLink}), as the group comes on screen: a runner whose key changed gets a Notice with
 * Remove.
 *
 * **Each pool's runner is asked its version too** ({@link askVersion}, #2749): a row whose
 * runner runs another version than this machine says so.
 *
 * **Each pool's runner keeps this machine's profiles** ({@link askProfiles}, #2049): adding a
 * pool ends with them, each to sign in there, and the profile page says where each is signed in.
 */
type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: PoolsShown };

export type Held = {
  read: Read;
  /** Whether the Add a Docker pool dialog is open. */
  adding: boolean;
  /** What each pool's runner answered, or that it is being asked, by the pool's name. */
  links: Readonly<Record<string, PoolLink | "asking">>;
  /** Which version each pool's runner runs, by the pool's name, once it said (#2749). */
  versions: Readonly<Record<string, PoolVersion>>;
  /** This machine's profiles as each pool's runner keeps them, by the pool's name (#2049): the
   *  rows, that they are being asked, or why they could not be. */
  profiles: Readonly<Record<string, PoolProfiles>>;
  /** The pool just added, whose profiles the dialog offers to sign in there, until Done. */
  added: string | undefined;
  /** The pool being removed, while it is. */
  removing: string | undefined;
  /** Why the last removal was refused, or what it left, in the core's words. */
  said: string | undefined;
};

const START: Held = {
  read: { state: "reading" },
  adding: false,
  links: {},
  versions: {},
  profiles: {},
  added: undefined,
  removing: undefined,
  said: undefined,
};
let held: Held = START;
const listeners = new Set<() => void>();
/** The newest read out: an answer to an older one is dropped. */
let reading = 0;

function set(change: (was: Held) => Held) {
  held = change(held);
  for (const one of listeners) one();
}

type Said<T> = { status: "ok"; data: T } | { status: "error"; error: string };

function caught<T>(sent: Promise<Said<T>>): Promise<Said<T>> {
  return sent.catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
}

/** Reads this machine's pools again. */
export async function rereadPools() {
  const mine = ++reading;
  const said = await caught(commands.dockerPools());
  if (mine !== reading) return;
  set((was) => ({
    ...was,
    read:
      said.status === "error"
        ? { state: "trouble", trouble: said.error }
        : // A core that answered nothing (an older one, or none) is said as trouble, never drawn.
          typeof said.data === "object" && said.data !== null && Array.isArray(said.data.pools)
          ? { state: "read", shown: said.data }
          : { state: "trouble", trouble: "purlis did not say which pools this machine keeps." },
  }));
}

function subscribe(listener: () => void) {
  if (listeners.size === 0) {
    held = { ...START, adding: held.adding, added: held.added };
    void rereadPools();
  }
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function usePools(): Held {
  return useSyncExternalStore(subscribe, () => held);
}

/** Whether this machine keeps a pool, as last read: the palette offers the Pools group only
 *  then. */
export function anyPoolKept(): boolean {
  return held.read.state === "read" && held.read.shown.pools.length > 0;
}

/** Opens the Add a Docker pool dialog. */
export function startAddingAPool() {
  set((was) => ({ ...was, adding: true }));
}

/** Closes it. */
export function stopAddingAPool() {
  set((was) => ({ ...was, adding: false }));
}

/** Adds `adding` on this machine: `undefined` once it is added, else why not. */
export async function addPool(adding: PoolAdding): Promise<string | undefined> {
  const said = await caught(commands.addDockerPool(adding));
  if (said.status === "error") return said.error;
  set((was) => ({ ...was, adding: false, added: adding.name, said: undefined }));
  // Its profiles are asked by what it ends with ({@link askProfiles}, #2049).
  await rereadPools();
  return undefined;
}

/** What a pool's runner keeps of this machine's profiles, as last asked. */
export type PoolProfiles = PoolProfileRow[] | "asking" | { trouble: string };

/** Asks pool `name`'s runner to keep each of this machine's profiles, and where each stands
 *  there (#2049). */
export async function askProfiles(name: string) {
  set((was) => ({ ...was, profiles: { ...was.profiles, [name]: "asking" } }));
  const said = await caught(commands.poolProfiles(name));
  const rows: PoolProfiles =
    said.status === "error"
      ? { trouble: said.error }
      : Array.isArray(said.data)
        ? said.data
        : { trouble: "purlis did not say which profiles the runner keeps." };
  set((was) => ({ ...was, profiles: { ...was.profiles, [name]: rows } }));
}

/** Closes what adding a pool ended with: its profiles to sign in there. */
export function doneWithTheAddedPool() {
  set((was) => ({ ...was, added: undefined }));
}

/** Asks pool `name`'s runner whether it answers with the key pinned for it. */
export async function askLink(name: string) {
  set((was) => ({ ...was, links: { ...was.links, [name]: "asking" } }));
  const said = await caught(commands.dockerPoolLink(name));
  const link: PoolLink =
    said.status === "error"
      ? { state: "down", why: said.error }
      : typeof said.data === "object" && said.data !== null
        ? said.data
        : { state: "down", why: "purlis did not say whether the runner answered." };
  set((was) => ({ ...was, links: { ...was.links, [name]: link } }));
}

/** Asks pool `name`'s runner which version it runs (#2749). A runner that does not say is drawn
 *  as before: its link already says why it does not answer. */
export async function askVersion(name: string) {
  const said = await caught(commands.dockerPoolVersion(name));
  if (said.status === "error" || typeof said.data !== "object" || said.data === null) return;
  const version = said.data;
  set((was) => ({ ...was, versions: { ...was.versions, [name]: version } }));
}

/** Removes pool `name`, its runner, its container and its home; answers what could not be
 *  removed, or why nothing was, in purlis's or Docker's words already shown safely. */
export async function removePool(name: string): Promise<string | undefined> {
  if (held.removing !== undefined)
    return `purlis is removing pool ${held.removing}: remove ${name} once it is gone.`;
  set((was) => ({ ...was, removing: name, said: undefined }));
  const said = await caught(commands.removeDockerPool(name));
  const left = said.status === "error" ? said.error : (said.data ?? undefined);
  set((was) => {
    const links = { ...was.links };
    Reflect.deleteProperty(links, name);
    const versions = { ...was.versions };
    Reflect.deleteProperty(versions, name);
    const profiles = { ...was.profiles };
    Reflect.deleteProperty(profiles, name);
    return {
      ...was,
      removing: undefined,
      links,
      versions,
      profiles,
      said: left,
    };
  });
  await rereadPools();
  return left;
}
