// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import {
  type TabMemoryLook,
  artifactTabBudgetKb,
  artifactTabVerdict,
  tabMemoryLook,
} from "../artifactTabMemory.js";
import { built } from "../harness.js";
import { appProcesses, logLine, sample, webAndHarnessMemory } from "../processes.js";
import { focusWorkspace } from "../reach.js";
import { closeOf, iconFor } from "../artifactTabs.js";

/**
 * **What an open artifact tab costs in memory: ADR 0086's row M8** (ADR 0122 §10, #5958), in
 * the built app on macOS and Linux, in `scenario tests`.
 *
 * It looks at the app with no artifact tab open, opens one script-free page pinned to a
 * workspace, waits for the page's own web content process and for the memory to settle, and
 * looks again. The difference is held to this platform's `artifactTabBudgetKb` (`artifactTabMemory.ts`,
 * whose tests show the verdict failing past it), and both looks go to `logs/artifact-tab.jsonl`
 * and the run's output, so a green run says what it measured too.
 *
 * It leaves the window as it found it: the tab it opens it closes, and the files it writes it
 * removes.
 */

const app = built(process.platform === "win32" ? "purlis-app.exe" : "purlis-app");
const NAME = "Memory page";
const FOLDER = "memory-page";
const PAGE = `<!doctype html><title>${NAME}</title><h1>Open issues: 12</h1>`;
const PINS = `version = 1
pin = [
  { id = "e2e000000m08", folder = "${FOLDER}", name = "${NAME}", icon = "chart", order = 1 },
]
`;
/** Two looks this close together, a second apart, are a settled reading. */
const SETTLED_KB = 2 * 1024;

/** What the app answered a command with, insisting it answered at all. */
async function ask<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer = await browser.executeAsync(
    (
      name: string,
      passed: Record<string, unknown>,
      done: (out: { ok?: unknown; trouble?: string }) => void,
    ) => {
      void window.__TAURI__.core
        .invoke(name, passed)
        .then((ok) => done({ ok }))
        .catch((e: unknown) => done({ trouble: String(e) }));
    },
    command,
    args,
  );
  if (answer.trouble !== undefined) throw new Error(`${command} refused: ${answer.trouble}`);
  return answer.ok as T;
}

/** The labels of the artifact webviews the app holds now. */
async function artifactWebviews(): Promise<string[]> {
  const all = await ask<{ label: string }[]>("plugin:webview|get_all_webviews");
  return all.map((one) => one.label).filter((label) => /^artifact-\d+$/.test(label));
}

/** The app's window process: one for the whole run. */
function theApp(): number {
  const { windows } = appProcesses(app);
  if (windows.length !== 1) {
    throw new Error(`expected one app process, found ${windows.length}: ${windows.join(", ")}`);
  }
  return windows[0];
}

/** One look at the app at `pid`, for M8. */
function look(pid: number): TabMemoryLook {
  const own = sample(pid);
  if (own === null) throw new Error(`the app (pid ${pid}) is gone`);
  return tabMemoryLook(own.rssKb, webAndHarnessMemory(pid), process.platform);
}

const total = (seen: TabMemoryLook) => seen.appKb + seen.webContentKb;

/** Where `settledLook` has got to, kept in one place its wait can write. */
interface Looking {
  last: TabMemoryLook | null;
  settled: TabMemoryLook | null;
  missed: unknown;
  gone: boolean;
}

/**
 * A look once two in a row, a second apart, agree within `SETTLED_KB` and `ready` holds.
 *
 * A look that fails is one more second to wait, not the verdict: `footprint` can miss a web
 * content process that `ps` still lists, one starting or exiting (#6232). Two good looks in a
 * row are still needed, and a look failing until the timeout fails the test with its own words.
 * The app itself gone fails it at once.
 */
async function settledLook(
  pid: number,
  ready: (seen: TabMemoryLook) => boolean,
  what: string,
): Promise<TabMemoryLook> {
  const looking: Looking = { last: null, settled: null, missed: null, gone: false };
  try {
    await browser.waitUntil(
      () => {
        if (sample(pid) === null) {
          looking.gone = true;
          return true;
        }
        let now: TabMemoryLook;
        try {
          now = look(pid);
        } catch (error) {
          looking.missed = error;
          looking.last = null;
          return false;
        }
        const before = looking.last;
        const steady = before !== null && Math.abs(total(now) - total(before)) <= SETTLED_KB;
        looking.last = now;
        if (steady && ready(now)) looking.settled = now;
        return looking.settled !== null;
      },
      { timeout: 60_000, interval: 1_000 },
    );
  } catch {
    // The last look says which half failed: totals still moving, or a page with no web content
    // process of its own (one WebKit had started before the tab, say), main run 43516240829.
    const seen = looking.last === null ? "" : `; the last look: ${JSON.stringify(looking.last)}`;
    const why = looking.missed === null ? "" : `; the last look failed: ${String(looking.missed)}`;
    throw new Error(`the app's memory never settled ${what}${seen}${why}`);
  }
  if (looking.gone || looking.settled === null) throw new Error(`the app (pid ${pid}) is gone`);
  return looking.settled;
}

describe("an open artifact tab's memory (ADR 0086 M8)", function () {
  this.timeout(180_000);

  let artifacts = "";

  before(async () => {
    const plane = (await ask<string[]>("open_planes"))[0];
    artifacts = join(plane, "workspaces", "alpha", "artifacts");
    mkdirSync(join(artifacts, FOLDER), { recursive: true });
    writeFileSync(join(artifacts, FOLDER, "index.html"), PAGE);
    writeFileSync(join(artifacts, "pins.toml"), PINS);
    await focusWorkspace("alpha");
  });

  after(async () => {
    const close = await $(closeOf(NAME));
    if (await close.isExisting()) {
      await close.click();
      await browser.waitUntil(async () => (await artifactWebviews()).length === 0, {
        timeout: 20_000,
        timeoutMsg: "the artifact's webview outlived its tab",
      });
    }
    if (artifacts !== "") rmSync(artifacts, { recursive: true, force: true });
  });

  it("adds no more than its budget once its page is drawn", async () => {
    const icon = await iconFor(NAME);
    expect(await artifactWebviews()).toEqual([]);
    const pid = theApp();

    const before = await settledLook(pid, () => true, "before the tab opened");
    await icon.click();
    await browser.waitUntil(async () => (await artifactWebviews()).length === 1, {
      timeout: 20_000,
      timeoutMsg: "no webview of its own was laid over the artifact's frame",
    });
    const after = await settledLook(
      pid,
      (seen) => seen.webContentProcesses > before.webContentProcesses,
      `with the tab open, or its page started no web content process of its own (before it: ${before.webContentProcesses})`,
    );

    const budgetKb = artifactTabBudgetKb(process.platform);
    const verdict = artifactTabVerdict(before, after, budgetKb);
    const record = { row: "M8", budgetKb, before, after, ...verdict };
    logLine("artifact-tab.jsonl", record);
    console.log(`purlis-artifact-tab ${JSON.stringify(record)}`);
    if (!verdict.within) throw new Error(verdict.said);
  });
});
