// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AddDockerPoolHost } from "./AddDockerPool";
import { startAddingAPool, stopAddingAPool } from "./pools";

afterEach(() => {
  act(() => stopAddingAPool());
  cleanup();
  clearMocks();
});

const core = () =>
  mockIPC((cmd) =>
    cmd === "docker_pools"
      ? { pools: [], clusters: [], defaults: null, suspend_after: null, yours: true }
      : null,
  );

describe("Add a pool… (#4484)", () => {
  it("opens Pool setup for a Docker pool in the project in front, and draws no form", async () => {
    core();
    const onPoolSetup = vi.fn();
    render(<AddDockerPoolHost plane="/p" onPoolSetup={onPoolSetup} />);
    act(() => startAddingAPool());
    await waitFor(() => expect(onPoolSetup).toHaveBeenCalledWith("/p"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps its dialog for a cluster pool", async () => {
    core();
    const onPoolSetup = vi.fn();
    render(<AddDockerPoolHost plane="/p" onPoolSetup={onPoolSetup} />);
    act(() => startAddingAPool("cluster"));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(onPoolSetup).not.toHaveBeenCalled();
  });
});
