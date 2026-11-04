// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A vault's tab with linked entries** (#5069): a key that reads another entry's value names
 * that entry under its own name, and deleting an entry other keys link to says how many links
 * break. The core is faked at its command boundary; no value is ever drawn.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { VaultContents, VaultSecret } from "./bindings";
import { VaultTab } from "./VaultTab";
import { expectFocusInSight } from "./stories.testkit";

function secret(key: string, over: Partial<VaultSecret> = {}): VaultSecret {
  return {
    key,
    size: "16–31 bytes",
    updated: "2026-09-24T11:32:17Z",
    link: null,
    linked_by: 0,
    no_seed: false,
    ...over,
  };
}

const LINKED: VaultContents = {
  name: "ops",
  provider: "keyring",
  count: 3,
  health: { ok: true, detail: "3 secrets in the system keyring" },
  secrets: [
    secret("API_TOKEN", { linked_by: 2 }),
    secret("STRIPE_KEY", { link: { vault: "team", key: "STRIPE" }, size: null, updated: null }),
    secret("WEBHOOK", { link: { vault: "ops", key: "API_TOKEN" }, size: null, updated: null }),
  ],
  refused: null,
  identity_unset_elsewhere: [],
  identity: [],
  identity_in_app_env: [],
};

function answers() {
  mockIPC((cmd) => (cmd === "vault_open" ? LINKED : new Promise(() => {})));
}

const meta: Meta<typeof VaultTab> = {
  title: "Views/Vault secrets",
  component: VaultTab,
  args: { plane: "p", vault: "ops", onChanged: () => {} },
};
export default meta;
type Story = StoryObj<typeof VaultTab>;

export const LinkedEntries: Story = {
  beforeEach: answers,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await page.findByText("Linked to STRIPE in team");
  },
};

/** The rows from the keyboard to a linked-to entry's menu, and its Delete… asking first. */
export const DeleteALinkedToEntry: Story = {
  beforeEach: answers,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const row = await page.findByRole("row", { name: /^API_TOKEN/ });
    row.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const body = within(document.body);
    const menu = await body.findByRole("menu", { name: "API_TOKEN" });
    expectFocusInSight();
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Delete…" }));
    const asking = await body.findByRole("alertdialog", { name: "Delete API_TOKEN from ops?" });
    await expect(asking).toHaveTextContent("2 linked entries read it.");
    await expect(within(asking).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expectFocusInSight();
  },
};

/** Deleting a link says the entry it reads keeps its value; Escape keeps both. */
export const DeleteALink: Story = {
  beforeEach: answers,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const row = await page.findByRole("row", { name: /^STRIPE_KEY/ });
    row.focus();
    await userEvent.keyboard("{Enter}");
    const body = within(document.body);
    const menu = await body.findByRole("menu", { name: "STRIPE_KEY" });
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Delete…" }));
    const asking = await body.findByRole("alertdialog", { name: "Delete STRIPE_KEY from ops?" });
    await expect(asking).toHaveTextContent("Only the link goes. STRIPE in team keeps its value.");
    expectFocusInSight();
    await userEvent.keyboard("{Escape}");
    await expect(body.queryByRole("alertdialog")).toBeNull();
  },
};

const HELD: VaultContents = {
  ...LINKED,
  secrets: [secret("APP_PASSWORD"), secret("MAIL_PASSWORD", { no_seed: true })],
  count: 2,
};

function holding() {
  mockIPC((cmd) => (cmd === "vault_open" ? HELD : new Promise(() => {})));
}

/** A key the person marked says so on its row, and its menu takes the mark off. */
export const MarkedAsNoSeed: Story = {
  beforeEach: holding,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    await page.findByText("Marked as no 2FA seed. Fills as a plain value.");
    const row = await page.findByRole("row", { name: /^MAIL_PASSWORD/ });
    row.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const menu = await within(document.body).findByRole("menu", { name: "MAIL_PASSWORD" });
    expectFocusInSight();
    await within(menu).findByRole("menuitem", { name: "Remove the no-seed mark" });
    await userEvent.keyboard("{Escape}");
  },
};

/**
 * The rows from the keyboard to Mark as no 2FA seed…, which says what it is for and starts on
 * Cancel, since the mark widens what a chat can fill. Escape marks nothing.
 */
export const MarkAsNoSeedAsksFirst: Story = {
  beforeEach: holding,
  play: async ({ canvasElement }) => {
    const page = within(canvasElement);
    const row = await page.findByRole("row", { name: /^APP_PASSWORD/ });
    row.focus();
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const body = within(document.body);
    const menu = await body.findByRole("menu", { name: "APP_PASSWORD" });
    expectFocusInSight();
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Mark as no 2FA seed…" }));
    const asking = await body.findByRole("alertdialog", {
      name: "Mark APP_PASSWORD as no 2FA seed?",
    });
    await expect(asking).toHaveTextContent("a 16-letter app password");
    await expect(within(asking).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expectFocusInSight();
    await within(asking).findByRole("button", { name: "Mark as no seed" });
  },
};
