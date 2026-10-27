// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **While you were away: a section per runner** (#2051, spec #2041 story 15).
 *
 * A person coming back to chats that ran on a pool's runner overnight reads one place, not a
 * flood: the summary says what this machine's chats did, as it always has, and then, for each
 * runner something happened on, **what finished there, what waits for them, and the branches
 * ready to push**: "On qa's runner: 2 finished, 1 waiting on you, 1 branch ready to push".
 *
 * What it moves is what `awayCounts.ts` counted: a finished task goes to the runner its own
 * chat runs on, and a chat waiting on you to the runner it runs on (`RunnerAway.chats`, as the
 * window routed each). What is not a runner's stays this machine's. A runner's branches are said
 * only beside something that happened on it while away, so a branch left unpushed for a week
 * does not bring the summary back each morning; every one of them is in Changes whatever this
 * says. Plain inputs and no state, like `awayCounts.ts`.
 */
import type { RunnerAway } from "./bindings";
import type { AwayItem, AwaySummaryOf } from "./awayCounts";

/** One runner's section: its pool, and what it counts. */
export type RunnerSection = {
  pool: string;
  finished: readonly AwayItem[];
  failed: readonly AwayItem[];
  waiting: readonly AwayItem[];
  branches: readonly AwayItem[];
};

/** The parts of a runner's section, in the order they are said. */
export type RunnerPartKind = "finished" | "failed" | "waiting" | "branches";

export const RUNNER_PARTS: readonly RunnerPartKind[] = [
  "finished",
  "failed",
  "waiting",
  "branches",
];

/** This machine's summary, and a section for each runner something happened on. */
export type AwayWhole = { here: AwaySummaryOf; runners: readonly RunnerSection[] };

/** How many things a runner's section counts: its branches are said only beside the rest. */
export function countedOnRunners(runners: readonly RunnerSection[]): number {
  return runners.reduce(
    (sum, one) => sum + one.finished.length + one.failed.length + one.waiting.length,
    0,
  );
}

/** The chat a counted item is about: a finished task's own chat, or a waiting chat. */
function chatOf(item: AwayItem): number | null {
  if (item.go.to === "chat") return item.go.session;
  if (item.go.to === "finished") return item.go.task.chat;
  return null;
}

/**
 * **`summary`, split by where each chat runs**: what a runner's chats did goes to that runner's
 * section, in the order `runners` names them, and the rest stays this machine's.
 */
export function awayByRunner(summary: AwaySummaryOf, runners: readonly RunnerAway[]): AwayWhole {
  if (runners.length === 0) return { here: summary, runners: [] };
  const poolOf = new Map<number, string>();
  for (const one of runners) for (const chat of one.chats) poolOf.set(chat, one.pool);
  const onRunner = (item: AwayItem) => {
    const chat = chatOf(item);
    return chat === null ? undefined : poolOf.get(chat);
  };
  const here: AwaySummaryOf = {
    done: summary.done.filter((item) => onRunner(item) === undefined),
    failed: summary.failed.filter((item) => onRunner(item) === undefined),
    waiting: summary.waiting.filter((item) => onRunner(item) === undefined),
    refused: summary.refused,
  };
  const sections = runners
    .map((one): RunnerSection => {
      const there = (item: AwayItem) => onRunner(item) === one.pool;
      const finished = summary.done.filter(there);
      const failed = summary.failed.filter(there);
      const waiting = summary.waiting.filter(there);
      const happened = finished.length + failed.length + waiting.length > 0;
      const branches = happened
        ? one.branches
            .filter((branch) => branch.ahead > 0)
            .map((branch): AwayItem => ({
              key: `branch:${branch.workspace}/${branch.repo}/${branch.branch}`,
              says: `${branch.branch} in ${branch.repo}, ${branch.ahead} ${
                branch.ahead === 1 ? "commit" : "commits"
              } ahead`,
              go: { to: "changes", workspace: branch.workspace },
            }))
        : [];
      return { pool: one.pool, finished, failed, waiting, branches };
    })
    .filter((one) => one.finished.length + one.failed.length + one.waiting.length > 0);
  return { here, runners: sections };
}

/** What each part of a runner's section says: `2 finished`, `1 waiting on you`. */
export function runnerPartsSaid(section: RunnerSection): { part: RunnerPartKind; says: string }[] {
  const said: { part: RunnerPartKind; says: string }[] = [];
  for (const part of RUNNER_PARTS) {
    const n = section[part].length;
    if (n === 0) continue;
    const words =
      part === "waiting"
        ? "waiting on you"
        : part === "branches"
          ? `${n === 1 ? "branch" : "branches"} ready to push`
          : part;
    said.push({ part, says: `${n} ${words}` });
  }
  return said;
}

/** The section's sentence, as a screen reader and a test read it. */
export function runnerSaid(section: RunnerSection): string {
  return `On ${section.pool}'s runner: ${runnerPartsSaid(section)
    .map((one) => one.says)
    .join(", ")}`;
}
