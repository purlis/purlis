// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { END_COOL_OFF, RULES_I_MADE } from "./AskRules";
import { runAsk } from "./secretAsk.testkit";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import type { RuleMadeShown, Shown } from "./bindings";

/**
 * **Ask less** (ADR 0121 decision 7, #6010): no ask's row offers a rule any more (M75 slice 19,
 * Q314.15); a rule made before is kept in Rules I made, where it is undone; and the asks a
 * cool-off holds are said at the head of Asks, with End cool-off.
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
const REQUEST = '["sandbox-host","api.example.com"]';

const host = (session: number): Shown => ({
  session,
  ask: `block:${session}:connect:host:api.example.com`,
  says: "The sandbox refused api.example.com",
  options: [
    { id: "you", label: "Allow for me on this machine", allows: true, scope: "project" },
    { id: "chat", label: "Allow only for this chat", allows: true, scope: "chat" },
    { id: "project", label: "Allow for everyone in this project", allows: true, scope: "project" },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
  rule: { request: REQUEST },
});

const RULE: RuleMadeShown = {
  id: "rule-1",
  plane: PLANE,
  project: "plane",
  at: 1_800_000_000,
  source: "sandbox-host",
  says: "The sandbox refused api.example.com",
  label: "Allow for me on this machine",
  scope: "project",
  saved: 3,
  undoes: true,
};

type Call = { cmd: string; args: Record<string, unknown> };

function core(answers: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    const all: Record<string, (args: Record<string, unknown>) => unknown> = {
      allow_sandbox_block: () => ({ said: "Allowed." }),
      ...answers,
    };
    return all[cmd]?.(args as Record<string, unknown>) ?? null;
  });
  return calls;
}

const sent = (calls: Call[], cmd: string) => calls.filter((one) => one.cmd === cmd);

const RUN_REQUEST = '["run-grant","[\\"/usr/bin/curl\\"]"]';

/** A reviewed program's ask, with the answers Allow once, for this chat and Always allow. */
const run = (): Shown => ({
  ...runAsk({ fills: [], remember: null }),
  options: [
    { id: "allow-once", label: "Allow once", allows: true },
    { id: "allow-chat", label: "Allow for this chat", allows: true, scope: "chat" },
    { id: "allow-always", label: "Always allow", allows: true, scope: "project" },
    { id: "keep", label: "Don't allow", allows: false },
  ],
  rule: { request: RUN_REQUEST },
});

describe("no rule is offered on an ask's row (M75 slice 19, Q314.15)", () => {
  it("offers none on a host's ask, whatever the person answered before", () => {
    core();
    render(<Inbox plane={PLANE} asks={[host(4)]} onGo={vi.fn()} onLeave={vi.fn()} />);
    expect(screen.queryByRole("group", { name: "Make a rule for steward 4" })).toBeNull();
    expect(screen.queryByText(/Make it a rule/)).toBeNull();
  });

  it("offers none on a run's ask, and Always allow stays one of its answers", () => {
    core();
    render(<Inbox plane={PLANE} asks={[run()]} onGo={vi.fn()} onLeave={vi.fn()} />);
    expect(screen.queryByRole("group", { name: "Make a rule for developer 7" })).toBeNull();
    expect(screen.queryByText(/Make it a rule/)).toBeNull();
  });
});

describe("every answer in Asks (#6010)", () => {
  it("is noted against its request, a refusal too", async () => {
    const calls = core();
    render(<Inbox plane={PLANE} asks={[host(4)]} onGo={vi.fn()} onLeave={vi.fn()} />);
    const answers = screen.getByRole("group", { name: "Answers to steward 4" });
    await userEvent.click(within(answers).getByRole("button", { name: "Keep blocked" }));
    await userEvent.click(within(answers).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() => expect(sent(calls, "ask_answered")).toHaveLength(1));
    expect(sent(calls, "ask_answered")[0].args).toMatchObject({ option: "keep", allows: false });
    expect(sent(calls, "rule_made")).toEqual([]);
  });

  it("is noted with when its ask was raised, one that names no request too (#6021)", async () => {
    const calls = core();
    const unnamed = { ...host(4), since: 1_800_000_000, rule: undefined };
    render(<Inbox plane={PLANE} asks={[unnamed]} onGo={vi.fn()} onLeave={vi.fn()} />);
    const answers = screen.getByRole("group", { name: "Answers to steward 4" });
    await userEvent.click(within(answers).getByRole("button", { name: "Keep blocked" }));
    await userEvent.click(within(answers).getByRole("button", { name: "Keep blocked" }));
    await waitFor(() => expect(sent(calls, "ask_answered")).toHaveLength(1));
    expect(sent(calls, "ask_answered")[0].args).toMatchObject({
      request: "",
      option: "keep",
      raised: 1_800_000_000,
    });
  });
});

describe("Rules I made (#6010)", () => {
  it("is linked only once a rule is made", async () => {
    core({ rules_made: () => [] });
    render(<Inbox plane={PLANE} asks={[]} onGo={vi.fn()} onLeave={vi.fn()} />);
    await act(async () => {});
    expect(screen.queryByRole("button", { name: RULES_I_MADE })).toBeNull();
  });

  it("lists each rule with what it allows and for whom, and undoes it", async () => {
    const calls = core({ rules_made: () => [RULE], undo_rule: () => [] });
    render(<Inbox plane={PLANE} asks={[]} onGo={vi.fn()} onLeave={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: RULES_I_MADE }));
    const list = await screen.findByRole("list", { name: RULES_I_MADE });
    expect(list).toHaveTextContent(
      "Allow for me on this machine · this project in plane · saved 3 asks",
    );
    await userEvent.click(within(list).getByRole("button", { name: /^Undo/ }));
    await waitFor(() => expect(sent(calls, "undo_rule")[0]?.args).toEqual({ id: "rule-1" }));
    expect(await screen.findByText(/No rules yet/)).toBeTruthy();
  });
});

describe("Rules I made, for an Always allow (#6177)", () => {
  it("undoes it as any rule, by its id", async () => {
    const always: RuleMadeShown = {
      ...RULE,
      id: "rule-2",
      source: "run-grant",
      says: "devops wants to run /usr/bin/curl with STRIPE_API_KEY from vault team",
      label: "Always allow",
    };
    const calls = core({ rules_made: () => [always], undo_rule: () => [] });
    render(<Inbox plane={PLANE} asks={[]} onGo={vi.fn()} onLeave={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: RULES_I_MADE }));
    const list = await screen.findByRole("list", { name: RULES_I_MADE });
    expect(list).toHaveTextContent("Always allow · this project in plane · saved 3 asks");
    await userEvent.click(within(list).getByRole("button", { name: /^Undo/ }));
    await waitFor(() => expect(sent(calls, "undo_rule")[0]?.args).toEqual({ id: "rule-2" }));
    expect(await screen.findByText(/No rules yet/)).toBeTruthy();
  });
});

describe("a cool-off (#6010)", () => {
  it("says whose asks it holds, and End cool-off brings them back", async () => {
    const calls = core();
    render(
      <Inbox
        plane={PLANE}
        asks={[]}
        cooling={{ asks: [host(4)], until: null }}
        onGo={vi.fn()}
        onLeave={vi.fn()}
      />,
    );
    expect(screen.getByText("steward 4 heard Don't allow twice: 1 ask held quietly.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: END_COOL_OFF }));
    await waitFor(() =>
      expect(sent(calls, "end_cool_off")[0]?.args).toEqual({
        plane: PLANE,
        session: 4,
        request: REQUEST,
      }),
    );
  });
});
