// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolSetupCheck, PoolSetupStep } from "../bindings";
import { TryStep } from "./TryStep";

/** A line of Try it, said of what its title names, as the core tags it. */
const check = (verdict: string, title: string, more: string | null = null): PoolSetupCheck => ({
  about: /identity|\bgh\b/.test(title) ? "identity" : /signed in/.test(title) ? "signins" : "chat",
  verdict,
  title,
  more,
});

const READY: PoolSetupStep = {
  id: "try",
  name: "Try it",
  state: "current",
  line: null,
  findings: [],
  next: {
    id: "start-test-chat",
    label: "Start a test chat",
    does: "Starts a real chat on dev-box.",
    command: null,
    url: null,
  },
  skippable: false,
  see: null,
  other: null,
  machine: null,
  cause: null,
  checks: [
    check("todo", "A chat starts on dev-box"),
    check("ok", "work is signed in"),
    check("todo", "The agent's identity is set"),
  ],
};

const PASSED: PoolSetupStep = {
  ...READY,
  state: "done",
  next: null,
  checks: [
    check("ok", "A chat started on dev-box"),
    check("ok", "work is signed in"),
    check("ok", "The agent's identity is set: gh through steward"),
  ],
};

function core(more: Record<string, (args: unknown) => unknown> = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd in more) return more[cmd]?.(args);
    return null;
  });
  return calls;
}

