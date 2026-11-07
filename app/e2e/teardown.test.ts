// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { endTheApp, endTheRunsApp, type Os, removeLeniently } from "./teardown.js";

/**
 * A machine whose processes go a few ticks after the signal that ends them, as a quitting app
 * does while it saves. `deaf` ignore a SIGTERM, as a process stuck in its last write would.
 */
function aMachine(running: number[], deaf: number[] = []) {
  const alive = new Set(running);
  const dying = new Map<number, number>();
  const said: string[] = [];
  let clock = 0;
  const os: Os = {
    signal(pid, signal) {
      said.push(`${signal} ${pid}`);
      if (!alive.has(pid)) return;
      if (signal === "SIGKILL") alive.delete(pid);
      else if (!deaf.includes(pid)) dying.set(pid, clock + 300);
    },
    alive: (pid) => alive.has(pid),
    async sleep(ms) {
      clock += ms;
      for (const [pid, at] of dying) if (at <= clock) alive.delete(pid);
    },
    now: () => clock,
  };
  return { os, said, alive };
}

describe("ending the run's app before its folders are removed", () => {
  it("ends the window first, then its session host, and returns once both have gone", async () => {
    const machine = aMachine([5100, 5200]);

    const left = await endTheApp(
      { windows: [5100], sessionHosts: [5200], programs: [] },
      machine.os,
    );

    expect(machine.said).toEqual(["SIGTERM 5100", "SIGTERM 5200"]);
    expect(left).toEqual([]);
    expect(machine.alive.size).toBe(0);
  });

  it("ends the programs its host started after the host, which a killed host leaves running", async () => {
    const machine = aMachine([5100, 5200, 5300]);

    const left = await endTheApp(
      { windows: [5100], sessionHosts: [5200], programs: [5300] },
      machine.os,
    );

    expect(machine.said).toEqual(["SIGTERM 5100", "SIGTERM 5200", "SIGTERM 5300"]);
    expect(left).toEqual([]);
  });

  it("kills what a SIGTERM did not end in time", async () => {
    const machine = aMachine([5100, 5200], [5200]);

    const left = await endTheApp(
      { windows: [5100], sessionHosts: [5200], programs: [] },
      machine.os,
    );

    expect(machine.said).toEqual(["SIGTERM 5100", "SIGTERM 5200", "SIGKILL 5200"]);
    expect(left).toEqual([]);
  });

  it("signals nothing when the run's app has already gone", async () => {
    const machine = aMachine([]);

    const left = await endTheApp(
      { windows: [5100], sessionHosts: [5200], programs: [] },
      machine.os,
    );

    expect(machine.said).toEqual([]);
    expect(left).toEqual([]);
  });
});

describe("ending the run's app as the last step of a run whose specs passed", () => {
  it("says what it found still running, so a run shows whether the app outlived its specs", async () => {
    const machine = aMachine([5100, 5200]);
    const said: string[] = [];

    await endTheRunsApp(
      { windows: [5100], sessionHosts: [5200], programs: [] },
      machine.os,
      (line) => said.push(line),
    );

    expect(said.join("\n")).toMatch(/window 5100.*session host 5200/s);
  });

  it("writes down a signal that failed, rather than failing the run", async () => {
    const machine = aMachine([5100]);
    machine.os.signal = () => {
      throw Object.assign(new Error("kill EPERM"), { code: "EPERM" });
    };
    const said: string[] = [];

    await expect(
      endTheRunsApp({ windows: [5100], sessionHosts: [], programs: [] }, machine.os, (line) =>
        said.push(line),
      ),
    ).resolves.toBeUndefined();
    expect(said.join("\n")).toContain("kill EPERM");
  });
});

/** An `rm` that fails with `codes`, one per call, then removes. */
function anRm(codes: string[]) {
  const calls: string[] = [];
  const rm = (path: string) => {
    calls.push(path);
    const code = codes.shift();
    if (code) throw Object.assign(new Error(`${code}, Directory not empty: '${path}'`), { code });
  };
  return { rm, calls };
}

describe("removing a run's temporary folder", () => {
  const folder = "/tmp/charter-scenario-run-x/plane-daily-y";

  it("tries again, after a wait, while the folder is still being written", async () => {
    const { os } = aMachine([]);
    const { rm, calls } = anRm(["ENOTEMPTY", "EBUSY"]);
    const said: string[] = [];

    const removed = await removeLeniently(folder, { rm, os, log: (line) => said.push(line) });

    expect(removed).toBe(true);
    expect(calls).toHaveLength(3);
    expect(os.now()).toBeGreaterThan(0);
    expect(said).toEqual([]);
  });

  it("says what is left and who holds it, rather than failing the run, when it never empties", async () => {
    const { os } = aMachine([]);
    const { rm } = anRm(Array(1000).fill("ENOTEMPTY"));
    const said: string[] = [];

    const removed = await removeLeniently(folder, {
      rm,
      os,
      log: (line) => said.push(line),
      probe: () => "daily/.purlis/app/reopen.json.tmp, held by 5200 purlis-app",
    });

    expect(removed).toBe(false);
    expect(said.join("\n")).toContain(folder);
    expect(said.join("\n")).toContain("ENOTEMPTY");
    expect(said.join("\n")).toContain("reopen.json.tmp, held by 5200 purlis-app");
  });

  it("does not wait out an error that waiting cannot mend", async () => {
    const { os } = aMachine([]);
    const { rm, calls } = anRm(["EACCES"]);
    const said: string[] = [];

    const removed = await removeLeniently(folder, {
      rm,
      os,
      log: (line) => said.push(line),
      probe: () => "",
    });

    expect(removed).toBe(false);
    expect(calls).toHaveLength(1);
    expect(said.join("\n")).toContain("EACCES");
  });

  it("removes a real folder and everything in it", async () => {
    const real = mkdtempSync(join(tmpdir(), "teardown-"));
    mkdirSync(join(real, "daily", ".purlis"), { recursive: true });
    writeFileSync(join(real, "daily", ".purlis", "state.json"), "{}");

    expect(await removeLeniently(real)).toBe(true);
    expect(existsSync(real)).toBe(false);
  });
});
