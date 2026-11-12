// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Accounts** (#1376, #5628, #6301): the forge accounts signed in on this machine, each
 * with how it signed in and its Sign out, then Signed commits on, with each account's signing and
 * the signing keys of the person's other devices, on the aligned page. Drawn, held to axe, walked
 * by the keyboard and pictured in every pictured theme cell (`docs/component-stories.md`). The
 * core is faked at its command boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type {
  ForgeAccountRow,
  SigningAccount,
  SigningAccountKeys,
  SigningState,
} from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { accountsGroup } from "./accounts";
import { OnItsPage } from "./settingsStory.testkit";

const ACCOUNTS: ForgeAccountRow[] = [
  {
    id: "github:github.com:octocat",
    kind: "github",
    host: "github.com",
    login: "octocat",
    method: "browser",
    signed_in: true,
    scopes: ["repo", "workflow"],
    wider: [],
  },
  {
    id: "gitlab:gitlab.example.org:sam",
    kind: "gitlab",
    host: "gitlab.example.org",
    login: "sam",
    method: "token",
    signed_in: true,
    scopes: ["api"],
    wider: ["api"],
  },
];

const GITHUB: SigningAccount = {
  kind: "github",
  login: "octocat",
  host: "github.com",
  forge: "GitHub",
  may_add: true,
  verified_as: "octocat@users.noreply.github.com",
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

const SIGNING: SigningState = { on: true, title: "purlis · studio", accounts: [GITHUB, GITLAB] };

const KEYS: SigningAccountKeys[] = [
  {
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
      {
        id: "21",
        title: "purlis",
        device: "MacBook Air",
        added: "2026-08-14",
        older_setup: true,
      },
    ],
    may_remove: true,
    keys_page: "https://github.com/settings/keys",
  },
  {
    kind: "gitlab",
    host: "gitlab.example.org",
    has_key: false,
    others: [],
    may_remove: true,
    keys_page: "https://gitlab.example.org/-/user_settings/ssh_keys",
  },
];

/** The core, faked: these accounts, signing on with these keys, and no gh or glab to import. */
function theCore(accounts: ForgeAccountRow[]) {
  mockIPC(
    (cmd) => {
      if (cmd === "forge_accounts") return accounts;
      if (cmd === "signing_state") return SIGNING;
      if (cmd === "signing_accounts_keys") return KEYS;
      if (cmd === "signing_key_shown")
        return { fingerprint: "SHA256:q3VbT1sMk2pQ1", public_key: "ssh-ed25519 AAAA example" };
      if (cmd === "forge_cli_logins") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
}

function Page() {
  return <OnItsPage group={accountsGroup()} />;
}

const meta = {
  component: Page,
  beforeEach: ({ parameters }) => {
    theCore((parameters.accounts as ForgeAccountRow[] | undefined) ?? ACCOUNTS);
    return () => clearMocks();
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **Two accounts, signing on**: each account's login, host and how it signed in with its Sign
 * out; each one's signing (verified, or the key still to add); and the other devices' keys, one
 * from an older setup, each with Remove….
 */
export const SignedInAndSigning: Story = {
  play: async ({ canvas, userEvent }) => {
    // The accounts, a table (#6301): a row an account, Sign out at its end.
    const accounts = await canvas.findByRole("grid", { name: "Forge accounts" });
    await expect(
      within(accounts)
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["octocat", "sam"]);
    await canvas.findByRole("button", {
      name: "Remove Studio Mac mini's signing key from octocat on GitHub…",
    });
    await expect(canvas.getByText("Older setup")).toBeVisible();
    const rows = () => within(accounts).getAllByRole("row").slice(1);
    (rows()[0] as HTMLElement).focus();
    await untilFocused(() => rows()[0] as HTMLElement, "the first account's row");
    await userEvent.keyboard("{ArrowDown}{ArrowRight}{ArrowRight}{ArrowRight}{ArrowRight}");
    await untilFocused(
      () =>
        within(rows()[1] as HTMLElement).getByRole("button", {
          name: "Sign out of sam on gitlab.example.org",
        }),
      "↓ and → reach the second account's Sign out",
    );
    expectFocusInSight();
  },
};

/** **No account**: the list says so. Words only. */
export const NoAccount: Story = {
  parameters: { accounts: [] },
  play: async ({ canvas }) => {
    const group = await canvas.findByRole("region", { name: "Accounts" });
    await within(group).findByText("No forge account is signed in on this machine.");
  },
};
