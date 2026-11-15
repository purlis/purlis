// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { UNDO_MS } from "./undo";
import type { InboxUpdate, Shown } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { forgetThisLaunch } from "./regions";
import { showNotices, showUpdates } from "./test-strips";
import { GLOBAL } from "./windowprefs";

/**
 * **Dismiss all, with Undo, on Mine, Project and Notices** (#5650, spec #5638, ADR 0119
 * decision 9), on the real window with the core mocked. Each list's Dismiss all is each line's
 * own Dismiss done to every line it lists: an update goes for its day, in the core's inbox
 * store; a Notice until its cause changes, in the layout file; a teammate's change is recorded
 * as seen. The lines go at once and an Undo stands for a few seconds; only when it goes is
 * anything kept, on this machine, and nothing is ever written into the plane.
 *
 * The pretend core keeps the updates as the core's inbox store does (`settle` marks them
 * dismissed, and a dismissed one is not read back), and the layout file's dismissals as
 * `set_dismissed` does; a relaunch hands the next window what the core left.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const AT = Math.floor(Date.now() / 1000) - 60;

const chat = {
  session: 1,
  name: "steward 1",
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
  in_front: true,
  resumed: null,
  fresh: null,
  profile: null,
  persona: null,
  unreported: null,
  guessed: null,
  untaken: null,
  pinned: false,
};

const update = (key: string, says: string): InboxUpdate & { dismissed?: boolean } => ({
  key,
  kind: "task-done",
  at: AT,
  session: null,
  chain: [],
  says,
  read: false,
});

type Asked = { cmd: string; args: Record<string, unknown> };

/**
 * What this machine keeps, past a relaunch: the inbox store's updates, the layout file's
 * dismissals, and the teammate changes recorded as seen.
 */
const machine: {
  updates: (InboxUpdate & { dismissed?: boolean })[];
  dismissed: Record<string, string[]>;
  hostsSeen: boolean;
  gone: string[];
  /** The window-only asks the core lists apart, each for its chat's finish row (R-329). */
  onFinishRows: Shown[];
} = { updates: [], dismissed: {}, hostsSeen: false, gone: [], onFinishRows: [] };

function core() {
  const asked: Asked[] = [];
  const kept = () => machine.updates.filter((one) => one.dismissed !== true);
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [
          {
            name: "alpha",
            path: `${PLANE}/workspaces/alpha`,
            vision: "",
            todos: [],
            chats: [chat],
          },
        ],
      };
    if (cmd === "opened_chats") return [chat];
    if (cmd === "reopened_layout") return aTabForEachChat([chat], () => "alpha");
    if (cmd === "plane_pins")
      return {
        project: false,
        workspaces: [],
        missing: [...machine.gone],
        order: [...machine.gone],
        certain: true,
      };
    if (cmd === "note_inbox_updates" && machine.onFinishRows.length > 0)
      // The finish rows the window notes are kept as the inbox store keeps them.
      for (const noted of a.noted as InboxUpdate[])
        if (noted.kind === "turn-finished" && !machine.updates.some((one) => one.key === noted.key))
          machine.updates.push({ ...noted, read: false });
    if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return kept();
    if (cmd === "asks_waiting")
      return { plane: PLANE, asks: [], with_asking_chat: [], on_finish_rows: machine.onFinishRows };
    if (cmd === "settle_inbox_updates") {
      const keys = a.keys as string[] | null;
      for (const one of machine.updates)
        if (keys === null || keys.includes(one.key)) {
          one.read = true;
          if (a.how === "dismissed") one.dismissed = true;
        }
      return kept();
    }
    if (cmd === "sandbox_state")
      return machine.hostsSeen
        ? {}
        : { hosts_changed: { added: ["api.example.com"], removed: [], now: ["api.example.com"] } };
    if (cmd === "acknowledge_project_hosts") {
      machine.hostsSeen = true;
      return {};
    }
    if (cmd === "set_dismissed") {
      const causes = a.causes as string[];
      const others = Object.entries(machine.dismissed).filter(([one]) => one !== a.plane);
      machine.dismissed = Object.fromEntries(
        causes.length > 0 ? [...others, [a.plane as string, causes]] : others,
      );
      return null;
    }
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    every: () => asked.map((one) => one.cmd),
  };
}

/** The window is handed the layout file as the core keeps it: Notices open in the Panel. */
function launch() {
  forgetThisLaunch();
  const dismissed = Object.keys(machine.dismissed).length > 0;
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: {
      path: "/home/op/.config/purlis/layout.json",
      found: true,
      document: {
        version: 2,
        regions: [
          { id: "navigation", side: "left", order: 0, collapsed: false },
          { id: "aside", side: "right", order: 0, collapsed: false, view: "asks" },
          { id: "panel-right", side: "right", order: 0, collapsed: false, view: "notices" },
        ],
        ...(dismissed ? { dismissed: structuredClone(machine.dismissed) } : {}),
      },
      trouble: null,
    },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  const one = core();
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  return one;
}

