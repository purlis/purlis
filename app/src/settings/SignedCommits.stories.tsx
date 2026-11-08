// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Accounts › Signed commits** (#5628, #5629, `prototype/commit-signing`): the card not
 * set up, with no forge account to add the key to, on and verified, on with no Check run, on
 * with GitHub and GitLab accounts, and with a GitLab too old to verify signed commits. Each is
 * drawn, held to axe, walked by the keyboard, and pictured in every pictured theme cell
 * (`docs/component-stories.md`). The core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SigningAccount, SigningState } from "../bindings";
import { expectFocusInSight } from "../stories.testkit";
import { SettingGroup, SettingRow } from "./components";
import type { LiveSetting } from "./groups";
import { signedCommits } from "./signing";

const ACCOUNT: SigningAccount = {
  kind: "github",
  login: "octocat",
  host: "github.com",
  forge: "GitHub",
  may_add: true,
  verified_as: null,
  verifies: true,
};

const GITLAB: SigningAccount = {
  kind: "gitlab",
  login: "sam",
  host: "gitlab.example.org",
  forge: "GitLab",
  may_add: true,
  verified_as: null,
  verifies: true,
};

const OFF: SigningState = {
  on: false,
  title: "purlis · studio",
  accounts: [ACCOUNT],
};

const ON: SigningState = {
  ...OFF,
  on: true,
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
    mockIPC((cmd) =>
      cmd === "signing_state"
        ? state
        : cmd === "signing_key_shown"
          ? { fingerprint: "SHA256:q3VbT1sMk2pQ1", public_key: "ssh-ed25519 AAAA example" }
          : null,
    );
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

/** **No forge account**: sign in first, and no setup offered. */
export const NoAccount: Story = {
  parameters: { signing: { ...OFF, accounts: [] } },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(
        within(canvasElement).getByText(/Sign in to GitHub or GitLab above first/),
      ).toBeVisible(),
    );
    await expect(
      within(canvasElement).queryByRole("button", { name: "Set up signing" }),
    ).toBeNull();
  },
};

/** **On, verified**: the account Verified on GitHub ✓, the email it commits as, and Details. */
export const OnVerified: Story = {
  parameters: {
    signing: { ...ON, accounts: [{ ...ACCOUNT, verified_as: "octocat@example.com" }] },
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

/** **On, unchecked in this window**: Check, never a ✓ GitHub didn't give. */
export const OnNotChecked: Story = {
  parameters: { signing: ON },
  play: async ({ canvasElement }) => {
    const check = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Check signing on octocat on GitHub" }),
    );
    await userEvent.tab();
    await expect(check).toHaveFocus();
    expectFocusInSight();
  },
};

/** **On, GitHub and GitLab**: a row per account; GitLab Verified, GitHub still to check (#5629). */
export const OnGitHubAndGitLab: Story = {
  parameters: {
    signing: { ...ON, accounts: [ACCOUNT, { ...GITLAB, verified_as: "sam@example.org" }] },
  },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(within(canvasElement).getByText("Verified on GitLab ✓")).toBeVisible(),
    );
    await expect(
      within(canvasElement).getByRole("button", { name: "Check signing on octocat on GitHub" }),
    ).toBeVisible();
  },
};

/** **A GitLab too old to verify**: said plainly, and no setup offered for it (#5629). */
export const OldGitLab: Story = {
  parameters: { signing: { ...OFF, accounts: [{ ...GITLAB, verifies: false }] } },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      expect(
        within(canvasElement).getByText(/This server can't verify signed commits/),
      ).toBeVisible(),
    );
    await expect(
      within(canvasElement).queryByRole("button", { name: "Set up signing" }),
    ).toBeNull();
  },
};
