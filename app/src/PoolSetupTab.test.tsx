// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupShown, PoolSetupStep } from "./bindings";
import { PoolSetupTab } from "./PoolSetupTab";

const step = (id: string, name: string, state: string, more: Partial<PoolSetupStep> = {}) =>
  ({
    id,
    name,
    state,
    line: null,
    findings: [],
    next: null,
    skippable: false,
    ...more,
  }) as PoolSetupStep;

const action = (id: string, label: string, url: string | null = null) => ({
  id,
  label,
  does: `${label}: what it does.`,
  command: null,
  url,
});

const setup = (at: string, steps: PoolSetupStep[], pool: string | null = null): PoolSetupShown => ({
  pool,
  name: pool,
  at,
  steps,
});

const READY = setup("where", [
  step("where", "Where chats run", "current", {
    findings: ["Docker is ready."],
    next: action("use-docker", "Continue with Docker"),
  }),
  step("pool", "Pool", "later"),
]);

const CHOSEN = setup("pool", [
  step("where", "Where chats run", "done", { line: "Docker on this machine" }),
  step("pool", "Pool", "current", { next: action("create-pool", "Create the pool") }),
]);

const MADE = setup(
  "signins",
  [
    step("where", "Where chats run", "done", { line: "Docker on this machine" }),
    step("pool", "Pool", "done", { line: "dev-box" }),
    step("signins", "Sign-ins", "current", {
      findings: ["This pool has none yet."],
      next: action("choose-profiles", "Choose profiles"),
    }),
  ],
  "dev-box",
);

const MISSING = setup("where", [
  step("where", "Where chats run", "attention", {
    findings: ["You need Docker first."],
    next: action("get-docker", "Open Docker's install page", "https://docs.docker.com/"),
  }),
  step("pool", "Pool", "later"),
]);

const mock = vi.hoisted(() => ({
  openUrl: vi.fn(async () => undefined),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: mock.openUrl }));

type Calls = { cmd: string; args: unknown }[];

function core(
  answers: () => PoolSetupShown,
  more: Record<string, (args: unknown) => unknown> = {},
) {
  const calls: Calls = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "pool_setup") return answers();
    if (cmd in more) return more[cmd]?.(args);
    if (cmd === "runner_image_of_this_version") return { state: "ours", image: "img@sha256:a" };
    return null;
  });
  return calls;
}

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

describe("PoolSetupTab", () => {
  it("shows each step's state as the core read it", async () => {
    core(() => READY);
    render(<PoolSetupTab plane="p" />);
    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
    expect(screen.getByText("Docker is ready.")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^Pool/ })).toHaveProperty("disabled", true);
  });

  it("says what the button does before it is pressed", async () => {
    core(() => READY);
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByText("Continue with Docker: what it does.")).toBeTruthy();
  });

  it("keeps the choice of Docker and moves on to the Pool step", async () => {
    let now = READY;
    const calls = core(() => now, {
      pool_setup_choose_docker: () => {
        now = CHOSEN;
        return null;
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Continue with Docker" }));
    expect(await screen.findByRole("heading", { name: "Name your pool" })).toBeTruthy();
    expect(calls.some((call) => call.cmd === "pool_setup_choose_docker")).toBe(true);
  });

  it("makes a real pool on the step's own button and goes on", async () => {
    let now = CHOSEN;
    const calls = core(() => now, {
      add_docker_pool: () => {
        now = MADE;
        return null;
      },
    });
    render(<PoolSetupTab plane="p" />);
    await userEvent.type(await screen.findByRole("textbox", { name: /Name/ }), "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    expect(calls.some((call) => call.cmd === "add_docker_pool")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(await screen.findByRole("heading", { name: "Sign-ins" })).toBeTruthy();
    const added = calls.find((call) => call.cmd === "add_docker_pool");
    expect(added?.args).toMatchObject({ adding: { name: "dev-box", image: "img@sha256:a" } });
  });

  it("keeps what was typed when the pool is refused, and says why", async () => {
    const calls = core(() => CHOSEN, {
      add_docker_pool: () => {
        throw "a pool named dev-box is on this machine";
      },
    });
    render(<PoolSetupTab plane="p" />);
    const box = await screen.findByRole("textbox", { name: /Name/ });
    await userEvent.type(box, "dev-box");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Create the pool" })).toHaveProperty(
        "disabled",
        false,
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Create the pool" }));
    expect(await screen.findByText("a pool named dev-box is on this machine")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: /Name/ })).toHaveProperty("value", "dev-box");
    expect(calls.filter((call) => call.cmd === "add_docker_pool")).toHaveLength(1);
  });

  it("resumes with the name given", async () => {
    core(() => ({ ...CHOSEN, name: "kept-name" }));
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByRole("textbox", { name: /Name/ })).toHaveProperty(
      "value",
      "kept-name",
    );
  });

  it("opens the official page without installing anything, and notices Docker by itself", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let now = MISSING;
    core(() => now);
    render(<PoolSetupTab plane="p" />);
    const open = await screen.findByRole("button", { name: "Open Docker's install page" });
    await userEvent.click(open);
    expect(mock.openUrl).toHaveBeenCalledWith("https://docs.docker.com/");
    now = READY;
    await vi.advanceTimersByTimeAsync(6000);
    expect(await screen.findByRole("button", { name: "Continue with Docker" })).toBeTruthy();
  });

  it("goes back to a step reached", async () => {
    core(() => CHOSEN);
    render(<PoolSetupTab plane="p" />);
    await screen.findByRole("heading", { name: "Name your pool" });
    await userEvent.click(screen.getByRole("button", { name: /^Where chats run/ }));
    expect(
      await screen.findByRole("heading", { name: "Where should your chats run?" }),
    ).toBeTruthy();
  });

  it("says what went wrong where the read failed, with something to try", async () => {
    let fails = true;
    mockIPC((cmd) => {
      if (cmd !== "pool_setup") return null;
      if (fails) throw "the choices can't be read";
      return READY;
    });
    render(<PoolSetupTab plane="p" />);
    expect(await screen.findByText("the choices can't be read")).toBeTruthy();
    fails = false;
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByRole("button", { name: "Continue with Docker" })).toBeTruthy();
  });
});
