// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Said } from "./actions";
import { clonesInAll } from "./repoClones";

/**
 * **What an action answered, in the person's words** (#3344, the sidebar-views prototype's Q3 A).
 *
 * The window once joined the core's CLI lines into the line under the title bar, word for word:
 * "Workspace 'git-ci-cd' ready (LIVE — charter + manifest + memory committed + shared +
 * auto-saved) → workspaces/git-ci-cd/ ⬢ No vision yet — ask the developer what this workspace is
 * for…". The second of those is written for an agent in a terminal, not for the person. Now a
 * known result is said here, in the app's own words: a short title, one plain sentence, its ways
 * on, and the core's lines kept behind Details, less every line written for an agent.
 */

/** What a result's notice draws: its title, its one sentence, and what the core said. */
export type Result = { title: string; says: string | undefined; details: string | undefined };

/** **A line the core writes for an agent**, never shown to the person: its `⬢` lines, which tell
 *  the agent reading a terminal what to ask its developer. */
function forAnAgent(line: string): boolean {
  return line.trimStart().startsWith("⬢") || /\bask the developer\b/i.test(line);
}

/** The core's lines a person may read: every one but those written for an agent. */
export function forThePerson(lines: readonly string[]): string[] {
  return lines.filter((line) => line.trim() !== "" && !forAnAgent(line));
}

/** **`words` less every sentence written for an agent**: from a `⬢` to the end of its line, or
 *  a line that asks the developer something. The core joins its lines with a space, so a `⬢`
 *  mid-line starts one. */
export function plainWords(words: string): string {
  return forThePerson(words.split(/\n|(?=⬢)/))
    .map((part) => part.trim())
    .join(" ");
}

/**
 * **What a result's notice draws, for any `Said`**: its own title where it has one, else its
 * words; and never a line the core wrote for an agent, whichever result it came with (#3344).
 */
export function resultOf(said: Omit<Said, "from"> & { from?: string }): Result {
  const words = plainWords(said.words);
  const kept = said.details === undefined ? [] : forThePerson(said.details.split("\n"));
  const details = kept.length === 0 ? undefined : kept.join("\n");
  return said.title === undefined
    ? { title: words, says: undefined, details }
    : { title: said.title, says: words === "" ? undefined : words, details };
}

/** What the core says when a workspace is made with no vision (`wscmd/create.rs`). */
const NO_VISION = /no vision yet/i;

/**
 * **A workspace made** (`purlis workspace create`, `wscmd/create.rs`): "Workspace git-ci-cd is
 * ready", and whether it has a vision yet, with Add a vision while it has none; the core's own
 * lines in its details. It is about the workspace it made, so it hangs from that workspace's chip
 * (`noticePlace.ts`). Making a workspace opens it (#3648), so it offers no Open: that has
 * already happened. A pin the store refused is said after the sentence: the workspace is made
 * all the same.
 */
export function workspaceMade(
  name: string,
  lines: readonly string[],
  { onAddVision, pinRefused }: { onAddVision: () => void; pinRefused?: string },
): Said {
  const none = lines.some((line) => NO_VISION.test(line));
  const vision = none
    ? "It has no vision yet. Add one so its chats know what it is for."
    : "Its vision is recorded in its workspace.md.";
  const kept = forThePerson(lines);
  return {
    from: "workspace.create",
    refused: false,
    title: `Workspace ${name} is ready`,
    words: pinRefused === undefined ? vision : `${vision} ${pinRefused}`,
    details: kept.length === 0 ? undefined : kept.join("\n"),
    ways: none ? [{ label: "Add a vision", onPress: onAddVision, primary: true }] : [],
    workspace: name,
  };
}

/** **A result as one line says it** (the palette's, #3344): the notice's title and sentence,
 *  and never a line the core wrote for an agent. */
export function saidInPlainWords(said: Omit<Said, "from"> & { from?: string }): string {
  const { title, says } = resultOf(said);
  return says === undefined ? title : `${title.replace(/[.!?]$/, "")}. ${says}`;
}

