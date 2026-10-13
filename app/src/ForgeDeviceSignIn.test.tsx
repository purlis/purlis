// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ForgeAccountRow, ForgeDeviceCode } from "./bindings";
import { ForgeDeviceSignIn } from "./ForgeDeviceSignIn";
import { ForgeSignInEnded, SIGN_IN_ENDED } from "./ForgeSignInEnded";
import { NoticeList } from "./Notice";
import { accountsGroup } from "./settings/accounts";
import type { LiveSetting } from "./settings/groups";

/**
 * **Sign in with GitHub** (#729): the device flow in the window. The dialog shows the code and
 * GitHub's own page, offers to open the browser, waits, and answers the account; every way the
 * flow ends says what to do next; Cancel stops the waiting. Settings › You › Accounts offers it
 * beside the token, and a sign-in the forge ended is a Notice with Sign in again.
 */

const CODE: ForgeDeviceCode = {
  flow: 7,
  host: "github.com",
  title: "GitHub",
  user_code: "WDJB-MJHT",
  verification_uri: "https://github.com/login/device",
  expires_in: 900,
  scopes: ["repo", "read:org"],
};

const OCTOCAT: ForgeAccountRow = {
  id: "01J9OCTOCAT",
  kind: "github",
  host: "github.com",
  login: "octocat",
  method: "browser",
  signed_in: true,
  scopes: ["repo", "read:org"],
  wider: [],
};

let accounts: ForgeAccountRow[];
/** Every command the window sent, with its arguments. */
let sent: { cmd: string; args: Record<string, unknown> }[];
/** Why the next start is refused, if it is. */
let refuseStart: string | undefined;
/** The flow number the next start answers with. */
let nextFlow: number;
/** Ends the wait in progress: with the account, or with the core's sentence. */
let confirm: () => void;
let end: (why: string) => void;