const draw = (step: PoolSetupStep, workspace?: string) => {
  const props = { changed: vi.fn(), onGo: vi.fn(), onOpenChat: vi.fn() };
  render(<TryStep step={step} pool="dev-box" plane="p" workspace={workspace} {...props} />);
  return props;
};

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("TryStep", () => {
  it("starts a real chat only on its own button", async () => {
    const calls = core({
      pool_setup_try: () => ({ failed: null, cancelled: false, workspace: "web" }),
    });
    const { changed } = draw(READY);
    expect(calls.some((call) => call.cmd === "pool_setup_try")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.find((call) => call.cmd === "pool_setup_try")?.args).toEqual({
      plane: "p",
      pool: "dev-box",
      workspace: null,
    });
  });

  it("starts its chat in the workspace the tab is filed in", async () => {
    const calls = core({
      pool_setup_try: () => ({ failed: null, cancelled: false, workspace: "web" }),
    });
    const { changed } = draw(READY, "web");
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.find((call) => call.cmd === "pool_setup_try")?.args).toEqual({
      plane: "p",
      pool: "dev-box",
      workspace: "web",
    });
  });

  it("says what is happening while the chat starts, and can't be pressed twice", async () => {
    core({ pool_setup_try: () => new Promise(() => undefined) });
    draw(READY);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    expect(await screen.findByRole("heading", { name: "Trying it for real" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Starting a chat on dev-box");
    // Only the chat's start is under way: what the try doesn't check doesn't spin.
    const lines = within(screen.getByRole("list", { name: "The checks" }));
    expect(lines.getByText(/A chat starts on dev-box/).closest("li")?.textContent).toMatch(
      /Running/,
    );
    expect(lines.getByText(/The agent's identity is set/).closest("li")?.textContent).not.toMatch(
      /Running/,
    );
    const busy = screen.getByRole("button", { name: "Starting a test chat" });
    expect(busy.hasAttribute("disabled") || busy.getAttribute("aria-disabled") === "true").toBe(
      true,
    );
  });

  it("can be cancelled while its chat starts, and goes back to its start with nothing kept", async () => {
    let answer: (tried: unknown) => void = () => undefined;
    const calls = core({
      pool_setup_try: () => new Promise((resolve) => (answer = resolve)),
      pool_setup_try_cancel: () => {
        answer({ failed: null, cancelled: true, workspace: null });
        return null;
      },
    });
    const { changed } = draw(READY);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(calls.find((call) => call.cmd === "pool_setup_try_cancel")?.args).toEqual({
      pool: "dev-box",
    });
    expect(await screen.findByRole("button", { name: "Start a test chat" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Try it for real" })).toBeTruthy();
    expect(screen.queryByText("What happened")).toBeNull();
    expect(changed).not.toHaveBeenCalled();
  });

  it("cancels a try still under way when the person leaves the step", async () => {
    const calls = core({ pool_setup_try: () => new Promise(() => undefined) });
    draw(READY);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await screen.findByRole("button", { name: "Cancel" });
    cleanup();
    expect(calls.some((call) => call.cmd === "pool_setup_try_cancel")).toBe(true);
  });

  it("cancels nothing when the person leaves a step with no try under way", () => {
    const calls = core();
    draw(READY);
    cleanup();
    expect(calls.some((call) => call.cmd === "pool_setup_try_cancel")).toBe(false);
  });

  it("says in the identity's line what the try proved of the identity itself", async () => {
    let step = READY;
    core({
      pool_setup_try: () => ({
        failed: null,
        cancelled: false,
        workspace: "web",
        identity: check(
          "ok",
          "The agent's identity read acme/web on GitHub as acme-agent[bot]",
          "purlis made a read-only token for it and revoked it at once.",
        ),
      }),
    });
    const props = { changed: vi.fn(), onGo: vi.fn(), onOpenChat: vi.fn() };
    const { rerender } = render(<TryStep step={step} pool="dev-box" plane="p" {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await waitFor(() => expect(props.changed).toHaveBeenCalled());
    step = PASSED;
    rerender(<TryStep step={step} pool="dev-box" plane="p" {...props} />);
    const lines = within(screen.getByRole("list", { name: "What was checked" }));
    expect(lines.getByText(/read acme\/web on GitHub as acme-agent\[bot\]/)).toBeTruthy();
    expect(lines.queryByText(/gh through steward/)).toBeNull();
  });

  it("reads works, except for this, where GitHub didn't confirm the identity", async () => {
    let step = READY;
    core({
      pool_setup_try: () => ({
        failed: null,
        cancelled: false,
        workspace: "web",
        identity: check(
          "missing",
          "GitHub didn't confirm the agent's identity",
          "It isn't installed.",
        ),
      }),
    });
    const props = { changed: vi.fn(), onGo: vi.fn(), onOpenChat: vi.fn() };
    const { rerender } = render(<TryStep step={step} pool="dev-box" plane="p" {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await waitFor(() => expect(props.changed).toHaveBeenCalled());
    step = PASSED;
    rerender(<TryStep step={step} pool="dev-box" plane="p" {...props} />);
    expect(screen.getByRole("heading", { name: "dev-box works, except for this" })).toBeTruthy();
    expect(screen.queryByText("Everything passed")).toBeNull();
  });

  it("ends on what passed, with one button to open a chat there", async () => {
    core();
    const { onOpenChat } = draw(PASSED);
    expect(screen.getByRole("heading", { name: "dev-box is ready" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open a chat on dev-box" }));
    expect(onOpenChat).toHaveBeenCalled();
  });

  it("opens a chat in the workspace its test chat started in", async () => {
    let step = READY;
    core({ pool_setup_try: () => ({ failed: null, cancelled: false, workspace: "web" }) });
    const props = { changed: vi.fn(), onGo: vi.fn(), onOpenChat: vi.fn() };
    const { rerender } = render(<TryStep step={step} pool="dev-box" plane="p" {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await waitFor(() => expect(props.changed).toHaveBeenCalled());
    step = PASSED;
    rerender(<TryStep step={step} pool="dev-box" plane="p" {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Open a chat on dev-box" }));
    expect(props.onOpenChat).toHaveBeenCalledWith("web");
  });

  it("never reads as done where a check was skipped", () => {
    core();
    draw({
      ...PASSED,
      checks: [...PASSED.checks, check("skip", "gh wasn't checked: agent identity was skipped")],
    });
    expect(screen.getByRole("heading", { name: "dev-box works, except for this" })).toBeTruthy();
    expect(screen.queryByText("Everything passed")).toBeNull();
    const lines = screen.getByRole("list", { name: "What was checked" });
    expect(within(lines).getByText(/Skipped/)).toBeTruthy();
  });

  it("keeps a failure on this step, says what happened, and sends the person where to fix it", async () => {
    core({
      pool_setup_try: () => ({
        failed: { why: "work isn't signed in on this pool.", step: "signins" },
        cancelled: false,
      }),
    });
    const { onGo, changed } = draw(READY);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    expect(await screen.findByText("work isn't signed in on this pool.")).toBeTruthy();
    expect(changed).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Go to Sign-ins" }));
    expect(onGo).toHaveBeenCalledWith("signins");
  });

  it("can be tried again beside the step it sends the person to", async () => {
    let tries = 0;
    core({
      pool_setup_try: () => {
        tries += 1;
        return tries === 1
          ? {
              failed: { why: "work isn't signed in on this pool.", step: "signins" },
              cancelled: false,
            }
          : { failed: null, cancelled: false, workspace: "web" };
      },
    });
    const { changed } = draw(READY);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await userEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
  });

  it("reads as passed only where every check passed", () => {
    core();
    draw(PASSED);
    expect(screen.getByText("Everything passed")).toBeTruthy();
  });

  it("tries again here after a failure with nothing else to fix", async () => {
    let tries = 0;
    const calls = core({
      pool_setup_try: () => {
        tries += 1;
        if (tries === 1) throw "the runner didn't answer";
        return { failed: null, cancelled: false, workspace: "web" };
      },
    });
    const { changed } = draw(READY);
    await userEvent.click(screen.getByRole("button", { name: "Start a test chat" }));
    await userEvent.click(await screen.findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.filter((call) => call.cmd === "pool_setup_try")).toHaveLength(2);
  });
});
