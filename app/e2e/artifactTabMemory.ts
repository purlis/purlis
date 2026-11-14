// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { WebAndHarnessMemory } from "./processes.js";

/**
 * **What an open artifact tab may cost in memory: ADR 0086's row M8** (ADR 0122 §10, #5958).
 *
 * An artifact is drawn in a native webview of its own (ADR 0122 §3), so opening its tab adds a
 * web content process to the app and a little to the window. The row is the two together: the
 * web content processes' memory and the app's resident memory after the tab has opened and its
 * page settled, less before. On macOS web content is counted by `footprint`, on Linux by resident
 * memory, as M2 is (`processes.ts`), so each system is its own measure.
 *
 * A value per platform (ADR 0086, amended 2026-11-10): macOS keeps the initial 192 MB, set
 * before anything was measured; Linux, whose web content is counted by resident memory, is held
 * to its most measured, 238 MB in four `ci` runs on 2026-11-10, with about a tenth more. Every
 * run prints what it measures (`logs/artifact-tab.jsonl`), from which the row's owner may lower
 * either.
 */
const BUDGET_KB = { darwin: 192 * 1024, linux: 264 * 1024 } as const;

/** M8's budget on `platform`, as `process.platform` names it: Linux's own, else macOS's. */
export function artifactTabBudgetKb(platform: string): number {
  return platform === "linux" ? BUDGET_KB.linux : BUDGET_KB.darwin;
}

/** One look at the app, for M8: the window's own memory and its web content's. */
export interface TabMemoryLook {
  /** The app's resident memory, in kilobytes. */
  appKb: number;
  /** The web content processes' memory, in kilobytes: footprint on macOS, resident on Linux. */
  webContentKb: number;
  webContentProcesses: number;
  /** Each web content process the look counted, by pid, with its own kilobytes. */
  webContentEach: { pid: number; kb: number }[];
}

/**
 * The look M8 takes, from the app's resident memory `appKb` and what `webAndHarnessMemory`
 * read on `platform`. A macOS look `footprint` could not read is refused in its own words,
 * never taken as resident memory, which reads far low there.
 */
export function tabMemoryLook(
  appKb: number,
  web: WebAndHarnessMemory,
  platform: string,
): TabMemoryLook {
  if (platform === "darwin") {
    if (web.webContentFootprintKb === null || web.footprintError !== null) {
      throw new Error(
        `no footprint of the web content: ${web.footprintError ?? "footprint gave no reading"}`,
      );
    }
    return {
      appKb,
      webContentKb: web.webContentFootprintKb,
      webContentProcesses: web.webContentProcesses,
      webContentEach: web.webContentEach,
    };
  }
  return {
    appKb,
    webContentKb: web.webContentKb,
    webContentProcesses: web.webContentProcesses,
    webContentEach: web.webContentEach,
  };
}

/** One web content process a look counted. */
type Process = { pid: number; kb: number };

const pidsOf = (seen: TabMemoryLook) => new Set(seen.webContentEach.map((one) => one.pid));

/**
 * The web content processes `seen` holds that `before`, the look before the tab opened, did
 * not: the tab's page's own, by pid (#6589). Never by count: WebKit ends a process an earlier
 * page left (a cached or prewarmed one) when it likes, so a new process and one gone keep the
 * count where it was, and the page's process would never be found.
 */
export function pageProcesses(before: TabMemoryLook, seen: TabMemoryLook): Process[] {
  const old = pidsOf(before);
  return seen.webContentEach.filter((one) => !old.has(one.pid));
}

/** What the tab can move in `seen`: the app's own memory and its page's processes. */
function ownKb(seen: TabMemoryLook, before: TabMemoryLook | null): number {
  const page = before === null ? [] : pageProcesses(before, seen);
  return page.reduce((sum, one) => sum + one.kb, seen.appKb);
}

/**
 * Whether two looks a second apart, `last` then `now`, are a settled reading of the tab opened
 * since `before` (`null`: of the app before any tab opened). They are when they saw the same web
 * content processes, by pid, and what the tab can move (the app's own memory and its page's
 * processes) agrees within `settledKb`.
 *
 * The window's own web content, the app's interface, and processes earlier pages left are not
 * held to the margin (#6589): the loop of #6592 measured the window's moving 3 to 5 MB between
 * looks a second apart in a quarter of its pairs, with nothing opened or closed, while the
 * page's process and the app held within 1 MB from the second look on. The verdict still counts
 * whatever those processes grew by.
 */
