// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DevicesShown, DiskHeld, RunnerAsked } from "./bindings";
import { DevicesView } from "./DevicesView";
import { answerWithinForTests, forgetDevicesForTests } from "./devices";
import { userEvent } from "@testing-library/user-event";

/**
 * **Devices** (#2052): this machine and every runner, each with its chats and browsers, its
 * version, how it is and when it last answered; then each pool's use, with money only where a
 * pool declares a price per hour.
 */

const PLANE = "/plane" as DevicesShown["placement"]["here"]["chats"][number]["plane"];
const LIMITS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

let shown: DevicesShown;
let asked: Record<string, RunnerAsked>;
let runnersAsked: string[];
/** Set by a test to hold every runner's answer until it is called. */
let holdAnswers: Promise<void> | undefined;
/** Set by a test: the core refuses to ask runner `qa/qa`, in these words. */
let refuse: string | undefined;
let disk: DiskHeld;
/** The pools the core was asked to remove, and what it answers. */
let removed: string[];
let removeSays: string | null;
let devicesRead: number;

beforeEach(() => {
  shown = {
    version: "0.9.0",
    placement: {
      here: {
        chats: [{ plane: PLANE, session: 1, name: "lint" }],
        browsers: [{ plane: PLANE, session: 1, program: "p-lint-b1", name: "b1", chat: "lint" }],
      },
      runners: [
        {
          pool: "builds",
          runner: "builds",
          draining: false,
          on: { chats: [{ plane: PLANE, session: 2, name: "verify" }], browsers: [] },
        },
        { pool: "qa", runner: "qa", draining: false, on: { chats: [], browsers: [] } },
      ],
      unkept: [],
      unkept_browsers: [],
    },
    owned: true,
    projects: 1,
    pools: [
      { pool: "builds", limits: LIMITS, millicpus: 2000, removed_ran_this_week: 0, price: null },
      { pool: "qa", limits: LIMITS, millicpus: 2000, removed_ran_this_week: 1800, price: null },
    ],
  };
  asked = {
    "builds/builds": {
      standing: { state: "running" },
      link: { state: "linked" },
      version: "0.8.1",
      contacted: Math.floor(Date.now() / 1000),
      holds: { open: [{ plane: PLANE, session: 2 }], browsers_open: [], chats: 1, browsers: 0 },
      ran_this_week: 9000,
    },
    "qa/qa": {
      standing: { state: "suspended" },
      link: null,
      version: null,
      contacted: null,
      holds: { open: [], browsers_open: [], chats: 0, browsers: 0 },
      ran_this_week: 3600,
    },
  };
  disk = {
    state: "read",
    pools: [
      { pool: "builds", kilobytes: 1_750_000, not_listed: 0 },
      { pool: "qa", kilobytes: 2_000, not_listed: 0 },
    ],
  };
  removed = [];
  removeSays = null;
  devicesRead = 0;
  runnersAsked = [];
  holdAnswers = undefined;
  refuse = undefined;
  answerWithinForTests(90_000);
  forgetDevicesForTests();
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as { pool?: string; runner?: string; name?: string };
    if (cmd === "devices") {
      devicesRead += 1;
      return structuredClone(shown);
    }
    if (cmd === "devices_disk") return structuredClone(disk);
    if (cmd === "remove_docker_pool") {
      removed.push(a.name ?? "");
      return removeSays;
    }
    if (cmd === "device_runner") {
      const key = `${a.pool}/${a.runner}`;
      runnersAsked.push(key);
      if (refuse !== undefined && key === "qa/qa") throw refuse;
      const answer = structuredClone(asked[key]);
      return holdAnswers === undefined ? answer : holdAnswers.then(() => answer);
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const section = (name: string) => screen.getByRole("rowgroup", { name });

describe("DevicesView", () => {
  it("lists this machine and every runner, each with its chats and browsers", async () => {
    render(<DevicesView />);
    await waitFor(() =>
      expect(screen.getByText("Runner builds of pool builds")).toBeInTheDocument(),
    );
    const here = section("This machine");
    expect(within(here).getByText("lint")).toBeInTheDocument();
    expect(within(here).getByText("Browser b1")).toBeInTheDocument();
    const builds = section("Runner builds of pool builds");
    expect(within(builds).getByText("verify")).toBeInTheDocument();
    expect(
      within(builds).getByText("1 chat of projects not open in this window"),
    ).toBeInTheDocument();
    expect(
      within(section("Runner qa of pool qa")).getByText("No chat or browser runs here"),
    ).toBeInTheDocument();
  });

  it("says each runner's version against this machine's, how it is and when it last answered", async () => {
    render(<DevicesView />);
    await waitFor(() =>
      expect(
        within(section("Runner builds of pool builds")).getByText("answers"),
      ).toBeInTheDocument(),
    );
    const builds = section("Runner builds of pool builds");
    expect(
      within(builds).getByText(/purlis 0\.8\.1, and this machine runs 0\.9\.0 · Last answered at/),
    ).toBeInTheDocument();
    const qa = section("Runner qa of pool qa");
    expect(within(qa).getByText("suspended")).toBeInTheDocument();
    expect(within(qa).getByText(/Not reached since purlis started/)).toBeInTheDocument();
    expect(runnersAsked.sort()).toEqual(["builds/builds", "qa/qa"]);
  });

  it("shows each pool's use per person with its hours this week, and no money", async () => {
    render(<DevicesView />);
    const table = await screen.findByRole("grid", { name: "Use per pool and person" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(3));
    const builds = within(table).getByRole("row", { name: /builds/ });
    expect(
      within(builds)
        .getAllByRole("gridcell")
        .map((cell) => cell.textContent),
    ).toEqual(["You", "1", "0", "2", "4096 MiB", "1.8 GB", "2.5"]);
    const qa = within(table).getByRole("row", { name: /qa/ });
    // An hour of its runner, and half an hour of one removed since.
    expect(
      within(qa)
        .getAllByRole("gridcell")
        .map((cell) => cell.textContent),
    ).toEqual(["You", "0", "1", "0", "0 MiB", "2.0 MB", "1.5"]);
    expect(screen.queryByText(/\$|cost|price/i)).toBeNull();
    expect(within(table).queryByRole("columnheader", { name: "Money this week" })).toBeNull();
  });

  it("shows money this week only for a pool that declares a price per hour, in its currency", async () => {
    shown.pools[0].price = { amount: "0.40", currency: "EUR" };
    render(<DevicesView />);
    const table = await screen.findByRole("grid", { name: "Use per pool and person" });
    expect(
      within(table).getByRole("columnheader", { name: "Money this week" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(
        within(within(table).getByRole("row", { name: /builds/ }))
          .getAllByRole("gridcell")
          .map((cell) => cell.textContent),
      ).toEqual(["You", "1", "0", "2", "4096 MiB", "1.8 GB", "2.5", "1.00 EUR"]),
    );
    // A pool that declares no price shows no money.
    expect(
      within(within(table).getByRole("row", { name: /qa/ }))
        .getAllByRole("gridcell")
        .map((cell) => cell.textContent),
    ).toEqual(["You", "0", "1", "0", "0 MiB", "2.0 MB", "1.5", ""]);
  });

  it("lists what runs on a pool this machine no longer keeps, chats and browsers alike", async () => {
    shown.placement.unkept = [{ plane: PLANE, session: 7, name: "orphan", pool: "removed" }];
    shown.placement.unkept_browsers = [
      { plane: PLANE, session: 7, name: "b2", chat: "orphan", pool: "removed" },
    ];
    render(<DevicesView />);
    const unkept = await screen.findByRole("rowgroup", {
      name: "On a pool this machine no longer keeps",
    });
    expect(within(unkept).getByText("Chat · on pool removed")).toBeInTheDocument();
    expect(within(unkept).getByText("Of orphan · on pool removed")).toBeInTheDocument();
  });

  it("names each chat's and browser's project when the window holds more than one", async () => {
    const OTHER = "/work/site" as typeof PLANE;
    shown.projects = 2;
    shown.placement.here.chats.push({ plane: OTHER, session: 1, name: "lint" });
    shown.placement.unkept = [{ plane: OTHER, session: 7, name: "orphan", pool: "removed" }];
    shown.placement.unkept_browsers = [
      { plane: OTHER, session: 7, name: "b2", chat: "orphan", pool: "removed" },
    ];
    render(<DevicesView />);
    await waitFor(() =>
      expect(screen.getByText("Runner builds of pool builds")).toBeInTheDocument(),
    );
    const here = section("This machine");
    expect(within(here).getByText("Chat in plane")).toBeInTheDocument();
    expect(within(here).getByText("Chat in site")).toBeInTheDocument();
    expect(within(here).getByText("Of lint in plane")).toBeInTheDocument();
    expect(
      within(section("Runner builds of pool builds")).getByText("Chat in plane"),
    ).toBeInTheDocument();
    const unkept = section("On a pool this machine no longer keeps");
    expect(within(unkept).getByText("Chat in site · on pool removed")).toBeInTheDocument();
    expect(within(unkept).getByText("Of orphan in site · on pool removed")).toBeInTheDocument();
  });

  it("names no project when the window holds one", async () => {
    render(<DevicesView />);
    await waitFor(() =>
      expect(screen.getByText("Runner builds of pool builds")).toBeInTheDocument(),
    );
    const here = section("This machine");
    expect(within(here).getByText("Chat")).toBeInTheDocument();
    expect(within(here).getByText("Of lint")).toBeInTheDocument();
  });

  it("draws what Docker said apart from purlis's own words, and says what to do", async () => {
    asked["qa/qa"] = {
      ...asked["qa/qa"],
      standing: { state: "unknown", why: "Cannot connect to the Docker daemon" },
    };
    render(<DevicesView />);
    const qa = await waitFor(() => section("Runner qa of pool qa"));
    await waitFor(() => expect(within(qa).getByText("not read")).toBeInTheDocument());
    expect(
      within(qa).getByText(
        "purlis could not read its container. Check that Docker runs, then press Refresh.",
      ),
    ).toBeInTheDocument();
    const said = within(qa).getByText("Cannot connect to the Docker daemon");
    expect(said.tagName).toBe("BDI");
  });

  it("says a runner whose ask never returns does not answer once its time is up, and Refresh asks again", async () => {
    answerWithinForTests(200);
    let release = () => {};
    holdAnswers = new Promise((resolve) => {
      release = resolve;
    });
    render(<DevicesView />);
    const builds = await waitFor(() => section("Runner builds of pool builds"));
    await waitFor(() => expect(within(builds).getByText("asking…")).toBeInTheDocument());
    // Refresh stays offered while a runner is being asked.
    expect(screen.getByRole("button", { name: "Refresh" })).not.toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await waitFor(() =>
      expect(
        within(section("Runner builds of pool builds")).getByText("does not answer"),
      ).toBeInTheDocument(),
    );
    holdAnswers = undefined;
    release();
    await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() =>
      expect(
        within(section("Runner builds of pool builds")).getByText("answers"),
      ).toBeInTheDocument(),
    );
    expect(runnersAsked.filter((one) => one === "builds/builds").length).toBeGreaterThan(1);
  });

  it("says in purlis's own words why it could not ask a runner", async () => {
    refuse = "No runner qa of pool qa is on this machine.";
    render(<DevicesView />);
    await waitFor(() =>
      expect(
        within(section("Runner qa of pool qa")).getByText(
          "purlis could not ask this runner: No runner qa of pool qa is on this machine.",
        ),
      ).toBeInTheDocument(),
    );
    expect(screen.queryByText(/Docker said/)).toBeNull();
  });

  it("names the person only where this machine's runners are shown to be theirs", async () => {
    shown.owned = false;
    render(<DevicesView />);
    const table = await screen.findByRole("grid", { name: "Use per pool and person" });
    await waitFor(() =>
      expect(within(table).getAllByText("Not known to be yours")).toHaveLength(2),
    );
  });

  it("shows the disk each pool's runners hold, and says why where Docker could not read it", async () => {
    disk = { state: "not_read", why: "Cannot connect to the Docker daemon" };
    render(<DevicesView />);
    const table = await screen.findByRole("grid", { name: "Use per pool and person" });
    expect(within(table).getByRole("columnheader", { name: "Disk held" })).toBeInTheDocument();
    await waitFor(() =>
      expect(
        within(within(table).getByRole("row", { name: /builds/ })).getByText("not read"),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/purlis could not read the disk the runners hold\. · Docker said:/),
    ).toBeInTheDocument();
    expect(screen.getByText("Cannot connect to the Docker daemon").tagName).toBe("BDI");
  });
});
