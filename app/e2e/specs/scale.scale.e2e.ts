// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { browser } from "@wdio/globals";
import { appProcess, sleep } from "../bench/window.js";
import { LOGS } from "../processes.js";

/**
 * **What only the real app can measure on a project at scale** (#5497, spec #5486): the
 * sidebar's answer, the watcher's setup and a minute of idle CPU with the window shown, on the
 * project `wdio.scale.conf.ts` opened. Evidence only: each number is a row of
 * `$PURLIS_SCALE_OUT`, which `tools/scale-report.mjs` holds against #5486's budgets, and nothing
 * here asserts a speed (ADR 0086). What it does assert is that each answer is a real one: a
 * sidebar of the project's workspaces, a watch that was set up.
 */

const plane = process.env.PURLIS_SCALE_PLANE ?? "";
const out = process.env.PURLIS_SCALE_OUT;
const os = process.platform === "darwin" ? "macos" : "linux";
/** How many workspaces the project holds, as `scale-project generate` said; 0 when not told. */
const workspaces = Number(process.env.PURLIS_SCALE_WORKSPACES ?? "0");

function row(budget: string, measured: Record<string, unknown>): void {
  console.log(`scale-run | ${budget} | ${JSON.stringify(measured)}`);
  if (out) appendFileSync(out, `${JSON.stringify({ budget, os, ...measured })}\n`);
}

/** The id the app opened the project under, once it has. */
async function opened(): Promise<string> {
  let id: string | undefined;
  await browser.waitUntil(
    async () => {
      const ids = await browser.executeAsync((done: (ids: string[]) => void) => {
        void window.__TAURI__.core
          .invoke("open_planes")
          .then((ids) => done(ids as string[]))
          .catch(() => done([]));
      });
      id = ids[0];
      return id !== undefined;
    },
    { timeout: 300_000, interval: 500, timeoutMsg: "the app never opened the project" },
  );
  return id as string;
}

/** `times` answers to `plane_sidebar`, each timed in the window from the ask to the answer. */
async function sidebarAnswers(
  id: string,
  times: number,
): Promise<{ ms: number[]; workspaces: number }> {
  return browser.executeAsync(
    (id: string, times: number, done: (said: { ms: number[]; workspaces: number }) => void) => {
      const ms: number[] = [];
      let workspaces = -1;
      const ask = async () => {
        for (let n = 0; n < times; n++) {
          const started = performance.now();
          const sidebar = (await window.__TAURI__.core.invoke("plane_sidebar", { plane: id })) as {
            workspaces: unknown[];
          };
          ms.push(Math.round((performance.now() - started) * 10) / 10);
          workspaces = sidebar.workspaces.length;
        }
      };
      void ask().then(() => done({ ms, workspaces }));
    },
    id,
    times,
  );
}

/** The watch's setup, as the app logged it for this project (`planes.rs`). */
function watcherSetupMs(): number | undefined {
  const said = new RegExp(`is watched, set up in (\\d+) ms`);
  for (const file of readdirSync(LOGS)) {
    if (!file.endsWith(".log")) continue;
    for (const line of readFileSync(join(LOGS, file), "utf8").split("\n")) {
      if (!line.includes(plane)) continue;
      const found = said.exec(line);
      if (found) return Number(found[1]);
    }
  }
  return undefined;
}

/**
 * CPU seconds the app's process and every process under it have used, the children already
 * ended counted in (a `git status` the app ran and reaped is the app's cost): `/proc`'s
 * `cutime`/`cstime` on Linux, `ps -S` on macOS.
 */
function cpuSeconds(pid: number): number {
  const tree = (rows: [number, number][]): Set<number> => {
    const mine = new Set([pid]);
    for (let grew = true; grew;) {
      grew = false;
      for (const [child, parent] of rows) {
        if (mine.has(parent) && !mine.has(child)) {
          mine.add(child);
          grew = true;
        }
      }
    }
    return mine;
  };
  if (process.platform === "linux") {
    const stats = readdirSync("/proc")
      .filter((name) => /^\d+$/.test(name))
      .flatMap((name) => {
        try {
          const text = readFileSync(`/proc/${name}/stat`, "utf8");
          // The command is in parentheses and may hold spaces: the fields start after it.
          const fields = text.slice(text.lastIndexOf(")") + 2).split(" ");
          return [
            { pid: Number(name), ppid: Number(fields[1]), ticks: fields.slice(11, 15).map(Number) },
          ];
        } catch {
          return [];
        }
      });
    const mine = tree(stats.map((s) => [s.pid, s.ppid]));
    // `sysconf(_SC_CLK_TCK)` is 100 on every Linux GitHub runs.
    return (
      stats
        .filter((s) => mine.has(s.pid))
        .reduce((sum, s) => sum + s.ticks.reduce((a, b) => a + b, 0), 0) / 100
    );
  }
  const ps = execFileSync("ps", ["-A", "-S", "-o", "pid=,ppid=,time="], { encoding: "utf8" });
  const rows = ps
    .split("\n")
    .map((line) => line.trim().split(/\s+/))
    .filter((cols) => cols.length === 3)
    .map(([p, pp, time]) => ({ pid: Number(p), ppid: Number(pp), seconds: clock(time) }));
  const mine = tree(rows.map((r) => [r.pid, r.ppid]));
  return rows.filter((r) => mine.has(r.pid)).reduce((sum, r) => sum + r.seconds, 0);
}

/** `ps`'s `[[dd-]hh:]mm:ss.cc` as seconds. */
function clock(time: string): number {
  const [days, rest] = time.includes("-") ? time.split("-") : ["0", time];
  return (
    Number(days) * 86_400 +
    rest
      .split(":")
      .map(Number)
      .reduce((sum, part) => sum * 60 + part, 0)
  );
}

describe("the app on a project at scale", () => {
  let id = "";

  before(async () => {
    id = await opened();
  });

  it("answers the sidebar with every workspace", async () => {
    const said = await sidebarAnswers(id, 10);
    if (workspaces > 0 && said.workspaces !== workspaces) {
      throw new Error(`the sidebar listed ${said.workspaces} workspaces of ${workspaces}`);
    }
    row("sidebar", { samples: said.ms });
  });

  it("set up its watch of the project", async () => {
    let setup: number | undefined;
    await browser.waitUntil(async () => (setup = watcherSetupMs()) !== undefined, {
      timeout: 300_000,
      interval: 1000,
      timeoutMsg: `the app's log in ${LOGS} never said the project is watched`,
    });
    row("watcher-setup", { samples: [setup as number] });
  });

  it("sits idle for a minute with its window shown", async () => {
    // Its first git standing and the rest of the launch's work done, so the minute is idle.
    await sleep(20_000);
    const { pid } = appProcess();
    const before = cpuSeconds(pid);
    const started = Date.now();
    await sleep(60_000);
    const used = cpuSeconds(pid) - before;
    const seconds = (Date.now() - started) / 1000;
    row("idle-cpu", { value: Math.round((used / seconds) * 1000) / 10 });
  });
});