beforeEach(() => {
  accounts = [];
  sent = [];
  refuseStart = undefined;
  nextFlow = CODE.flow;
  confirm = () => undefined;
  end = () => undefined;
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      sent.push({ cmd, args: given });
      switch (cmd) {
        case "forge_device_sign_in_start":
          if (refuseStart !== undefined) throw refuseStart;
          return { ...CODE, flow: nextFlow++ };
        case "forge_device_sign_in_wait":
          return new Promise<ForgeAccountRow>((resolve, reject) => {
            confirm = () => {
              accounts = [...accounts.filter((one) => one.id !== OCTOCAT.id), OCTOCAT];
              resolve(OCTOCAT);
            };
            end = (why) => reject(why);
          });
        case "forge_device_sign_in_cancel":
          return null;
        case "forge_accounts":
          return structuredClone(accounts);
        case "forge_sign_out":
          accounts = accounts.filter((one) => one.id !== given.id);
          return null;
        case "forge_cli_logins":
          return [];
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const commandsSent = (cmd: string) => sent.filter((one) => one.cmd === cmd);

describe("ForgeDeviceSignIn", () => {
  it("shows the code and GitHub's page, explains each scope, and waits", async () => {
    render(<ForgeDeviceSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    const dialog = await screen.findByRole("dialog", { name: "Sign in with GitHub" });
    expect(await within(dialog).findByText("WDJB-MJHT")).toBeTruthy();
    expect(dialog).toHaveTextContent("https://github.com/login/device");
    expect(dialog).toHaveTextContent("repo: your repositories, private ones too");
    expect(dialog).toHaveTextContent("read:org: the names of your organisations");
    expect(dialog).toHaveTextContent("purlis never hands it to a chat");
    expect(dialog).toHaveTextContent("Waiting for you to approve it on GitHub…");
    // Asked of github.com, and of nothing else; the browser is offered, never opened unasked.
    expect(commandsSent("forge_device_sign_in_start")).toEqual([
      { cmd: "forge_device_sign_in_start", args: { kind: "github", host: "github.com" } },
    ]);
    await waitFor(() =>
      expect(commandsSent("forge_device_sign_in_wait")).toEqual([
        { cmd: "forge_device_sign_in_wait", args: { flow: 7 } },
      ]),
    );
    expect(commandsSent("plugin:opener|open_url")).toEqual([]);
  });

  it("opens GitHub's own page in the browser when asked", async () => {
    render(<ForgeDeviceSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Open GitHub in your browser" }),
    );
    await waitFor(() =>
      expect(commandsSent("plugin:opener|open_url")[0]?.args).toMatchObject({
        url: "https://github.com/login/device",
      }),
    );
  });

  it("copies the code, and says so", async () => {
    const written = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: written },
      configurable: true,
    });
    render(<ForgeDeviceSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Copy code" }));
    expect(written).toHaveBeenCalledWith("WDJB-MJHT");
    expect(await screen.findByText("The code is on the clipboard.")).toBeTruthy();
  });

  it("answers with the account once the code is confirmed", async () => {
    const signedIn = vi.fn();
    render(<ForgeDeviceSignIn onSignedIn={signedIn} onCancel={vi.fn()} />);
    await screen.findByText("WDJB-MJHT");
    await waitFor(() => expect(commandsSent("forge_device_sign_in_wait")).toHaveLength(1));
    act(() => confirm());
    await waitFor(() => expect(signedIn).toHaveBeenCalledOnce());
    expect(signedIn.mock.calls[0][0]).toMatchObject({ login: "octocat", method: "browser" });
  });

  it("says how the flow ended in the core's sentence, and tries again with a new code", async () => {
    const signedIn = vi.fn();
    render(<ForgeDeviceSignIn onSignedIn={signedIn} onCancel={vi.fn()} />);
    await screen.findByText("WDJB-MJHT");
    await waitFor(() => expect(commandsSent("forge_device_sign_in_wait")).toHaveLength(1));
    act(() =>
      end("The code expired before it was confirmed on GitHub. Sign in again to get a new one."),
    );
    expect((await screen.findByRole("alert")).textContent).toContain("The code expired");
    expect(screen.queryByText("WDJB-MJHT")).toBeNull();
    expect(signedIn).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("WDJB-MJHT")).toBeTruthy();
    await waitFor(() =>
      expect(commandsSent("forge_device_sign_in_wait").map((one) => one.args)).toEqual([
        { flow: 7 },
        { flow: 8 },
      ]),
    );
  });

  it("says why a sign-in could not start, with Try again and Cancel", async () => {
    refuseStart = "purlis could not reach github.com: timed out. Check your connection.";
    render(<ForgeDeviceSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    expect((await screen.findByRole("alert")).textContent).toContain("could not reach github.com");
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    expect(commandsSent("forge_device_sign_in_wait")).toEqual([]);
  });

  it("stops the waiting on Cancel, and an answer that comes later signs nobody in", async () => {
    const signedIn = vi.fn();
    const cancelled = vi.fn();
    const drawn = render(<ForgeDeviceSignIn onSignedIn={signedIn} onCancel={cancelled} />);
    await screen.findByText("WDJB-MJHT");
    await waitFor(() => expect(commandsSent("forge_device_sign_in_wait")).toHaveLength(1));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancelled).toHaveBeenCalledOnce();
    // The surface that opened it takes it down, and the polling is told to stop.
    drawn.unmount();
    await waitFor(() =>
      expect(commandsSent("forge_device_sign_in_cancel")).toEqual([
        { cmd: "forge_device_sign_in_cancel", args: { flow: 7 } },
      ]),
    );
    act(() => confirm());
    expect(signedIn).not.toHaveBeenCalled();
  });

  it("is reached from the keyboard, and Escape is Cancel", async () => {
    const cancelled = vi.fn();
    render(<ForgeDeviceSignIn onSignedIn={vi.fn()} onCancel={cancelled} />);
    const open = await screen.findByRole("button", { name: "Open GitHub in your browser" });
    for (const button of screen.getAllByRole("button")) {
      expect(button).toHaveAttribute("tabindex", "0");
      expect(button.className).toContain("ui-button");
    }
    open.focus();
    await userEvent.keyboard("{Escape}");
    expect(cancelled).toHaveBeenCalledOnce();
  });
});

/** The Accounts group's one row, drawn as its control. */
function AccountsRow() {
  const setting = accountsGroup().settings[0] as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: "accounts", labelledBy: "accounts-label" })}</>;
}

