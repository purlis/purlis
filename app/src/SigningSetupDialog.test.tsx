// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type {
  ForgeAccountRow,
  ForgeDeviceCode,
  SigningAccount,
  SigningCheck,
  SigningState,
} from "./bindings";
import { NEW_SIGNING_KEY_PAGE, SigningSetupDialog } from "./SigningSetupDialog";
import { accountsGroup } from "./settings/accounts";
import type { LiveSetting } from "./settings/groups";

/**
 * **Set up signing** (#5628, ADR 0116 §4): from the Signed commits card, a person with no key
 * reaches *Verified on GitHub ✓* without a terminal, on the permission path and on the by-hand
 * path. Keychain Deny, a declined permission and closing half-way each leave things true.
 */

const PUBLIC = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK2Vb9test purlis signs commits made for chats";

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

const ON_GITHUB = { kind: "github", host: "github.com" };
const ON_GITLAB = { kind: "gitlab", host: "gitlab.example.org" };

const OFF: SigningState = {
  on: false,
  title: "purlis · studio",
  accounts: [ACCOUNT],
};

const ON: SigningState = {
  ...OFF,
  on: true,
};

const CODE: ForgeDeviceCode = {
  flow: 9,
  host: "github.com",
  title: "GitHub",
  user_code: "WDJB-MJHT",
  verification_uri: "https://github.com/login/device",
  expires_in: 900,
  scopes: ["repo", "read:org", "admin:ssh_signing_key", "user:email"],
};

const VERIFIED: SigningCheck = {
  verified: true,
  reason: "valid",
  email: "octocat@example.com",
  repo: "octocat/hello",
};

let state: SigningState;
let sent: { cmd: string; args: Record<string, unknown> }[];
/** How the next key is made: kept, or refused by macOS. */
let makeRefused: string | undefined;
/** What adding the key answers. */
let addAnswers: "added" | "needs_permission";
/** Whether GitHub's public list has the key yet. */
let listed: boolean;
/** What Check answers. */
let checked: SigningCheck;
/** Ends the permission wait in progress. */
let allow: () => void;
let refuse: (why: string) => void;

beforeEach(() => {
  state = structuredClone(OFF);
  sent = [];
  makeRefused = undefined;
  addAnswers = "added";
  listed = false;
  checked = VERIFIED;
  allow = () => undefined;
  refuse = () => undefined;
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText: vi.fn(() => Promise.resolve()) },
    configurable: true,
  });
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    switch (cmd) {
      case "signing_state":
        return structuredClone(state);
      case "signing_make_key":
        if (makeRefused !== undefined) throw makeRefused;
        state = { ...state, on: true };
        return "SHA256:q3VbT1sM";
      case "forge_signing_permission_start":
        return CODE;
      case "forge_signing_permission_wait":
        return new Promise<ForgeAccountRow>((resolve, reject) => {
          allow = () => {
            addAnswers = "added";
            resolve({
              id: "01J9OCTOCAT",
              kind: "github",
              host: "github.com",
              login: "octocat",
              method: "browser",
              signed_in: true,
              scopes: CODE.scopes,
              wider: [],
            });
          };
          refuse = (why) => reject(why);
        });
      case "signing_add_key":
        return addAnswers;
      case "signing_key_listed":
        return listed;
      case "signing_key_shown":
        return { fingerprint: "SHA256:q3VbT1sM", public_key: PUBLIC };
      case "signing_check":
        return checked;
      default:
        return null;
    }
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const commandsSent = (cmd: string) => sent.filter((one) => one.cmd === cmd);

async function theDialog() {
  return screen.findByRole("dialog", { name: "Set up signing" });
}

