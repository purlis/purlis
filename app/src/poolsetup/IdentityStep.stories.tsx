// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Pool setup's Agent identity step** (#4490): every state, drawn, held to axe, walked by the
 * keyboard and pictured in every pictured theme cell. The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { IdentitiesShown, IdentityRow, PoolSetupStep } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
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
    does: "Gives the agent its own account on your forge. Your own sign-in never reaches a chat.",
    command: null,
    url: null,
  },
  skippable: true,
  also: null,
  parts: [],
  admin: null,
  undo: null,
  see: null,
  other: null,
  machine: null,
  cause: null,
  checks: [],
};

const ROW: IdentityRow = {
  host: "github.com",
  app: "Iv1.0a1b2c3d4e5f",
  personas: ["steward"],
  acts_as: "acme-purlis-agent[bot]",
  repos: 12,
  settings_page: "https://github.com/organizations/acme/settings/apps/acme-purlis-agent",
  trouble: null,
  by_hand: false,
};

const shown = (rows: IdentityRow[]): IdentitiesShown => ({
  rows,
  pending: [],
  owners: [{ name: "acme", personal: false }],
  personas: ["steward", "reviewer"],
});

const faked = (identities: IdentitiesShown) => () => {
  mockIPC((cmd) => {
    if (cmd === "agent_identities") return identities;
    if (cmd === "agent_identity_lines")
      return '[sandbox.personas.steward.tools]\ngh = { vault = "agent-identity", mint = "github-app", app = "<client id>", key = "agent-identity-github-<client id>" }';
    return null;
  });
};

const meta = {
  component: IdentityStep,
  args: {
    step: TO_SET_UP,
    pool: "dev-box",
    plane: "p",
    changed: () => undefined,
    onNext: () => undefined,
    onOpenIdentity: () => undefined,
    onOpenSettings: () => undefined,
  },
  beforeEach: faked(shown([])),
  decorators: [
    (Story) => (
      <div style={{ maxWidth: 720 }} className="pool-setup-page">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof IdentityStep>;

export default meta;
type Story = StoryObj<typeof meta>;

/** No identity yet: the flow the Agent identity tab uses, with Skip for now beside it. */
export const SetUp: Story = {
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const create = await canvas.findByRole("button", { name: "Create the GitHub App" });
    create.focus();
    await untilFocused(() => create, "Create the GitHub App");
    await expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Skip for now" }), "Skip for now");
    await expectFocusInSight();
  },
};

/** An identity already there: done, with what it acts as and on how many repos. */
export const AlreadySetUp: Story = {
  beforeEach: faked(shown([ROW])),
  args: {
    step: { ...TO_SET_UP, state: "done", line: "gh through steward", findings: [], next: null },
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    await canvas.findByText(/Acting as acme-purlis-agent\[bot\] on 12 repos/);
    const go = canvas.getByRole("button", { name: "Continue to Try it" });
    go.focus();
    await untilFocused(() => go, "Continue to Try it");
    await expectFocusInSight();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open Agent identity" }),
      "Open Agent identity",
    );
    await expectFocusInSight();
  },
};

/** Repos on a host purlis can't set up by itself yet: said plainly, with the line by hand. */
export const AnotherHost: Story = {
  args: {
    step: {
      ...TO_SET_UP,
      findings: [
        "purlis can't set up git.acme.internal by itself yet: one-click setup works for github.com. You can still do this by hand, and it works just as well. Make a token on git.acme.internal for the agent, not for you, keep it in a vault of its own and add the tool line below. Once this machine allows it, this step is done.",
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
    },
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await untilFocused(() => again, "Check again");
    await expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Skip for now" }), "Skip for now");
    await expectFocusInSight();
  },
};

/** Skipped: what that means in a chat, and the way back. */
export const Skipped: Story = {
  args: { step: { ...TO_SET_UP, state: "skipped", findings: [], next: null } },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText("You skipped this for now")).toBeVisible();
    const now = canvas.getByRole("button", { name: "Set it up now" });
    now.focus();
    await untilFocused(() => now, "Set it up now");
    await expectFocusInSight();
  },
};

/** A persona's tool to allow on this machine: Settings, where it is allowed, and Check again. */
export const ToAllow: Story = {
  args: {
    step: {
      ...TO_SET_UP,
      findings: ["The persona steward declares gh, and this machine hasn't allowed it."],
      next: {
        id: "allow-tool",
        label: "Allow it in Settings › Project",
        does: "Opens Settings › Project: allow the persona's tool there, under Sandbox, on this machine. It never holds your own sign-in. This step is done once it's allowed.",
        command: null,
        url: null,
      },
    },
  },
  play: async ({ canvasElement, userEvent }) => {
    const canvas = within(canvasElement);
    const allow = canvas.getByRole("button", { name: "Allow it in Settings › Project" });
    allow.focus();
    await untilFocused(() => allow, "Allow it in Settings › Project");
    await expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Check again" }), "Check again");
    await expectFocusInSight();
  },
};

/** Repos on a host purlis knows no forge at: what to declare, and no line to copy. */
export const UnknownHost: Story = {
  args: {
    step: {
      ...TO_SET_UP,
      findings: [
        "Your repos are on bitbucket.org, and purlis knows no forge there, so it can't give the agent an identity on it. If bitbucket.org is a GitHub Enterprise Server or a GitLab, add it in Settings › Forges, then check again. Or skip this for now.",
      ],
      next: {
        id: "check-again",
        label: "Check again",
        does: "Reads the project's settings again. It changes nothing.",
        command: null,
        url: null,
      },
    },
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const again = canvas.getByRole("button", { name: "Check again" });
    again.focus();
    await untilFocused(() => again, "Check again");
    await expectFocusInSight();
  },
};
