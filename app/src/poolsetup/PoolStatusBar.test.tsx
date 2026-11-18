// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PoolStatusBar } from "./PoolStatusBar";
import { poolStateOf, POOL_STATES } from "./poolState";

afterEach(cleanup);

const bar = (
  state: (typeof POOL_STATES)[number],
  more: Partial<Parameters<typeof PoolStatusBar>[0]> = {},
) => {
  const on = {
    onTryNow: vi.fn(),
    onCheckPool: vi.fn(),
    onCheckKey: vi.fn(),
    onRemake: vi.fn(),
    onForget: vi.fn(),
  };
  render(
    <PoolStatusBar
      pool="build-box"
      machine="build-box.local"
      status={{ state }}
      {...on}
      {...more}
    />,
  );
  return on;
};

describe("PoolStatusBar (#7191)", () => {
  it("says an unreachable pool is being retried, and offers Try now", async () => {
    const on = bar("unreachable");
    expect(screen.getByText(/Unreachable: pool build-box/)).toBeInTheDocument();
    expect(screen.getByText(/can't reach build-box\.local and keeps trying/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Try now" }));
    expect(on.onTryNow).toHaveBeenCalled();
  });

  it("says a refusal's reason, and that purlis stopped trying", async () => {
    const on = bar("refused", { status: { state: "refused", reason: "the disk is full" } });
    expect(
      screen.getByText(/answers, and refused: the disk is full\. purlis stopped trying/),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Check pool" }));
    expect(on.onCheckPool).toHaveBeenCalled();
  });

  it("stops at a changed host key and offers only to check the new one", async () => {
    const on = bar("host-key-changed");
    expect(screen.getByText(/shows a different host key/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Check the new key" }));
    expect(on.onCheckKey).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Try now" })).not.toBeInTheDocument();
  });

  it("says the volume with the sign-ins is still there where the runner is gone", async () => {
    const on = bar("runner-gone");
    expect(screen.getByText(/home volume, with your sign-ins, is still there/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Make the runner again" }));
    expect(on.onRemake).toHaveBeenCalled();
  });

  it("offers Forget pool… in every state", async () => {
    for (const state of ["unreachable", "refused", "host-key-changed", "runner-gone"] as const) {
      const on = bar(state);
      await userEvent.click(screen.getByRole("button", { name: "Forget pool…" }));
      expect(on.onForget).toHaveBeenCalled();
      cleanup();
    }
  });

  it("draws nothing for a forgotten pool, which has no state to hold", () => {
    bar("pool-forgotten");
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });
});

describe("poolStateOf, the one place the core's state is read (#7180)", () => {
  it("reads nothing where the core says nothing of the pool's state", () => {
    expect(poolStateOf({})).toBeUndefined();
  });
  it("reads a state and its reason as the core names them", () => {
    expect(poolStateOf({ pool_state: { state: "refused", reason: "image missing" } })).toEqual({
      state: "refused",
      reason: "image missing",
    });
  });
  it("ignores a state it doesn't know", () => {
    expect(poolStateOf({ pool_state: { state: "sleeping" } })).toBeUndefined();
  });
});
