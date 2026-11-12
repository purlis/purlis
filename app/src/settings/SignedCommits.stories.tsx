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
import type { SigningAccount, SigningAccountKeys, SigningState } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
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

/** GitHub's account, holding this Mac's key and the person's laptop's. */
const HELD: SigningAccountKeys = {
  kind: "github",
  host: "github.com",
  has_key: true,
  others: [
    {
      id: "17",
      title: "purlis · Studio Mac mini",
      device: "Studio Mac mini",
      added: "2026-10-01",
      older_setup: false,
    },
  ],
  may_remove: true,
  keys_page: "https://github.com/settings/keys",
};

/** GitLab's account, without this Mac's key yet. */
const GITLAB_WITHOUT: SigningAccountKeys = {
  kind: "gitlab",
  host: "gitlab.example.org",
  has_key: false,
  others: [],
  may_remove: true,
  keys_page: "https://gitlab.example.org/-/user_settings/ssh_keys",
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
    const keys = (parameters.keys as SigningAccountKeys[] | undefined) ?? [];
    mockIPC((cmd) =>
      cmd === "signing_state"
        ? state
        : cmd === "signing_accounts_keys"
          ? keys
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
    // The accounts are a table (#6301): Tab lands on its row, and → walks its cells to Check.
    await userEvent.tab();
    const table = within(canvasElement).getByRole("grid", { name: "Signing on your accounts" });
    await untilFocused(() => within(table).getAllByRole("row")[1] as HTMLElement, "the row");
    await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    await untilFocused(() => check, "→ reaches the row's Check");
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

/** **On, managed** (#5632): GitHub Verified, GitLab without the key (Add to this account), the
 *  other Mac's key with Remove, Replace key and Remove signing. */
export const OnManaged: Story = {
  parameters: {
    signing: {
      ...ON,
      accounts: [{ ...ACCOUNT, verified_as: "octocat@example.com" }, GITLAB],
    },
    keys: [HELD, GITLAB_WITHOUT],
  },
  play: async ({ canvasElement }) => {
    const add = await waitFor(() =>
      within(canvasElement).getByRole("button", { name: "Add to this account: sam on GitLab" }),
    );
    await expect(
      within(canvasElement).getByRole("grid", { name: "Signing keys of your other devices" }),
    ).toHaveTextContent("Studio Mac mini");
    add.focus();
    await expect(add).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Other devices' keys** (#5918): the heading and what removing does, each key by its device and
 *  the day added, an older setup's key marked, and a quiet Remove… on each. */
export const OtherDevicesKeys: Story = {
  parameters: {
    signing: { ...ON, accounts: [{ ...ACCOUNT, verified_as: "octocat@example.com" }] },
    keys: [
      {
        ...HELD,
        others: [
          ...HELD.others,
          {
            id: "18",
            title: "purlis on old-mini",
            device: "old-mini",
            added: "2026-08-14",
            older_setup: true,
          },
        ],
      },
    ],
  },
  play: async ({ canvasElement }) => {
    const table = await waitFor(() =>
      within(canvasElement).getByRole("grid", { name: "Signing keys of your other devices" }),
    );
    await expect(table).toHaveTextContent("Older setup");
    const remove = within(table).getByRole("button", {
      name: "Remove old-mini's signing key from octocat on GitHub…",
    });
    remove.focus();
    await expect(remove).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Removing another device's key, confirmed** (#5918): the device named, what its commits show
 *  until it sets up again, and that this Mac is not affected; the confirm's Remove key is red. */
export const RemoveOtherDevicesKey: Story = {
  parameters: {
    signing: { ...ON, accounts: [{ ...ACCOUNT, verified_as: "octocat@example.com" }] },
    keys: [HELD],
    screenshot: "page",
  },
  play: async ({ canvasElement }) => {
    const table = await waitFor(() =>
      within(canvasElement).getByRole("grid", { name: "Signing keys of your other devices" }),
    );
    await userEvent.click(
      within(table).getByRole("button", {
        name: "Remove Studio Mac mini's signing key from octocat on GitHub…",
      }),
    );
    const confirm = await waitFor(() =>
      within(document.body).getByRole("dialog", {
        name: "Remove Studio Mac mini's signing key from octocat on GitHub?",
      }),
    );
    await expect(confirm).toHaveTextContent("This Mac is not affected.");
  },
};

/** **Remove signing's confirmation**: the accounts it is removed from, and what stays signed. */
export const RemoveSigning: Story = {
  parameters: {
    signing: { ...ON, accounts: [{ ...ACCOUNT, verified_as: "octocat@example.com" }] },
    keys: [HELD],
    screenshot: "page",
  },
  play: async ({ canvasElement }) => {
    await waitFor(() =>
      within(canvasElement).getByRole("grid", { name: "Signing keys of your other devices" }),
    );
    await userEvent.click(within(canvasElement).getByRole("button", { name: "Remove signing…" }));
    const confirm = await waitFor(() =>
      within(document.body).getByRole("dialog", { name: "Remove signing from this Mac?" }),
    );
    await expect(confirm).toHaveTextContent("Commits already signed stay as they are");
  },
};
