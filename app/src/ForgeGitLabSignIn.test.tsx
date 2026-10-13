// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ForgeAccountRow } from "./bindings";
import { ForgeBrowserSignIn } from "./ForgeBrowserSignIn";
import { ForgeGitLabSignIn } from "./ForgeGitLabSignIn";
import { ForgeSignInEnded } from "./ForgeSignInEnded";
import { NoticeList } from "./Notice";

/**
 * **Sign in with GitLab** (#730): the dialog starts the browser sign-in as it opens, waits,
 * hands back the account, says the core's refusal with Try again, and stops the wait on Cancel.
 * It sends the host process a forge, a host and a name for the try, and nothing else.
 */

const ACCOUNT: ForgeAccountRow = {
  id: "01J9GITLAB",
  kind: "gitlab",
  host: "gitlab.com",
  login: "jdoe",
  method: "browser",
  signed_in: true,
  scopes: ["api"],
  wider: [],
};

/** Every command the window sent, with its arguments. */
let sent: { cmd: string; args: Record<string, unknown> }[];
/** The sign-ins the host process is still waiting on, by the try's name. */
/** The accounts the host process lists. */
let accounts: ForgeAccountRow[];
let waiting: Map<string, { ok: (row: ForgeAccountRow) => void; no: (why: string) => void }>;

const starts = () => sent.filter((one) => one.cmd === "forge_pkce_sign_in");
const cancels = () => sent.filter((one) => one.cmd === "forge_pkce_sign_in_cancel");
const tryOf = (call: { args: Record<string, unknown> }) => String(call.args.attempt);

