// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PolicyLog, PolicyState } from "../bindings";
import { forgetPolicies } from "../projectPolicy";
import { forgetGroups, settingsPlace, useShownGroup } from "./links";
import { POLICY_LOG, PolicyLogList, PolicyStanding, WouldHaveBlocked, policyGroup } from "./policy";

/** Settings › Project › Policy (Q308a, instruction 45): it says the policy, and never resolves it. */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetPolicies();
  forgetGroups();
});

const PLANE = "/home/dev/plane";
const IDS = { id: "x", labelledBy: "x-label", describedBy: "x-help" };

export const WATCH: PolicyState = {
  waiting: [],
  refused: [],
  unsealed: false,
  digest: "d1",
  template: "watch",
  template_default: true,
  overrides: [],
  locked: [],
};

function core(state: PolicyState, log: PolicyLog = { count: 0, lines: [] }) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "project_policy_state" || cmd === "accept_project_policy") return state;
    if (cmd === "policy_log") return log;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("the project's policy in Settings", () => {
  it("says the default template where the project names none", async () => {
    core(WATCH);
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    expect(await screen.findByText("watch, the default")).toBeTruthy();
    expect(screen.getByText("No family is set apart from the template.")).toBeTruthy();
  });

  it("lists each family set apart, and says one that runs stricter than the file", async () => {
    core({
      ...WATCH,
      template: "team",
      template_default: false,
      overrides: [
        { family: "network", said: "off", runs: "enforce" },
        { family: "guards.release", said: "log", runs: "log" },
      ],
      locked: ["dispatch"],
    });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    const list = await screen.findByRole("list", { name: "Set apart from the template" });
    expect(list.textContent).toContain("network off, enforced for now");
    expect(list.textContent).toContain("guards.release log");
    expect(
      screen.getByText(/Locked enforced by this machine's administrator: dispatch/),
    ).toBeTruthy();
  });

  it("offers the sealed press for a loosening, and sends the digest it showed", async () => {
    const sent = core({
      ...WATCH,
      digest: "shown",
      waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }],
    });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    await userEvent.click(await screen.findByRole("button", { name: "Accept" }));
    await waitFor(() =>
      expect(sent("accept_project_policy")).toEqual([{ plane: PLANE, shown: "shown" }]),
    );
  });

  it("offers no Accept where the page is not the window", async () => {
    core({
      ...WATCH,
      waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }],
    });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept={false} />);
    expect(await screen.findByText(/Accept it from purlis's own window/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
  });

  it("counts what would have been blocked, and opens the enforcement log", async () => {
    core(WATCH, {
      count: 3,
      lines: [],
    });
    render(<WouldHaveBlocked plane={PLANE} ids={IDS} />);
    expect(await screen.findByText("Would have blocked: 3")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open the enforcement log" }));
    const shown = renderHook(() => useShownGroup(settingsPlace("project", PLANE)));
    expect(shown.result.current?.group).toBe(POLICY_LOG);
  });

  it("offers no way to an empty log", async () => {
    core(WATCH);
    render(<WouldHaveBlocked plane={PLANE} ids={IDS} />);
    expect(await screen.findByText("Would have blocked: 0")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open the enforcement log" })).toBeNull();
  });

  it("lists the log newest first, by rule, family and chat, never a command", async () => {
    core(WATCH, {
      count: 2,
      lines: [
        { at: "2026-11-17T11:00:00Z", rule: "A3", family: "guards.git", session: 12, host: null },
        {
          at: "2026-11-17T10:00:00Z",
          rule: "egress",
          family: "network",
          session: null,
          host: "example.com",
        },
      ],
    });
    render(<PolicyLogList plane={PLANE} ids={IDS} />);
    const items = await screen.findAllByRole("listitem");
    expect(items.map((one) => one.textContent)).toEqual([
      "A3 guards.git would have refused it in #12 2026-11-17T11:00:00Z",
      "egress network would have refused it in a chat that has closed, about example.com 2026-11-17T10:00:00Z",
    ]);
  });

  it("is a group of Settings › Project with the log's page under it", () => {
    const group = policyGroup(PLANE);
    expect(group.id).toBe("project.policy");
    expect(group.settings.map((one) => one.label)).toEqual([
      "Template and families",
      "Would have blocked",
    ]);
  });
});