/** Quits, and launches again on what this machine kept. */
function relaunch() {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  return launch();
}

beforeEach(() => {
  machine.updates = [update("task:a", "Fix the build: done"), update("task:b", "Ship it: done")];
  machine.dismissed = {};
  machine.hostsSeen = false;
  machine.gone = ["delta", "echo"];
  machine.onFinishRows = [];
  vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  Reflect.deleteProperty(globalThis, GLOBAL);
  forgetThisLaunch();
  vi.useRealTimers();
});

/** Waits out the Undo. */
const undoGoes = () => act(() => vi.advanceTimersByTime(UNDO_MS + 1));

const mine = () => within(document.body).queryAllByText(/: done$/);
const dormant = () => within(document.body).queryAllByText(/is gone, kept dormant/);

async function mineListed() {
  const view = await showUpdates();
  await waitFor(() => expect(mine()).toHaveLength(2));
  return view;
}

async function noticesListed() {
  const view = await showNotices();
  await waitFor(() => expect(dormant()).toHaveLength(2));
  return view;
}

describe("Dismiss all on Mine (#5650)", () => {
  it("takes every line away at once, and keeps nothing while its Undo stands", async () => {
    const { asked } = launch();
    const view = await mineListed();

    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    expect(mine()).toHaveLength(0);
    expect(within(view).getByText("Dismissed 2 lines.")).toBeInTheDocument();
    expect(within(view).getByRole("button", { name: "Undo" })).toBeInTheDocument();
    expect(asked("settle_inbox_updates")).toEqual([]);
  });

  it("keeps them dismissed in the inbox store once the Undo goes, past a relaunch", async () => {
    const { asked } = launch();
    const view = await mineListed();
    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    undoGoes();

    await waitFor(() =>
      expect(asked("settle_inbox_updates")).toEqual([
        { plane: PLANE, keys: ["task:a", "task:b"], how: "dismissed" },
      ]),
    );
    expect(within(view).queryByRole("button", { name: "Undo" })).toBeNull();
    expect(mine()).toHaveLength(0);

    relaunch();
    await showUpdates();
    await screen.findByText("Nothing new from your chats");
    expect(mine()).toHaveLength(0);
  });

  it("brings every line back on Undo, and tells the core nothing", async () => {
    const { asked } = launch();
    const view = await mineListed();
    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    await userEvent.click(within(view).getByRole("button", { name: "Undo" }));
    undoGoes();

    expect(mine()).toHaveLength(2);
    expect(within(view).queryByText("Dismissed 2 lines.")).toBeNull();
    expect(asked("settle_inbox_updates")).toEqual([]);
  });

  it("stands an Undo after one line's Dismiss too", async () => {
    const { asked } = launch();
    const view = await mineListed();
    const line = screen.getByText("Fix the build: done").closest('[role="row"]') as HTMLElement;

    await userEvent.click(within(line).getByRole("button", { name: /^Dismiss/ }));

    expect(mine()).toHaveLength(1);
    expect(within(view).getByText("Dismissed 1 line.")).toBeInTheDocument();
    await userEvent.click(within(view).getByRole("button", { name: "Undo" }));
    expect(mine()).toHaveLength(2);
    expect(asked("settle_inbox_updates")).toEqual([]);
  });
});

describe("Dismiss all on Project (#5650)", () => {
  it("records a teammate's change as seen once the Undo goes, so it stays away", async () => {
    const { asked } = launch();
    const view = await showUpdates();
    await userEvent.click(within(view).getByRole("radio", { name: "Project, from the project" }));
    await within(view).findByText(/Added: api.example.com/);

    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    expect(within(view).queryByText(/Added: api.example.com/)).toBeNull();
    expect(asked("acknowledge_project_hosts")).toEqual([]);
    undoGoes();
    await waitFor(() => expect(asked("acknowledge_project_hosts")).toHaveLength(1));

    relaunch();
    const again = await showUpdates();
    await userEvent.click(within(again).getByRole("radio", { name: "Project, from the project" }));
    await within(again).findByText("Nothing new from the project");
  });

  it("brings a teammate's change back on Undo", async () => {
    launch();
    const view = await showUpdates();
    await userEvent.click(within(view).getByRole("radio", { name: "Project, from the project" }));
    await within(view).findByText(/Added: api.example.com/);
    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    await userEvent.click(within(view).getByRole("button", { name: "Undo" }));

    expect(within(view).getByText(/Added: api.example.com/)).toBeInTheDocument();
  });
});

