// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DevicesShown, DiskHeld, PoolNamedBy, RunnerAsked } from "./bindings";
import { DevicesView } from "./DevicesView";
import { answerWithinForTests, forgetDevicesForTests } from "./devices";
import { userEvent } from "@testing-library/user-event";
import { findGrid } from "./ui/table/table.testkit";

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
/** What `pool_named_by` says names a pool (#4678). */
let namedBy: PoolNamedBy[];
/** The runners `pool_held_runners` names, and what each removal was told to discard (#4860). */
let heldRunners: string[];
let discarded: unknown[];
/** The held runners the core was asked to remove. */
let heldRemoved: string[];
/** The held runners whose conversations the core was asked to bring back, and what it says. */
let broughtBack: string[];
let bringBackSays: { error: string } | undefined;
/** The kept runners the core was asked to let go of, and why it did not, where it did not. */
let letGone: string[];
let letGoSays: { error: string } | undefined;
let letGoShown: unknown[];
let devicesRead: number;
/** The moment the fixture's contacts are said against: one for the fixture and the view, so a
 *  run across midnight never puts them on two days ("Last answered Nov 3 at …"). */
let now: Date;

beforeEach(() => {
  now = new Date();
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
          left: [],
          not_checked: null,
          on: { chats: [{ plane: PLANE, session: 2, name: "verify" }], browsers: [] },
        },
        {
          pool: "qa",
          runner: "qa",
          draining: false,
          left: [],
          not_checked: null,
          on: { chats: [], browsers: [] },
        },
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
      contacted: Math.floor(now.getTime() / 1000),
      holds: { open: [{ plane: PLANE, session: 2 }], browsers_open: [], chats: 1, browsers: 0 },
      ran_this_week: 9000,
      held: false,
    },
    "qa/qa": {
      standing: { state: "suspended" },
      link: null,
      version: null,
      contacted: null,
      holds: { open: [], browsers_open: [], chats: 0, browsers: 0 },
      ran_this_week: 3600,
      held: false,
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
  namedBy = [];
  heldRunners = [];
  discarded = [];
  heldRemoved = [];
  broughtBack = [];
  bringBackSays = undefined;
  letGone = [];
  letGoSays = undefined;
  letGoShown = [];
  devicesRead = 0;
  runnersAsked = [];
  holdAnswers = undefined;
  refuse = undefined;
  answerWithinForTests(90_000);
  forgetDevicesForTests();
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as {
      pool?: string;
      runner?: string;
      name?: string;
      discarding?: unknown;
      shown?: unknown;
    };
    if (cmd === "devices") {
      devicesRead += 1;
      return structuredClone(shown);
    }
    if (cmd === "devices_disk") return structuredClone(disk);
    if (cmd === "pool_named_by") return structuredClone(namedBy);
    if (cmd === "pool_held_runners") return structuredClone(heldRunners);
    if (cmd === "remove_docker_pool") {
      removed.push(a.name ?? "");
      discarded.push(a.discarding);
      return removeSays;
    }
    if (cmd === "docker_pool_remove_held_runner") {
      heldRemoved.push(a.runner ?? "");
      return null;
    }
    if (cmd === "docker_pool_bring_back_held_runner") {
      broughtBack.push(a.runner ?? "");
      if (bringBackSays !== undefined) throw bringBackSays.error;
      return null;
    }
    if (cmd === "docker_pool_let_runner_go") {
      letGone.push(a.runner ?? "");
      letGoShown.push(a.shown);
      if (letGoSays !== undefined) throw letGoSays.error;
      return null;
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
    render(<DevicesView now={now} />);
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
    const table = await findGrid(screen, "Use per pool and person");
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
    const table = await findGrid(screen, "Use per pool and person");
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
    const table = await findGrid(screen, "Use per pool and person");
    await waitFor(() =>
      expect(within(table).getAllByText("Not known to be yours")).toHaveLength(2),
    );
  });

  it("shows the disk each pool's runners hold, and says why where Docker could not read it", async () => {
    disk = { state: "not_read", why: "Cannot connect to the Docker daemon" };
    render(<DevicesView />);
    const table = await findGrid(screen, "Use per pool and person");
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

  it("offers a held runner's Notice alone, and its Discard asks twice before anything goes", async () => {
    asked = { ...asked, "qa/qa": { ...(asked["qa/qa"] as RunnerAsked), held: true } };
    render(<DevicesView />);
    const notice = await waitFor(() => {
      const found = document.querySelector('[data-cause="devices-held-runner:qa"]');
      if (!(found instanceof HTMLElement)) throw new Error("no Notice yet");
      return found;
    });
    expect(notice.textContent).toContain("purlis kept runner qa of pool qa");
    expect(document.querySelector('[data-cause="devices-held-runner:builds"]')).toBeNull();
    const held = section("Runner qa of pool qa");
    expect(held.textContent).toContain("until you bring their conversations back or discard them");
    expect(held.textContent).not.toContain("It starts again when");

    await userEvent.click(within(notice).getByRole("button", { name: "Discard…" }));
    const first = await screen.findByRole("alertdialog", {
      name: "Discard runner qa's conversations?",
    });
    await userEvent.click(within(first).getByRole("button", { name: "Discard…" }));
    const again = await screen.findByRole("alertdialog", { name: "Discard them for good?" });
    // The second ask opens on its way back, so a second press of the same spot discards nothing.
    await waitFor(() =>
      expect(document.activeElement).toBe(within(again).getByRole("button", { name: "Cancel" })),
    );
    expect(heldRemoved).toEqual([]);
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    // The keyboard goes back to Discard…, though the ask that opened the second is gone.
    await waitFor(() =>
      expect(document.activeElement).toBe(within(notice).getByRole("button", { name: "Discard…" })),
    );
    expect(heldRemoved).toEqual([]);
  });

  it("brings a held runner's conversations back on one press, removing nothing itself", async () => {
    asked = { ...asked, "qa/qa": { ...(asked["qa/qa"] as RunnerAsked), held: true } };
    render(<DevicesView />);

    await userEvent.click(await screen.findByRole("button", { name: "Bring back conversations" }));

    await vi.waitFor(() => expect(broughtBack).toEqual(["qa"]));
    expect(heldRemoved).toEqual([]);
  });

  it("discards a held runner's conversations once asked twice", async () => {
    asked = { ...asked, "qa/qa": { ...(asked["qa/qa"] as RunnerAsked), held: true } };
    render(<DevicesView />);
    const discard = await screen.findByRole("button", { name: "Discard…" });

    await userEvent.click(discard);
    await userEvent.click(
      within(
        await screen.findByRole("alertdialog", { name: "Discard runner qa's conversations?" }),
      ).getByRole("button", { name: "Discard…" }),
    );
    await userEvent.click(
      within(await screen.findByRole("alertdialog", { name: "Discard them for good?" })).getByRole(
        "button",
        { name: "Discard conversations" },
      ),
    );

    await vi.waitFor(() => expect(heldRemoved).toEqual(["qa"]));
  });

  it("says what stopped a bring back, and keeps the runner's Notice", async () => {
    asked = { ...asked, "qa/qa": { ...(asked["qa/qa"] as RunnerAsked), held: true } };
    bringBackSays = { error: "1 file of 2 files of its chats' conversations came back." };
    render(<DevicesView />);

    await userEvent.click(await screen.findByRole("button", { name: "Bring back conversations" }));

    expect(
      await screen.findByText(/Bringing back runner qa's conversations did not finish:/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("1 file of 2 files of its chats' conversations came back.").tagName,
    ).toBe("BDI");
  });

  /** Runner qa of pool qa drains, kept for a stash no fetch brings. */
  const keptForAStash = () => {
    const runners = shown.placement.runners;
    runners[1] = {
      ...(runners[1] as (typeof runners)[number]),
      draining: true,
      left: ["in project alpha, the project's own repository on the runner holds 1 stashed change"],
    };
  };
  const keptNotice = () =>
    waitFor(() => {
      const found = document.querySelector('[data-cause="devices-kept-runner:qa"]');
      if (!(found instanceof HTMLElement)) throw new Error("no Notice yet");
      return found;
    });

  it("offers Let it go on an older runner kept for work it holds, naming what goes, and lets only it go once asked", async () => {
    keptForAStash();
    render(<DevicesView />);
    const notice = await keptNotice();
    expect(notice.textContent).toContain("purlis keeps older runner qa of pool qa");
    expect(section("Runner qa of pool qa").textContent).toContain(
      "An older runner purlis keeps, since it holds work purlis can't bring back",
    );
    const reads = devicesRead;

    await userEvent.click(within(notice).getByRole("button", { name: "Let it go…" }));
    const asking = await screen.findByRole("alertdialog", { name: "Let runner qa go?" });
    expect(asking.textContent).toContain("1 stashed change");
    expect(asking.textContent).toContain("Pool qa and its runner now stay as they are.");
    // It opens on its way back, so the press that opened it lets nothing go.
    await waitFor(() =>
      expect(document.activeElement).toBe(within(asking).getByRole("button", { name: "Cancel" })),
    );
    expect(letGone).toEqual([]);
    await userEvent.click(within(asking).getByRole("button", { name: "Let it go" }));

    await vi.waitFor(() => expect(letGone).toEqual(["qa"]));
    // Only what the ask named goes: the core is told it, and keeps a runner that holds more.
    expect(letGoShown).toEqual([
      ["in project alpha, the project's own repository on the runner holds 1 stashed change"],
    ]);
    await vi.waitFor(() => expect(devicesRead).toBeGreaterThan(reads));
    expect(removed).toEqual([]);
    expect(heldRemoved).toEqual([]);
  });

  it("lets nothing go when asked and cancelled, and the keyboard goes back to Let it go…", async () => {
    keptForAStash();
    render(<DevicesView />);
    const notice = await keptNotice();
    const letGo = within(notice).getByRole("button", { name: "Let it go…" });

    await userEvent.click(letGo);
    await screen.findByRole("alertdialog", { name: "Let runner qa go?" });
    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(letGo));
    expect(letGone).toEqual([]);
  });

  it("says why a runner was not let go, and keeps its Notice", async () => {
    keptForAStash();
    letGoSays = { error: "1 chat still runs on runner qa, so it stays until it ends." };
    render(<DevicesView />);
    const notice = await keptNotice();
    const reads = devicesRead;

    await userEvent.click(within(notice).getByRole("button", { name: "Let it go…" }));
    await userEvent.click(
      within(await screen.findByRole("alertdialog", { name: "Let runner qa go?" })).getByRole(
        "button",
        { name: "Let it go" },
      ),
    );

    expect(await screen.findByText(/Runner qa was not let go:/)).toBeInTheDocument();
    expect(
      screen.getByText("1 chat still runs on runner qa, so it stays until it ends.").tagName,
    ).toBe("BDI");
    expect(document.querySelector('[data-cause="devices-kept-runner:qa"]')).not.toBeNull();
    // What it holds now is read again, so the next ask names it.
    await vi.waitFor(() => expect(devicesRead).toBeGreaterThan(reads));
  });

  /** Runner qa of pool qa drains, kept as it could not be checked: `left` says what is left of it. */
  const keptUnchecked = (left: "stopped" | "home_only") => {
    const runners = shown.placement.runners;
    runners[1] = { ...(runners[1] as (typeof runners)[number]), draining: true, not_checked: left };
  };

  it("says an older runner that stopped before it was checked stays until it runs again, and asks before it goes unchecked", async () => {
    keptUnchecked("stopped");
    render(<DevicesView />);
    const notice = await keptNotice();
    expect(notice.textContent).toContain(
      "purlis keeps older runner qa of pool qa, since it stopped before purlis could check it for work only it holds.",
    );
    expect(notice.textContent).toContain("Start it there and purlis checks it");
    expect(notice.textContent).not.toContain("holds work purlis can't bring back");
    expect(section("Runner qa of pool qa").textContent).toContain(
      "An older runner purlis keeps, since it stopped before purlis could check it",
    );

    await userEvent.click(within(notice).getByRole("button", { name: "Let it go…" }));
    const asking = await screen.findByRole("alertdialog", { name: "Let runner qa go?" });
    expect(asking.textContent).toContain(
      "Its stopped container and its home are removed from your Docker. purlis never checked them, so any work only they hold is lost with them.",
    );
    expect(asking.textContent).toContain("Pool qa and its runner now stay as they are.");
    await userEvent.click(within(asking).getByRole("button", { name: "Let it go" }));

    await vi.waitFor(() => expect(letGone).toEqual(["qa"]));
    expect(letGoShown).toEqual([[]]);
  });

  it("says only the saved home of a runner removed outside purlis is left, and that purlis can't check it", async () => {
    keptUnchecked("home_only");
    render(<DevicesView />);
    const notice = await keptNotice();
    expect(notice.textContent).toContain(
      "purlis keeps the saved home of older runner qa of pool qa.",
    );
    expect(notice.textContent).toContain("purlis can't check the home");
    expect(notice.textContent).not.toContain("container with Docker");

    await userEvent.click(within(notice).getByRole("button", { name: "Let it go…" }));
    const asking = await screen.findByRole("alertdialog", { name: "Let runner qa go?" });
    expect(asking.textContent).toContain("Its saved home is removed from your Docker.");
    expect(asking.textContent).not.toContain("container");
  });

  it("offers Let it go on no runner that nothing keeps", async () => {
    const runners = shown.placement.runners;
    runners[1] = { ...(runners[1] as (typeof runners)[number]), draining: true };
    render(<DevicesView />);
    await waitFor(() => expect(screen.getByText("Runner qa of pool qa")).toBeInTheDocument());
    await waitFor(() => expect(runnersAsked).toContain("qa/qa"));

    expect(document.querySelector('[data-cause^="devices-kept-runner"]')).toBeNull();
    expect(screen.queryByRole("button", { name: "Let it go…" })).toBeNull();
  });

  it("leads a key-changed runner's row to removing its pool, asking first", async () => {
    asked["builds/builds"] = { ...asked["builds/builds"], link: { state: "key_changed" } };
    render(<DevicesView />);
    const builds = await waitFor(() => section("Runner builds of pool builds"));
    const remove = await within(builds).findByRole("button", { name: "Remove pool builds" });
    // No other runner's row offers it.
    expect(
      within(section("Runner qa of pool qa")).queryByRole("button", { name: /Remove pool/ }),
    ).toBeNull();

    await userEvent.click(remove);
    const asking = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    await userEvent.click(within(asking).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(removed).toEqual([]);

    const read = devicesRead;
    removeSays = "runner builds's home volume could not be removed";
    await userEvent.click(
      within(section("Runner builds of pool builds")).getByRole("button", {
        name: "Remove pool builds",
      }),
    );
    const again = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    // Remove pool is pressed once the question names what goes back (#4678).
    await waitFor(() =>
      expect(within(again).getByRole("button", { name: "Remove pool" })).toBeEnabled(),
    );
    await userEvent.click(within(again).getByRole("button", { name: "Remove pool" }));
    await waitFor(() => expect(removed).toEqual(["builds"]));
    // What could not be removed is said, and every device is read again.
    expect(
      await screen.findByText(/runner builds's home volume could not be removed/),
    ).toBeInTheDocument();
    await waitFor(() => expect(devicesRead).toBeGreaterThan(read));
  });

  it("asks the same question as Settings, naming what names the pool", async () => {
    asked["builds/builds"] = { ...asked["builds/builds"], link: { state: "key_changed" } };
    namedBy = [
      { path: "/home/op/docs", name: "docs", project: false, workspaces: ["site", "wiki"] },
    ];
    render(<DevicesView />);
    const builds = await waitFor(() => section("Runner builds of pool builds"));
    await userEvent.click(
      await within(builds).findByRole("button", { name: "Remove pool builds" }),
    );
    const asking = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    const list = await within(asking).findByRole("list", {
      name: "These go back to this machine:",
    });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((one) => one.textContent),
    ).toEqual(["Workspace site in docs", "Workspace wiki in docs"]);
    expect(removed).toEqual([]);
  });

  it("removes a pool whose older runner may hold conversations only after asking again, and says which (#4860)", async () => {
    asked["builds/builds"] = { ...asked["builds/builds"], link: { state: "key_changed" } };
    heldRunners = ["builds-0a0b0c0d"];
    render(<DevicesView />);
    const builds = await waitFor(() => section("Runner builds of pool builds"));
    await userEvent.click(
      await within(builds).findByRole("button", { name: "Remove pool builds" }),
    );
    const asking = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    await userEvent.click(await within(asking).findByRole("button", { name: "Remove pool…" }));
    const again = await screen.findByRole("alertdialog", {
      name: "Delete the conversations on runner builds-0a0b0c0d?",
    });

    await userEvent.click(
      within(again).getByRole("button", { name: "Remove pool and conversations" }),
    );

    await waitFor(() => expect(removed).toEqual(["builds"]));
    expect(discarded).toEqual([["builds-0a0b0c0d"]]);
  });

  it("leaves the line out of the question where nothing names the pool", async () => {
    asked["builds/builds"] = { ...asked["builds/builds"], link: { state: "key_changed" } };
    render(<DevicesView />);
    const builds = await waitFor(() => section("Runner builds of pool builds"));
    await userEvent.click(
      await within(builds).findByRole("button", { name: "Remove pool builds" }),
    );
    const asking = await screen.findByRole("alertdialog", { name: "Remove pool builds?" });
    // Once the question has read what names the pool, it says nothing of any.
    await waitFor(() =>
      expect(within(asking).getByRole("button", { name: "Remove pool" })).toBeEnabled(),
    );
    expect(within(asking).queryByText(/go back to this machine/)).toBeNull();
  });
});
