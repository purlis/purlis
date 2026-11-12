// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { IdentitiesShown, IdentityRow, PoolSetupStep } from "../bindings";
import { IdentityStep } from "./IdentityStep";

const TO_SET_UP: PoolSetupStep = {
  id: "identity",
  name: "Agent identity",
  state: "current",
  line: null,
  findings: [
    "The agent has no account of its own, so gh and git push won't work in a chat on the pool.",
  ],
  next: {
    id: "set-up-identity",
    label: "Set up an identity",
    does: "Gives the agent its own account on your forge.",
    command: null,
    url: null,
  },
  skippable: true,
  see: null,
  other: null,
  machine: null,
  cause: null,
  checks: [],
};

const SET_UP: PoolSetupStep = {
  ...TO_SET_UP,
  state: "done",
  line: "gh through steward",
  findings: [],
  next: null,
};

const BY_HAND: PoolSetupStep = {
  ...TO_SET_UP,
  findings: [
    "purlis can't set up git.acme.internal by itself yet: one-click setup works for github.com.",
  ],
  next: {
    id: "check-again",
    label: "Check again",
    does: "Reads the project's settings again. It changes nothing.",
    command: null,
    url: null,
  },
  see: {
    note: "These make a vault of the agent's own and keep its token there, then the line goes in your project's settings. Never use your own token: a chat would act as you.",
    text: 'purlis vault add agent-forge --provider keyring\npurlis secret set agent-forge token --stdin\n\n[sandbox.personas.steward.tools]\ngh = { vault = "agent-forge", as = "GH_ENTERPRISE_TOKEN env", key = "token", host = "git.acme.internal" }',
  },
};

const ROW: IdentityRow = {
  host: "github.com",
  app: "Iv1.abc",
  personas: ["steward"],
  acts_as: "acme-agent[bot]",
  repos: 12,
  settings_page: "https://github.com/organizations/acme/settings/apps/acme-agent",
  trouble: null,
  by_hand: false,
};

const shown = (rows: IdentityRow[] = []): IdentitiesShown => ({
  rows,
  pending: [],
  owners: [{ name: "acme", personal: false }],
  personas: ["steward"],
});

function core(more: Record<string, (args: unknown) => unknown> = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd in more) return more[cmd]?.(args);
    if (cmd === "agent_identities") return shown();
    if (cmd === "agent_identity_lines") return "[sandbox.personas.steward.tools]\ngh = {}";
    return null;
  });
  return calls;
}

const draw = (step: PoolSetupStep) => {
  const props = { changed: vi.fn(), onNext: vi.fn(), onOpenIdentity: vi.fn() };
  render(<IdentityStep step={step} pool="dev-box" plane="p" {...props} />);
  return props;
};

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("IdentityStep", () => {
  it("sets an identity up in the step itself, with the same flow as the Agent identity tab", async () => {
    core();
    draw(TO_SET_UP);
    expect(
      await screen.findByRole("heading", { name: "Give the agent its own GitHub identity" }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create the GitHub App" })).toBeTruthy();
  });

  it("can be skipped from the flow, and says so to the core", async () => {
    const calls = core();
    const { changed } = draw(TO_SET_UP);
    await userEvent.click(await screen.findByRole("button", { name: "Skip for now" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.find((call) => call.cmd === "pool_setup_skip")?.args).toEqual({
      pool: "dev-box",
      step: "identity",
      skipped: true,
    });
  });

  it("shows an identity already there as done, with what it acts as and on how many repos", async () => {
    core({ agent_identities: () => shown([ROW]) });
    const { onNext, onOpenIdentity } = draw(SET_UP);
    expect(screen.getByRole("heading", { name: "Already set up" })).toBeTruthy();
    expect(await screen.findByText(/Acting as acme-agent\[bot\] on 12 repos/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Create the GitHub App" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Continue to Try it" }));
    expect(onNext).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Open Agent identity" }));
    expect(onOpenIdentity).toHaveBeenCalled();
  });

  it("says what a line written by hand acts through where GitHub has nothing to say", async () => {
    core({
      agent_identities: () => shown([{ ...ROW, acts_as: null, repos: null, by_hand: true }]),
    });
    draw(SET_UP);
    expect(await screen.findByText(/Set up as gh through steward/)).toBeTruthy();
  });

  it("tells another forge host plainly, with the line to add by hand, and never dead-ends", async () => {
    const calls = core();
    const { changed } = draw(BY_HAND);
    expect(screen.getByText(/can't set up git\.acme\.internal by itself/)).toBeTruthy();
    expect(screen.getByText(/GH_ENTERPRISE_TOKEN env/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Create the GitHub App" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(changed).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Skip for now" }));
    await waitFor(() => expect(calls.some((call) => call.cmd === "pool_setup_skip")).toBe(true));
  });

  it("sends the person to Settings to allow a persona's tool, and checks again", async () => {
    core();
    const props = {
      changed: vi.fn(),
      onNext: vi.fn(),
      onOpenIdentity: vi.fn(),
      onOpenSettings: vi.fn(),
    };
    render(
      <IdentityStep
        step={{
          ...TO_SET_UP,
          findings: ["The persona steward declares gh, and this machine hasn't allowed it."],
          next: {
            id: "allow-tool",
            label: "Allow it in Settings › Project",
            does: "Opens Settings › Project: allow the persona's tool there, under Sandbox, on this machine.",
            command: null,
            url: null,
          },
        }}
        pool="dev-box"
        plane="p"
        {...props}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Allow it in Settings › Project" }));
    expect(props.onOpenSettings).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(props.changed).toHaveBeenCalled();
  });

  it("offers to come back to a step that was skipped", async () => {
    const calls = core();
    const { changed } = draw({ ...TO_SET_UP, state: "skipped", next: null, findings: [] });
    expect(screen.getByText("You skipped this for now")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Set it up now" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.find((call) => call.cmd === "pool_setup_skip")?.args).toEqual({
      pool: "dev-box",
      step: "identity",
      skipped: false,
    });
  });
});
