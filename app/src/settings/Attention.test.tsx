// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DispatchLimits, DispatchLimitRow } from "../bindings";
import { notificationPrefs } from "../notificationPrefs";
import { forgetThisLaunch } from "../regions";
import { DISPATCH, dispatchGroup } from "./dispatch";
import type { LiveSetting } from "./groups";
import { notificationsGroup } from "./notifications";

/**
 * **Settings › You › Attention** (#6019, ADR 0121 decision 22): one page for Quiet, the
 * notifications per source, the Expiring sound and the Stuck? thresholds, which moved off
 * Settings › Project › Dispatch and stay the project's.
 */

const PLANE = "/home/dev/plane";

const LIMITS: DispatchLimits["limits"] = [
  ["running-per-chat", "Running per chat", 6],
  ["depth", "Depth", 3],
  ["stuck-silent-minutes", "Silent minutes", 15],
  ["stuck-failing-times", "Failing times", 3],
  ["stuck-long-minutes", "Turn minutes", 60],
].map(([word, label, standard]) => ({
  word: word as string,
  label: label as string,
  help: `What ${String(label).toLowerCase()} limits.`,
  default: standard as number,
  persona_only: false,
  most: 10000,
  ceiling: null,
}));

const DEFAULTS = [6, 3, 15, 3, 60];

function row(scope: string, name: string, values: (number | null)[]): DispatchLimitRow {
  return { scope, name, values, beneath: DEFAULTS, ignored: [] };
}

function page(rows: DispatchLimitRow[]): DispatchLimits {
  return {
    file: "purlis.toml",
    local_file: "purlis.local.toml",
    base: "schema = 1\n",
    local_base: null,
    limits: LIMITS,
    rows,
    mine: [],
    locked_by: null,
    refused: [],
    local_left_out: null,
    policy_refused: false,
  };
}

let sent: { cmd: string; args: unknown }[] = [];

function core(limits: DispatchLimits = page([row("project", "", [null, null, null, null, null])])) {
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    if (cmd === "dispatch_limits") return limits;
    if (cmd === "start_options") return { profiles: [], refused: [], personas: [] };
    if (cmd === "plane_sidebar") return { workspaces: [{ name: "alpha" }] };
    if (cmd === "save_project_settings") return { kind: "saved", file: {} };
    if (cmd === "quiet_now") return { quiet: false, by_hand: false, ended: null };
    return null;
  });
}

beforeEach(() => {
  forgetThisLaunch();
  sent = [];
  core();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

function Row({ id, plane }: { id: string; plane?: string }) {
  const setting = [...notificationsGroup(plane).settings, ...dispatchGroup(PLANE).settings].find(
    (one) => one.id === id,
  ) as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: "row", label: setting.label, labelledBy: undefined })}</>;
}

const label = (id: string, plane?: string) =>
  notificationsGroup(plane).settings.find((one) => one.id === id)?.label;

describe("the Attention page", () => {
  it("holds Quiet, the sources, the Expiring sound and, in a project, Stuck?", () => {
    const group = notificationsGroup(PLANE);
    expect(group.label).toBe("Attention");
    // Notifications' address, kept: a link that named it still lands here.
    expect(group.id).toBe("you.notifications");
    expect(group.settings.map((one) => one.label)).toEqual([
      "Quiet hours",
      "Quiet from",
      "Quiet until",
      "Quiet now",
      "Quiet while you type",
      "Asks that stop a chat",
      "Notify when a task fails",
      "Notify when a long task finishes",
      "Long means more than",
      "Sound for an expiring ask",
      "Stuck? for this project",
    ]);
    expect(notificationsGroup().settings.map((one) => one.id)).not.toContain(
      "you.notifications.stuck",
    );
  });

  it("says which settings are this project's and which are this machine's", () => {
    const group = notificationsGroup(PLANE);
    expect(group.help).toContain("kept on this machine, except Stuck?, which is this project's");
    const stuck = group.settings.find((one) => one.id === "you.notifications.stuck");
    expect(stuck?.help).toContain("This project's: kept in its settings files");
  });
});

