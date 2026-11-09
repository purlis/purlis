// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Set up signing** (#5628, #5629, `prototype/commit-signing` A with C's steps that run on): the
 * three-step dialog in each state a person can meet, on GitHub and on GitLab. Drawn with the core faked at its command
 * boundary, held to axe, walked by the keyboard, and pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type {
  SigningAccount,
  SigningCheck,
  SigningEmails,
  SigningForge,
  SigningState,
} from "./bindings";
import { SigningSetupDialog } from "./SigningSetupDialog";
import { expectFocusInSight } from "./stories.testkit";

const ACCOUNT: SigningAccount = {
  kind: "github",
  login: "octocat",
  host: "github.com",
  forge: "GitHub",
  may_add: false,
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

const ON_GITLAB: SigningForge = { kind: "gitlab", host: "gitlab.example.org" };

const OFF: SigningState = {
  on: false,
  title: "purlis · studio",
  accounts: [ACCOUNT],
};

const ON: SigningState = {
  ...OFF,
  on: true,
};

/** The account's verified addresses, primary first and GitHub's noreply last. */
const EMAILS: SigningEmails = {
  emails: [
    { email: "octocat@example.com", primary: true, private: false },
    { email: "583231+octocat@users.noreply.github.com", primary: false, private: true },
  ],
  settings: "https://github.com/settings/emails",
};

/** The core, faked: `state` as setup opens, and how each step answers. */
function theCore({
  state,
  makeRefused,
  added = "needs_permission",
  checked = {
    verified: true,
    reason: "valid",
    email: "octocat@example.com",
    repo: "octocat/hello",
    email_is_why: false,
  },
  waitRefused,
  shownRefused,
  emails = EMAILS,
}: {
  state: SigningState;
  makeRefused?: string;
  added?: "added" | "needs_permission";
  checked?: SigningCheck;
  waitRefused?: string;
  /** Why the Keychain wouldn't give the key, where macOS was told no. */
  shownRefused?: string;
  emails?: SigningEmails;
}) {
  mockIPC((cmd) => {
    switch (cmd) {
      case "signing_state":
        return state;
      case "signing_make_key":
        if (makeRefused) throw makeRefused;
        return "SHA256:q3VbT1sMk2pQ1";
      case "signing_add_key":
        return added;
      case "forge_signing_permission_start":
        return {
          flow: 1,
          host: "github.com",
          title: "GitHub",
          user_code: "WDJB-MJHT",
          verification_uri: "https://github.com/login/device",
          expires_in: 900,
          scopes: ["repo", "read:org", "admin:ssh_signing_key", "user:email"],
        };
      case "forge_signing_permission_wait":
        return waitRefused ? Promise.reject(waitRefused) : new Promise(() => undefined);
      case "signing_key_listed":
        return false;
      case "signing_key_shown":
        if (shownRefused) throw shownRefused;
        return {
          fingerprint: "SHA256:q3VbT1sMk2pQ1",
          public_key:
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK2Vb9pQ1example purlis signs commits made for chats",
        };
      case "signing_check":
        return checked;
      case "signing_emails":
        return emails;
      default:
        return null;
    }
  });
}

function Shown({ forge = null }: { forge?: SigningForge | null }) {
  return <SigningSetupDialog forge={forge} onClose={() => undefined} />;
}

const meta = {
  component: Shown,
  // A dialog is drawn over the page, outside the story's frame: the page is pictured.
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Shown>;

export default meta;
type Story = StoryObj<typeof meta>;

async function theDialog(): Promise<HTMLElement> {
  return waitFor(() => within(document.body).getByRole("dialog", { name: "Set up signing" }));
}

/** **Make your key**: the sentence before macOS asks, and Make my key. */
export const MakeYourKey: Story = {
  beforeEach: () => {
    theCore({ state: OFF });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/macOS will ask next/);
    const make = within(dialog).getByRole("button", { name: "Make my key" });
    make.focus();
    await expect(make).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Make your key, refused**: macOS was told no; Try again. */
export const KeychainDenied: Story = {
  beforeEach: () => {
    theCore({
      state: OFF,
      makeRefused:
        "macOS said no, so purlis couldn't keep the key. Try again and choose Always Allow.",
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await userEvent.click(await within(dialog).findByRole("button", { name: "Make my key" }));
    await within(dialog).findByRole("alert");
    const again = within(dialog).getByRole("button", { name: "Try again" });
    await expect(again).toBeEnabled();
    again.focus();
    await expect(again).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Add it to GitHub**: what the permission is for, Continue to GitHub, and the way by hand. */
export const AddItToGitHub: Story = {
  beforeEach: () => {
    theCore({ state: ON });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/manage your signing keys/);
    await userEvent.tab();
    expectFocusInSight();
    await expect(within(dialog).getByRole("button", { name: "I'll add it myself" })).toBeEnabled();
  },
};

/** **Add it to GitHub, waiting**: the code to enter on GitHub's page. */
export const TheCodeForGitHub: Story = {
  beforeEach: () => {
    theCore({ state: ON });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Continue to GitHub" }),
    );
    await within(dialog).findByText("WDJB-MJHT");
    await userEvent.keyboard("{Tab}");
    expectFocusInSight();
  },
};

/** **Add it yourself**: the key copied, GitHub's page, the type to choose, and the watching. */
export const ByHand: Story = {
  beforeEach: () => {
    theCore({
      state: ON,
      waitRefused:
        "You didn't allow it on GitHub, so nothing changed. You can add the key yourself instead.",
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Continue to GitHub" }),
    );
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "I'll add it myself" }),
    );
    await within(dialog).findByText(/Waiting for it to appear on octocat/);
    await expect(dialog).toHaveTextContent("Signing Key");
    const added = within(dialog).getByRole("button", { name: "I've added it" });
    added.focus();
    await expect(added).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Add it yourself, the key unread** (#5681): the Keychain wouldn't give the key, so the step
 *  says so and offers Try again, with no box for a key that wasn't read. */
export const ByHandKeyUnread: Story = {
  beforeEach: () => {
    theCore({
      state: ON,
      waitRefused:
        "You didn't allow it on GitHub, so nothing changed. You can add the key yourself instead.",
      shownRefused: "macOS said no, so purlis couldn't read the key.",
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Continue to GitHub" }),
    );
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "I'll add it myself" }),
    );
    await within(dialog).findByText(/couldn't read the key from the Keychain/);
    const again = within(dialog).getByRole("button", { name: "Try again" });
    again.focus();
    await expect(again).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Check, the email**: the commit email isn't verified on GitHub; the account's to choose (#5631). */
export const CheckNotVerified: Story = {
  beforeEach: () => {
    theCore({
      state: { ...ON, accounts: [{ ...ACCOUNT, may_add: true }] },
      added: "added",
      checked: {
        verified: false,
        reason: "unverified_email",
        email: "octo@studio.local",
        repo: "octocat/hello",
        email_is_why: false,
      },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    const picked = await within(dialog).findByRole("radio", { name: "octocat@example.com" });
    await expect(picked).toBeChecked();
    await userEvent.tab();
    expectFocusInSight();
    await expect(within(dialog).getByRole("button", { name: "Use this email" })).toBeEnabled();
  },
};

/** **Check, a key GitHub doesn't know**: the account lists no key of this Mac's, so the reason is
 *  the key's, and the email the test commit was made as is named (#5916). */
export const CheckKeyNotOnAccount: Story = {
  beforeEach: () => {
    theCore({
      state: { ...ON, accounts: [{ ...ACCOUNT, may_add: true }] },
      added: "added",
      checked: {
        verified: false,
        reason: "unknown_key",
        email: "octocat@example.com",
        repo: "octocat/hello",
        email_is_why: false,
      },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/doesn't know this key as a signing key/);
    await expect(dialog).toHaveTextContent("The test commit was made as octocat@example.com.");
    await userEvent.tab();
    expectFocusInSight();
  },
};

/** **Check, the key unmatched by the email**: GitHub said it doesn't know a key the account lists,
 *  because the test commit's email isn't one of the account's; the account's to choose, never
 *  "add it" (#5916). */
export const CheckKeyUnmatchedByEmail: Story = {
  beforeEach: () => {
    theCore({
      state: { ...ON, accounts: [{ ...ACCOUNT, may_add: true }] },
      added: "added",
      checked: {
        verified: false,
        reason: "unknown_key",
        email: "octo@studio.local",
        repo: "octocat/hello",
        email_is_why: true,
      },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    const picked = await within(dialog).findByRole("radio", { name: "octocat@example.com" });
    await expect(picked).toBeChecked();
    await expect(dialog).toHaveTextContent("picks the account to check a signature");
    await userEvent.tab();
    expectFocusInSight();
    await expect(within(dialog).getByRole("button", { name: "Use this email" })).toBeEnabled();
  },
};

/** **Check, no verified email**: the sentence, the forge's email settings, and no ✓ (#5631). */
export const NoVerifiedEmail: Story = {
  beforeEach: () => {
    theCore({
      state: { ...ON, accounts: [{ ...ACCOUNT, may_add: true }] },
      added: "added",
      checked: {
        verified: false,
        reason: "unverified_email",
        email: "octo@studio.local",
        repo: "octocat/hello",
        email_is_why: false,
      },
      emails: { emails: [], settings: "https://github.com/settings/emails" },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/has no verified email on GitHub yet/);
    const open = within(dialog).getByRole("button", { name: "Open GitHub's email settings" });
    open.focus();
    await expect(open).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Verified on GitHub ✓**: GitHub's own answer, and Done. */
export const Verified: Story = {
  beforeEach: () => {
    theCore({ state: { ...ON, accounts: [{ ...ACCOUNT, may_add: true }] }, added: "added" });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText("Verified on GitHub ✓");
    const done = within(dialog).getByRole("button", { name: "Done" });
    done.focus();
    await expect(done).toHaveFocus();
    expectFocusInSight();
  },
};

/** **GitLab, adding**: the key added as a signing key with the sign-in purlis has, no consent. */
export const GitLabAdding: Story = {
  args: { forge: ON_GITLAB },
  beforeEach: () => {
    // Adding never answers: the story stays on the step that adds it.
    theCore({
      state: { ...ON, accounts: [GITLAB] },
      added: new Promise(() => undefined) as unknown as "added",
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/as a signing key, with the sign-in purlis already has/);
    await expect(dialog).toHaveTextContent("Add it to GitLab");
    await expect(within(dialog).queryByRole("button", { name: "I'll add it myself" })).toBeNull();
  },
};

/** **Verified on GitLab ✓**: GitLab's own answer on a signed commit a chat pushed there. */
export const GitLabVerified: Story = {
  args: { forge: ON_GITLAB },
  beforeEach: () => {
    theCore({
      state: { ...ON, accounts: [GITLAB] },
      added: "added",
      checked: {
        verified: true,
        reason: "valid",
        email: "sam@example.org",
        repo: "platform/billing",
      },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText("Verified on GitLab ✓");
    const done = within(dialog).getByRole("button", { name: "Done" });
    done.focus();
    await expect(done).toHaveFocus();
    expectFocusInSight();
  },
};

/** **GitLab, nothing pushed yet**: signing is on; the ✓ waits for GitLab to see a signed commit. */
export const GitLabNothingPushed: Story = {
  args: { forge: ON_GITLAB },
  beforeEach: () => {
    theCore({
      state: { ...ON, accounts: [GITLAB] },
      added: "added",
      checked: {
        verified: false,
        reason: "nothing_pushed",
        email: "sam@example.org",
        repo: "",
        email_is_why: false,
      },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/the ✓ shows once a chat pushes its first signed commit/);
    await expect(dialog).not.toHaveTextContent("Verified on GitLab ✓");
    const done = within(dialog).getByRole("button", { name: "Done" });
    done.focus();
    await expect(done).toHaveFocus();
    expectFocusInSight();
  },
};

/** **A GitLab too old to verify**: said plainly, and no step offered. */
export const GitLabTooOld: Story = {
  args: { forge: ON_GITLAB },
  beforeEach: () => {
    theCore({ state: { ...OFF, accounts: [{ ...GITLAB, verifies: false }] } });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByText(/This server can't verify signed commits/);
    await expect(within(dialog).queryByRole("button", { name: "Make my key" })).toBeNull();
  },
};
