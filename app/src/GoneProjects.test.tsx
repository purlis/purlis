// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  configure,
  render as renderBare,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { forgetThisLaunch } from "./regions";
import {
  dismissGone,
  forgetGoneDismissals,
  settleGone,
  useGoneDismissed,
} from "./GoneProjectNotice";
import type { GoneProject } from "./bindings";
import { openTheInbox, stripNamed, closeFromTheRail } from "./test-strips";
import { GLOBAL } from "./windowprefs";
import { dismissedIn, goneKept, letGoneGo } from "./dismissals";

/**
 * **A project that is gone offers Locate… and Forget** (NO-5, #1237): in the window's Notice
 * for a project the last quit had open, and on the opener for a recent. Never done by charter
 * on its own, because a disk that is unplugged may come back.
 *
 * Forget is the core's (ST-2), so a gone project stops coming back at every launch. Locate…
 * asks for a folder, the core checks there is a project there and re-points the entry, and the
 * window opens what it found, through the trust gate like any open.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);
// A whole window comes up through several answers; under a loaded machine that takes more
// than the default second.
configure({ asyncUtilTimeout: 5_000 });

afterEach(() => {
  cleanup();
  clearMocks();
  // An Inbox a test opened is remembered as the side's view: the next test starts without it.
  forgetThisLaunch();
  forgetGoneDismissals();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

const TWO = "/home/dev/two";
const FOUND = "/media/dev/moved";
const GONE: GoneProject = { path: "/home/dev/gone", said: "~/dev/gone is no longer there" };

function sidebarOf(root: string) {
  return { root, workspaces: [], personas: [], persona: null, unfiled: [] };
}

/**
 * A core with one gone project, either among the last quit's tabs (`restore`) or among the
 * recents (`recent`), with every call kept. `picked` is what the folder picker answers, and
 * `locate` what the core answers to the re-point.
 */
function core(over: {
  /** "restore, a recent": the last quit had it open, and it is a recent as well. */
  where: "restore" | "recent" | "both" | "restore, a recent";
  /** What the folder picker answers; `{ failed }` is a picker that could not finish. */
  picked?: string | null | { failed: string };
  locate?: { ok: string } | { refused: string };
}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let remembered = true;
  /** Whether the gone project is still gone where the store remembers it. */
  let away = true;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return {
        // "both": nothing came back, so the window draws the opener, which lists it too.
        windows: over.where.startsWith("restore") ? [{ planes: [TWO], active: 0 }] : [],
        dropped: [],
        gone: over.where !== "recent" && remembered ? [GONE] : [],
      };
    if (cmd === "recent_planes")
      return {
        planes: [{ path: TWO, name: "two", approved: true }],
        dropped: [],
        gone: over.where !== "restore" && remembered && away ? [GONE] : [],
        forgetful: null,
      };
    if (cmd === "pick_project") {
      if (over.picked === undefined) return FOUND;
      if (over.picked !== null && typeof over.picked === "object") throw over.picked.failed;
      return over.picked;
    }
    if (cmd === "locate_project") {
      const answer = over.locate ?? { ok: FOUND };
      if ("refused" in answer) throw answer.refused;
      remembered = false;
      return answer.ok;
    }
    if (cmd === "forget_project") {
      remembered = false;
      return null;
    }
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "close_plane") return null;
    if (cmd === "plane_sidebar") return sidebarOf((given.plane as string) ?? "");
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "extensions_on") return [];
    return null;
  });
  return {
    asked,
    /** The disk holding the gone project comes back (`true`) or goes again. */
    plugged: (back: boolean) => {
      away = !back;
    },
    sent: (name: string) => asked.filter((one) => one.cmd === name).map((one) => one.args),
  };
}

const projectTabs = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .map((tab) => tab.getAttribute("aria-label"));

/** The Notice about the gone project, wherever it stands. */
const gone = async () =>
  (await screen.findByText(GONE.said)).closest<HTMLElement>("[data-cause]") as HTMLElement;

