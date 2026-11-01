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
  type PoolLimitsPage,
  type PoolProfiles as PoolProfilesAnswered,
  type PoolRemoved,
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
 *
 * **A runner removed outside purlis is made again on the person's word** ({@link
 * makeRunnerAgain}, #4149): its row, and the Notice of a refused start, offer Make the runner
 * again, which ends as adding a pool does, with its profiles kept there again.
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
  /** Each pool's limits, by the pool's name, once read, or why they could not be (#4285). */
  limits: Readonly<Record<string, PoolLimitsPage | { trouble: string }>>;
  /** The pool just added, whose profiles the dialog offers to sign in there, until Done. */
  added: string | undefined;
  /** The pool being removed, while it is. */
  removing: string | undefined;
  /** The pool whose runner is being made again, while it is (#4149). */
  remaking: string | undefined;
  /** What Make the runner again did, by the pool's name, for its row to say (#4149). */
  remade: Readonly<Record<string, string>>;
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
  limits: {},
  added: undefined,
  removing: undefined,
  remaking: undefined,
  remade: {},
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

/** The pools whose profiles are being asked again quietly. */
const askingQuietly = new Set<string>();

/** How long a pool's profiles are shown before they are asked again while its runner fetches
 *  a profile's harness (#4008). */
export const ASK_AGAIN_WHILE_FETCHING_MS = 3000;

/** Asks pool `name`'s runner to keep the profiles chosen for it, and where each stands there
 *  (#2049, #4047): a pool that names none keeps the default of the open project `plane`. Asked
 *  `quietly`, the rows shown stay until the answer comes (#4008). */
export async function askProfiles(name: string, plane?: string, { quietly = false } = {}) {
  // One quiet ask of a pool at a time, however many places show its profiles.
  if (quietly && askingQuietly.has(name)) return;
  if (quietly) askingQuietly.add(name);
  else set((was) => ({ ...was, profiles: { ...was.profiles, [name]: "asking" } }));
  const asked = caught(commands.poolProfiles(name, { plane: plane ?? null }));
  const said = await asked.finally(() => {
    if (quietly) askingQuietly.delete(name);
  });
  const rows: PoolProfiles =
    said.status === "error"
      ? { trouble: said.error }
      : typeof said.data === "object" && said.data !== null && Array.isArray(said.data.kept)
        ? said.data
        : { trouble: "purlis did not say which profiles the runner keeps." };
  set((was) => ({ ...was, profiles: { ...was.profiles, [name]: rows } }));
}

/** The newest read of each pool's limits out: an answer to an older one is dropped. */
const readingLimits = new Map<string, number>();

/**
 * Reads pool `name`'s limits (#4285). What is shown stays until the answer comes, and stays if the
 * read fails, so a box being typed in isn't lost: the trouble is said only with nothing to show.
 */
export async function askLimits(name: string) {
  const mine = (readingLimits.get(name) ?? 0) + 1;
  readingLimits.set(name, mine);
  const said = await caught(commands.poolLimits(name));
  if (readingLimits.get(name) !== mine) return;
  const limits =
    said.status === "error"
      ? { trouble: said.error }
      : typeof said.data === "object" && said.data !== null && Array.isArray(said.data.groups)
        ? said.data
        : { trouble: "purlis did not say what this pool's limits are." };
  set((was) => {
    const shown = was.limits[name];
    if ("trouble" in limits && shown !== undefined && "groups" in shown) return was;
    return { ...was, limits: { ...was.limits, [name]: limits } };
  });
}

/** **The value in the words `typed` for limit `key`**, or why they aren't one: the core's check. */
export function checkLimit(key: string, typed: string) {
  return caught(commands.poolLimitCheck(key, typed));
}

/**
 * **Saves `values` to pool `name`'s limits**, a `null` returning that one to purlis's default
 * (#4285), then reads them again, so an open page shows them. Why not, in the core's words: those
 * before it stay saved.
 */
export async function saveLimits(
  name: string,
  values: ReadonlyArray<readonly [string, number | null]>,
): Promise<string | undefined> {
  let refused: string | undefined;
  for (const [key, value] of values) {
    const saved = await caught(commands.poolLimitSet(name, key, value));
    if (saved.status === "error") {
      refused = saved.error;
      break;
    }
  }
  await askLimits(name);
  return refused;
}

