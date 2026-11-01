// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Channel } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { commands, type CloneProgress, type CloneRefused, type PlaneId } from "./bindings";

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
  /** `stage` and `percent` are how far it has got, once git has said (#4079). */
  | { state: "cloning"; stage?: string; percent?: number }
  /** `said` is what `clone` answered, in its own words, for a caller that has to quote it. */
  | { state: "cloned"; said?: string }
  /** The person stopped it, and nothing of it was kept. */
  | { state: "stopped" }
  /** `signIn` is the server the person needs to sign in to, where that is why. */
  | { state: "failed"; said: string; signIn?: string };

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
/** How a repo ends that the person stopped. */
const STOPPED = (repo: string) => `${repo} was stopped.`;
/** **The repos the person stopped** (#4079), by workspace: one still waiting is never started. */
const stopping = new Map<string, Set<string>>();
const listeners = new Set<() => void>();

function keyOf(plane: PlaneId, workspace: string): string {
  return `${plane}\u0000${workspace}`;
}

function set(plane: PlaneId, workspace: string, repo: string, state: CloneState) {
  const key = keyOf(plane, workspace);
  const next = new Map(byWorkspace.get(key) ?? EMPTY);
  next.set(repo, state);
  byWorkspace.set(key, next);
  tell();
}

/**
 * **Tells every listener of the store a change, each on its own** (#4177, #4178's review): one
 * that throws neither stops the rest from hearing it nor throws out of the store, where it could
 * stop a run before it is chained or ended and leave a repo waiting for good. Its error is
 * thrown again once they have all heard, in a microtask of its own, so it is still reported.
 */
function tell() {
  for (const listener of listeners) {
    try {
      listener();
    } catch (err: unknown) {
      queueMicrotask(() => {
        throw err;
      });
    }
  }
}

/**
 * Each repo that could not be cloned, with charter's own sentence about it. `stopped` is the
 * person's own Cancel rather than a failure; `signIn` the server they need to sign in to.
 */
export type Failed = { repo: string; said: string; stopped?: true; signIn?: string }[];

/** A clone's stage and percent, as a row says them: "Downloading 45%". Nothing until git says. */
export function cloningWords(state: CloneState | undefined): string {
  if (state?.state !== "cloning" || state.stage === undefined) return "";
  return `${state.stage} ${state.percent ?? 0}%`;
}

/** **A batch's overall percent** (#4079): `done` repos of `total` landed, the next `percent` in. */
export function clonesInAll(done: number, total: number, percent: number): number {
  return Math.floor((done * 100 + percent) / total);
}

/**
 * **Stops those repos' clones** (#4079): the person's Cancel. git is stopped, what it made is
 * taken away by the core, and a repo still waiting its turn is never started. A repo stopped ends
 * `stopped`, so it can be asked for again.
 */
export function stopRepoClones(plane: PlaneId, workspace: string, repos: readonly string[]) {
  const key = keyOf(plane, workspace);
  const asked = stopping.get(key) ?? new Set<string>();
  stopping.set(key, asked);
  for (const repo of repos) {
    const state = byWorkspace.get(key)?.get(repo);
    if (!underWay(state)) continue;
    asked.add(repo);
    if (state?.state === "cloning")
      void commands.cancelCloneRepo(plane, workspace, repo).catch(() => undefined);
  }
}

/** What a refused clone said: `clone_repo`'s answer, or a sentence where Tauri itself refused. */
function refusedWith(error: CloneRefused | string): { said: string; signIn?: string } {
  if (typeof error === "string") return { said: error };
  return error.sign_in === null || error.sign_in === undefined
    ? { said: error.said }
    : { said: error.said, signIn: error.sign_in };
}

/** Whether a repo's clone is asked for and not finished: waiting its turn, or running. */
export function underWay(state: CloneState | undefined): boolean {
  return state?.state === "waiting" || state?.state === "cloning";
}

/**
 * Add `repos` to the plane's inventory, then clone them into `workspace` one after another,
 * after any clone this window already asked for there. A repo already under way is left to
 * the call that asked first. `onEach` hears each repo as it finishes, so a list can be read
 * again as it lands rather than once the batch ends, and `onMove` hears how far the running one
 * has got (#4079).
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
  onMove?: (repo: string, now: CloneProgress) => void,
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
    const failed = await cloneInTurn(plane, workspace, mine, settle, onEach, onMove);
    const waited: Failed = [];
    for (const { repo, end } of theirs) {
      // A repo with no end recorded was not this window's to wait on: as it stands.
      const state = end === undefined ? byWorkspace.get(key)?.get(repo) : await end;
      if (state?.state === "failed") waited.push(failure(repo, state));
      else if (state?.state === "stopped")
        waited.push({ repo, said: STOPPED(repo), stopped: true });
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

/** A failed repo's line of a call's answer. */
function failure(repo: string, state: Extract<CloneState, { state: "failed" }>): Failed[number] {
  return state.signIn === undefined
    ? { repo, said: state.said }
    : { repo, said: state.said, signIn: state.signIn };
}

