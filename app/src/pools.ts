// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import {
  commands,
  type ClusterContexts,
  type ClusterPoolAdding,
  type ClusterPoolKey,
  type PoolAdding,
  type PoolLink,
  type PoolProfiles as PoolProfilesAnswered,
  type PoolVersion,
  type PoolsShown,
  type RunnerImageOffered,
} from "./bindings";

/**
 * **This machine's Docker pools, as the window holds them** (#2679): one read, shared by the
 * Pools settings group, the palette and the Add a pool dialog, and read again after every add
 * and removal, so what is drawn is what the machine keeps. The Pools group is always offered (the
 * person's ruling of 2026-10-30), and says what a pool is until one is kept; Devices and the
 * other runner surfaces are offered only once one is ({@link anyPoolKept}). Add a pool…, in the
 * group and in the palette, is the way to the first.
 *
 * **A new pool's image is the one this version's release names**, or a newer one's where an
 * update is waiting ({@link readReleaseImage}, the person's ruling of 2026-10-30), asked as the
 * dialog opens.
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
 *
 * **A cluster pool is added from the same dialog** (#3292): its kind chosen there, then its
 * kubeconfig's context and the cluster's Pool, then the cluster operator key that Pool names,
 * read ({@link checkClusterPoolKey}) and shown for the person to check before Add pool pins it
 * ({@link addClusterPool}). Its runner keeps none of this machine's profiles: it is a team's
 * cluster's, so adding one ends with nothing to sign in.
 */
type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; shown: PoolsShown };

