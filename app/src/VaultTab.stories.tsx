// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A vault's tab with linked entries** (#5069): a key that reads another entry's value names
 * that entry under its own name, and deleting an entry other keys link to says how many links
 * break. The core is faked at its command boundary; no value is ever drawn.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
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

/** A key that may be remembered for `shop`, which names no vault (#5240). */
function answersWithPersonas() {
  mockIPC((cmd) => {
    if (cmd === "vault_open") return LINKED;
    if (cmd === "secret_need_personas")
      return [
        {
          persona: "shop",
          file: "personas/shop/persona.md",
          declares: [],
          names_no_vault: true,
        },
      ];
    return new Promise(() => {});
  });
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

/**
 * #5240: *Offer as a Secret need ▸* opens the exact line, its purpose to type and the `vault:`
 * line, reached and added by the keyboard alone.
 */
export const OfferAsASecretNeed: Story = {
  parameters: { screenshot: "page" },
  beforeEach: () => answersWithPersonas(),
  play: async ({ canvasElement }) => {
    const page = within(canvasElement.ownerDocument.body);
    const more = await page.findByRole("button", { name: "More for API_TOKEN" });
    more.focus();
    await userEvent.keyboard("{Enter}");
    (await page.findByRole("menuitem", { name: "Offer as a Secret need" })).focus();
    await userEvent.keyboard("{Enter}");
    (await page.findByRole("menuitem", { name: "shop" })).focus();
    await userEvent.keyboard("{Enter}");
    const dialog = await page.findByRole("dialog", { name: "Remember API_TOKEN for shop" });
    // Once the dialog has come in.
    await waitFor(() => expect(within(dialog).getByText("secret-need.API_TOKEN:")).toBeVisible());
    await expect(within(dialog).getByText("vault: ops")).toBeVisible();
    const add = within(dialog).getByRole("button", { name: "Add the line" });
    await expect(add).toBeDisabled();
    const purpose = within(dialog).getByRole("textbox", { name: "What API_TOKEN is for" });
    purpose.focus();
    await userEvent.keyboard("reach the API");
    await expect(add).toBeEnabled();
    expectFocusInSight();
  },
};