describe("a project the last quit had open that is gone", () => {
  it("offers Locate… and Forget, and opens the rest", async () => {
    core({ where: "restore" });
    render(<App />);
    // A project is in front: the window's own lines are listed in its Inbox (#1695).
    await openTheInbox();

    const notice = await gone();
    expect(within(notice).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    expect(within(notice).getByRole("button", { name: "Forget" })).toBeInTheDocument();
    await waitFor(() => expect(projectTabs()).toEqual(["two"]));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("forgets it on this machine, so it no longer comes back at launch", async () => {
    const { sent } = core({ where: "restore" });
    render(<App />);
    // A project is in front: the window's own lines are listed in its Inbox (#1695).
    await openTheInbox();

    await userEvent.click(within(await gone()).getByRole("button", { name: "Forget" }));

    await waitFor(() => expect(screen.queryByText(GONE.said)).not.toBeInTheDocument());
    expect(sent("forget_project")).toEqual([{ path: GONE.path }]);
  });

  it("re-points it at a picked folder once the core has checked it, and opens it", async () => {
    const { sent } = core({ where: "restore" });
    render(<App />);
    // A project is in front: the window's own lines are listed in its Inbox (#1695).
    await openTheInbox();
    await waitFor(() => expect(projectTabs()).toEqual(["two"]));

    await userEvent.click(within(await gone()).getByRole("button", { name: "Locate…" }));

    await waitFor(() => expect(projectTabs()).toEqual(["two", "moved"]));
    expect(sent("locate_project")).toEqual([{ gone: GONE.path, picked: FOUND }]);
    // Through `open_plane`, the trust gate: the approval did not travel with the path.
    expect(sent("open_plane").map((one) => one.path)).toContain(FOUND);
    expect(screen.queryByText(GONE.said)).not.toBeInTheDocument();
  });

  it("says why the core refused a folder, and keeps the Notice", async () => {
    const refused = "~/Downloads is not a project: purlis found none there or above it";
    const { sent } = core({ where: "restore", picked: "/home/dev/Downloads", locate: { refused } });
    render(<App />);
    // A project is in front: the window's own lines are listed in its Inbox (#1695).
    await openTheInbox();

    await userEvent.click(within(await gone()).getByRole("button", { name: "Locate…" }));

    expect(await screen.findByText(refused)).toBeInTheDocument();
    expect(within(await gone()).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    expect(sent("open_plane").map((one) => one.path)).not.toContain("/home/dev/Downloads");
  });

  it("does nothing when the picker is cancelled", async () => {
    const { sent } = core({ where: "restore", picked: null });
    render(<App />);
    // A project is in front: the window's own lines are listed in its Inbox (#1695).
    await openTheInbox();

    await userEvent.click(within(await gone()).getByRole("button", { name: "Locate…" }));

    await waitFor(() => expect(sent("pick_project")).toHaveLength(1));
    expect(sent("locate_project")).toEqual([]);
    expect(await gone()).toBeInTheDocument();
  });

  it("says so when the picker fails, unlike a cancel, and keeps the Notice", async () => {
    const failed = "the folder picker did not finish: the dialog went away";
    const { sent } = core({ where: "restore", picked: { failed } });
    render(<App />);
    // A project is in front: the window's own lines are listed in its Inbox (#1695).
    await openTheInbox();

    await userEvent.click(within(await gone()).getByRole("button", { name: "Locate…" }));

    expect(await screen.findByText(failed)).toBeInTheDocument();
    expect(sent("locate_project")).toEqual([]);
    expect(within(await gone()).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
  });
});

describe("a recent that is gone, on the opener", () => {
  it("offers Locate… and Forget, and Forget takes it off the list", async () => {
    const { sent } = core({ where: "recent" });
    render(<App />);

    const notice = await gone();
    expect(within(notice).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    await userEvent.click(within(notice).getByRole("button", { name: "Forget" }));

    await waitFor(() => expect(screen.queryByText(GONE.said)).not.toBeInTheDocument());
    expect(sent("forget_project")).toEqual([{ path: GONE.path }]);
    expect(screen.getByRole("row", { name: /two/ })).toBeInTheDocument();
  });

  it("re-points it at a picked folder and opens the project found there", async () => {
    const { sent } = core({ where: "recent" });
    render(<App />);

    await userEvent.click(within(await gone()).getByRole("button", { name: "Locate…" }));

    await waitFor(() => expect(sent("open_plane").map((one) => one.path)).toEqual([FOUND]));
    expect(sent("locate_project")).toEqual([{ gone: GONE.path, picked: FOUND }]);
  });
});

describe("a gone project both the restore and the opener name", () => {
  // Nothing came back, so the window draws the opener, and both lists hold the same project.
  // The opener owns it: one Notice, and settling it leaves no copy behind anywhere.
  const copies = () => screen.queryAllByText(GONE.said);

  it("is drawn once", async () => {
    core({ where: "both" });
    render(<App />);

    await screen.findByRole("heading", { level: 1 });
    await waitFor(() => expect(copies()).toHaveLength(1));
    expect(within(await gone()).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
  });

  it("leaves nothing stale once it is forgotten", async () => {
    const { sent } = core({ where: "both" });
    render(<App />);
    await waitFor(() => expect(copies()).toHaveLength(1));

    await userEvent.click(within(await gone()).getByRole("button", { name: "Forget" }));

    await waitFor(() => expect(sent("recent_planes").length).toBeGreaterThan(1));
    await waitFor(() => expect(copies()).toHaveLength(0));
    expect(sent("forget_project")).toEqual([{ path: GONE.path }]);
  });

  it("leaves nothing stale once it is located and open", async () => {
    core({ where: "both" });
    render(<App />);
    await waitFor(() => expect(copies()).toHaveLength(1));

    await userEvent.click(within(await gone()).getByRole("button", { name: "Locate…" }));

    await waitFor(() => expect(projectTabs()).toEqual(["moved"]));
    expect(copies()).toHaveLength(0);
  });
});

describe("a gone project dismissed (#1291)", () => {
  it("stays dismissed for the run when the opener comes back and lists it", async () => {
    core({ where: "restore, a recent" });
    render(<App />);
    await openTheInbox();
    await userEvent.click(within(await gone()).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.queryByText(GONE.said)).not.toBeInTheDocument());

    // The last project closes: the opener is drawn, and its recents still list the gone one.
    await closeFromTheRail("two");

    await screen.findByRole("heading", { level: 1 });
    await screen.findByRole("row", { name: /two/ });
    expect(screen.queryByText(GONE.said)).not.toBeInTheDocument();
  });

  it("shows again once the core found it, and it goes again", async () => {
    const { plugged } = core({ where: "restore, a recent" });
    render(<App />);
    await openTheInbox();
    await userEvent.click(within(await gone()).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.queryByText(GONE.said)).not.toBeInTheDocument());

    // The disk comes back: the opener's answer no longer lists it as gone.
    plugged(true);
    await closeFromTheRail("two");
    await userEvent.click(await screen.findByRole("row", { name: /two/ }));
    await waitFor(() => expect(projectTabs()).toEqual(["two"]));

    // And it goes again: a new cause, so its line is drawn.
    plugged(false);
    await closeFromTheRail("two");
    expect(await gone()).toBeInTheDocument();
  });
});

describe("the run's list of dismissed gone projects (#1291)", () => {
  it("holds a dismissal for its own path only, and lets it go when the core stops listing it", () => {
    const { result } = renderHook(() => useGoneDismissed());
    act(() => dismissGone("/disk/one"));
    expect(result.current.has("/disk/one")).toBe(true);
    // Another gone project, even one whose path starts the same, is not hidden by it.
    expect(result.current.has("/disk/one-more")).toBe(false);
    expect(result.current.has("/disk")).toBe(false);

    // The core still lists both as gone: nothing changes.
    act(() => settleGone(["/disk/one", "/disk/one-more"]));
    expect(result.current.has("/disk/one")).toBe(true);
    // It no longer lists it (back, located or forgotten): let go.
    act(() => settleGone(["/disk/one-more"]));
    expect(result.current.has("/disk/one")).toBe(false);
  });
});

describe("a gone project dismissed, at the next launch (#1291)", () => {
  const KEPT_CAUSE = `project-gone:${GONE.path}`;

  /** Quits, and launches again on a layout file that keeps `dismissed`, as the core left it. */
  function relaunch(dismissed: Record<string, string[]>) {
    cleanup();
    clearMocks();
    forgetThisLaunch();
    forgetGoneDismissals();
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "layout.json",
        found: true,
        document: { version: 1, regions: [], dismissed },
        trouble: null,
      },
      theme: { path: "theme.json", found: false, document: null, trouble: null },
    };
  }

  it("is kept under its own path with its own cause, though no window holds it", async () => {
    const { sent } = core({ where: "restore" });
    render(<App />);
    await openTheInbox();
    await userEvent.click(within(await gone()).getByRole("button", { name: "Dismiss" }));

    await waitFor(() =>
      expect(sent("set_dismissed")).toEqual([{ plane: GONE.path, causes: [KEPT_CAUSE] }]),
    );
  });

  it("is not drawn by the window's line nor by the opener once relaunched", async () => {
    relaunch({ [GONE.path]: [KEPT_CAUSE] });
    const { sent } = core({ where: "restore, a recent" });
    render(<App />);
    await openTheInbox();
    await waitFor(() => expect(projectTabs()).toEqual(["two"]));
    // The core is asked whether it is still gone, and it is: the dismissal stands.
    await waitFor(() => expect(sent("recent_planes").length).toBeGreaterThan(0));
    expect(screen.queryByText(GONE.said)).not.toBeInTheDocument();

    await closeFromTheRail("two");
    await screen.findByRole("row", { name: /two/ });
    expect(screen.queryByText(GONE.said)).not.toBeInTheDocument();
    expect(sent("set_dismissed")).toEqual([]);
  });

  it("is let go at launch once the core no longer lists it as gone", async () => {
    relaunch({ [GONE.path]: [KEPT_CAUSE] });
    const { sent, plugged } = core({ where: "restore, a recent" });
    plugged(true);
    render(<App />);

    await waitFor(() => expect(sent("set_dismissed")).toEqual([{ plane: GONE.path, causes: [] }]));
  });

  it("is let go when it is located or forgotten, wherever that was done", async () => {
    // Its line is not drawn, so the press was elsewhere (Settings › You › This machine), and
    // the window hears of it through `whenRecentSettled`, which lets the kept dismissal go.
    relaunch({ [GONE.path]: [KEPT_CAUSE] });
    const { sent } = core({ where: "recent" });
    expect(goneKept()).toEqual([GONE.path]);

    letGoneGo(GONE.path);

    expect(goneKept()).toEqual([]);
    await waitFor(() => expect(sent("set_dismissed")).toEqual([{ plane: GONE.path, causes: [] }]));
  });

  it("keeps a gone project's cause only under its own path", () => {
    relaunch({ "/home/dev/other": [KEPT_CAUSE], [GONE.path]: [KEPT_CAUSE, "project-gone:/x"] });
    expect(goneKept()).toEqual([GONE.path]);
    expect([...dismissedIn(GONE.path)]).toEqual([KEPT_CAUSE]);
    expect(dismissedIn("/home/dev/other").size).toBe(0);
  });
});
