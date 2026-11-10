// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { DoctorRow, InboxBaseline, InboxUpdate } from "./bindings";
import { forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";
import { viewTab } from "./test-strips";

/**
 * **The first-open baseline** (#5648, ADR 0119), against the whole window: the first time a
 * plane opens on a machine with no seen record for it, what is already there counts as seen
 * and is listed under Updates' Project, read; the open asks the person may answer stay in Asks.
 *
 * The core keeps the record and writes the teammate Notices' seen records (its own tests say
 * how). This fake answers as the core does: the project's hosts and presets read as changed
 * until `inbox_first_open` has run on a machine that had never seen them.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const NOW = Date.parse("2026-11-07T12:00:00Z");
const secs = (ms: number) => Math.floor(ms / 1000);

/** What a teammate committed: a host and a preset, which this machine has never told. */
const TEAMMATES = {
  hosts_changed: { added: ["api.example.com"], removed: [], now: ["api.example.com"] },
  presets_changed: {
    added: ["Package registries"],
    removed: [],
    widens: [],
    now: ["model-providers", "toolchains"],
  },
};

const IDENTITY: DoctorRow = {
  name: "git identity",
  status: "fail",
  detail: "not set: user.email",
  hint: "",
  checked: true,
  settings: null,
  fix: "git-identity",
};
const PERSONAS: DoctorRow = {
  name: "personas",
  status: "warn",
  detail: "reviewer has no charter",
  hint: "",
  checked: true,
  settings: null,
  fix: "persona-agents",
};

type Asked = { cmd: string; args: Record<string, unknown> };

/**
 * The core, for one project. `record` is the baseline this machine already holds for it, or
 * none: then `inbox_first_open` makes one, fresh where `seenBefore` is false.
 */
function core({
  record = null,
  seenBefore = false,
  findings = [IDENTITY],
  asks = [],
  kept = [],
}: {
  record?: InboxBaseline | null;
  seenBefore?: boolean;
  findings?: DoctorRow[];
  asks?: unknown[];
  kept?: InboxUpdate[];
}) {
  const asked: Asked[] = [];
  let baseline = record;
  const updates = [...kept];
  /** Whether this machine holds the teammate Notices' seen records. */
  const told = () => seenBefore || baseline?.fresh === true;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          { name: "alpha", path: `${PLANE}/workspaces/alpha`, vision: "", todos: [], chats: [] },
        ],
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "plane_doctor")
      return {
        rows: [{ ...IDENTITY, status: "ok" }, ...findings],
        app_rows: [],
        full: false,
        path: "",
      };
    if (cmd === "inbox_first_open") {
      baseline ??= { at: secs(NOW), fresh: !seenBefore, findings: null };
      return baseline;
    }
    if (cmd === "set_aside_inbox_findings") {
      if (baseline?.fresh === true && baseline.findings === null)
        baseline = { ...baseline, findings: given.keys as string[] };
      return updates;
    }
    if (cmd === "sandbox_state")
      return told() ? { hosts_changed: null, presets_changed: null } : TEAMMATES;
    if (cmd === "asks_waiting") return { plane: PLANE, asks };
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    if (cmd === "inbox_updates") return updates;
    if (cmd === "note_inbox_updates") {
      for (const one of given.noted as InboxUpdate[])
        if (!updates.some((kept) => kept.key === one.key)) updates.push({ ...one, read: false });
      return updates;
    }
    if (cmd === "opened_chats" || cmd === "chat_states" || cmd === "running_sessions") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return asked;
}

const tab = (name: string) =>
  within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", {
    name: viewTab(name),
  });

/** The Notices view (#5645), in the Panel's right half, which the window opens on. */
const notices = async () => screen.findByRole("tabpanel", { name: "Notices" });
const noticesTab = () =>
  within(screen.getByRole("tablist", { name: "Panel, right half" })).getByRole("tab", {
    name: /^Notices\b/,
  });

/** Updates, shown as a person presses its icon. */
async function showUpdates() {
  await screen.findByRole("tabpanel", { name: "Asks" });
  await userEvent.click(tab("Updates"));
  return screen.findByRole("tabpanel", { name: "Updates" });
}

/** Updates, switched to Project. */
async function project() {
  const updates = screen.queryByRole("tabpanel", { name: "Updates" }) ?? (await showUpdates());
  await userEvent.click(within(updates).getByRole("radio", { name: "Project, from the project" }));
  return updates;
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(NOW));
  inboxOpenAtLaunch();
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
  vi.useRealTimers();
});