/** Pool `name`'s profiles as last asked, outside React (#4048). */
export function profilesHeld(name: string): PoolProfiles | undefined {
  return held.profiles[name];
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

/** **Signs profile `profile` out on pool `name`'s runner and its draining runners** (#4043,
 *  #4121): the harness's own sign-out, run there, then the pool's profiles are asked again. Why it
 *  did not, in the core's words, or what it did on the draining runners. */
export async function signProfileOutOfPool(
  name: string,
  profile: string,
  plane?: string,
): Promise<{ failed: string } | { draining: string | undefined }> {
  const said = await caught(commands.poolSignOut(name, profile));
  await askProfiles(name, plane);
  return said.status === "error" ? { failed: said.error } : { draining: said.data ?? undefined };
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
 *  removed, or why nothing was, in purlis's or Docker's words already shown safely. `elsewhere`
 *  is what the person was told: that its runner is on an engine Docker no longer reaches, so nothing
 *  there goes. Where that no longer holds, or the other way round, nothing is done (#4325). */
export async function removePool(name: string, elsewhere = false): Promise<string | undefined> {
  if (held.removing !== undefined)
    return `purlis is removing pool ${held.removing}: remove ${name} once it is gone.`;
  set((was) => ({ ...was, removing: name, said: undefined }));
  const said = await caught(commands.removeDockerPool(name, elsewhere));
  const left = said.status === "error" ? said.error : (said.data ?? undefined);
  set((was) => {
    const links = { ...was.links };
    Reflect.deleteProperty(links, name);
    const versions = { ...was.versions };
    Reflect.deleteProperty(versions, name);
    const profiles = { ...was.profiles };
    Reflect.deleteProperty(profiles, name);
    const limits = { ...was.limits };
    Reflect.deleteProperty(limits, name);
    return {
      ...was,
      removing: undefined,
      links,
      versions,
      profiles,
      limits,
      said: left,
    };
  });
  await rereadPools();
  return left;
}

/** **The Docker pools whose runner was removed outside purlis** (#4149): what a refused start's
 *  Notice offers to make again. None where purlis could not ask. */
export async function readRemovedPools(): Promise<readonly PoolRemoved[]> {
  const said = await caught(commands.dockerPoolsRemoved());
  return said.status === "ok" && Array.isArray(said.data) ? said.data : [];
}

/** What a Notice says of a runner removed outside purlis (#4149), by whether its home is still
 *  there. */
export function removedSaid(name: string, homeKept: boolean): string {
  return homeKept
    ? `Pool ${name}'s runner was removed outside purlis: Docker has no container of its id, so purlis no longer links to it. If Docker now points at another engine, point it back instead. Its home is still there: Make the runner again makes a runner from the pool's image and limits, with the sign-ins kept there.`
    : `Pool ${name}'s runner was removed outside purlis: Docker has no container of its id, so purlis no longer links to it. If Docker now points at another engine, point it back instead. Its home went too, and with it the sign-ins kept there: Make the runner again makes a runner from the pool's image and limits, whose profiles you sign in again.`;
}

/** What a row says of a runner made on another engine than Docker reaches now (#4166). */
export function anotherEngineSaid(name: string): string {
  return `Docker points at another engine than the one pool ${name}'s runner was made on. Point Docker back at that engine and its chats come back. If that engine is gone for good, remove the pool and add it again; anything still on that engine stays there.`;
}

/** What a row says once its runner was made again, by whether its sign-ins were carried. */
export function madeAgainSaid(name: string, signInsKept: boolean): string {
  return signInsKept
    ? `Pool ${name}'s runner was made again, with its sign-ins.`
    : `Pool ${name}'s runner was made again: sign its profiles in again there.`;
}

/**
 * **Make the runner again** (#4149): pool `name`'s runner, removed outside purlis, made again
 * from the pool's own image and limits, with the sign-ins of its home where that is still there,
 * and paired. Then
 * its link and version are asked again and it keeps the pool's profiles again, as after Add a
 * pool, for the open project `plane`. `undefined` once it is made, else why not.
 */
export async function makeRunnerAgain(name: string, plane?: string): Promise<string | undefined> {
  if (held.remaking !== undefined)
    return `purlis is making pool ${held.remaking}'s runner again: try ${name} once it is done.`;
  set((was) => ({ ...was, remaking: name }));
  const said = await caught(commands.dockerPoolMakeAgain(name));
  const why = said.status === "error" ? said.error : undefined;
  set((was) => {
    const remade = { ...was.remade };
    if (said.status === "ok") remade[name] = madeAgainSaid(name, said.data.sign_ins_kept);
    else Reflect.deleteProperty(remade, name);
    return { ...was, remaking: undefined, remade };
  });
  if (why !== undefined) return why;
  await Promise.all([askLink(name), askVersion(name), askProfiles(name, plane)]);
  return undefined;
}