describe("Settings › You › Accounts, signing in with GitHub", () => {
  it("offers Sign in with GitHub beside the token, signs in and lists the account", async () => {
    render(<AccountsRow />);
    expect(await screen.findByText("No forge account is signed in on this machine.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign in with a token…" })).toBeTruthy();
    const browser = screen.getByRole("button", { name: "Sign in with GitHub" });
    expect(browser.className).toContain("ui-button");

    await userEvent.click(browser);
    await screen.findByText("WDJB-MJHT");
    await waitFor(() => expect(commandsSent("forge_device_sign_in_wait")).toHaveLength(1));
    act(() => confirm());

    expect(await screen.findByText("octocat")).toBeTruthy();
    expect(screen.getByText("browser")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("says a browser sign-in lapses at GitHub, and where to revoke it, on Sign out", async () => {
    accounts = [OCTOCAT];
    render(<AccountsRow />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign out of octocat on github.com" }),
    );
    const said = await screen.findByRole("status");
    expect(said).toHaveTextContent("Signed out of octocat on github.com.");
    expect(said).toHaveTextContent("lapse within eight hours");
    expect(said).toHaveTextContent("https://github.com/settings/applications");
    expect(said).not.toHaveTextContent("The token still works");
  });
});

describe("a browser sign-in the forge ended", () => {
  const ENDED: ForgeAccountRow = { ...OCTOCAT, signed_in: false };

  function Lines() {
    return (
      <NoticeList>
        <ForgeSignInEnded />
      </NoticeList>
    );
  }

  it("is a Notice naming the account, with Sign in again", async () => {
    accounts = [ENDED];
    render(<Lines />);
    const notice = await screen.findByText(/sign-in to github.com as octocat has ended/);
    expect(notice.closest("[data-cause]")).toHaveAttribute(
      "data-cause",
      "forge-sign-in-ended:01J9OCTOCAT",
    );

    await userEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    await screen.findByRole("dialog", { name: "Sign in with GitHub" });
    await screen.findByText("WDJB-MJHT");
    await waitFor(() => expect(commandsSent("forge_device_sign_in_wait")).toHaveLength(1));
    act(() => confirm());

    // Signed in again: the dialog and the Notice both go.
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() =>
      expect(screen.queryByText(/sign-in to github.com as octocat has ended/)).toBeNull(),
    );
  });

  it("is drawn the moment the core says a sign-in ended", async () => {
    accounts = [OCTOCAT];
    render(<Lines />);
    await waitFor(() => expect(commandsSent("forge_accounts")).toHaveLength(1));
    expect(screen.queryByText(/has ended/)).toBeNull();

    accounts = [ENDED];
    await act(() => emit(SIGN_IN_ENDED));
    expect(await screen.findByText(/sign-in to github.com as octocat has ended/)).toBeTruthy();
  });

  it("says nothing of an account that is signed in, signed out of a token, or dismissed", async () => {
    accounts = [OCTOCAT, { ...ENDED, id: "01J9TOKEN", login: "jdoe", method: "token" }];
    const drawn = render(<Lines />);
    await waitFor(() => expect(commandsSent("forge_accounts")).toHaveLength(1));
    expect(screen.queryByText(/has ended/)).toBeNull();
    drawn.unmount();

    accounts = [ENDED];
    render(<Lines />);
    await screen.findByText(/sign-in to github.com as octocat has ended/);
    await userEvent.click(screen.getByRole("button", { name: /Dismiss/ }));
    await waitFor(() => expect(screen.queryByText(/has ended/)).toBeNull());
  });
});
