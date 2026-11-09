// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { WebAndHarnessMemory } from "./processes.js";

/**
 * **What an open artifact tab may cost in memory: ADR 0086's row M8** (ADR 0120 §10, #5958).
 *
 * An artifact is drawn in a native webview of its own (ADR 0120 §3), so opening its tab adds a
 * web content process to the app and a little to the window. The row is the two together: the
 * web content processes' memory and the app's resident memory after the tab has opened and its
 * page settled, less before. On macOS web content is counted by `footprint`, on Linux by resident
 * memory, as M2 is (`processes.ts`), so each system is its own measure.
 *
 * An initial value (ADR 0086, decided in drafting, 7): no measurement existed when it was set, and
 * the `scenario tests` runs on `main` print what they measure (`logs/artifact-tab.jsonl`), from
 * which the row's owner may lower it.
 */
export const ARTIFACT_TAB_BUDGET_KB = 192 * 1024;

/** One look at the app, for M8: the window's own memory and its web content's. */
export interface TabMemoryLook {
  /** The app's resident memory, in kilobytes. */
  appKb: number;
  /** The web content processes' memory, in kilobytes: footprint on macOS, resident on Linux. */
  webContentKb: number;
  webContentProcesses: number;
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
    };
  }
  return { appKb, webContentKb: web.webContentKb, webContentProcesses: web.webContentProcesses };
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
 * **A page drawn in no web content process of its own fails**: the row's measure is that
 * process, and ADR 0120 §3 chose the native webview because it runs the page apart from the
 * window. With none added, nothing was measured, and a pass would say nothing.
 */
export function artifactTabVerdict(
  before: TabMemoryLook,
  after: TabMemoryLook,
  budgetKb: number = ARTIFACT_TAB_BUDGET_KB,
): TabMemoryVerdict {
  const appAddedKb = after.appKb - before.appKb;
  const webContentAddedKb = after.webContentKb - before.webContentKb;
  const addedKb = appAddedKb + webContentAddedKb;
  const processesAdded = after.webContentProcesses - before.webContentProcesses;
  const parts = `${megabytes(webContentAddedKb)} of web content and ${megabytes(appAddedKb)} of the app`;
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
