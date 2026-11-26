// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectPolicyNotice } from "./ProjectPolicyNotice";
import type { PolicyState } from "./bindings";

/**
 * **A loosening of the project's `[policy]` waits for the person's press** (Q308a, Q308b): the
 * window names each family that waits and offers Accept, the window's alone; what the file says
 * that purlis could not read is said beside it, each value enforced meanwhile.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const QUIET: PolicyState = {
  waiting: [],
  refused: [],
  not_in_force: [],
  unsealed: false,
  key_foreign: false,
  new_key: false,
  nevers_new_key: false,
  nevers_to_keep: null,
  digest: "d1",
  template: "watch",
  template_default: true,
  overrides: [],
  locked: [],
};

const LOOSENED: PolicyState = {
  ...QUIET,
  waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }],
};

function core(state: PolicyState, after: PolicyState = QUIET) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "project_policy_state") return state;
    if (cmd === "accept_project_policy") return after;
    if (cmd === "keep_project_nevers") return after;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("the project's policy (Q308a)", () => {
  it("draws nothing, and never breaks the window, for an answer of another shape", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      return null;
    });
    const { container } = render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(asked).toContain("project_policy_state"));
    expect(container.querySelector("[data-cause]")).toBeNull();
  });

  it("says nothing where nothing waits and every value reads", async () => {
    const asked = core(QUIET);
    const { container } = render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(asked("project_policy_state")).toHaveLength(1));
    expect(container.querySelector("[data-cause]")).toBeNull();
  });

  it("names each family a loosening waits on, and Accept presses it", async () => {
    const asked = core(LOOSENED);
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(screen.getByText(/guards\.merge from enforce to log/)).toBeTruthy());
    await userEvent.click(screen.getByRole("button", { name: "Accept" }));
    await waitFor(() =>
      expect(asked("accept_project_policy")).toEqual([{ plane: PLANE, shown: "d1" }]),
    );
    await waitFor(() => expect(screen.queryByText(/guards\.merge from/)).toBeNull());
  });

  it("after a new seal key, shows the never-dispatch pairs and keeps exactly those", async () => {
    // #7714, H1 of #7734's round 3: its own press, never Accept's, naming the pairs shown.
    const asked = core({
      ...QUIET,
      nevers_new_key: true,
      nevers_to_keep: { pairs: [{ asking: "steward", target: "devops" }], digest: "n1" },
    });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() =>
      expect(screen.getByText(/It holds steward to devops\. Keep these nevers seals exactly that pair\./)).toBeTruthy(),
    );
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Keep these nevers" }));
    await waitFor(() =>
      expect(asked("keep_project_nevers")).toEqual([{ plane: PLANE, shown: "n1" }]),
    );
    expect(asked("accept_project_policy")).toEqual([]);
    await waitFor(() => expect(screen.queryByText(/It holds steward to devops/)).toBeNull());
  });

  it("never offers Keep these nevers to a page that is not the window", async () => {
    core({
      ...QUIET,
      nevers_new_key: true,
      nevers_to_keep: { pairs: [], digest: "n0" },
    });
    render(<ProjectPolicyNotice plane={PLANE} canAccept={false} />);
    await waitFor(() =>
      expect(screen.getByText(/Keep them from purlis's own window/)).toBeTruthy(),
    );
    expect(screen.queryByRole("button", { name: "Keep these nevers" })).toBeNull();
  });

  it("never offers Accept to a page that is not the window", async () => {
    core(LOOSENED);
    render(<ProjectPolicyNotice plane={PLANE} canAccept={false} />);
    await waitFor(() =>
      expect(screen.getByText(/Accept it from purlis's own window/)).toBeTruthy(),
    );
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
  });

  it("says each value it could not read, enforced meanwhile", async () => {
    core({
      ...QUIET,
      refused: ["policy.network is not off, log or enforce, so it is enforced"],
    });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() =>
      expect(screen.getByText(/policy\.network is not off, log or enforce/)).toBeTruthy(),
    );
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
  });

  it("says an acceptance that does not hold enforces everything, and offers the press again", async () => {
    const asked = core({ ...QUIET, unsealed: true, digest: "d2" });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() =>
      expect(screen.getByText(/does not hold, so every family is enforced/)).toBeTruthy(),
    );
    // The press accepts the committed file as it was shown, never what was kept before.
    await userEvent.click(screen.getByRole("button", { name: "Accept" }));
    await waitFor(() =>
      expect(asked("accept_project_policy")).toEqual([{ plane: PLANE, shown: "d2" }]),
    );
  });

  it("names the sandbox's level that waits, and says what runs without a sandbox leaves out", async () => {
    core({
      ...QUIET,
      waiting: [{ family: "sandbox", accepted: "floor", committed: "off" }],
      not_in_force: ["network", "ports", "processes"],
    });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(screen.getByText(/sandbox from floor to off/)).toBeTruthy());
    expect(
      screen.getByText(/Without a sandbox, network, ports and processes are not in force\./),
    ).toBeTruthy();
  });

  it("says the merge opt-in as a change of the forge route, held until accepted", async () => {
    core({
      ...QUIET,
      waiting: [{ family: "merges through the forge route", accepted: "held", committed: "log" }],
    });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() =>
      expect(screen.getByText(/merges through the forge route from held to log/)).toBeTruthy(),
    );
  });
});