export function tabSteady(
  last: TabMemoryLook,
  now: TabMemoryLook,
  before: TabMemoryLook | null,
  settledKb: number,
): boolean {
  const was = pidsOf(last);
  const same =
    was.size === now.webContentEach.length && now.webContentEach.every((one) => was.has(one.pid));
  return same && Math.abs(ownKb(now, before) - ownKb(last, before)) <= settledKb;
}

/** What M8 says of one open tab. */
export interface TabMemoryVerdict {
  addedKb: number;
  appAddedKb: number;
  webContentAddedKb: number;
  processesAdded: number;
  within: boolean;
  /** The verdict in a sentence, for the failure and the log. */
  said: string;
}

const megabytes = (kb: number) => `${Math.round(kb / 1024)} MB`;

/**
 * Whether the tab opened between `before` and `after` kept within `budgetKb`.
 *
 * Web content is counted by pid (#6589): the page's new processes whole, and what each process
 * both looks saw grew by. A process `before` held that has exited by `after` is neither the
 * tab's cost nor a saving of it, so it is left out and named in the verdict.
 *
 * **A page drawn in no web content process of its own fails**: the row's measure is that
 * process, and ADR 0122 §3 chose the native webview because it runs the page apart from the
 * window. With none added, nothing was measured, and a pass would say nothing.
 */
export function artifactTabVerdict(
  before: TabMemoryLook,
  after: TabMemoryLook,
  budgetKb: number,
): TabMemoryVerdict {
  const was = new Map(before.webContentEach.map((one) => [one.pid, one.kb]));
  const page = pageProcesses(before, after);
  const grown = after.webContentEach
    .filter((one) => was.has(one.pid))
    .reduce((sum, one) => sum + one.kb - (was.get(one.pid) ?? 0), 0);
  const living = pidsOf(after);
  const gone = before.webContentEach.filter((one) => !living.has(one.pid));
  const appAddedKb = after.appKb - before.appKb;
  const webContentAddedKb = page.reduce((sum, one) => sum + one.kb, grown);
  const addedKb = appAddedKb + webContentAddedKb;
  const processesAdded = page.length;
  const exited =
    gone.length === 0
      ? ""
      : `; left out, as an earlier page's: ${gone.map((one) => `pid ${one.pid} exited`).join(", ")}`;
  const parts = `${megabytes(webContentAddedKb)} of web content and ${megabytes(appAddedKb)} of the app${exited}`;
  const measured = { addedKb, appAddedKb, webContentAddedKb, processesAdded };
  if (processesAdded < 1) {
    return {
      ...measured,
      within: false,
      said: `the artifact's page was drawn in no web content process of its own, so its memory was not measured (${parts})`,
    };
  }
  const within = addedKb <= budgetKb;
  return {
    ...measured,
    within,
    said: `an open artifact tab added ${megabytes(addedKb)} (${parts}); its budget is ${megabytes(budgetKb)}`,
  };
}

/** One look `settledLook` took, a second after the one before: what it read, or why it failed. */
export type LookTaken = { look: TabMemoryLook; pids: number[] } | { missed: string };

/** How many of the last looks a failure names. */
export const LOOKS_SAID = 5;

const lookSaid = (taken: LookTaken) =>
  "missed" in taken
    ? `failed: ${taken.missed}`
    : `${taken.look.webContentProcesses} web content processes (pid ${taken.pids.join(", ") || "none"}), ${megabytes(total(taken.look))}`;

const total = (seen: TabMemoryLook) => seen.appKb + seen.webContentKb;

/**
 * What a look that never settled says of what it saw: the look before the tab opened, where
 * there was one, and the last `LOOKS_SAID` looks, oldest first. A wait for a web content
 * process of the tab's own that never came then says whether the count never rose (the page
 * was drawn in a process already counted) or rose while another went, and a wait for the
 * memory to settle says how far apart its looks stayed.
 */
export function unsettledSaid(
  before: { look: TabMemoryLook; pids: number[] } | null,
  looks: LookTaken[],
): string {
  const last = looks.slice(-LOOKS_SAID);
  const parts: string[] = [];
  if (before !== null) parts.push(`before the tab opened: ${lookSaid(before)}`);
  parts.push(
    last.length === 0
      ? "no look was taken"
      : `its last ${last.length} looks, a second apart: ${last.map(lookSaid).join("; ")}`,
  );
  return parts.join("; ");
}