describe("SigningSetupDialog", () => {
  it("says macOS will ask before it makes the key, then asks GitHub for the permission and ends Verified", async () => {
    addAnswers = "needs_permission";
    const closed = vi.fn();
    render(<SigningSetupDialog onClose={closed} />);
    const dialog = await theDialog();
    expect(await within(dialog).findByText(/macOS will ask next/)).toBeTruthy();
    expect(dialog).toHaveTextContent("Always Allow");
    expect(dialog).toHaveTextContent("Step 1 of 3");
    expect(commandsSent("signing_make_key")).toEqual([]);

    await userEvent.click(within(dialog).getByRole("button", { name: "Make my key" }));
    await within(dialog).findByText(/manage your signing keys/);
    expect(dialog).toHaveTextContent("read your email addresses");
    expect(dialog).toHaveTextContent("Step 2 of 3");

    await userEvent.click(within(dialog).getByRole("button", { name: "Continue to GitHub" }));
    expect(await within(dialog).findByText("WDJB-MJHT")).toBeTruthy();
    await userEvent.click(within(dialog).getByRole("button", { name: "Open GitHub" }));
    await waitFor(() =>
      expect(commandsSent("plugin:opener|open_url")[0]?.args).toMatchObject({
        url: "https://github.com/login/device",
      }),
    );

    allow();
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
    expect(dialog).toHaveTextContent("as octocat@example.com");
    // Added once, after the permission: not tried before it.
    expect(commandsSent("signing_add_key").length).toBe(1);
    expect(commandsSent("signing_check")).toEqual([
      { cmd: "signing_check", args: { forge: ON_GITHUB, repo: null, commit: null } },
    ]);
    await userEvent.click(within(dialog).getByRole("button", { name: "Done" }));
    expect(closed).toHaveBeenCalled();
  });

  it("says the key couldn't be kept when macOS was told no, and makes it on Try again", async () => {
    makeRefused =
      "macOS said no, so purlis couldn't keep the key. Try again and choose Always Allow.";
    state = { ...structuredClone(OFF), accounts: [{ ...ACCOUNT, may_add: true }] };
    render(<SigningSetupDialog onClose={vi.fn()} />);
    const dialog = await theDialog();
    await userEvent.click(await within(dialog).findByRole("button", { name: "Make my key" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(
      "couldn't keep the key",
    );
    expect(dialog).toHaveTextContent("Step 1 of 3");

    makeRefused = undefined;
    await userEvent.click(within(dialog).getByRole("button", { name: "Try again" }));
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
  });

  it("after the permission is declined, copies the key, opens GitHub's page and carries on once the key is there", async () => {
    state = structuredClone(ON);
    addAnswers = "needs_permission";
    render(<SigningSetupDialog onClose={vi.fn()} />);
    const dialog = await theDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Continue to GitHub" }),
    );
    await within(dialog).findByText("WDJB-MJHT");
    refuse(
      "You didn't allow it on GitHub, so nothing changed. You can add the key yourself instead.",
    );
    expect((await within(dialog).findByRole("alert")).textContent).toContain("You didn't allow it");

    await userEvent.click(within(dialog).getByRole("button", { name: "I'll add it myself" }));
    expect(await within(dialog).findByText("The key is copied.", { exact: false })).toBeTruthy();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(PUBLIC);
    expect(dialog).toHaveTextContent("Signing Key");
    expect(dialog).toHaveTextContent("purlis · studio");
    await waitFor(() =>
      expect(commandsSent("plugin:opener|open_url")[0]?.args).toMatchObject({
        url: NEW_SIGNING_KEY_PAGE,
      }),
    );

    // Not there yet: it keeps waiting. Then the person adds it.
    // Asked once as the dialog opened (not there yet), and again on I've added it.
    await userEvent.click(within(dialog).getByRole("button", { name: "I've added it" }));
    await waitFor(() => expect(commandsSent("signing_key_listed").length).toBe(2));
    expect(dialog).toHaveTextContent("Waiting for it to appear on octocat");
    listed = true;
    await userEvent.click(within(dialog).getByRole("button", { name: "I've added it" }));
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
  });

  it("a key added by hand before, and listed on GitHub, goes straight to Check with no permission asked", async () => {
    state = structuredClone(ON);
    addAnswers = "needs_permission";
    listed = true;
    render(<SigningSetupDialog onClose={vi.fn()} />);
    const dialog = await theDialog();
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
    expect(commandsSent("forge_signing_permission_start")).toEqual([]);
    expect(commandsSent("signing_key_shown")).toEqual([]);
  });

  it("an account that may add keys already runs on from the key to Verified by itself", async () => {
    state = { ...structuredClone(ON), accounts: [{ ...ACCOUNT, may_add: true }] };
    render(<SigningSetupDialog onClose={vi.fn()} />);
    const dialog = await theDialog();
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
    expect(commandsSent("forge_signing_permission_start")).toEqual([]);
  });

  it("names why GitHub didn't verify the test commit, and checks again", async () => {
    state = { ...structuredClone(ON), accounts: [{ ...ACCOUNT, may_add: true }] };
    checked = { ...VERIFIED, verified: false, reason: "unverified_email" };
    render(<SigningSetupDialog onClose={vi.fn()} />);
    const dialog = await theDialog();
    const why = await within(dialog).findByRole("alert");
    expect(why.textContent).toContain(
      "octocat@example.com, which isn't a verified email on octocat",
    );
    expect(dialog).not.toHaveTextContent("Verified on GitHub ✓");
    checked = VERIFIED;
    await userEvent.click(within(dialog).getByRole("button", { name: "Check again" }));
    expect(await within(dialog).findByText("Verified on GitHub ✓")).toBeTruthy();
  });

  it("closed half-way, stops GitHub's permission wait and leaves the key as it was", async () => {
    state = structuredClone(ON);
    addAnswers = "needs_permission";
    const closed = vi.fn();
    render(<SigningSetupDialog onClose={closed} />);
    const dialog = await theDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Continue to GitHub" }),
    );
    await within(dialog).findByText("WDJB-MJHT");
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(closed).toHaveBeenCalled();
    cleanup();
    await waitFor(() =>
      expect(commandsSent("forge_device_sign_in_cancel")).toEqual([
        { cmd: "forge_device_sign_in_cancel", args: { flow: 9 } },
      ]),
    );
    expect(commandsSent("signing_check")).toEqual([]);
  });
});

describe("SigningSetupDialog on GitLab (#5629)", () => {
  it("a GitLab account with no key runs on from the key to Verified on GitLab, with no consent", async () => {
    state = { ...structuredClone(OFF), accounts: [GITLAB] };
    checked = { ...VERIFIED, email: "sam@example.org", repo: "platform/billing" };
    render(<SigningSetupDialog forge={ON_GITLAB} onClose={vi.fn()} />);
    const dialog = await theDialog();
    expect(dialog).toHaveTextContent("Add it to GitLab");
    await userEvent.click(await within(dialog).findByRole("button", { name: "Make my key" }));
    expect(await within(dialog).findByText("Verified on GitLab ✓")).toBeTruthy();
    expect(commandsSent("forge_signing_permission_start")).toEqual([]);
    expect(commandsSent("signing_add_key")).toEqual([
      { cmd: "signing_add_key", args: { forge: ON_GITLAB } },
    ]);
    expect(commandsSent("signing_check")).toEqual([
      { cmd: "signing_check", args: { forge: ON_GITLAB, repo: null, commit: null } },
    ]);
  });

  it("from a refusal, asks GitLab about the commit it refused", async () => {
    state = { ...structuredClone(ON), accounts: [GITLAB] };
    render(
      <SigningSetupDialog
        forge={ON_GITLAB}
        repo="platform/billing"
        commit="0123abc"
        onClose={vi.fn()}
      />,
    );
    const dialog = await theDialog();
    await within(dialog).findByText(/Verified on GitLab ✓/);
    expect(commandsSent("signing_check")[0]?.args).toEqual({
      forge: ON_GITLAB,
      repo: "platform/billing",
      commit: "0123abc",
    });
  });

  it("with no signed commit pushed to GitLab yet, shows no ✓ and says when it will", async () => {
    state = { ...structuredClone(ON), accounts: [GITLAB] };
    checked = { verified: false, reason: "nothing_pushed", email: "sam@example.org", repo: "" };
    const closed = vi.fn();
    render(<SigningSetupDialog forge={ON_GITLAB} onClose={closed} />);
    const dialog = await theDialog();
    expect(
      await within(dialog).findByText(/shows once a chat pushes its first signed commit/),
    ).toBeTruthy();
    expect(dialog).not.toHaveTextContent("Verified on GitLab ✓");
    await userEvent.click(within(dialog).getByRole("button", { name: "Done" }));
    expect(closed).toHaveBeenCalled();
  });

  it("a GitLab sign-in that may not add keys goes by hand to GitLab's page, with Usage type Signing", async () => {
    state = { ...structuredClone(ON), accounts: [{ ...GITLAB, may_add: false }] };
    render(<SigningSetupDialog forge={ON_GITLAB} onClose={vi.fn()} />);
    const dialog = await theDialog();
    await within(dialog).findByText(/Waiting for it to appear on sam/);
    expect(dialog).toHaveTextContent("Usage type Signing");
    expect(commandsSent("forge_signing_permission_start")).toEqual([]);
    await waitFor(() =>
      expect(commandsSent("plugin:opener|open_url")[0]?.args).toMatchObject({
        url: "https://gitlab.example.org/-/user_settings/ssh_keys",
      }),
    );
  });

  it("a GitLab too old to verify signed commits says so, and offers no step", async () => {
    state = { ...structuredClone(OFF), accounts: [{ ...GITLAB, verifies: false }] };
    render(<SigningSetupDialog forge={ON_GITLAB} onClose={vi.fn()} />);
    const dialog = await theDialog();
    expect((await within(dialog).findByRole("alert")).textContent).toContain(
      "This server can't verify signed commits",
    );
    expect(within(dialog).queryByRole("button", { name: "Make my key" })).toBeNull();
    expect(commandsSent("signing_make_key")).toEqual([]);
  });
});

describe("the Signed commits card", () => {
  const card = () =>
    accountsGroup().settings.find((one) => one.id === "you.accounts.signing") as LiveSetting;

  function Card() {
    const control = card().useControl();
    return <>{control.control({ id: "c", labelledBy: "l", describedBy: "d" })}</>;
  }

  it("not set up, says what signing is and offers Set up signing, with no SSH words", async () => {
    render(<Card />);
    expect(await screen.findByText("Not set up")).toBeTruthy();
    // Opening the card asks the Keychain nothing.
    expect(commandsSent("signing_key_shown")).toEqual([]);
    expect(screen.getByText(/stays in its Keychain/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Set up signing" })).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/SSH|Ed25519|gpg/);
    await userEvent.click(screen.getByRole("button", { name: "Set up signing" }));
    expect(await theDialog()).toBeTruthy();
  });

  it("with no forge account, says to sign in first and offers no setup", async () => {
    state = { ...structuredClone(OFF), accounts: [] };
    render(<Card />);
    expect(await screen.findByText(/Sign in to GitHub or GitLab above first/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Set up signing" })).toBeNull();
  });

  it("on, shows the account Verified on GitHub with the email it commits as, and SSH only in Details", async () => {
    state = {
      ...structuredClone(ON),
      accounts: [{ ...ACCOUNT, may_add: true, verified_as: "octocat@example.com" }],
    };
    render(<Card />);
    expect(await screen.findByText("On")).toBeTruthy();
    expect(screen.getByText("Verified on GitHub ✓")).toBeTruthy();
    expect(screen.getByText("Commits as octocat@example.com")).toBeTruthy();
    const details = screen.getByRole("button", { name: "Details" });
    // Until Details is opened, nothing says SSH or Ed25519.
    expect(screen.getByText(/Ed25519 SSH key/)).not.toBeVisible();
    await userEvent.click(details);
    expect(await screen.findByText(/Ed25519 SSH key, SHA256:q3VbT1sM/)).toBeVisible();
    expect(screen.getByText(/purlis signing setup --key/)).toBeTruthy();
  });

  it("an old GitLab, alone, gets the plain message and no setup", async () => {
    state = { ...structuredClone(OFF), accounts: [{ ...GITLAB, verifies: false }] };
    render(<Card />);
    expect(await screen.findByText(/This server can't verify signed commits/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Set up signing" })).toBeNull();
  });

  it("on, shows a row per account on either forge", async () => {
    state = {
      ...structuredClone(ON),
      accounts: [ACCOUNT, { ...GITLAB, verified_as: "sam@example.org" }],
    };
    render(<Card />);
    expect(await screen.findByText("Verified on GitLab ✓")).toBeTruthy();
    expect(screen.getByText("Commits as sam@example.org")).toBeTruthy();
    await userEvent.click(
      screen.getByRole("button", { name: "Check signing on octocat on GitHub" }),
    );
    expect(await theDialog()).toBeTruthy();
  });

  it("on, with no Check run in this window, offers Check rather than a ✓", async () => {
    state = structuredClone(ON);
    render(<Card />);
    expect(
      await screen.findByRole("button", { name: "Check signing on octocat on GitHub" }),
    ).toBeTruthy();
    expect(screen.queryByText("Verified on GitHub ✓")).toBeNull();
  });
});
