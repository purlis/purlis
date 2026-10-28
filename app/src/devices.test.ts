// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type {
  BrowserOnADevice,
  DevicesShown,
  PoolRequests,
  RunnerAsked,
  RunnerOnTheList,
} from "./bindings";
import {
  NOT_KNOWN_TO_BE_YOURS,
  arranged,
  contactSaid,
  healthSaid,
  hoursSaid,
  runningSaid,
  usageOf,
  versionSaid,
} from "./devices";

const PLANE = "/plane" as DevicesShown["placement"]["here"]["chats"][number]["plane"];

const chat = (session: number, name: string) => ({ plane: PLANE, session, name });

const browser = (session: number, program: string): BrowserOnADevice => ({
  plane: PLANE,
  session,
  program,
  name: "b1",
  chat: `chat ${session}`,
});

const runner = (
  pool: string,
  name: string,
  chats = [] as ReturnType<typeof chat>[],
  browsers = [] as BrowserOnADevice[],
) =>
  ({
    pool,
    runner: name,
    draining: false,
    on: { chats, browsers },
  }) satisfies RunnerOnTheList;

const answered = (
  state: "running" | "suspended",
  open: number[] = [],
  browsersOpen: string[] = [],
  ranThisWeek: number | null = 0,
): RunnerAsked => ({
  standing: { state },
  link: state === "running" ? { state: "linked" } : null,
  version: state === "running" ? "0.9.0" : null,
  contacted: state === "running" ? 1_700_000_000 : null,
  holds: {
    open: open.map((session) => ({ plane: PLANE, session })),
    browsers_open: browsersOpen,
    chats: 0,
    browsers: 0,
  },
  ran_this_week: ranThisWeek,
});

const LIMITS = { cpus: "1.5", memory_mib: 2048, processes: 512, tmp_mib: 512 };
const pool = (name: string, removedRan = 0): PoolRequests => ({
  pool: name,
  limits: LIMITS,
  millicpus: 1500,
  removed_ran_this_week: removedRan,
});

const shownWith = (placement: Partial<DevicesShown["placement"]>): DevicesShown => ({
  version: "0.9.0",
  placement: {
    here: { chats: [], browsers: [] },
    runners: [],
    unkept: [],
    unkept_browsers: [],
    ...placement,
  },
  pools: [],
  owned: true,
});

describe("arranged", () => {
  it("puts each chat and browser where the window started it, and on a runner of its pool that says it holds it", () => {
    const shown = shownWith({
      here: { chats: [chat(1, "lint")], browsers: [] },
      runners: [
        runner("builds", "builds"),
        runner(
          "builds",
          "builds-2b1f",
          [chat(2, "old"), chat(3, "new")],
          [browser(2, "p-old"), browser(3, "p-new")],
        ),
      ],
    });
    const rows = arranged(shown, {
      "builds/builds": answered("running", [2], ["p-old"]),
      "builds/builds-2b1f": "asking",
    });
    expect(rows.map((row) => [row.key, row.chats.map((one) => one.session)])).toEqual([
      ["here", [1]],
      ["builds/builds", [2]],
      ["builds/builds-2b1f", [3]],
    ]);
    expect(rows.map((row) => [row.key, row.browsers.map((one) => one.program)])).toEqual([
      ["here", []],
      ["builds/builds", ["p-old"]],
      ["builds/builds-2b1f", ["p-new"]],
    ]);
  });

  it("never moves a chat or browser off this machine or another pool for what a runner says", () => {
    const shown = shownWith({
      here: { chats: [chat(1, "lint")], browsers: [browser(1, "p-here")] },
      runners: [
        runner("builds", "builds", [chat(2, "verify")], [browser(2, "p-builds")]),
        runner("qa", "qa"),
      ],
    });
    const rows = arranged(shown, {
      "builds/builds": answered("running"),
      "qa/qa": answered("running", [1, 2], ["p-here", "p-builds"]),
    });
    expect(
      rows.map((row) => [
        row.key,
        row.chats.map((one) => one.session),
        row.browsers.map((one) => one.program),
      ]),
    ).toEqual([
      ["here", [1], ["p-here"]],
      ["builds/builds", [2], ["p-builds"]],
      ["qa/qa", [], []],
    ]);
  });
});

describe("arranged, when two runners of one pool say they hold a chat", () => {
  it("keeps it on the runner the window placed it on", () => {
    const shown = shownWith({
      runners: [
        runner("builds", "builds"),
        runner("builds", "builds-2b1f", [chat(2, "verify")], [browser(2, "p-verify")]),
      ],
    });
    const rows = arranged(shown, {
      "builds/builds": answered("running", [2], ["p-verify"]),
      "builds/builds-2b1f": answered("running", [2], ["p-verify"]),
    });
    expect(rows.map((row) => [row.key, row.chats.length, row.browsers.length])).toEqual([
      ["here", 0, 0],
      ["builds/builds", 0, 0],
      ["builds/builds-2b1f", 1, 1],
    ]);
  });
});