/** **The track a new workspace's clone is on** (#3649): its progress, then its result, are one
 *  notice on that workspace's chip, which each line updates in place. `run` is which clone of
 *  that name it is: a track ends with its run, so a workspace made again under the same name
 *  starts a notice of its own (#3649's review). A Retry stays on its run's track. */
export const cloneTrack = (workspace: string, run: number) => `clone:${workspace}:${run}`;

/** "1 repo", "2 repos". */
const reposCounted = (count: number) => (count === 1 ? "1 repo" : `${count} repos`);

/**
 * **A new workspace's repos cloning** (#3649, spec #3647): "Cloning 2 repos…", which ones into
 * where, and how many have landed so far. On its workspace's chip, and on the clone's track, so
 * each repo that lands updates it in place and its result takes its place. Its bar says how far
 * the batch has got across its repos, in one percent, and its Cancel stops the clone (#4079).
 */
export function reposCloning(
  workspace: string,
  repos: readonly string[],
  done: number,
  run: number,
  { moved, onCancel }: { moved?: { stage: string; percent: number }; onCancel?: () => void } = {},
): Said {
  const into = `${repos.join(", ")} into ${workspace}.`;
  return {
    from: "workspace.clone",
    refused: false,
    title: `Cloning ${reposCounted(repos.length)}…`,
    words: done === 0 ? into : `${into} ${done} of ${repos.length} done.`,
    progress: true,
    meter: {
      label: moved?.stage ?? "Connecting",
      percent: clonesInAll(done, repos.length, moved?.percent ?? 0),
    },
    ways: onCancel === undefined ? [] : [{ label: "Cancel", onPress: onCancel }],
    workspace,
    track: cloneTrack(workspace, run),
  };
}

/** **A new workspace's repos, cloned** (#3649): the clone's notice, turned into its result. */
export function reposCloned(workspace: string, repos: readonly string[], run: number): Said {
  return {
    from: "workspace.clone",
    refused: false,
    title: `Cloned ${reposCounted(repos.length)}`,
    words: `${repos.join(", ")} into ${workspace}.`,
    workspace,
    track: cloneTrack(workspace, run),
  };
}

/**
 * **A new workspace's repos that did not clone** (#3649): which, in the core's own words, with
 * Retry, which clones those again on the same track, and the way to the workspace's Repos
 * settings (#1201, SE-22), or to Accounts where a sign-in is what it needs (#4079). A warning, so
 * the chip's dot is ▲ while it is hidden.
 */
export function reposNotCloned(
  workspace: string,
  failed: readonly { repo: string; said: string; signIn?: string }[],
  onRetry: () => void,
  run: number,
): Said {
  const signIn = failed.some((one) => one.signIn !== undefined);
  return {
    from: "workspace.clone",
    refused: true,
    title: `Could not clone ${failed.map((one) => one.repo).join(", ")} into ${workspace}`,
    words: failed.map((one) => one.said).join(" "),
    ways: [{ label: "Retry", onPress: onRetry, primary: true }],
    settings: signIn
      ? { label: "Open accounts", link: { group: "you.accounts" } }
      : {
          label: `Open ${workspace}'s repo settings`,
          link: { group: "workspace.repos", workspace },
        },
    workspace,
    track: cloneTrack(workspace, run),
  };
}

/** **A clone the person stopped** (#4079): the clone's notice, turned into that, with Retry. */
export function reposStopped(
  workspace: string,
  repos: readonly string[],
  onRetry: () => void,
  run: number,
): Said {
  return {
    from: "workspace.clone",
    refused: false,
    title: `Stopped cloning ${repos.join(", ")}`,
    words: "Nothing was kept. Retry starts again.",
    ways: [{ label: "Retry", onPress: onRetry, primary: true }],
    workspace,
    track: cloneTrack(workspace, run),
  };
}