describe("the first-open baseline (#5648)", () => {
  it("opens a fresh clone quiet: no teammate change listed as new, no finding standing", async () => {
    const asked = core({});
    render(<App />);
    const updates = await project();

    // The teammates' host and preset: told by the baseline, so not listed as new.
    await waitFor(() => expect(asked.map((one) => one.cmd)).toContain("sandbox_state"));
    expect(within(updates).queryByText(/api\.example\.com/)).toBeNull();
    expect(within(updates).queryByText(/Package registries/)).toBeNull();
    // The doctor's finding already there: put away until it changes, so no line stands and
    // the Notices icon has no dot.
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "set_aside_inbox_findings",
        args: { plane: PLANE, keys: ["doctor-finding:git-identity"] },
      }),
    );
    await waitFor(() =>
      expect(
        within(screen.getByRole("tabpanel", { name: "Notices" })).queryByText(/^Doctor: /),
      ).toBeNull(),
    );
    expect(noticesTab().querySelector('[data-part="count"]')).toBeNull();
    expect(tab("Updates").querySelector('[data-part="count"]')).toBeNull();
    expect(asked.filter((one) => one.cmd === "inbox_first_open")).toHaveLength(1);
  });

  it("keeps the open asks the person may answer in Asks", async () => {
    core({
      asks: [
        {
          session: 3,
          ask: "ask-3",
          says: "Run cargo test",
          options: [
            { id: "allow", label: "Allow", allows: true },
            { id: "deny", label: "Deny", allows: false },
          ],
          source: "permission",
          chain: ["steward 3"],
          answer: { via: "hook" },
        },
      ],
    });
    render(<App />);
    await waitFor(() => expect(tab("Asks")).toHaveAccessibleName("Asks 1 thing waits on you"));
  });

  it("lists a line baselined at the first open under Project, read, never under Mine", async () => {
    core({
      record: { at: secs(NOW) - 60, fresh: true, findings: [] },
      findings: [],
      kept: [
        {
          key: "task:a",
          kind: "task-done",
          at: secs(NOW) - 3600,
          session: 3,
          chain: ["steward 3"],
          says: "Fix the build: done",
          read: true,
          baselined: true,
        },
      ],
    });
    render(<App />);
    const updates = await showUpdates();
    expect(await within(updates).findByText("Nothing new from your chats")).toBeInTheDocument();
    expect(within(updates).queryByText(/Fix the build/)).toBeNull();
    await project();
    expect(await within(updates).findByText(/Fix the build: done/)).toBeInTheDocument();
    expect(tab("Updates").querySelector('[data-part="count"]')).toBeNull();
  });

  it("lists a doctor finding that appeared after the first open as new", async () => {
    const asked = core({
      record: { at: secs(NOW) - 600, fresh: true, findings: ["doctor-finding:git-identity"] },
      // A failure, so it is trouble the dot speaks for.
      findings: [{ ...PERSONAS, status: "fail" }],
    });
    render(<App />);
    expect(await within(await notices()).findByText("Doctor: 1 finding")).toBeInTheDocument();
    await waitFor(() => expect(noticesTab().querySelector('[data-part="count"]')).not.toBeNull());
    expect(asked.map((one) => one.cmd)).not.toContain("set_aside_inbox_findings");
  });

  it("lists a finding the doctor finds when asked again after a fresh first open", async () => {
    // Nothing to find at the first open: its report is set aside with no finding.
    const found: DoctorRow[] = [];
    const asked = core({ findings: found });
    render(<App />);
    await waitFor(() =>
      expect(asked).toContainEqual({
        cmd: "set_aside_inbox_findings",
        args: { plane: PLANE, keys: [] },
      }),
    );
    // Then the identity goes, and the person asks the doctor again from its status item.
    found.push({ ...PERSONAS, status: "fail" });
    await userEvent.click(screen.getByTestId("status-doctor"));
    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "plane_doctor").length).toBeGreaterThan(1),
    );
    await userEvent.keyboard("{Escape}");
    expect(await within(await notices()).findByText("Doctor: 1 finding")).toBeInTheDocument();
  });

  it("never baselines a plane this machine has a record for: an update keeps its unread lines", async () => {
    const asked = core({
      record: { at: secs(NOW) - 86_000, fresh: false, findings: null },
      kept: [
        {
          key: "task:a",
          kind: "task-done",
          at: secs(NOW) - 60,
          session: 3,
          chain: ["steward 3"],
          says: "Fix the build: done",
          read: false,
        },
      ],
    });
    render(<App />);
    await waitFor(() =>
      expect(tab("Updates")).toHaveAccessibleName("Updates 1 unread update from your chats"),
    );
    expect(await within(await notices()).findByText("Doctor: 1 finding")).toBeInTheDocument();
    expect(asked.map((one) => one.cmd)).not.toContain("set_aside_inbox_findings");
  });
});
