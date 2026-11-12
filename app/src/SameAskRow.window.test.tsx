// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { MOVED_JUST_NOW } from "./InboxAsks";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { forgetAsksScope } from "./asksScope";
import type { Shown } from "./bindings";

/**
 * **Answer once for all identical requests** (ADR 0121 decision 6, #6009): the same host asked
 * by three chats is one row in Asks, whose Allow for all answers each chat once with the
 * narrowest grant, and which opens to answer each chat on its own.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const PLANE = "/home/dev/plane";

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const host = (session: number, target = "api.example.com"): Shown => ({
  session,
  ask: `block:${session}:connect:host:${target}`,
  says: `The sandbox refused ${target}`,
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target },
  },
});

/** What the core says of an Allow that went through. */
const ALLOWED = { allow_sandbox_block: () => ({ said: "Allowed." }) };

function core(answers: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    const answer = { ...ALLOWED, ...answers }[cmd] as
      ((args: Record<string, unknown>) => unknown) | undefined;
    return answer === undefined ? null : answer(args as Record<string, unknown>);
  });
  return calls;
}

function draw(asks: readonly Shown[]) {
  const onAnswered = vi.fn();
  render(
    <Inbox plane={PLANE} asks={asks} onGo={vi.fn()} onLeave={vi.fn()} onAnswered={onAnswered} />,
  );
  return { onAnswered };
}

const allowed = (calls: { cmd: string; args: Record<string, unknown> }[]) =>
  calls
    .filter(({ cmd }) => cmd === "allow_sandbox_block")
    .map(({ args }) => [args.session, args.level]);

const sameRow = () => screen.getByRole("row", { name: "3 chats want to reach api.example.com" });

describe("identical requests from several chats (#6009)", () => {
  it("are one row that says how many chats want it", () => {
    core();
    draw([host(4), host(5), host(6), host(7, "cdn.example.com")]);
    expect(sameRow()).toBeTruthy();
    // Each chat's own row is not drawn until the group is opened; the other host keeps its own.
    expect(screen.queryByRole("group", { name: "Answers to steward 4" })).toBeNull();
    expect(screen.getByRole("group", { name: "Answers to steward 7" })).toBeTruthy();
  });

  it("allows each chat once with its narrowest grant on Allow for all", async () => {
    const calls = core();
    const { onAnswered } = draw([host(4), host(5), host(6)]);
    read();
    await userEvent.click(within(sameRow()).getByRole("button", { name: "Allow for all" }));
    await waitFor(() =>
      expect(allowed(calls)).toEqual([
        [4, "chat"],
        [5, "chat"],
        [6, "chat"],
      ]),
    );
    expect(onAnswered).toHaveBeenCalledTimes(3);
  });

  it("allows nothing pressed just after it was drawn, and says so", async () => {
    const calls = core();
    draw([host(4), host(5), host(6)]);
    await userEvent.click(within(sameRow()).getByRole("button", { name: "Allow for all" }));
    expect(within(sameRow()).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(allowed(calls)).toEqual([]);
  });

  it("says which chat was not allowed, and how many were", async () => {
    core({
      allow_sandbox_block: (args) => {
        if (args.session === 5) throw "This host is kept blocked by policy.";
        return { said: "Allowed." };
      },
    });
    draw([host(4), host(5), host(6)]);
    read();
    await userEvent.click(within(sameRow()).getByRole("button", { name: "Allow for all" }));
    await waitFor(() =>
      expect(within(sameRow()).getByRole("status")).toHaveTextContent(
        "Allowed for 2 of 3 chats. steward 5: This host is kept blocked by policy.",
      ),
    );
  });

  it("opens to answer each chat on its own row", async () => {
    const calls = core();
    draw([host(4), host(5), host(6)]);
    const each = within(sameRow()).getByRole("button", { name: /^Answer each/ });
    expect(each).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(each);
    expect(each).toHaveAttribute("aria-expanded", "true");
    read();
    const steward5 = screen.getByRole("group", { name: "Answers to steward 5" });
    await userEvent.click(within(steward5).getByRole("button", { name: "Keep blocked" }));
    await userEvent.click(within(steward5).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() =>
      expect(calls.find(({ cmd }) => cmd === "keep_sandbox_block")?.args.session).toBe(5),
    );
    expect(allowed(calls)).toEqual([]);
  });
});

describe("within the scope in view (ADR 0121, amended 2026-11-12, decision 8)", () => {
  /** Chat 3 has focus and dispatched 4; chat 6 is its sibling, in the same workspace. */
  const PARENT: Record<number, number> = { 3: 1, 4: 3, 6: 1 };
  const scoped = {
    here: "alpha",
    workspaceOf: () => "alpha",
    chat: { session: 3, name: "steward 3", tasks: 1 },
    askedBy: (session: number) => PARENT[session],
  };

  afterEach(() => forgetAsksScope(PLANE));

  function drawScoped(asks: readonly Shown[]) {
    render(<Inbox plane={PLANE} asks={asks} onGo={vi.fn()} onLeave={vi.fn()} {...scoped} />);
  }

  const pick = (name: string) =>
    userEvent.click(
      within(screen.getByRole("radiogroup", { name: "Show what needs you in" })).getByRole(
        "radio",
        { name: new RegExp(`^${name}`) },
      ),
    );

  it("lists under Chat only the ask in its set, as its chat's own row, and groups both under Workspace", async () => {
    const calls = core();
    drawScoped([host(3), host(6)]);
    expect(screen.queryByRole("row", { name: /chats want to reach/ })).toBeNull();
    expect(screen.getByRole("group", { name: "Answers to steward 3" })).toBeTruthy();
    expect(screen.queryByRole("group", { name: "Answers to steward 6" })).toBeNull();
    await pick("Workspace");
    const both = screen.getByRole("row", { name: "2 chats want to reach api.example.com" });
    read();
    await userEvent.click(within(both).getByRole("button", { name: "Allow for all" }));
    await waitFor(() =>
      expect(allowed(calls)).toEqual([
        [3, "chat"],
        [6, "chat"],
      ]),
    );
  });

  it("never answers on Allow for all an ask the scope does not list", async () => {
    const calls = core();
    drawScoped([host(3), host(4), host(6)]);
    read();
    const listed = screen.getByRole("row", { name: "2 chats want to reach api.example.com" });
    await userEvent.click(within(listed).getByRole("button", { name: "Allow for all" }));
    await waitFor(() =>
      expect(allowed(calls)).toEqual([
        [3, "chat"],
        [4, "chat"],
      ]),
    );
  });
});
