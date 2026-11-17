// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { forgetDismissals } from "./dismissals";
import { POLICY_WATCH_ANNOUNCED, PolicyWatchAnnounced } from "./PolicyWatchAnnounced";
import { forgetPolicies, holdPolicy } from "./projectPolicy";
import { WATCH } from "./projectPolicy.fixture";

/** The upgrade to `watch`, announced once on this machine (Q308a, instruction 45). */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetPolicies();
  forgetDismissals();
});

const PLANE = "/home/dev/plane";

function core(seen: boolean) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "seen_on_this_machine") return seen;
    return null;
  });
  return (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);
}

describe("blocks are off by default, said once", () => {
  it("says it in a project that names no policy, and how to turn a family back on", async () => {
    core(false);
    render(<PolicyWatchAnnounced plane={PLANE} />);
    act(() => holdPolicy(PLANE, WATCH));
    expect(await screen.findByText(/Blocks are now off by default/)).toBeTruthy();
    expect(screen.getByText(/set it under \[policy\] in the project's settings file/)).toBeTruthy();
  });

  it("keeps it seen on this machine on Dismiss", async () => {
    const sent = core(false);
    render(<PolicyWatchAnnounced plane={PLANE} />);
    act(() => holdPolicy(PLANE, WATCH));
    await userEvent.click(await screen.findByRole("button", { name: /Dismiss/ }));
    await waitFor(() =>
      expect(sent("see_on_this_machine")).toEqual([{ cause: POLICY_WATCH_ANNOUNCED }]),
    );
    expect(screen.queryByText(/Blocks are now off by default/)).toBeNull();
  });

  it("says nothing where another window already showed it", async () => {
    const sent = core(true);
    render(<PolicyWatchAnnounced plane={PLANE} />);
    act(() => holdPolicy(PLANE, WATCH));
    await waitFor(() => expect(sent("seen_on_this_machine")).toHaveLength(1));
    expect(screen.queryByText(/Blocks are now off by default/)).toBeNull();
  });

  it("says nothing in a project that names its own template", async () => {
    const sent = core(false);
    render(<PolicyWatchAnnounced plane={PLANE} />);
    act(() => holdPolicy(PLANE, { ...WATCH, template: "strict", template_default: false }));
    await waitFor(() => expect(screen.queryByText(/Blocks are now off by default/)).toBeNull());
    expect(sent("seen_on_this_machine")).toEqual([]);
  });

  it("says nothing, and keeps its once, where the acceptance does not hold", async () => {
    const sent = core(false);
    render(<PolicyWatchAnnounced plane={PLANE} />);
    act(() => holdPolicy(PLANE, { ...WATCH, unsealed: true }));
    await waitFor(() => expect(screen.queryByText(/Blocks are now off by default/)).toBeNull());
    expect(sent("seen_on_this_machine")).toEqual([]);
    expect(sent("see_on_this_machine")).toEqual([]);
  });
});
