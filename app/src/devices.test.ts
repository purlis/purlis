// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { DevicesShown, PoolRequests, RunnerAsked, RunnerOnTheList } from "./bindings";
import { arranged, contactSaid, healthSaid, usageOf, versionSaid } from "./devices";

const PLANE = "/plane" as DevicesShown["placement"]["here"]["chats"][number]["plane"];

const chat = (session: number, name: string) => ({ plane: PLANE, session, name });

const runner = (pool: string, name: string, chats = [] as ReturnType<typeof chat>[]) =>
  ({
    pool,
    runner: name,
    draining: false,
    on: { chats, browsers: [] },
  }) satisfies RunnerOnTheList;

const answered = (state: "running" | "suspended", open: number[] = []): RunnerAsked => ({
  standing: { state },
  link: state === "running" ? { state: "linked" } : null,
  version: state === "running" ? "0.9.0" : null,
  contacted: state === "running" ? 1_700_000_000 : null,
  holds: { open: open.map((session) => ({ plane: PLANE, session })), chats: 0, browsers: 0 },
});

const LIMITS = { cpus: "1.5", memory_mib: 2048, processes: 512, tmp_mib: 512 };
const pool = (name: string): PoolRequests => ({ pool: name, limits: LIMITS, millicpus: 1500 });

describe("arranged", () => {
  it("puts each chat where the window started it, and on a runner that says it holds it", () => {
    const shown: DevicesShown = {
      version: "0.9.0",
      placement: {
        here: { chats: [chat(1, "lint")], browsers: [] },
        runners: [
          runner("builds", "builds"),
          runner("builds", "builds-2b1f", [chat(2, "old"), chat(3, "new")]),
        ],
        unkept: [],
      },
      pools: [],
    };
    const rows = arranged(shown, {
      "builds/builds": answered("running", [2]),
      "builds/builds-2b1f": "asking",
    });
    const sessions = rows.map((row) => [row.key, row.chats.map((one) => one.session)]);
    expect(sessions).toEqual([
      ["here", [1]],
      ["builds/builds", [2]],
      ["builds/builds-2b1f", [3]],
    ]);
  });
});

describe("healthSaid", () => {
  it("says a suspended runner is suspended, not in trouble", () => {
    expect(healthSaid(answered("suspended"))).toMatchObject({
      badge: "suspended",
      trouble: false,
    });
  });

  it("says why a runner does not answer", () => {
    const asked = { ...answered("running"), link: { state: "down" as const, why: "timed out" } };
    expect(healthSaid(asked)).toEqual({ badge: "does not answer", line: "timed out", trouble: true });
  });

  it("says one not asked yet is being asked", () => {
    expect(healthSaid("asking").badge).toBe("asking…");
  });
});

describe("versionSaid", () => {
  it("names this machine's version only where the runner's is another", () => {
    expect(versionSaid("0.9.0", "0.9.0")).toBe("purlis 0.9.0");
    expect(versionSaid("0.8.1", "0.9.0")).toBe("purlis 0.8.1, and this machine runs 0.9.0");
    expect(versionSaid(null, "0.9.0")).toBeUndefined();
  });
});

describe("contactSaid", () => {
  it("says a runner not reached since purlis started as such, never as a time", () => {
    expect(contactSaid(null, new Date())).toBe("Not reached since purlis started");
  });

  it("says a time today and a date before", () => {
    const now = new Date(2026, 9, 27, 12, 30);
    const today = new Date(2026, 9, 27, 9, 5).getTime() / 1000;
    const before = new Date(2026, 9, 25, 9, 5).getTime() / 1000;
    expect(contactSaid(today, now, "en-GB")).toBe("Last answered at 09:05");
    expect(contactSaid(before, now, "en-GB")).toBe("Last answered 25 Oct at 09:05");
  });
});

describe("usageOf", () => {
  it("counts a pool's runners that run and are suspended, and what those that run request", () => {
    const runners = [runner("builds", "builds"), runner("builds", "builds-2b1f"), runner("qa", "qa")];
    const use = usageOf([pool("builds"), pool("qa")], runners, {
      "builds/builds": answered("running"),
      "builds/builds-2b1f": answered("running"),
      "qa/qa": answered("suspended"),
    });
    expect(use).toEqual([
      { pool: "builds", person: "You", running: 2, suspended: 0, cpus: "3", memoryMib: 4096, asking: 0 },
      { pool: "qa", person: "You", running: 0, suspended: 1, cpus: "0", memoryMib: 0, asking: 0 },
    ]);
  });

  it("says a CPU to the thousandth and counts a runner not answered yet apart", () => {
    const use = usageOf([pool("builds")], [runner("builds", "builds"), runner("builds", "b2")], {
      "builds/builds": answered("running"),
      "builds/b2": "asking",
    });
    expect(use[0]).toMatchObject({ running: 1, cpus: "1.5", memoryMib: 2048, asking: 1 });
  });
});
