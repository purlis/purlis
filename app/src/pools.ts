// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands, type PoolAdding, type PoolLink, type PoolsShown } from "./bindings";

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
  /** The pool being removed, while it is. */
  removing: string | undefined;
  /** Why the last removal was refused, or what it left, in the core's words. */
  said: string | undefined;
};

const START: Held = {
  read: { state: "reading" },
  adding: false,
  links: {},
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
    held = { ...START, adding: held.adding };
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
  set((was) => ({ ...was, adding: false, said: undefined }));
  await rereadPools();
  return undefined;
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

/** Removes pool `name`, its runner, its container and its home. */
export async function removePool(name: string) {
  if (held.removing !== undefined) return;
  set((was) => ({ ...was, removing: name, said: undefined }));
  const said = await caught(commands.removeDockerPool(name));
  set((was) => {
    const links = { ...was.links };
    Reflect.deleteProperty(links, name);
    return {
      ...was,
      removing: undefined,
      links,
      said: said.status === "error" ? said.error : (said.data ?? undefined),
    };
  });
  await rereadPools();
}
