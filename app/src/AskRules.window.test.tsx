// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { END_COOL_OFF, RULES_I_MADE, alwaysLineSaid } from "./AskRules";
import { runAsk } from "./secretAsk.testkit";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import type { RuleMadeShown, Shown } from "./bindings";

/**
 * **Ask less** (ADR 0121 decision 7, #6010): after the same Allow twice, an ask's row offers its
 * wider grants as a rule, the suggested one first; a rule made is kept in Rules I made, where it
 * is undone; and the asks a cool-off holds are said at the head of Asks, with End cool-off.
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

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const host = (session: number, offer?: string[]): Shown => ({
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
  rule: {
    request: REQUEST,
    ...(offer === undefined ? {} : { offer: { answered: "chat", times: 2, options: offer } }),
  },
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

describe("a rule offered on an ask's row (#6010)", () => {
  it("offers nothing on an ask answered fewer than twice", () => {
    core();
    render(<Inbox plane={PLANE} asks={[host(4)]} onGo={vi.fn()} onLeave={vi.fn()} />);
    expect(screen.queryByRole("group", { name: "Make a rule for steward 4" })).toBeNull();
  });

  it("offers its wider grants, the suggested one first", () => {
    core();
    render(
      <Inbox plane={PLANE} asks={[host(4, ["you", "project"])]} onGo={vi.fn()} onLeave={vi.fn()} />,
    );
    const rule = screen.getByRole("group", { name: "Make a rule for steward 4" });
    expect(
      within(rule)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Allow for me on this machine", "Allow for everyone in this project"]);
    expect(screen.getByText(/the same Allow 2 times this week\. Make it a rule/)).toBeTruthy();
  });

  it("answers the ask with the grant pressed, and keeps it as a rule", async () => {
    const calls = core();
    render(
      <Inbox plane={PLANE} asks={[host(4, ["you", "project"])]} onGo={vi.fn()} onLeave={vi.fn()} />,
    );
    read();
    const rule = screen.getByRole("group", { name: "Make a rule for steward 4" });
    await userEvent.click(
      within(rule).getByRole("button", { name: /Allow for me on this machine/ }),
    );
    await waitFor(() => expect(sent(calls, "rule_made")).toHaveLength(1));
    expect(sent(calls, "allow_sandbox_block")[0].args).toMatchObject({ session: 4, level: "you" });
    expect(sent(calls, "rule_made")[0].args.made).toMatchObject({
      session: 4,
      request: REQUEST,
      option: "you",
      scope: "project",
      what: "host",
      target: "api.example.com",
    });
    // And the answer is noted against its request, as every answer is.
    expect(sent(calls, "ask_answered")[0].args).toMatchObject({
      session: 4,
      request: REQUEST,
      option: "you",
      allows: true,
    });
  });

  it("makes no rule pressed just after it was drawn", async () => {
    const calls = core();
    render(
      <Inbox plane={PLANE} asks={[host(4, ["you", "project"])]} onGo={vi.fn()} onLeave={vi.fn()} />,
    );
    const rule = screen.getByRole("group", { name: "Make a rule for steward 4" });
    await userEvent.click(
      within(rule).getByRole("button", { name: /Allow for me on this machine/ }),
    );
    expect(sent(calls, "allow_sandbox_block")).toEqual([]);
    expect(sent(calls, "rule_made")).toEqual([]);
  });
});

const RUN_REQUEST = '["run-grant","[\\"/usr/bin/curl\\"]"]';
const LINE = 'curl = { vault = "team", as = "STRIPE_API_KEY env", key = "STRIPE_API_KEY" }';

/** A reviewed program's ask, whose rule offers `offer` after the same Allow twice. */
const run = (offer: string[], risky = false): Shown => ({
  ...runAsk({ fills: [], remember: null }),
  options: [
    { id: "allow-once", label: "Allow once", allows: true },
    { id: "allow-chat", label: "Allow for this chat", allows: true, scope: "chat" },
    { id: "allow-always", label: "Always allow", allows: true, scope: "project" },
    { id: "keep", label: "Don't allow", allows: false },
  ],
  rule: { request: RUN_REQUEST, offer: { answered: "allow-chat", times: 2, options: offer } },
  ...(risky
    ? {
        risk: {
          class: "production-host" as const,
          why: "Looks like production",
          confirm: "Allow it?",
        },
      }
    : {}),
});

describe("a run's Always allow, offered as a rule (#6177)", () => {
  it("shows the line it writes before it is pressed", () => {
    core();
    render(<Inbox plane={PLANE} asks={[run(["allow-always"])]} onGo={vi.fn()} onLeave={vi.fn()} />);
    const rule = screen.getByRole("group", { name: "Make a rule for developer 7" });
    expect(within(rule).getByRole("button", { name: /Always allow/ })).toBeTruthy();
    expect(screen.getByText(alwaysLineSaid("devops"))).toBeTruthy();
    expect(screen.getAllByText(LINE).length).toBeGreaterThan(0);
  });

  it("answers the ask with Always allow and keeps its line with the rule, for Undo", async () => {
    const calls = core({ answer_run_grant: () => ({}) });
    render(<Inbox plane={PLANE} asks={[run(["allow-always"])]} onGo={vi.fn()} onLeave={vi.fn()} />);
    read();
    const rule = screen.getByRole("group", { name: "Make a rule for developer 7" });
    await userEvent.click(within(rule).getByRole("button", { name: /Always allow/ }));
    await waitFor(() => expect(sent(calls, "rule_made")).toHaveLength(1));
    expect(sent(calls, "answer_run_grant")[0].args).toMatchObject({ id: 9, press: "always" });
    expect(sent(calls, "rule_made")[0].args.made).toMatchObject({
      request: RUN_REQUEST,
      option: "allow-always",
      scope: "project",
      declared: { persona: "devops", line: LINE },
    });
  });

  it("is never offered on an ask marked risky", () => {
    core();
    render(
      <Inbox plane={PLANE} asks={[run(["allow-always"], true)]} onGo={vi.fn()} onLeave={vi.fn()} />,
    );
    expect(screen.queryByRole("group", { name: "Make a rule for developer 7" })).toBeNull();
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
