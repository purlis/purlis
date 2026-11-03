// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Agent identity** (#4485): every state a person meets the tab in, drawn, held to axe and
 * pictured in every pictured theme cell. The core is faked at its command boundary. These stories
 * are the design's record (the person's ruling of 2026-11-01).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { IdentitiesShown, IdentityRow, IdentityStand } from "./bindings";
import { AgentIdentityTab } from "./AgentIdentityTab";

const NONE: IdentitiesShown = {
  rows: [],
  pending: [],
  owners: [{ name: "acme", personal: false }],
  personas: ["steward", "reviewer", "writer"],
};

const ROW: IdentityRow = {
  host: "github.com",
  app: "Iv23liExample",
  personas: ["steward", "reviewer"],
  acts_as: "acme-purlis-agent",
  repos: 12,
  settings_page: "https://github.com/organizations/acme/settings/apps/acme-purlis-agent",
  trouble: null,
  by_hand: false,
};

const STAND: IdentityStand = {
  acts_as: "acme-purlis-agent",
  owner: "acme",
  installed: true,
  workflows: false,
  install_page: "https://github.com/apps/acme-purlis-agent/installations/new",
  settings_page: "https://github.com/organizations/acme/settings/apps/acme-purlis-agent",
};

const LINES = `[sandbox.personas.steward.tools]
gh = { vault = "agent-identity", mint = "github-app", app = "Iv23liExample", key = "agent-identity-github-Iv23liExample" }`;

function answers(shown: IdentitiesShown, more: Record<string, unknown> = {}) {
  mockIPC((cmd) => {
    if (cmd === "agent_identities") return shown;
    if (cmd === "agent_identity_lines") return LINES;
    if (cmd === "agent_identity_stand") return shown.pending[0]?.stand;
    if (cmd in more) {
      const said = more[cmd];
      return typeof said === "function" ? (said as () => unknown)() : said;
    }
    return new Promise(() => {});
  });
}

const meta: Meta<typeof AgentIdentityTab> = {
  title: "Views/Agent identity",
  component: AgentIdentityTab,
  args: { plane: "p" },
};
export default meta;
type Story = StoryObj<typeof AgentIdentityTab>;

const PENDING = { app: "Iv23liExample", stand: STAND, trouble: null };

export const Reading: Story = {
  beforeEach: () => mockIPC(() => new Promise(() => {})),
};

export const ReadRefused: Story = {
  beforeEach: () => mockIPC(() => Promise.reject("The project's settings file doesn't parse.")),
};

export const Start: Story = {
  beforeEach: () => answers(NONE),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const first = await page.findByRole("checkbox", { name: "steward" });
    await userEvent.tab();
    await expect(first).toHaveFocus();
  },
};

export const NoOwnerFound: Story = { beforeEach: () => answers({ ...NONE, owners: [] }) };

export const NoPersonas: Story = { beforeEach: () => answers({ ...NONE, personas: [] }) };

export const NotMade: Story = {
  beforeEach: () =>
    answers(NONE, { agent_identity_create: () => Promise.reject("GitHub didn't make the App.") }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await userEvent.click(await page.findByRole("button", { name: "Create the GitHub App" }));
    await page.findByText("GitHub didn't make the App.");
  },
};

export const OnGitHub: Story = {
  beforeEach: () => answers(NONE),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await userEvent.click(await page.findByRole("button", { name: "Create the GitHub App" }));
    await page.findByRole("heading", { name: "Finish on GitHub" });
  },
};

export const ReadingTheApp: Story = {
  beforeEach: () =>
    answers({
      ...NONE,
      pending: [{ app: "Iv23liExample", stand: null, trouble: "GitHub didn't answer." }],
    }),
};

export const ChooseItsRepos: Story = {
  beforeEach: () =>
    answers({ ...NONE, pending: [{ ...PENDING, stand: { ...STAND, installed: false } }] }),
};

export const OneLastLook: Story = { beforeEach: () => answers({ ...NONE, pending: [PENDING] }) };

export const WorkflowsToo: Story = {
  beforeEach: () =>
    answers({ ...NONE, pending: [{ ...PENDING, stand: { ...STAND, workflows: true } }] }),
};

export const FinishFailed: Story = {
  beforeEach: () =>
    answers(
      { ...NONE, pending: [PENDING] },
      { agent_identity_finish: () => Promise.reject("GitHub refused a token. Try again.") },
    ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await userEvent.click(await page.findByRole("button", { name: "Finish" }));
    await page.findByText("GitHub refused a token. Try again.");
  },
};

export const Proved: Story = {
  beforeEach: () =>
    answers(
      { ...NONE, pending: [PENDING] },
      { agent_identity_finish: { row: ROW, read: "acme/web", to_allow: ["steward"] } },
    ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await userEvent.click(await page.findByRole("button", { name: "Finish" }));
    await page.findByText(/and read acme\/web with it/);
  },
};

export const SetUp: Story = { beforeEach: () => answers({ ...NONE, rows: [ROW] }) };

export const GitHubCouldNotConfirm: Story = {
  beforeEach: () =>
    answers({
      ...NONE,
      rows: [
        {
          ...ROW,
          acts_as: null,
          repos: null,
          settings_page: null,
          trouble: "This machine holds no key for this App, so purlis can't check it.",
        },
      ],
    }),
};

export const WrittenByHand: Story = {
  beforeEach: () =>
    answers({
      ...NONE,
      rows: [{ ...ROW, by_hand: true, acts_as: null, repos: null, settings_page: null }],
    }),
};

export const RemoveAsksFirst: Story = {
  beforeEach: () => answers({ ...NONE, rows: [ROW] }),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    (await page.findByRole("button", { name: "Remove" })).focus();
    await userEvent.keyboard("{Enter}");
    const body = within(document.body);
    await body.findByRole("alertdialog");
    await userEvent.keyboard("{Escape}");
    await expect(body.queryByRole("alertdialog")).toBeNull();
  },
};

export const Removed: Story = {
  beforeEach: () =>
    answers(
      { ...NONE, rows: [ROW] },
      {
        agent_identity_remove: { settings_page: ROW.settings_page },
      },
    ),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await userEvent.click(await page.findByRole("button", { name: "Remove" }));
    const dialog = within(await within(document.body).findByRole("alertdialog"));
    await userEvent.click(dialog.getByRole("button", { name: "Remove" }));
    await page.findByRole("button", { name: "Delete it on GitHub" });
  },
};
