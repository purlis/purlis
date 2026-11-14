// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { $, browser, expect } from "@wdio/globals";
import {
  type LookTaken,
  type TabMemoryLook,
  artifactTabBudgetKb,
  artifactTabVerdict,
  pageProcesses,
  tabSteady,
  tabMemoryLook,
  unsettledSaid,
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
/**
 * Two looks of the same processes, a second apart, whose app and page agree this closely are a
 * settled reading (`tabSteady`).
 */
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

/** A look that read, with the web content processes it counted. */
type Seen = { look: TabMemoryLook; pids: number[] };

/** One look at the app at `pid`, for M8. */
function look(pid: number): Seen {
  const own = sample(pid);
  if (own === null) throw new Error(`the app (pid ${pid}) is gone`);
  const web = webAndHarnessMemory(pid);
  return { look: tabMemoryLook(own.rssKb, web, process.platform), pids: web.webContentPids };
}

/** Where `settledLook` has got to, kept in one place its wait can write. */
interface Looking {
  last: TabMemoryLook | null;
  settled: Seen | null;
  taken: LookTaken[];
  /** Whether two good looks in a row ever agreed, `ready` or not. */
  heldStill: boolean;
  gone: boolean;
}

/**
 * A look once two in a row, a second apart, are steady by `tabSteady` and `ready` holds: the
 * same web content processes, by pid, with the app's memory and the page's (its processes
 * `wanted.before` did not hold) within `SETTLED_KB`. The window's own web content and processes
 * earlier pages left are not held to the margin (#6589).
 *
 * A look that fails is one more second to wait, not the verdict: `footprint` can miss a web
 * content process that `ps` still lists, one starting or exiting (#6232). Two good looks in a
 * row are still needed, and a look failing until the timeout fails the test with its own words.
 * The app itself gone fails it at once. A wait that times out says what it saw: the look
 * `before`, where there was one, and its last looks, each with the pids it counted
 * (`unsettledSaid`). It says apart the two ways to time out (#6589): the memory never held
 * still, or it held but `ready` never did, said as `unready`.
 */
async function settledLook(
  pid: number,
  what: string,
  wanted: { ready: (seen: TabMemoryLook) => boolean; unready: string; before: Seen } | null,
): Promise<Seen> {
  const ready = wanted?.ready ?? (() => true);
  const looking: Looking = { last: null, settled: null, taken: [], heldStill: false, gone: false };
  try {
    await browser.waitUntil(
      () => {
        if (sample(pid) === null) {
          looking.gone = true;
          return true;
        }
        let now: Seen;
        try {
          now = look(pid);
        } catch (error) {
          looking.taken.push({ missed: String(error) });
          looking.last = null;
          return false;
        }
        looking.taken.push(now);
        const previous = looking.last;
        const steady =
          previous !== null &&
          tabSteady(previous, now.look, wanted?.before.look ?? null, SETTLED_KB);
        looking.last = now.look;
        if (steady) looking.heldStill = true;
        if (steady && ready(now.look)) looking.settled = now;
        return looking.settled !== null;
      },
      { timeout: 60_000, interval: 1_000 },
    );
  } catch {
    const verdict =
      looking.heldStill && wanted !== null
        ? `the app's memory settled ${what}, but ${wanted.unready}`
        : `the app's memory never settled ${what}`;
    throw new Error(`${verdict}; ${unsettledSaid(wanted?.before ?? null, looking.taken)}`);
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

    const seenBefore = await settledLook(pid, "before the tab opened", null);
    const before = seenBefore.look;
    await icon.click();
    await browser.waitUntil(async () => (await artifactWebviews()).length === 1, {
      timeout: 20_000,
      timeoutMsg: "no webview of its own was laid over the artifact's frame",
    });
    const { look: after } = await settledLook(pid, "with the tab open", {
      ready: (seen) => pageProcesses(before, seen).length > 0,
      unready: `no settled look held a web content process the look before did not (pid ${seenBefore.pids.join(", ") || "none"}): its page started none of its own`,
      before: seenBefore,
    });

    const budgetKb = artifactTabBudgetKb(process.platform);
    const verdict = artifactTabVerdict(before, after, budgetKb);
    const record = { row: "M8", budgetKb, before, after, ...verdict };
    logLine("artifact-tab.jsonl", record);
    console.log(`purlis-artifact-tab ${JSON.stringify(record)}`);
    if (!verdict.within) throw new Error(verdict.said);
  });
});
