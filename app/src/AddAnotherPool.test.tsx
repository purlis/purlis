// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AddDockerPoolHost } from "./AddDockerPool";
import type { PoolSetupShown, PoolSetupStep } from "./bindings";
import { PoolSetupTab } from "./PoolSetupTab";
import { forgetPoolSetupStarts, openSetupOf, startAddingAPool, stopAddingAPool } from "./pools";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => undefined) }));

const step = (id: string, name: string, state: string, more: Partial<PoolSetupStep> = {}) =>
  ({
    id,
    name,
    state,
    line: null,
    findings: [],
    next: null,
    skippable: false,
    see: null,
    checks: [],
    ...more,
  }) as PoolSetupStep;

const action = (id: string, label: string) => ({
  id,
  label,
  does: `${label}: what it does.`,
  command: null,
  url: null,
});

/** Setup ended on dev-box: every step done, as the core reads it once Try it passed. */
const FINISHED: PoolSetupShown = {
  kind: "docker",
  pool: "dev-box",
  name: null,
  at: null,
  noticed: false,
  others: [],
  recommended: "docker",
  contexts: { names: [], recommended: null, chosen: null, pools: [] },
  steps: [
    step("where", "Where chats run", "done", { line: "Docker on this Mac" }),
    step("pool", "Pool", "done", { line: "dev-box" }),
    step("signins", "Sign-ins", "done"),
    step("identity", "Agent identity", "skipped"),
    step("try", "Try it", "done"),
  ],
};

/** A new pool's setup, from its first step, with dev-box kept. */
const NEW: PoolSetupShown = {
  kind: "docker",
  pool: null,
  name: null,
  at: "where",
  noticed: false,
  others: ["dev-box"],
  recommended: "docker-2",
  contexts: { names: [], recommended: null, chosen: null, pools: [] },
  steps: [
    step("where", "Where chats run", "current", {
      findings: ["Docker is ready."],
      next: action("use-docker", "Continue with Docker"),
    }),
    step("pool", "Pool", "later"),
    step("signins", "Sign-ins", "later"),
    step("identity", "Agent identity", "later"),
    step("try", "Try it", "later"),
  ],
};

const NAMING: PoolSetupShown = {
  ...NEW,
  at: "pool",
  steps: [
    step("where", "Where chats run", "done", { line: "Docker on this Mac" }),
    step("pool", "Pool", "current", { next: action("create-pool", "Create the pool") }),
    step("signins", "Sign-ins", "later"),
    step("identity", "Agent identity", "later"),
    step("try", "Try it", "later"),
  ],
};

/** A new pool's setup left on the Pool step with a name typed. */
const UNFINISHED: PoolSetupShown = { ...NAMING, name: "builds" };

/** The core as Pool setup's commands reach it: what it reads, and what Add and Start over do. */
function core(start: PoolSetupShown) {
  let now = start;
  const calls: string[] = [];
  mockIPC((cmd) => {
    calls.push(cmd);
    if (cmd === "pool_setup") return now;
    if (cmd === "pool_setup_add") {
      // A finished setup is never resumed by Add; an unfinished new one is.
      if (now.at === null) now = NEW;
      return now !== NEW && now.pool === null && now.name !== null;
    }
    if (cmd === "pool_setup_start_over") {
      now = NEW;
      return null;
    }
    if (cmd === "pool_setup_pool") {
      now = FINISHED;
      return null;
    }
    if (cmd === "pool_setup_choose_docker") {
      now = NAMING;
      return null;
    }
    if (cmd === "runner_image_of_this_version") return { state: "ours", image: "img@sha256:a" };
    return null;
  });
  return calls;
}

/** The window as Add a pool… reaches it: the host that routes it, and the Pool setup tab. */
function Window({ open = false }: { open?: boolean }) {
  return (
    <>
      <AddDockerPoolHost plane="/p" onPoolSetup={() => undefined} />
      {open && <PoolSetupTab plane="/p" />}
    </>
  );
}

afterEach(() => {
  act(() => {
    stopAddingAPool();
    forgetPoolSetupStarts();
  });
  cleanup();
  clearMocks();
});

describe("Add a pool… with a pool already made (#6256)", () => {
  it("opens step 1 of a new pool, not the last pool's finished setup", async () => {
    const calls = core(FINISHED);
    render(<Window open />);
    expect(await screen.findByRole("heading", { name: "dev-box is ready" })).toBeTruthy();

    act(() => startAddingAPool());

    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
    expect(calls).toContain("pool_setup_add");
    expect(screen.queryByText("Continuing the pool you started")).toBeNull();
  });

  it("leaves the name empty and recommends one no pool has", async () => {
    core(FINISHED);
    render(<Window open />);
    await screen.findByRole("heading", { name: "dev-box is ready" });
    act(() => startAddingAPool());
    await userEvent.click(await screen.findByRole("button", { name: "Continue with Docker" }));

    const name = await screen.findByRole("textbox", { name: "Name" });
    expect((name as HTMLInputElement).value).toBe("");
    expect(name.getAttribute("placeholder")).toBe("docker-2");
    expect(screen.getByText(/Recommended: docker-2\./)).toBeTruthy();
  });

  it("continues a new pool's unfinished setup, says so, and starts over on a press", async () => {
    const calls = core(UNFINISHED);
    render(<Window open />);
    await screen.findByRole("heading", { name: "Name your pool" });

    act(() => startAddingAPool());

    expect(await screen.findByText("Continuing the pool you started")).toBeTruthy();
    expect(((await screen.findByRole("textbox", { name: "Name" })) as HTMLInputElement).value).toBe(
      "builds",
    );

    await userEvent.click(screen.getByRole("button", { name: "Start over" }));

    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
    expect(calls).toContain("pool_setup_start_over");
    await waitFor(() => expect(screen.queryByText("Continuing the pool you started")).toBeNull());
  });

  it("ends a finished setup with Add another pool, which starts a new one", async () => {
    const calls = core(FINISHED);
    render(<Window open />);
    await userEvent.click(await screen.findByRole("button", { name: "Add another pool" }));

    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
    expect(calls).toContain("pool_setup_add");
  });

  it("reaches a finished pool's setup from its own Open setup, nothing redone", async () => {
    const calls = core(NEW);
    render(<Window open />);
    await screen.findByRole("heading", { name: "Where should your chats run?" });

    act(() => openSetupOf("dev-box"));

    expect(await screen.findByRole("heading", { name: "dev-box is ready" })).toBeTruthy();
    expect(calls).toContain("pool_setup_pool");
    expect(calls).not.toContain("pool_setup_add");
  });
});
