// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";
import { commands, type PlaneId } from "./bindings";

/**
 * **The repos this window is cloning into a workspace, one at a time** (ADR 0055).
 *
 * A workspace is made at once and its repos land after it, so the operator can start a chat
 * while they clone. Each repo is one `clone_repo`, asked in turn: that is what lets each one's
 * own state be drawn as it lands and a failed one be asked again.
 *
 * **One clone at a time per workspace, whoever asked** (#1215). Every call joins ONE chain per
 * project and workspace, so two rows pressed a second apart wait for each other rather than
 * run side by side: no two clones race to write the workspace's manifest, the inventory's
 * read-modify-write, or the layer `clone` wires into a sibling still being cloned. A repo
 * already waiting or cloning is not asked for again. The state is this window's, for as long
 * as it is open — what is actually cloned is read from disk (`workspace_repos`), never from
 * here.
 */

export type CloneState =
  | { state: "waiting" }
  | { state: "cloning" }
  /** `said` is what `clone` answered, in its own words, for a caller that has to quote it. */
  | { state: "cloned"; said?: string }
  | { state: "failed"; said: string };

type Clones = ReadonlyMap<string, CloneState>;

const EMPTY: Clones = new Map();
const byWorkspace = new Map<string, Clones>();
/** The tail of each workspace's chain: the next call starts when this settles. */
const chains = new Map<string, Promise<unknown>>();
/** **How the clone of each repo now under way will end** (#4003), by workspace and repo: the
 *  run that owns it settles it, whatever later asks set the shared state to meanwhile. */
const ends = new Map<string, Map<string, Promise<CloneState>>>();

/** How a repo ends that the run which had it never reached: it stopped part way. */
const NOT_FINISHED = (repo: string): Extract<CloneState, { state: "failed" }> => ({
  state: "failed",
  said: `${repo} was not finished cloning. Ask for it again.`,
});
const listeners = new Set<() => void>();

function keyOf(plane: PlaneId, workspace: string): string {
  return `${plane}\u0000${workspace}`;
}

function set(plane: PlaneId, workspace: string, repo: string, state: CloneState) {
  const key = keyOf(plane, workspace);
  const next = new Map(byWorkspace.get(key) ?? EMPTY);
  next.set(repo, state);
  byWorkspace.set(key, next);
  for (const listener of listeners) listener();
}

/** Each repo that could not be cloned, with charter's own sentence about it. */
export type Failed = { repo: string; said: string }[];

/** Whether a repo's clone is asked for and not finished: waiting its turn, or running. */
export function underWay(state: CloneState | undefined): boolean {
  return state?.state === "waiting" || state?.state === "cloning";
}

/**
 * Add `repos` to the plane's inventory, then clone them into `workspace` one after another,
 * after any clone this window already asked for there. A repo already under way is left to
 * the call that asked first. `onEach` hears each repo as it finishes, so a list can be read
 * again as it lands rather than once the batch ends.
 *
 * **A clone that answered is checked against the workspace before it counts** (V91b): if the
 * workspace's panels still list the repo as not cloned, it is a failure — with a Retry wherever
 * failures are drawn — and never a "Cloned" row with nothing to press. Here, so every caller
 * gets it: the explorer's rows, a new workspace's repos and Settings › Repos.
 *
 * Resolves with the ones that failed, once every one has been tried.
 */
export function cloneRepos(
  plane: PlaneId,
  workspace: string,
  repos: string[],
  onEach?: (repo: string, state: CloneState) => void,
): Promise<Failed> {
  const key = keyOf(plane, workspace);
  const mine = repos.filter((repo) => !underWay(byWorkspace.get(key)?.get(repo)));
  // A repo another call is already cloning is not asked for again, but it is waited on: this
  // call answers with how that call's clone of it ended (#3649's review), read from the end that
  // call recorded, not from the shared state, which a later ask may have set again (#4003).
  const theirs = repos.flatMap((repo) =>
    mine.includes(repo) ? [] : [{ repo, end: ends.get(key)?.get(repo) }],
  );
  const settle = new Map<string, (state: CloneState) => void>();
  const owned = ends.get(key) ?? new Map<string, Promise<CloneState>>();
  ends.set(key, owned);
  for (const repo of mine) {
    set(plane, workspace, repo, { state: "waiting" });
    owned.set(repo, new Promise((resolve) => settle.set(repo, resolve)));
  }
  const run = (chains.get(key) ?? Promise.resolve()).then(async () => {
    const failed = await cloneInTurn(plane, workspace, mine, settle, onEach);
    const waited: Failed = [];
    for (const { repo, end } of theirs) {
      // A repo with no end recorded was not this window's to wait on: as it stands.
      const state = end === undefined ? byWorkspace.get(key)?.get(repo) : await end;
      if (state?.state === "failed") waited.push({ repo, said: state.said });
      else if (underWay(state)) waited.push({ repo, said: NOT_FINISHED(repo).said });
    }
    return [...failed, ...waited];
  });
  chains.set(
    key,
    run.catch(() => undefined),
  );
  return run;
}