export type Held = {
  read: Read;
  /** Whether the Add a pool dialog is open. */
  adding: boolean;
  /** Which kind of pool it adds, as last chosen there (#3292). */
  kind: PoolKind;
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

/** A pool's kind: a runner on this machine's Docker, or one in a team's cluster (#3292). */
export type PoolKind = "docker" | "cluster";

const START: Held = {
  read: { state: "reading" },
  adding: false,
  kind: "docker",
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
    held = { ...START, adding: held.adding, added: held.added, kind: held.kind };
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

/** Whether this machine keeps a pool of either kind, as last read: the palette offers Devices
 *  only then. */
export function anyPoolKept(): boolean {
  return (
    held.read.state === "read" &&
    held.read.shown.pools.length + (held.read.shown.clusters ?? []).length > 0
  );
}

/** Opens the add pool dialog, on a Docker pool unless `kind` says otherwise. */
export function startAddingAPool(kind: PoolKind = "docker") {
  set((was) => ({ ...was, adding: true, kind }));
}

/** Chooses which kind of pool the dialog adds (#3292). */
export function choosePoolKind(kind: PoolKind) {
  set((was) => ({ ...was, kind }));
}

/** Closes it. */
export function stopAddingAPool() {
  set((was) => ({ ...was, adding: false }));
}

/** The runner image a new pool is offered, as purlis said it, or why it could not say. */
export type ReleaseImage = RunnerImageOffered | { state: "trouble"; trouble: string };

/**
 * **The runner image a new pool is offered** (the person's ruling of 2026-10-30): the Add a
 * pool dialog's default image, which a pool added with the Image box empty is made from. This
 * version's own; on a machine with an update waiting on its channel, the newer version's, said
 * so; none from a local build or a release that names none. Asked as the dialog opens; the core
 * keeps this version's image once read.
 */
export async function readReleaseImage(): Promise<ReleaseImage> {
  const said = await caught(commands.runnerImageOfThisVersion());
  if (said.status === "error") return { state: "trouble", trouble: said.error };
  const offered = said.data as RunnerImageOffered | null;
  // A core that answered nothing it knows (an older one, or none) names no image.
  return typeof offered === "object" && offered !== null && typeof offered.state === "string"
    ? offered
    : { state: "none_named" };
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

/** The contexts of the kubeconfig `kubeconfig` names (empty for kubectl's own default), or
 *  why they could not be read (#3292). */
export async function readClusterContexts(
  kubeconfig: string,
): Promise<ClusterContexts | { trouble: string }> {
  const said = await caught(commands.clusterContexts(kubeconfig.trim() === "" ? null : kubeconfig));
  if (said.status === "error") return { trouble: said.error };
  return typeof said.data === "object" && said.data !== null && Array.isArray(said.data.contexts)
    ? said.data
    : { trouble: "purlis did not say which contexts the kubeconfig has." };
}

/** The cluster operator key the cluster's Pool `pool` names, through `context`, for the person to
 *  check (#3292), or why it could not be read. */
export async function checkClusterPoolKey(
  kubeconfig: string,
  context: string,
  pool: string,
): Promise<ClusterPoolKey | { trouble: string }> {
  const said = await caught(
    commands.clusterPoolKey(kubeconfig.trim() === "" ? null : kubeconfig, context, pool),
  );
  if (said.status === "error") return { trouble: said.error };
  return typeof said.data === "object" && said.data !== null && typeof said.data.key === "string"
    ? said.data
    : { trouble: "purlis did not say which key the cluster's pool names." };
}

/** Adds the cluster pool `adding`, pinning the key the person checked: `undefined` once it is
 *  added, else why not (#3292). It ends with nothing to sign in: its runner keeps no profile of
 *  this machine's. */
export async function addClusterPool(adding: ClusterPoolAdding): Promise<string | undefined> {
  const said = await caught(commands.addClusterPool(adding));
  if (said.status === "error") return said.error;
  set((was) => ({ ...was, adding: false, said: undefined }));
  await rereadPools();
  return undefined;
}

/** Marks cluster pool `name` as the person's own, or takes the mark back (#4071): `undefined`
 *  once it is kept, else why not. The window's alone. */
export async function markPoolOwned(name: string, owned: boolean): Promise<string | undefined> {
  const said = await caught(commands.setClusterPoolOwned(name, owned));
  await rereadPools();
  return said.status === "error" ? said.error : undefined;
}

/** What a pool's runner keeps of this machine's profiles, as last asked: the ones chosen for
 *  it and the others Add a profile… offers (#4047), that they are being asked, or why not. */
export type PoolProfiles = PoolProfilesAnswered | "asking" | { trouble: string };

/** Asks pool `name`'s runner to keep the profiles chosen for it, and where each stands there
 *  (#2049, #4047): a pool that names none keeps the default of the open project `plane`. */
export async function askProfiles(name: string, plane?: string) {
  set((was) => ({ ...was, profiles: { ...was.profiles, [name]: "asking" } }));
  const said = await caught(commands.poolProfiles(name, { plane: plane ?? null }));
  const rows: PoolProfiles =
    said.status === "error"
      ? { trouble: said.error }
      : typeof said.data === "object" && said.data !== null && Array.isArray(said.data.kept)
        ? said.data
        : { trouble: "purlis did not say which profiles the runner keeps." };
  set((was) => ({ ...was, profiles: { ...was.profiles, [name]: rows } }));
}

/** **Adds this machine's profile `profile` to pool `name`** (#4047, Add a profile…), then asks
 *  the pool's profiles again, so its runner keeps it. Why not, in the core's words. */
export async function addProfileToPool(
  name: string,
  profile: string,
  plane?: string,
): Promise<string | undefined> {
  const said = await caught(commands.poolProfileAdd(name, profile, { plane: plane ?? null }));
  await askProfiles(name, plane);
  return said.status === "error" ? said.error : undefined;
}

/** **Takes profile `profile` off pool `name`** (#4047): its runner forgets it, sign-in included,
 *  then the pool's profiles are asked again. What the core said of it, or why not. */
export async function removeProfileFromPool(
  name: string,
  profile: string,
  plane?: string,
): Promise<string | undefined> {
  const said = await caught(commands.poolProfileRemove(name, profile, { plane: plane ?? null }));
  await askProfiles(name, plane);
  return said.status === "error" ? said.error : (said.data ?? undefined);
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