describe("healthSaid", () => {
  it("says a runner not answered in time does not answer", () => {
    const asked: RunnerAsked = {
      ...answered("running"),
      standing: { state: "unanswered" },
      link: null,
    };
    expect(healthSaid(asked)).toEqual({
      badge: "does not answer",
      line: "It did not answer in time. purlis asks again when you press Refresh.",
    });
  });

  it("says in purlis's words why it could not ask, and names no one else as saying it", () => {
    expect(healthSaid({ failed: "There is no config home on this machine." })).toEqual({
      badge: "not asked",
      line: "purlis could not ask this runner: There is no config home on this machine.",
    });
  });

  it("says a suspended runner is suspended, with nothing said for it elsewhere", () => {
    const health = healthSaid(answered("suspended"));
    expect(health.badge).toBe("suspended");
    expect(health.said).toBeUndefined();
  });

  it("says why a runner does not answer apart from purlis's own words", () => {
    const asked = { ...answered("running"), link: { state: "down" as const, why: "timed out" } };
    expect(healthSaid(asked)).toEqual({
      badge: "does not answer",
      line: "It does not answer. purlis asks again when you press Refresh.",
      said: { by: "The link said", text: "timed out" },
    });
  });

  it("says what to do where a container could not be read, and what Docker said apart", () => {
    const asked: RunnerAsked = {
      ...answered("running"),
      standing: { state: "unknown", why: "Cannot connect to the Docker daemon" },
      link: null,
    };
    expect(healthSaid(asked)).toEqual({
      badge: "not read",
      line: "purlis could not read its container. Check that Docker runs, then press Refresh.",
      said: { by: "Docker said", text: "Cannot connect to the Docker daemon" },
    });
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
    expect(contactSaid(today, now, "en-GB")).toBe("Last answered at 9:05");
    expect(contactSaid(before, now, "en-GB")).toBe("Last answered 25 Oct at 9:05");
  });
});

describe("usageOf", () => {
  it("counts a pool's runners that run and are suspended, and what those that run request", () => {
    const runners = [
      runner("builds", "builds"),
      runner("builds", "builds-2b1f"),
      runner("qa", "qa"),
    ];
    const use = usageOf(
      [pool("builds"), pool("qa")],
      runners,
      {
        "builds/builds": answered("running"),
        "builds/builds-2b1f": answered("running"),
        "qa/qa": answered("suspended"),
      },
      true,
    );
    expect(use).toEqual([
      {
        pool: "builds",
        person: "You",
        running: 2,
        suspended: 0,
        cpus: "3",
        memoryMib: 4096,
        asking: 0,
        ranThisWeek: 0,
        hoursNotKnown: 0,
      },
      {
        pool: "qa",
        person: "You",
        running: 0,
        suspended: 1,
        cpus: "0",
        memoryMib: 0,
        asking: 0,
        ranThisWeek: 0,
        hoursNotKnown: 0,
      },
    ]);
  });

  it("names the person only where this machine's runners are shown to be theirs", () => {
    const use = usageOf(
      [pool("builds")],
      [runner("builds", "builds")],
      { "builds/builds": answered("running") },
      false,
    );
    expect(use[0].person).toBe(NOT_KNOWN_TO_BE_YOURS);
  });

  it("says a CPU to the thousandth and counts a runner not answered yet apart", () => {
    const use = usageOf(
      [pool("builds")],
      [runner("builds", "builds"), runner("builds", "b2")],
      {
        "builds/builds": answered("running"),
        "builds/b2": "asking",
      },
      true,
    );
    expect(use[0]).toMatchObject({ running: 1, cpus: "1.5", memoryMib: 2048, asking: 1 });
    expect(runningSaid(use[0])).toBe("1, 1 not answered yet");
    expect(runningSaid({ running: 2, asking: 0 })).toBe("2");
  });

  it("adds a pool's hours this week over its runners and those no longer kept", () => {
    const use = usageOf(
      [pool("builds", 1800)],
      [runner("builds", "builds"), runner("builds", "b2")],
      {
        "builds/builds": answered("running", [], [], 3 * 3600),
        "builds/b2": answered("suspended", [], [], 5400),
      },
      true,
    );
    // Three hours, an hour and a half, and half an hour of a runner removed since.
    expect(use[0]).toMatchObject({ ranThisWeek: 5 * 3600, hoursNotKnown: 0 });
    expect(hoursSaid(use[0])).toBe("5.0");
  });

  it("says hours to the tenth, and the runners whose hours are not known yet apart", () => {
    const use = usageOf(
      [pool("builds")],
      [runner("builds", "builds"), runner("builds", "b2"), runner("builds", "b3")],
      {
        "builds/builds": answered("running", [], [], 4500),
        "builds/b2": "asking",
        "builds/b3": answered("suspended", [], [], null),
      },
      true,
    );
    expect(use[0]).toMatchObject({ ranThisWeek: 4500, hoursNotKnown: 2 });
    expect(hoursSaid(use[0])).toBe("1.3, 2 not answered yet");
    expect(hoursSaid({ ranThisWeek: 0, hoursNotKnown: 0 })).toBe("0.0");
    expect(hoursSaid({ ranThisWeek: 100 * 3600 - 1, hoursNotKnown: 0 })).toBe("100.0");
  });
});
