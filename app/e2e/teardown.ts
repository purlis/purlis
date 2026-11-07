// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { spawnSync } from "node:child_process";
import { readdirSync, rmSync } from "node:fs";
import process from "node:process";
import type { AppProcesses } from "./processes.js";

/**
 * **What a run that removes its temporary folders does first: end the app that writes in them**
 * (#5653).
 *
 * WebdriverIO runs a config's `onComplete` before every service's (`@wdio/cli`, "user hooks are
 * run before service hooks"), and on macOS the Tauri service's `onComplete` is what stops the
 * app it launched. So a config that removes a plane there removes it under a live app, and the
 * app's last writes — each a temporary file renamed into place — land in a folder being
 * emptied: `ENOTEMPTY` (merge queue run 42376613047). And a signal is not a quit: the session
 * host the app started, in a process group of its own, keeps its projects for the grace period
 * (#2006), so it is ended too, after its window, as `tools/relaunch.mjs` ends it.
 */

/** What ending a process asks of the operating system; a test gives a machine of its own. */
export interface Os {
  /** Sends `signal` to `pid`; one already gone is ended, not an error. */
  signal(pid: number, signal: "SIGTERM" | "SIGKILL"): void;
  alive(pid: number): boolean;
  sleep(ms: number): Promise<void>;
  now(): number;
}

/** This machine. */
export const THIS_MACHINE: Os = {
  signal(pid, signal) {
    try {
      process.kill(pid, signal);
    } catch (error) {
      // It went between the look and the signal, as a host does when its window's end takes it.
      if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
    }
  },
  alive(pid) {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  },
  sleep: (ms) => new Promise((done) => setTimeout(done, ms)),
  now: () => Date.now(),
};

/** How long a SIGTERM is given before a SIGKILL: the app's own host gives its end two seconds. */
const GIVEN = 5_000;
const TICK = 100;

/** Waits until none of `pids` is alive, or `within` ms have passed; returns those still alive. */
async function gone(pids: number[], os: Os, within: number): Promise<number[]> {
  const until = os.now() + within;
  let left = pids.filter((pid) => os.alive(pid));
  while (left.length > 0 && os.now() < until) {
    await os.sleep(TICK);
    left = left.filter((pid) => os.alive(pid));
  }
  return left;
}

/** Ends `pids` with a SIGTERM, then a SIGKILL for any that has not gone; returns any left. */
async function end(pids: number[], os: Os): Promise<number[]> {
  const running = pids.filter((pid) => os.alive(pid));
  for (const pid of running) os.signal(pid, "SIGTERM");
  const deaf = await gone(running, os, GIVEN);
  for (const pid of deaf) os.signal(pid, "SIGKILL");
  return gone(deaf, os, GIVEN);
}

/**
 * Ends the run's app: its windows first, so each finishes its last saves, then the session
 * hosts they started. Returns once every one has gone, with the pids of any that would not.
 */
export async function endTheApp(app: AppProcesses, os: Os = THIS_MACHINE): Promise<number[]> {
  const windows = await end(app.windows, os);
  const hosts = await end(app.sessionHosts, os);
  return [...windows, ...hosts];
}

/**
 * The errors a folder still being written or held gives a removal, which a wait can mend. Node
 * 24's `rmSync` says `ENOTEMPTY` for an entry it could not remove, whatever the reason.
 */
const BUSY = new Set(["ENOTEMPTY", "EBUSY", "EPERM"]);

/** The waits between tries: about five seconds in all. */
const WAITS = [100, 200, 400, 800, 1_500, 2_000];

/** How many entries of a folder that would not go are written down. */
const LISTED = 40;

/**
 * What is left in `path` and which processes hold files in it, for the line that says the
 * folder would not go: the entries first, then `lsof +D`'s answer.
 */
export function whatHolds(path: string): string {
  let entries: string[];
  try {
    entries = readdirSync(path, { recursive: true, encoding: "utf8" });
  } catch (error) {
    entries = [`(could not list it: ${(error as Error).message})`];
  }
  const shown = entries.slice(0, LISTED);
  if (entries.length > LISTED) shown.push(`… and ${entries.length - LISTED} more`);
  const held = spawnSync("lsof", ["-n", "-P", "+D", path], { encoding: "utf8", timeout: 20_000 });
  const holders =
    held.error !== undefined
      ? `(lsof: ${held.error.message})`
      : held.stdout.trim() || "(lsof found no process holding a file in it)";
  return [`left in it:`, ...shown.map((entry) => `  ${entry}`), `held:`, holders].join("\n");
}

/** What a removal is given; a test gives its own. */
export interface Removal {
  rm?: (path: string) => void;
  os?: Os;
  log?: (line: string) => void;
  probe?: (path: string) => string;
}

/**
 * Removes `path` and everything in it, trying again for a few seconds while it is busy, and
 * returns whether it went. **It never throws:** a folder that will not go is written down with
 * what is left in it and who holds it ({@link whatHolds}), and the run it was the last step of
 * keeps the verdict its specs gave. The folder is in the system's temporary directory, which
 * the system empties.
 *
 * The tries are this loop's, not `rmSync`'s `maxRetries`: on Node 24 that sleeps whole seconds
 * (`sleep(attempt * retryDelay / 1000)`), so a delay of 200 ms tried the first four times with
 * no wait at all.
 */
export async function removeLeniently(path: string, given: Removal = {}): Promise<boolean> {
  const rm = given.rm ?? ((at: string) => rmSync(at, { recursive: true, force: true }));
  const os = given.os ?? THIS_MACHINE;
  const log = given.log ?? ((line: string) => console.warn(line));
  const probe = given.probe ?? whatHolds;
  for (let attempt = 0; ; attempt += 1) {
    try {
      rm(path);
      return true;
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code ?? "";
      if (BUSY.has(code) && attempt < WAITS.length) {
        await os.sleep(WAITS[attempt]);
        continue;
      }
      log(
        `The run's temporary folder ${path} could not be removed (${(error as Error).message}), ` +
          `after ${attempt + 1} tries; the run keeps its specs' verdict.\n${probe(path)}`,
      );
      return false;
    }
  }
}