beforeEach(() => {
  sent = [];
  waiting = new Map();
  accounts = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    sent.push({ cmd, args: given });
    if (cmd === "forge_pkce_sign_in") {
      return new Promise<ForgeAccountRow>((ok, no) => {
        waiting.set(String(given.attempt), { ok, no });
      });
    }
    if (cmd === "forge_accounts") return structuredClone(accounts);
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

function opened() {
  const onSignedIn = vi.fn();
  const onCancel = vi.fn();
  const view = render(<ForgeGitLabSignIn onSignedIn={onSignedIn} onCancel={onCancel} />);
  return { onSignedIn, onCancel, view };
}

describe("Sign in with GitLab", () => {
  it("starts the sign-in on gitlab.com as it opens, and says it is waiting for the browser", async () => {
    opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    expect(starts()[0].args).toEqual({
      kind: "gitlab",
      host: "gitlab.com",
      attempt: expect.stringMatching(/^[0-9a-zA-Z-]{1,64}$/) as unknown,
    });
    expect(screen.getByRole("dialog", { name: "Sign in with GitLab" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Waiting for your browser");
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("hands back the account the sign-in made", async () => {
    const { onSignedIn, onCancel } = opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    waiting.get(tryOf(starts()[0]))?.ok(ACCOUNT);
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledWith(ACCOUNT));
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("says the core's refusal in full, and Try again starts a new try under a new name", async () => {
    const { onSignedIn } = opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    const why =
      "purlis could not take the browser's answer: ports 47823, 47824 and 47825 on this machine are all in use by other software.";
    waiting.get(tryOf(starts()[0]))?.no(why);
    expect((await screen.findByRole("alert")).textContent).toBe(why);
    expect(screen.queryByRole("status")).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(starts()).toHaveLength(2));
    expect(tryOf(starts()[1])).not.toBe(tryOf(starts()[0]));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("status")).toBeTruthy();

    waiting.get(tryOf(starts()[1]))?.ok(ACCOUNT);
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledTimes(1));
  });

  it("stops the try it started on Cancel, and drops that try's late answer", async () => {
    const { onSignedIn, onCancel } = opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    const mine = tryOf(starts()[0]);
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(cancels().map(tryOf)).toEqual([mine]);

    waiting.get(mine)?.ok(ACCOUNT);
    await Promise.resolve();
    expect(onSignedIn).not.toHaveBeenCalled();
  });

  it("stops the try on Escape, and when it is closed from outside", async () => {
    const first = opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    await userEvent.keyboard("{Escape}");
    expect(first.onCancel).toHaveBeenCalledTimes(1);
    expect(cancels().map(tryOf)).toEqual([tryOf(starts()[0])]);
    first.view.unmount();

    opened().view.unmount();
    await waitFor(() => expect(starts()).toHaveLength(2));
    expect(cancels().map(tryOf)).toEqual([tryOf(starts()[0]), tryOf(starts()[1])]);
  });

  it("puts nothing of the sign-in on the page or in a call: no address, code or token", async () => {
    opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    waiting.get(tryOf(starts()[0]))?.ok(ACCOUNT);
    await Promise.resolve();
    for (const call of sent) {
      expect(Object.keys(call.args).sort()).toEqual(
        call.cmd === "forge_pkce_sign_in" ? ["attempt", "host", "kind"] : ["attempt"],
      );
    }
    expect(sent.some((one) => one.cmd === "plugin:opener|open_url")).toBe(false);
    expect(document.body.innerHTML).not.toMatch(/oauth\/authorize|code_challenge|[?&]state=/);
  });

  it("can be worked from the keyboard: Cancel is the first stop while it waits", async () => {
    opened();
    await waitFor(() => expect(starts()).toHaveLength(1));
    await userEvent.tab();
    expect(document.activeElement?.textContent).toBe("Cancel");
  });
});

describe("the browser sign-ins, side by side", () => {
  it("offers Sign in with GitLab beside GitHub's, which opens the sign-in and hands its account up", async () => {
    const onSignedIn = vi.fn();
    render(<ForgeBrowserSignIn onSignedIn={onSignedIn} />);
    expect(
      screen
        .getAllByRole("button")
        .map((button) => button.textContent)
        .filter((name) => name?.startsWith("Sign in with")),
    ).toEqual(["Sign in with GitHub", "Sign in with GitLab"]);
    expect(starts()).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Sign in with GitLab" }));
    await screen.findByRole("dialog", { name: "Sign in with GitLab" });
    await waitFor(() => expect(starts()).toHaveLength(1));
    waiting.get(tryOf(starts()[0]))?.ok(ACCOUNT);
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledWith(ACCOUNT));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("goes back to the two buttons on Cancel, with nothing signed in", async () => {
    const onSignedIn = vi.fn();
    render(<ForgeBrowserSignIn onSignedIn={onSignedIn} />);
    await userEvent.click(screen.getByRole("button", { name: "Sign in with GitLab" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(cancels()).toHaveLength(1);
    expect(onSignedIn).not.toHaveBeenCalled();
  });
});

describe("a gitlab.com sign-in GitLab would not renew", () => {
  const ENDED: ForgeAccountRow = { ...ACCOUNT, signed_in: false };

  function Lines() {
    return (
      <NoticeList>
        <ForgeSignInEnded />
      </NoticeList>
    );
  }

  it("is a Notice naming the account, whose Sign in again opens the GitLab sign-in", async () => {
    accounts = [ENDED];
    render(<Lines />);
    const notice = await screen.findByText(/sign-in to gitlab.com as jdoe has ended/);
    expect(notice.closest("[data-cause]")).toHaveAttribute(
      "data-cause",
      "forge-sign-in-ended:01J9GITLAB",
    );

    await userEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    await screen.findByRole("dialog", { name: "Sign in with GitLab" });
    await waitFor(() => expect(starts()).toHaveLength(1));
    expect(starts()[0].args).toMatchObject({ kind: "gitlab", host: "gitlab.com" });

    // Signed in again: the dialog and the Notice both go.
    accounts = [ACCOUNT];
    waiting.get(tryOf(starts()[0]))?.ok(ACCOUNT);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(screen.queryByText(/has ended/)).toBeNull());
  });

  it("is not drawn for a self-managed GitLab, which has no browser sign-in to open", async () => {
    accounts = [{ ...ENDED, host: "git.example.com" }];
    render(<Lines />);
    await waitFor(() => expect(sent.some((one) => one.cmd === "forge_accounts")).toBe(true));
    await Promise.resolve();
    expect(screen.queryByText(/has ended/)).toBeNull();
  });
});
