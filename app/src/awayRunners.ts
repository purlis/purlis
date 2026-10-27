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
 *
 * **What the runner itself recorded ending there** joins its section (#3393): each run of a
 * chat that ended on the runner while the person was away, as the window read it over the link
 * (`RunnerAway.ended`), said at its time in the person's time zone, as finished, failed or
 * stopped: only a run that completed is counted finished. A run of a chat whose task is counted
 * already is not said twice. **A runner whose link is down is said so** ("its link is down, so
 * what happened there is not known yet"), never left out as one where nothing happened, and one
 * some of whose record was not read says that too.
 */
import type { RunEnded, RunnerAway, RunnerLogRead } from "./bindings";
import { whileAway, type Away, type AwayItem, type AwaySummaryOf } from "./awayCounts";

/** One runner's section: its pool, and what it counts. */
export type RunnerSection = {
  pool: string;
  /** How the window reads what it records now. */
  log: RunnerLogRead;
  finished: readonly AwayItem[];
  failed: readonly AwayItem[];
  stopped: readonly AwayItem[];
  waiting: readonly AwayItem[];
  branches: readonly AwayItem[];
};

/** The parts of a runner's section, in the order they are said. */
export type RunnerPartKind = "finished" | "failed" | "stopped" | "waiting" | "branches";

export const RUNNER_PARTS: readonly RunnerPartKind[] = [
  "finished",
  "failed",
  "stopped",
  "waiting",
  "branches",
];

/** This machine's summary, and a section for each runner something happened on. */
export type AwayWhole = { here: AwaySummaryOf; runners: readonly RunnerSection[] };

/** How many things a runner's section counts: its branches are said only beside the rest, and
 *  a runner whose link is down is one thing to say. */
export function countedOnRunners(runners: readonly RunnerSection[]): number {
  return runners.reduce((sum, one) => sum + happenedOn(one) + (one.log === "link_down" ? 1 : 0), 0);
}

/** What happened on a runner while away, as its section counts it. */
function happenedOn(
  section: Pick<RunnerSection, "finished" | "failed" | "stopped" | "waiting">,
): number {
  return (
    section.finished.length +
    section.failed.length +
    section.stopped.length +
    section.waiting.length
  );
}

/** How a run's end is said after its chat's name. */
const ENDED_SAID: Record<RunEnded, string> = {
  completed: "finished",
  failed: "failed",
  stopped: "was stopped",
};

/** A time, as the person's clock says it. */
function timeHere(at: number): string {
  return new Date(at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
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
export function awayByRunner(
  summary: AwaySummaryOf,
  runners: readonly RunnerAway[],
  { away, nameOf }: { away: readonly Away[]; nameOf: (session: number) => string },
): AwayWhole {
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
      const tasks = summary.done.filter(there);
      const failed = summary.failed.filter(there);
      const waiting = summary.waiting.filter(there);
      // A chat whose task is counted already is not said again for its run's end.
      const counted = new Set(
        [...tasks, ...failed].map(chatOf).filter((chat): chat is number => chat !== null),
      );
      const runs = one.ended
        .filter((run) => whileAway(away, run.at * 1000) && !counted.has(run.chat))
        .map((run) => ({
          ended: run.ended,
          item: {
            key: `run:${run.device}:${run.chat}:${run.at}`,
            says: `${nameOf(run.chat)} ${ENDED_SAID[run.ended]} at ${timeHere(run.at * 1000)}`,
            go: { to: "chat", session: run.chat },
          } satisfies AwayItem,
        }));
      const ran = (ended: RunEnded) =>
        runs.filter((run) => run.ended === ended).map((run) => run.item);
      const finished = [...tasks, ...ran("completed")];
      const failedHere = [...failed, ...ran("failed")];
      const stopped = ran("stopped");
      const happened = finished.length + failedHere.length + stopped.length + waiting.length > 0;
      const branches = happened
        ? one.branches.flatMap((clone) =>
            clone.branches
              .filter((branch) => branch.ahead > 0)
              .map((branch): AwayItem => ({
                key: `branch:${clone.workspace}/${clone.repo}/${branch.branch}`,
                says: `${branch.branch} in ${clone.repo}, ${branch.ahead} ${
                  branch.ahead === 1 ? "commit" : "commits"
                } ahead`,
                go: { to: "changes", workspace: clone.workspace },
              })),
          )
        : [];
      return {
        pool: one.pool,
        log: one.log,
        finished,
        failed: failedHere,
        stopped,
        waiting,
        branches,
      };
    })
    .filter((one) => happenedOn(one) > 0 || one.log === "link_down");
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
  const parts = runnerPartsSaid(section)
    .map((one) => one.says)
    .join(", ");
  return `On ${section.pool}'s runner: ${parts}${runnerNote(section, parts !== "")}`;
}

/** What a section says of how what its runner records was read, after its parts (`afterParts`)
 *  or alone: where its link is down, that; where some of it was not read, that. */
export function runnerNote(section: RunnerSection, afterParts: boolean): string {
  const and = afterParts ? ", and " : "";
  if (section.log === "link_down")
    return afterParts
      ? `${and}its link is down, so what else happened there is not known yet`
      : "its link is down, so what happened there is not known yet";
  if (section.log === "partly") return `${and}some of what happened there was not read`;
  return "";
}
