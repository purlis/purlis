// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PolicyLog, PolicyState } from "../bindings";
import { forgetPolicies } from "../projectPolicy";
import { TWO_LINES, WATCH } from "../projectPolicy.fixture";
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

function core(
  state: PolicyState,
  log: PolicyLog = { count: 0, lines: [], dropped: 0 },
  accept: () => unknown = () => state,
) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "project_policy_state") return state;
    if (cmd === "accept_project_policy") return accept();
    if (cmd === "keep_project_nevers")
      return { ...state, nevers_new_key: false, nevers_to_keep: null };
    if (cmd === "seal_unvouched_project_nevers")
      return { ...state, nevers_unvouched: false, nevers_to_seal: null };
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
      screen.getByText(/This machine's administrator keeps these enforced: dispatch/),
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

  it("reads the policy again after a press the core refused, so the next names what is shown", async () => {
    const sent = core(
      { ...WATCH, waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }] },
      undefined,
      () => {
        throw "the project's policy changed since it was shown, so nothing was accepted; look at it again";
      },
    );
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    await userEvent.click(await screen.findByRole("button", { name: "Accept" }));
    expect(await screen.findByText(/changed since it was shown/)).toBeTruthy();
    await waitFor(() => expect(sent("project_policy_state")).toHaveLength(2));
  });

  it("where the acceptance does not hold, says every family is enforced and offers Accept", async () => {
    const sent = core({ ...WATCH, unsealed: true });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    expect(await screen.findByText(/does not hold, so every family is enforced/)).toBeTruthy();
    expect(screen.queryByText("watch, the default")).toBeNull();
    expect(screen.getByText("watch")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Accept" }));
    await waitFor(() =>
      expect(sent("accept_project_policy")).toEqual([{ plane: PLANE, shown: "d1" }]),
    );
  });

  it("where a new seal key left the never-dispatch list unread, shows its pairs and keeps exactly those", async () => {
    // #7714, H1 of #7734's round 3: Accept never keeps the list. Keep these nevers is its own
    // press, and it names the pairs shown by their digest.
    const kept = {
      pairs: [
        { asking: "steward", target: "devops" },
        { asking: "devops", target: "steward" },
      ],
      digest: "n1",
    };
    const sent = core({ ...WATCH, nevers_new_key: true, nevers_to_keep: kept });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    expect(await screen.findByText(/cannot vouch for your never-dispatch list/)).toBeTruthy();
    const pairs = screen.getByRole("list", { name: "The pairs Keep these nevers seals" });
    expect(pairs.textContent).toContain("steward never dispatches to devops");
    expect(pairs.textContent).toContain("devops never dispatches to steward");
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Keep these nevers" }));
    await waitFor(() =>
      expect(sent("keep_project_nevers")).toEqual([{ plane: PLANE, shown: "n1" }]),
    );
    expect(sent("accept_project_policy")).toEqual([]);
  });

  it("where purlis cannot vouch for the never-dispatch list, Seal these nevers seals exactly the pairs shown", async () => {
    // #7852: the way out of a list no delete helps, with the grants it clears said beside it.
    const sent = core({
      ...WATCH,
      nevers_unvouched: true,
      nevers_to_seal: { pairs: [{ asking: "steward", target: "devops" }], digest: "u1" },
    });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    expect(
      await screen.findByText(/cannot vouch that your never-dispatch list is whole/),
    ).toBeTruthy();
    const pairs = screen.getByRole("list", { name: "The pairs Seal these nevers seals" });
    expect(pairs.textContent).toContain("steward never dispatches to devops");
    await userEvent.click(screen.getByRole("button", { name: "Seal these nevers" }));
    await waitFor(() =>
      expect(sent("seal_unvouched_project_nevers")).toEqual([{ plane: PLANE, shown: "u1" }]),
    );
    expect(sent("keep_project_nevers")).toEqual([]);
  });

  it("offers no Seal these nevers where the page is not the window", async () => {
    core({
      ...WATCH,
      nevers_unvouched: true,
      nevers_to_seal: { pairs: [], digest: "u0" },
    });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept={false} />);
    expect(await screen.findByText(/Seal them from purlis's own window/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Seal these nevers" })).toBeNull();
  });

  it("where the list's file does not read, offers nothing to keep", async () => {
    core({ ...WATCH, nevers_new_key: true, nevers_to_keep: null });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    expect(await screen.findByText(/there is nothing to keep yet/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Keep these nevers" })).toBeNull();
  });

  it("where the list holds a name purlis cannot show, says so and offers no press", async () => {
    // #7734's round 4, M1.
    core({ ...WATCH, nevers_new_key: true, nevers_to_keep: null, nevers_unshowable: true });
    render(<PolicyStanding plane={PLANE} ids={IDS} canAccept />);
    expect(await screen.findByText(/It holds a name no persona can have/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Keep these nevers" })).toBeNull();
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
    core(WATCH, { count: 3, lines: [], dropped: 0 });
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
    core(WATCH, TWO_LINES);
    render(<PolicyLogList plane={PLANE} ids={IDS} />);
    const items = await screen.findAllByRole("listitem");
    expect(items.map((one) => one.textContent)).toEqual([
      "A3 guards.git would have blocked it in #12 2026-11-17T11:00:00Z",
      "host-ask asks would have blocked it in a chat purlis does not know, about example.com:443 2026-11-17T10:00:00Z",
    ]);
    expect(screen.queryByText(/are shown/)).toBeNull();
  });

  it("says how many of the log are shown where it holds more than the window is handed", async () => {
    core(WATCH, { ...TWO_LINES, count: 501 });
    render(<PolicyLogList plane={PLANE} ids={IDS} />);
    expect(await screen.findByText("The newest 2 of 501 are shown.")).toBeTruthy();
  });

  it("says how many lines it left out that purlis did not write", async () => {
    core(WATCH, { ...TWO_LINES, dropped: 3 });
    render(<PolicyLogList plane={PLANE} ids={IDS} />);
    expect(
      await screen.findByText(
        "3 lines of the log are not as purlis writes them, so they are left out.",
      ),
    ).toBeTruthy();
  });

  it("reads the count again when the window is focused", async () => {
    const sent = core(WATCH);
    render(<WouldHaveBlocked plane={PLANE} ids={IDS} />);
    await screen.findByText("Would have blocked: 0");
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => expect(sent("policy_log")).toHaveLength(2));
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
