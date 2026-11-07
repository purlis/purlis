// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Set up signing** (#5628, `prototype/commit-signing` A with C's steps that run on): the
 * three-step dialog in each state a person can meet. Drawn with the core faked at its command
 * boundary, held to axe, walked by the keyboard, and pictured in every pictured theme cell.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SigningCheck, SigningState } from "./bindings";
import { SigningSetupDialog } from "./SigningSetupDialog";
import { expectFocusInSight } from "./stories.testkit";

const ACCOUNT = {
  login: "octocat",
  host: "github.com",
  forge: "GitHub",
  may_add: false,
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
  },
  waitRefused,
}: {
  state: SigningState;
  makeRefused?: string;
  added?: "added" | "needs_permission";
  checked?: SigningCheck;
  waitRefused?: string;
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
      case "signing_check":
        return checked;
      default:
        return null;
    }
  });
}

function Shown() {
  return <SigningSetupDialog onClose={() => undefined} />;
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
    await expect(within(dialog).getByRole("button", { name: "Try again" })).toBeEnabled();
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
    expectFocusInSight();
  },
};

/** **Check, not verified**: why GitHub didn't verify it, and Check again. */
export const CheckNotVerified: Story = {
  beforeEach: () => {
    theCore({
      state: { ...ON, account: { ...ACCOUNT, may_add: true } },
      added: "added",
      checked: {
        verified: false,
        reason: "unverified_email",
        email: "octo@studio.local",
        repo: "octocat/hello",
      },
    });
    return () => clearMocks();
  },
  play: async () => {
    const dialog = await theDialog();
    await within(dialog).findByRole("alert");
    const again = within(dialog).getByRole("button", { name: "Check again" });
    again.focus();
    await expect(again).toHaveFocus();
    expectFocusInSight();
  },
};

/** **Verified on GitHub ✓**: GitHub's own answer, and Done. */
export const Verified: Story = {
  beforeEach: () => {
    theCore({ state: { ...ON, account: { ...ACCOUNT, may_add: true } }, added: "added" });
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
