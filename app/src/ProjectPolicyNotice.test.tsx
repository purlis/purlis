// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectPolicyNotice } from "./ProjectPolicyNotice";
import type { PolicyPress, PolicyState } from "./bindings";

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
  digest: "d1",
  template: "watch",
  template_default: true,
  overrides: [],
  locked: [],
  press: null,
};

const LOOSENED: PolicyState = {
  ...QUIET,
  waiting: [{ family: "guards.merge", accepted: "enforce", committed: "log" }],
};

const PRESS: PolicyPress = {
  reason: "loosening",
  rules: [
    { id: "guards-merge-ban", kind: "loosens", what: "removed" },
    { id: "guards-merge-new", kind: "tightens", what: "added" },
    { id: "guards-merge-edit", kind: "changes", what: "edited" },
  ],
  modes: [{ key: "guards.merge", before: "enforce", after: "log", kind: "loosens" }],
  limits: [{ key: "dispatch.running-per-chat", before: "3", after: "9", kind: "loosens" }],
  local: [{ key: "template", was: "strict", now: "open" }],
  other: ["policy.sets.protected-branches"],
  commit: { sha: "0123abcd".repeat(5), subject: "Drop the merge ban", author: "Ada <ada@x>" },
  lost: null,
};
const PRESSED: PolicyState = { ...QUIET, press: PRESS };

function core(state: PolicyState, after: PolicyState = QUIET) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "project_policy_state") return state;
    if (cmd === "accept_project_policy") return after;
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

  it("project_policy_notice_offers_accept_and_not_now", async () => {
    const asked = core(PRESSED);
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(screen.getByRole("button", { name: "Accept" })).toBeTruthy());
    // Not now puts the line away and asks the core nothing.
    await userEvent.click(screen.getByRole("button", { name: "Not now" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Accept" })).toBeNull());
    expect(asked("accept_project_policy")).toEqual([]);
    // "Use it" and "Keep mine" are gone, and so is "Project recommends".
    expect(screen.queryByText(/Use it|Keep mine|Project recommends/)).toBeNull();
  });

  it("draws the press: each rule as loosens, tightens or changes, each mode, limit and your own file's tightening", async () => {
    core(PRESSED);
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(screen.getByText(/guards-merge-ban/)).toBeTruthy());
    expect(screen.getByText("loosens: removed")).toBeTruthy();
    expect(screen.getByText("tightens: added")).toBeTruthy();
    expect(screen.getByText("changes: edited")).toBeTruthy();
    expect(screen.getByText("loosens: enforce to log")).toBeTruthy();
    expect(screen.getByText("loosens: 3 to 9")).toBeTruthy();
    expect(screen.getByText("your own file loosens: strict to open")).toBeTruthy();
    expect(screen.getByText("0123abcd")).toBeTruthy();
    expect(screen.getByText(/Drop the merge ban/)).toBeTruthy();
  });

  it("an_unverified_author_is_shown_as_unverified_at_the_press", async () => {
    core(PRESSED);
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    // The author is the commit's own claim, and the window says it cannot be checked.
    await waitFor(() =>
      expect(screen.getByText(/The commit names its author as Ada <ada@x>\./)).toBeTruthy(),
    );
    expect(screen.getByText(/purlis can't check who wrote it/)).toBeTruthy();
    expect(screen.queryByText(/verified on/)).toBeNull();
  });

  it("names a part no typed row covers as a change that waits", async () => {
    core(PRESSED);
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(screen.getByText("policy.sets.protected-branches")).toBeTruthy());
    expect(screen.getAllByText("changes").length).toBeGreaterThan(0);
  });

  it("says Accept starts the policy afresh, not that earlier acceptances hold, where nothing is allowed until it", async () => {
    core({ ...QUIET, press: { ...PRESS, reason: "past-the-cap" } });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() => expect(screen.getByText(/A commit went past the most/)).toBeTruthy());
    expect(screen.queryByText(/they keep what you accepted before/)).toBeNull();
  });

  it("raises the press for a removal from your own file, with no commit and no author", async () => {
    core({
      ...QUIET,
      press: {
        reason: "local-removal",
        rules: [],
        modes: [],
        limits: [],
        local: [{ key: "template", was: "strict", now: "open" }],
        other: [],
        commit: null,
        lost: null,
      },
    });
    render(<ProjectPolicyNotice plane={PLANE} canAccept />);
    await waitFor(() =>
      expect(screen.getByText(/A tightening is gone from your own/)).toBeTruthy(),
    );
    expect(screen.getByText("your own file loosens: strict to open")).toBeTruthy();
    expect(screen.queryByText(/Author/)).toBeNull();
    expect(screen.getByRole("button", { name: "Accept" })).toBeTruthy();
  });
});
