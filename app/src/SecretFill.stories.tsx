// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { SecretFill } from "./SecretFill";
import type { VaultEntries, VaultEntry } from "./VaultEntryPicker";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

/** The entries the person keeps, vault by vault: names only (#5073). */
const KEPT: VaultEntries[] = [
  { vault: "own", keys: ["GITHUB_TOKEN", "STRIPE"] },
  { vault: "team", keys: ["DB_PASSWORD", "KUBECONFIG_PROD"] },
  { vault: "prod", keys: [], unlisted: "1Password is locked" },
];

/** The fill, holding the entry the person picks as the Inbox row does. */
function Held(
  props: Omit<Parameters<typeof SecretFill>[0], "picked" | "onPicked" | "remember" | "onRemember">,
) {
  const [picked, setPicked] = useState<VaultEntry>();
  const [remember, setRemember] = useState<string>();
  return (
    <SecretFill
      {...props}
      picked={picked}
      onPicked={setPicked}
      remember={remember}
      onRemember={setRemember}
    />
  );
}

const meta = {
  title: "Notices/Missing secret",
  component: Held,
  args: { entries: () => Promise.resolve(KEPT) },
} satisfies Meta<typeof Held>;

export default meta;
type Story = StoryObj<typeof meta>;

const secret = {
  persona: "devops",
  vault: "team",
  key: "KUBECONFIG",
  tool: "kubectl",
  declared: true,
  waiting: 1,
};

/** #5243: under policy's lock no Allow is offered, says why, and a key can still be filled. */
export const NoAllowUnderPolicy: Story = {
  args: {
    secret: {
      ...secret,
      tool: null,
      no_allow:
        "Policy turns off allowing a program with a vault's keys, so curl needs to be declared as a tool of devops's. Locked by policy, set by Platform team in /etc/purlis/policy.json.",
    },
    box: null,
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByTestId("secret-no-allow")).toHaveTextContent("Policy turns off");
    await expect(canvas.getByLabelText("Value of KUBECONFIG")).toBeVisible();
  },
};

/** #5068: a key a declared tool's run missed, with the box its value is typed in. */
export const ForATool: Story = {
  args: { secret, box: null },
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByLabelText("Value of KUBECONFIG");
    // New value is the way chosen, first in the Tab order; the box is next.
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("radio", { name: "New value" }), "New value");
    await userEvent.tab();
    await untilFocused(() => box, "the value's box");
    expectFocusInSight();
    await userEvent.keyboard("s3cret");
    // Masked: what was typed is never drawn.
    await expect(box).toHaveAttribute("type", "password");
  },
};

/** A key no declared tool names. */
export const NoTool: Story = {
  args: { secret: { ...secret, key: "API_TOKEN", tool: null }, box: null },
};

/**
 * #5073: "Use an entry I have" opens a picker over the person's vaults, grouped by vault with a
 * filter on top (the ruling on #5070); a pick says the key becomes a link, never a copy.
 */
export const UseAnEntry: Story = {
  args: { secret, box: null },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const entry = canvas.getByRole("radio", { name: "Use an entry I have" });
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("radio", { name: "New value" }), "New value");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => entry, "Use an entry I have");
    expectFocusInSight();
    await userEvent.tab();
    const picker = canvas.getByRole("button", { name: /Entry KUBECONFIG reads$/ });
    await untilFocused(() => picker, "the picker");
    await userEvent.keyboard("{Enter}");
    const filter = await screen.findByRole("textbox", { name: "Filter" });
    await untilFocused(() => filter, "the filter takes the keyboard");
    const list = screen.getByRole("listbox");
    await expect(within(list).getAllByRole("group")).toHaveLength(3);
    await userEvent.keyboard("stripe");
    const stripe = await within(list).findByRole("option", { name: "STRIPE" });
    await waitFor(() => expect(filter).toHaveAttribute("aria-activedescendant", stripe.id));
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent(
        "KUBECONFIG becomes a link to STRIPE in own",
      ),
    );
  },
};

/** #5073: nothing to link to yet, said in place of the picker's help. */
export const NoEntryToUse: Story = {
  args: { secret, box: null, entries: () => Promise.resolve([]) },
  play: async ({ canvas, userEvent }) => {
    // By the keyboard, so nothing is under a pointer when it is pictured.
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("radio", { name: "New value" }), "New value");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(
      () => canvas.getByRole("radio", { name: "Use an entry I have" }),
      "Use an entry I have",
    );
    await waitFor(() =>
      expect(canvas.getByRole("status")).toHaveTextContent("No vault holds an entry"),
    );
  },
};

/** #5075: the chat says the value doesn't work; the key is a link, so the entry it reads is named. */
export const ReplaceALink: Story = {
  args: {
    secret: {
      ...secret,
      key: "STRIPE_KEY",
      tool: null,
      replace: "the API answers 401 Unauthorized",
      reads: { vault: "own", key: "STRIPE" },
    },
    box: null,
  },
};

/** #5075: a link whose entry is gone, named so the person can mend it. */
export const BrokenLink: Story = {
  args: {
    secret: { ...secret, key: "STRIPE_KEY", tool: null, gone: { vault: "own", key: "STRIPE" } },
    box: null,
  },
};

/**
 * #5240, spec #5235: Remember is unticked; ticking it by keyboard shows the exact line with the
 * chat's words to edit, and a `vault:` line where the persona names none.
 */
export const RememberTheNeed: Story = {
  args: {
    secret: {
      ...secret,
      key: "STRIPE_KEY",
      tool: null,
      declared: false,
      remember: {
        file: "personas/devops/persona.md",
        need: true,
        purpose: "charge test cards in checkout",
        vault: "team",
      },
    },
    box: null,
  },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    const remember = canvas.getByRole("checkbox", { name: "Remember this need for the persona" });
    await expect(remember).not.toBeChecked();
    await expect(canvas.queryByText("secret-need.STRIPE_KEY:")).toBeNull();
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => remember, "Remember");
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await expect(remember).toBeChecked();
    await expect(canvas.getByText("secret-need.STRIPE_KEY:")).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: "What STRIPE_KEY is for" }),
      "the purpose's box",
    );
    expectFocusInSight();
  },
};

/** #5240: a key the persona declares, in no vault it names: the `vault:` line alone, no box. */
export const RememberOnlyTheVault: Story = {
  args: {
    secret: {
      ...secret,
      remember: { file: "personas/devops/persona.md", need: false, purpose: "", vault: "team" },
    },
    box: null,
  },
  parameters: { screenshot: "page" },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(
      canvas.getByRole("checkbox", { name: "Remember this need for the persona" }),
    );
    await expect(canvas.getByText("vault: team")).toBeVisible();
    await expect(canvas.queryByRole("textbox", { name: "What KUBECONFIG is for" })).toBeNull();
  },
};