describe("notifications per source", () => {
  it("never offers a way to turn off an ask that stops a chat", () => {
    render(<Row id="you.notifications.blocking" />);
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.getByText("Always notify")).toBeInTheDocument();
    expect(label("you.notifications.blocking")).toBe("Asks that stop a chat");
  });

  it("turns a failed task's notification off, and keeps it in the layout file", async () => {
    render(<Row id="you.notifications.task-failed" />);
    const on = screen.getByRole("switch");
    expect(on).toBeChecked();

    await userEvent.click(on);

    expect(notificationPrefs().off).toEqual(["task-failed"]);
    await waitFor(() => expect(screen.getByRole("switch")).not.toBeChecked());
  });

  it("puts the minutes of a long finish out of reach while long finishes are off", async () => {
    render(
      <>
        <Row id="you.notifications.task-done" />
        <Row id="you.notifications.finishedAfter" />
      </>,
    );
    const minutes = screen.getByRole("textbox");
    expect(minutes).toHaveValue("5");
    expect(minutes).toBeEnabled();

    fireEvent.change(minutes, { target: { value: "12" } });
    fireEvent.blur(minutes);
    expect(notificationPrefs().finishedAfter).toBe(12);

    await userEvent.click(screen.getByRole("switch"));
    await waitFor(() => expect(screen.getByRole("textbox")).toBeDisabled());
  });
});

describe("Quiet now", () => {
  it("asks the core to hold notifications", async () => {
    render(<Row id="you.notifications.quietNow" />);
    await userEvent.click(screen.getByRole("switch"));
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "set_quiet_now")?.args).toEqual({ on: true }),
    );
  });
});

describe("the Expiring sound", () => {
  it("is off until turned on", async () => {
    render(<Row id="you.notifications.expiringSound" />);
    const on = screen.getByRole("switch");
    expect(on).not.toBeChecked();
    await userEvent.click(on);
    expect(notificationPrefs().expiringSound).toBe(true);
  });

  it("says when it plays, and no longer that it does not", () => {
    const sound = notificationsGroup().settings.find(
      (one) => one.id === "you.notifications.expiringSound",
    );
    expect(sound?.help).toContain("15 seconds left");
    expect(sound?.help).not.toContain("plays no sound");
  });
});

describe("the Stuck? thresholds, moved off Dispatch", () => {
  it("are drawn on Attention alone, with the project's row", async () => {
    render(<Row id="you.notifications.stuck" plane={PLANE} />);
    const table = await screen.findByRole("table", { name: "Stuck? thresholds" });
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((one) => one.getAttribute("aria-label") ?? one.textContent),
    ).toEqual(["Level", "Silent minutes", "Failing times", "Turn minutes", "Actions"]);
    expect(within(table).getByRole("rowheader", { name: "Project" })).toBeInTheDocument();
    expect(screen.getByText(/Kept in purlis\.toml, which your team sees/)).toBeInTheDocument();
  });

  it("are not a column of Dispatch, which points to Attention", async () => {
    render(<Row id={`${DISPATCH}.limits`} />);
    const table = await screen.findByRole("table", { name: "Dispatch limits" });
    const columns = within(table)
      .getAllByRole("columnheader")
      .map((one) => one.getAttribute("aria-label") ?? one.textContent);
    expect(columns).toEqual(["Level", "Running per chat", "Depth", "Actions"]);
    expect(dispatchGroup(PLANE).links).toEqual([
      { label: "Go to Attention…", action: "settings.group:you.notifications" },
    ]);
  });

  it("are written to the project's file, at the key #6014 reads", async () => {
    render(<Row id="you.notifications.stuck" plane={PLANE} />);
    const box = await screen.findByLabelText("Silent minutes for the project");
    fireEvent.change(box, { target: { value: "20" } });
    fireEvent.blur(box);
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "save_project_settings")?.args).toMatchObject({
        which: "shared",
        change: {
          kind: "edits",
          edits: [
            {
              path: [{ key: "dispatch" }, { key: "stuck-silent-minutes" }],
              value: { kind: "integer", value: 20 },
            },
          ],
        },
      }),
    );
  });

  it("stay when an override is removed on Dispatch: only its dispatch limits go", async () => {
    core(
      page([
        row("project", "", [null, null, null, null, null]),
        row("workspace", "alpha", [2, null, 30, null, null]),
      ]),
    );
    render(<Row id={`${DISPATCH}.limits`} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove the override for the workspace alpha" }),
    );
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "save_project_settings")?.args).toMatchObject({
        change: {
          kind: "edits",
          edits: [
            {
              path: [
                { key: "dispatch" },
                { key: "workspaces" },
                { key: "alpha" },
                { key: "running-per-chat" },
              ],
              value: null,
            },
          ],
        },
      }),
    );
  });
});
