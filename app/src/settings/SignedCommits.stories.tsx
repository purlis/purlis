// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Accounts › Signed commits** (#5628, `prototype/commit-signing`): the card not set up,
 * with no GitHub account to add the key to, on and verified, and on with no Check yet. Each is
 * drawn, held to axe, walked by the keyboard, and pictured in every pictured theme cell
 * (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SigningState } from "../bindings";
import { expectFocusInSight } from "../stories.testkit";
import { SettingGroup, SettingRow } from "./components";
import type { LiveSetting } from "./groups";
import { signedCommits } from "./signing";

const ACCOUNT = {
  login: "octocat",
  host: "github.com",
  forge: "GitHub",
  may_add: true,
  verified_as: null,
};

const OFF: SigningState = {
  on: false,
  fingerprint: null,
  public_key: null,
  title: "purlis · studio",
  account: ACCOUNT,
};

const ON: SigningState = {
  ...OFF,
  on: true,
  fingerprint: "SHA256:q3VbT1sMk2pQ1",
  public_key:
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK2Vb9pQ1example purlis signs commits made for chats",
};

/** The card's row, as the Settings tab draws a live setting. */
function Card() {
  const setting = signedCommits as LiveSetting;
  const { control, grouped, error } = setting.useControl();
  return (
    <SettingGroup label="Accounts">
      <SettingRow
        setting={setting.id}
        label={setting.label}
        help={setting.help}
        grouped={grouped}
        error={error}
        control={control}
      />
    </SettingGroup>
  );
}

const meta = {
  component: Card,
  beforeEach: ({ parameters }) => {
    const state = (parameters.signing as SigningState | undefined) ?? OFF;
    mockIPC((cmd) => (cmd === "signing_state" ? state : null));
    return () => clearMocks();
  },
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **Not set up**: what signing is, that the key stays on this Mac, and Set up signing. */
export const NotSetUp: Story = {
  parameters: { signing: OFF },
  play: async ({ canvasElement }) => {
    const set = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Set up signing" }),
    );
    await userEvent.tab();
    await expect(set).toHaveFocus();
    expectFocusInSight();
  },
};

/** **No GitHub account**: sign in first, and no setup offered. */
export const NoAccount: Story = {
  parameters: { signing: { ...OFF, account: null } },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(within(canvasElement).getByText(/Sign in to GitHub above first/)).toBeVisible(),
    );
    await expect(
      within(canvasElement).queryByRole("button", { name: "Set up signing" }),
    ).toBeNull();
  },
};

/** **On, verified**: the account Verified on GitHub ✓, the email it commits as, and Details. */
export const OnVerified: Story = {
  parameters: {
    signing: { ...ON, account: { ...ACCOUNT, verified_as: "octocat@example.com" } },
  },
  play: async ({ canvasElement }) => {
    const details = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Details" }),
    );
    await expect(within(canvasElement).getByText("Verified on GitHub ✓")).toBeVisible();
    details.focus();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(canvasElement).getByText(/purlis signing setup --key/)).toBeVisible(),
    );
    expectFocusInSight();
  },
};

/** **On, not checked yet in this window**: Check, never a ✓ GitHub didn't give. */
export const OnNotChecked: Story = {
  parameters: { signing: ON },
  play: async ({ canvasElement }) => {
    const check = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Check signing on octocat" }),
    );
    await userEvent.tab();
    await expect(check).toHaveFocus();
    expectFocusInSight();
  },
};