async function cloneInTurn(
  plane: PlaneId,
  workspace: string,
  repos: string[],
  settle: ReadonlyMap<string, (state: CloneState) => void>,
  onEach?: (repo: string, state: CloneState) => void,
  onMove?: (repo: string, now: CloneProgress) => void,
): Promise<Failed> {
  const key = keyOf(plane, workspace);
  const done = new Set<string>();
  /** A repo's end: its state, told to the calls waiting on it. A stop asked for it ends with
   *  its run (#4176): one that came while the clone was being checked, or for a repo the run
   *  never reached, is not kept to stop the next ask. */
  const end = (repo: string, state: CloneState) => {
    stopping.get(key)?.delete(repo);
    // Recorded and told before the store hears it (#4177): a listener of the store that throws
    // must never leave a call waiting on this repo unanswered, or the workspace's chain behind it.
    done.add(repo);
    settle.get(repo)?.(state);
    set(plane, workspace, repo, state);
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
      let answer:
        { status: "ok"; data: string[] | null } | { status: "error"; error: CloneRefused | string };
      if (stopping.get(key)?.has(repo)) {
        answer = { status: "ok", data: null };
      } else {
        set(plane, workspace, repo, { state: "cloning" });
        const progress = new Channel<CloneProgress>();
        let answered = false;
        progress.onmessage = (now) => {
          // A line that arrives after the answer must not make the repo cloning again.
          if (answered) return;
          set(plane, workspace, repo, { state: "cloning", ...now });
          onMove?.(repo, now);
        };
        answer = await commands
          .cloneRepo(plane, workspace, repo, progress)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        answered = true;
      }
      // The stop mark, read above, is cleared by `end()`, the one place every way ends a repo.
      if (answer.status === "ok" && answer.data === null) {
        end(repo, { state: "stopped" });
        failed.push({ repo, said: STOPPED(repo), stopped: true });
        onEach?.(repo, { state: "stopped" });
        continue;
      }
      const refused = answer.status === "error" ? refusedWith(answer.error) : undefined;
      const cloned: CloneState =
        answer.status === "ok"
          ? { state: "cloned", said: (answer.data ?? []).join(" ") }
          : {
              state: "failed",
              ...refused,
              said:
                taken.status === "error"
                  ? `${refused?.said ?? ""}\n${taken.error}`
                  : (refused?.said ?? ""),
            };
      const state = cloned.state === "cloned" ? await seen(plane, workspace, repo, cloned) : cloned;
      end(repo, state);
      if (state.state === "failed") failed.push(failure(repo, state));
      onEach?.(repo, state);
    }
    return failed;
  } catch (err: unknown) {
    // **Stopped part way** (#4003): a repo this run never reached is not left waiting for as
    // long as the window is open, which would make every later ask answer "not finished". It
    // ends failed, so asking again clones it; or stopped, where the person stopped it (#4178's
    // review). Then the run's own throw is said.
    for (const repo of repos.filter((one) => !done.has(one)))
      end(repo, stopping.get(key)?.has(repo) ? { state: "stopped" } : NOT_FINISHED(repo));
    throw err;
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

/** **The store's subscription**, as `useRepoClones` reads it: `listener` hears every change,
 *  until the function it answers is called. Exported as the tests' way to stand in a listener
 *  (D-4177-3); keep it so rather than folding it back into the hook. */
export function subscribeRepoClones(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** What this window has asked to clone into `workspace`, by repo. */
export function useRepoClones(plane: PlaneId, workspace: string): Clones {
  return useSyncExternalStore(
    subscribeRepoClones,
    () => byWorkspace.get(keyOf(plane, workspace)) ?? EMPTY,
  );
}

/**
 * Forget a clone that landed once `clones`, what the panels list as cloned here, names its repo.
 * Until then it stays busy, and has a row even in a workspace that did not name it. Forgetting it
 * then means a folder deleted later can be cloned again, and a repo dropped later leaves no row.
 */
export function settleRepoClones(plane: PlaneId, workspace: string, clones: readonly string[]) {
  const key = keyOf(plane, workspace);
  const held = byWorkspace.get(key);
  if (held === undefined) return;
  const next = new Map(
    [...held].filter(([repo, one]) => one.state !== "cloned" || !clones.includes(repo)),
  );
  if (next.size === held.size) return;
  byWorkspace.set(key, next);
  tell();
}

/** For tests: forget every clone. */
export function forgetRepoClones() {
  byWorkspace.clear();
  chains.clear();
  stopping.clear();
  ends.clear();
}