async function cloneInTurn(
  plane: PlaneId,
  workspace: string,
  repos: string[],
  settle: ReadonlyMap<string, (state: CloneState) => void>,
  onEach?: (repo: string, state: CloneState) => void,
): Promise<Failed> {
  const done = new Set<string>();
  /** A repo's end: its state, told to the calls waiting on it. */
  const end = (repo: string, state: CloneState) => {
    set(plane, workspace, repo, state);
    done.add(repo);
    settle.get(repo)?.(state);
  };
  try {
    if (repos.length === 0) return [];
    const taken = await commands
      .takeRepos(plane, repos)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    // A pick the inventory could not take still reaches `clone`, which says in its own words
    // what it could not find — one sentence per repo, where the operator is looking.
    const failed: Failed = [];
    for (const repo of repos) {
      set(plane, workspace, repo, { state: "cloning" });
      const answer = await commands
        .cloneRepo(plane, workspace, repo)
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
      const cloned: CloneState =
        answer.status === "ok"
          ? { state: "cloned", said: answer.data.join(" ") }
          : {
              state: "failed",
              said: taken.status === "error" ? `${answer.error}\n${taken.error}` : answer.error,
            };
      const state = cloned.state === "cloned" ? await seen(plane, workspace, repo, cloned) : cloned;
      end(repo, state);
      if (state.state === "failed") failed.push({ repo, said: state.said });
      onEach?.(repo, state);
    }
    return failed;
  } finally {
    // **Stopped part way** (#4003): a repo this run never reached is not left waiting for as
    // long as the window is open, which would make every later ask answer "not finished". It
    // ends failed, so asking again clones it.
    for (const repo of repos) if (!done.has(repo)) end(repo, NOT_FINISHED(repo));
  }
}

/**
 * A clone that answered, as the workspace now reads: still `cloned`, or `failed` when the
 * workspace's panels still list the repo as not cloned here. A read that does not answer
 * leaves the clone as it said — the panels read again will draw what is there.
 */
async function seen(
  plane: PlaneId,
  workspace: string,
  repo: string,
  cloned: Extract<CloneState, { state: "cloned" }>,
): Promise<CloneState> {
  const read = await commands
    .workspacePanels(plane, workspace)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  // `data` can be absent from an answer the window was not built against (a test's core), and
  // that reads as "nothing to say", never as a failure.
  const absent =
    read.status === "ok" ? ((read.data as Partial<typeof read.data> | null)?.absent ?? []) : [];
  if (!absent.includes(repo)) return cloned;
  return {
    state: "failed",
    said:
      `${repo} was cloned, but purlis does not see it in ${workspace}.` +
      (cloned.said ? ` purlis said: ${cloned.said}` : ""),
  };
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** What this window has asked to clone into `workspace`, by repo. */
export function useRepoClones(plane: PlaneId, workspace: string): Clones {
  return useSyncExternalStore(subscribe, () => byWorkspace.get(keyOf(plane, workspace)) ?? EMPTY);
}

/**
 * Forget what this window knows about repos that are no longer missing from `workspace` — the
 * ones `absent` no longer names and that finished. A `cloned` repo stays busy until the list
 * read from disk drops it; forgetting it then means a folder deleted later can be cloned again.
 */
export function settleRepoClones(plane: PlaneId, workspace: string, absent: readonly string[]) {
  const key = keyOf(plane, workspace);
  const held = byWorkspace.get(key);
  if (held === undefined) return;
  const next = new Map(
    [...held].filter(([repo, one]) => one.state !== "cloned" || absent.includes(repo)),
  );
  if (next.size === held.size) return;
  byWorkspace.set(key, next);
  for (const listener of listeners) listener();
}

/** For tests: forget every clone. */
export function forgetRepoClones() {
  byWorkspace.clear();
  chains.clear();
  ends.clear();
}