/** Chat 1's refused mint, for a clone purlis has no record of, its turn ended (R-330). */
const TRUST_CLONE: Shown = {
  session: 1,
  ask: "trust-clone:1:workspaces/alpha/api|github.com/acme/api",
  says: "Trust this clone so chats can use GitHub: workspaces/alpha/api was not cloned by purlis, and its remote is github.com/acme/api. Trust it only if you made this clone yourself.",
  options: [{ id: "press", label: "Trust this clone", allows: true }],
  source: "trust-clone",
  chain: ["steward 1"],
  answer: { via: "trust-clone", folder: "workspaces/alpha/api", place: "github.com/acme/api" },
  since: AT,
};

describe("a finish row that holds a window-only ask (Q274)", () => {
  const trustRow = () =>
    screen.queryByText(/^Trust this clone so chats can use GitHub/)?.closest('[role="row"]') ??
    null;

  it("has no Dismiss, and Dismiss all leaves it while it waits", async () => {
    machine.onFinishRows = [TRUST_CLONE];
    const { asked } = launch();
    const view = await mineListed();
    await waitFor(() => expect(trustRow()).not.toBeNull());
    expect(
      within(trustRow() as HTMLElement).queryByRole("button", { name: /^Dismiss/ }),
    ).toBeNull();

    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));
    undoGoes();

    await waitFor(() => expect(asked("settle_inbox_updates")).toHaveLength(1));
    expect(asked("settle_inbox_updates")).toEqual([
      { plane: PLANE, keys: ["task:a", "task:b"], how: "dismissed" },
    ]);
    expect(mine()).toHaveLength(0);
    expect(trustRow()).not.toBeNull();
  });
});

describe("Dismiss all on Notices (#5650)", () => {
  it("is offered only while the view lists a line", async () => {
    machine.gone = [];
    launch();
    const view = await showNotices();
    await within(view).findByText("No Notices");
    expect(within(view).queryByRole("button", { name: "Dismiss all" })).toBeNull();
  });

  it("takes every Notice away with an Undo, and keeps each until its cause changes", async () => {
    const { asked } = launch();
    const view = await noticesListed();

    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    expect(dormant()).toHaveLength(0);
    expect(within(view).getByText("Dismissed 2 lines.")).toBeInTheDocument();
    expect(asked("set_dismissed")).toEqual([]);
    undoGoes();
    await waitFor(() =>
      expect(machine.dismissed[PLANE]).toEqual(["pin-dormant:delta", "pin-dormant:echo"]),
    );

    // A relaunch keeps them away while their cause stands.
    relaunch();
    await showNotices();
    await screen.findByText("No Notices");

    // Delta comes back and goes again: a new cause, and its Notice returns.
    machine.gone = ["echo"];
    relaunch();
    await showNotices();
    await waitFor(() => expect(machine.dismissed[PLANE]).toEqual(["pin-dormant:echo"]));
    machine.gone = ["delta", "echo"];
    relaunch();
    await showNotices();
    await waitFor(() => expect(dormant()).toHaveLength(1));
    expect(screen.getByText(/^delta is gone, kept dormant/)).toBeInTheDocument();
  });

  it("brings every Notice back on Undo, and keeps nothing", async () => {
    const { asked } = launch();
    const view = await noticesListed();
    await userEvent.click(within(view).getByRole("button", { name: "Dismiss all" }));

    await userEvent.click(within(view).getByRole("button", { name: "Undo" }));
    undoGoes();

    expect(dormant()).toHaveLength(2);
    expect(asked("set_dismissed")).toEqual([]);
  });

  it("stands an Undo after one Notice's Dismiss too", async () => {
    launch();
    const view = await noticesListed();
    const delta = screen.getByText(/^delta is gone, kept dormant/).closest("[data-cause]");

    await userEvent.click(within(delta as HTMLElement).getByRole("button", { name: "Dismiss" }));

    expect(dormant()).toHaveLength(1);
    await userEvent.click(within(view).getByRole("button", { name: "Undo" }));
    expect(dormant()).toHaveLength(2);
  });
});

describe("what Dismiss all keeps (#5650)", () => {
  it("is kept on this machine alone: nothing is written into the plane", async () => {
    const { every } = launch();
    const notices = await noticesListed();
    await userEvent.click(within(notices).getByRole("button", { name: "Dismiss all" }));
    const updates = await mineListed();
    await userEvent.click(within(updates).getByRole("button", { name: "Dismiss all" }));
    const before = every().length;
    undoGoes();
    await waitFor(() => expect(every().length).toBeGreaterThan(before));

    // What was written is the inbox store and the layout file, the machine's (ADR 0069), and
    // nothing of the plane's.
    const writes = new Set(
      every()
        .slice(before)
        .filter((cmd) => /^(set_|settle_|acknowledge_|write_|save_|note_|commit)/.test(cmd)),
    );
    expect([...writes].sort()).toEqual(["set_dismissed", "settle_inbox_updates"]);
  });
});
