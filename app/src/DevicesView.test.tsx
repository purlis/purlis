// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DevicesShown, RunnerAsked } from "./bindings";
import { DevicesView } from "./DevicesView";

/**
 * **Devices** (#2052): this machine and every runner, each with its chats and browsers, its
 * version, how it is and when it last answered; then each pool's use, with no money.
 */

const PLANE = "/plane" as DevicesShown["placement"]["here"]["chats"][number]["plane"];
const LIMITS = { cpus: "2", memory_mib: 4096, processes: 1024, tmp_mib: 1024 };

let shown: DevicesShown;
let asked: Record<string, RunnerAsked>;
let runnersAsked: string[];
/** Set by a test to hold every runner's answer until it is called. */
let holdAnswers: Promise<void> | undefined;

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
    pools: [
      { pool: "builds", limits: LIMITS, millicpus: 2000 },
      { pool: "qa", limits: LIMITS, millicpus: 2000 },
    ],
  };
  asked = {
    "builds/builds": {
      standing: { state: "running" },
      link: { state: "linked" },
      version: "0.8.1",
      contacted: Math.floor(Date.now() / 1000),
      holds: { open: [{ plane: PLANE, session: 2 }], browsers_open: [], chats: 1, browsers: 0 },
    },
    "qa/qa": {
      standing: { state: "suspended" },
      link: null,
      version: null,
      contacted: null,
      holds: { open: [], browsers_open: [], chats: 0, browsers: 0 },
    },
  };
  runnersAsked = [];
  holdAnswers = undefined;
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as { pool?: string; runner?: string };
    if (cmd === "devices") return structuredClone(shown);
    if (cmd === "device_runner") {
      const key = `${a.pool}/${a.runner}`;
      runnersAsked.push(key);
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

  it("shows each pool's use per person, and no money", async () => {
    render(<DevicesView />);
    const table = await screen.findByRole("grid", { name: "Use per pool and person" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(3));
    const builds = within(table).getByRole("row", { name: /builds/ });
    expect(
      within(builds)
        .getAllByRole("gridcell")
        .map((cell) => cell.textContent),
    ).toEqual(["You", "1", "0", "2", "4096 MiB"]);
    const qa = within(table).getByRole("row", { name: /qa/ });
    expect(
      within(qa)
        .getAllByRole("gridcell")
        .map((cell) => cell.textContent),
    ).toEqual(["You", "0", "1", "0", "0 MiB"]);
    expect(screen.queryByText(/\$|cost|price/i)).toBeNull();
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

  it("offers Refresh only once every runner has answered", async () => {
    let release = () => {};
    holdAnswers = new Promise((resolve) => {
      release = resolve;
    });
    render(<DevicesView />);
    const refresh = await screen.findByRole("button", { name: "Refresh" });
    expect(refresh).toHaveAttribute("aria-disabled", "true");
    release();
    await waitFor(() => expect(refresh).not.toHaveAttribute("aria-disabled", "true"));
  });

  it("names the person only where this machine's runners are shown to be theirs", async () => {
    shown.owned = false;
    render(<DevicesView />);
    const table = await screen.findByRole("grid", { name: "Use per pool and person" });
    await waitFor(() =>
      expect(within(table).getAllByText("Not known to be yours")).toHaveLength(2),
    );
  });
});
