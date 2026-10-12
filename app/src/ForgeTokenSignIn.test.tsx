// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ForgeAccountRow } from "./bindings";
import { ForgeTokenSignIn } from "./ForgeTokenSignIn";
import { accountsGroup } from "./settings/accounts";
import type { LiveSetting } from "./settings/groups";

/**
 * **Sign in with a token** (#1814) and **Settings › You › Accounts**: the dialog lists the exact
 * scopes for the host, hands the pasted token to the core once and keeps it off the page, says
 * the core's refusal, and the Accounts list signs in through it and signs out.
 */

const TOKEN = "pasted-in-the-window";

let accounts: ForgeAccountRow[];
/** Every command the window sent, with its arguments. */
let sent: { cmd: string; args: Record<string, unknown> }[];
/** The refusal the next sign-in is answered with, if any. */
let refuse: string | undefined;

function row(login: string, host: string, more: Partial<ForgeAccountRow> = {}): ForgeAccountRow {
  return {
    id: `01J9${login.toUpperCase()}`,
    kind: host.includes("lab") ? "gitlab" : "github",
    host,
    login,
    method: "token",
    signed_in: true,
    scopes: ["repo", "read:org"],
    wider: [],
    ...more,
  };
}

beforeEach(() => {
  accounts = [];
  sent = [];
  refuse = undefined;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    sent.push({ cmd, args: given });
    switch (cmd) {
      case "forge_token_help": {
        const host = String(given.host);
        if (!host.includes(".")) throw "That is not a host name.";
        return given.kind === "gitlab"
          ? {
              host,
              scopes: ["api"],
              create_at: `https://${host}/-/user_settings/personal_access_tokens`,
            }
          : {
              host,
              scopes: ["repo", "read:org"],
              create_at: `https://${host}/settings/tokens/new`,
            };
      }
      case "forge_sign_in_with_token": {
        if (refuse !== undefined) throw refuse;
        const signed = row("jdoe", String(given.host), { kind: String(given.kind) });
        accounts.push(signed);
        return signed;
      }
      case "forge_accounts":
        return structuredClone(accounts);
      case "forge_sign_out":
        accounts = accounts.filter((one) => one.id !== given.id);
        return null;
      case "plugin:opener|open_url":
        return null;
      default:
        return null;
    }
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("ForgeTokenSignIn", () => {
  it("lists the exact scopes for the host, and moves the host with the forge until one is typed", async () => {
    render(<ForgeTokenSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    expect(await screen.findByText("read:org")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Host" })).toHaveProperty("value", "github.com");

    await userEvent.click(screen.getByRole("radio", { name: /GitLab/ }));
    expect(screen.getByRole("textbox", { name: "Host" })).toHaveProperty("value", "gitlab.com");
    await waitFor(() => expect(screen.queryByText("read:org")).toBeNull());
    expect(screen.getByText("api")).toBeTruthy();

    const host = screen.getByRole("textbox", { name: "Host" });
    await userEvent.clear(host);
    await userEvent.type(host, "git.example.com");
    await userEvent.click(screen.getByRole("radio", { name: /GitHub/ }));
    expect(host).toHaveProperty("value", "git.example.com");
  });

  it("says why a host is not one, and offers no Sign in until it is", async () => {
    render(<ForgeTokenSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    const host = screen.getByRole("textbox", { name: "Host" });
    await userEvent.clear(host);
    await userEvent.type(host, "oneword");
    expect(await screen.findByText("That is not a host name.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign in" })).toHaveProperty("disabled", true);
  });

  it("hands the token to the core once, empties the box, and answers with the account", async () => {
    const signedIn = vi.fn();
    render(<ForgeTokenSignIn forge="gitlab" onSignedIn={signedIn} onCancel={vi.fn()} />);
    await screen.findByText("api");
    const box = screen.getByLabelText("Token");
    await userEvent.type(box, TOKEN);
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    await waitFor(() => expect(signedIn).toHaveBeenCalledOnce());
    expect(signedIn.mock.calls[0][0]).toMatchObject({ login: "jdoe", host: "gitlab.com" });
    const given = sent.filter((one) => one.cmd === "forge_sign_in_with_token");
    expect(given).toEqual([
      {
        cmd: "forge_sign_in_with_token",
        args: { kind: "gitlab", host: "gitlab.com", token: TOKEN },
      },
    ]);
    expect(box).toHaveProperty("value", "");
    expect(document.body.innerHTML).not.toContain(TOKEN);
  });

  it("says the core's refusal, never with the token, and lets the person try again", async () => {
    refuse =
      "This token is missing the read:org scope. purlis needs repo and read:org on github.com.";
    const signedIn = vi.fn();
    render(<ForgeTokenSignIn onSignedIn={signedIn} onCancel={vi.fn()} />);
    await screen.findByText("read:org");
    await userEvent.type(screen.getByLabelText("Token"), TOKEN);
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect((await screen.findByRole("alert")).textContent).toContain("missing the read:org scope");
    expect(signedIn).not.toHaveBeenCalled();
    expect(document.body.innerHTML).not.toContain(TOKEN);
    expect(screen.getByRole("button", { name: "Sign in" })).toHaveProperty("disabled", false);
  });

  it("opens the forge's own token page", async () => {
    render(<ForgeTokenSignIn onSignedIn={vi.fn()} onCancel={vi.fn()} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Make a token on github.com" }),
    );
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "plugin:opener|open_url")?.args).toMatchObject({
        url: "https://github.com/settings/tokens/new",
      }),
    );
  });
});

/** The Accounts group's one row, drawn as its control. */
function AccountsRow() {
  const setting = accountsGroup().settings[0] as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: "accounts", labelledBy: "accounts-label" })}</>;
}

describe("Settings › You › Accounts", () => {
  it("lists each account and signs out of one, saying the token still works at the forge", async () => {
    accounts = [row("octocat", "github.com"), row("jdoe", "git.example.com", { wider: ["sudo"] })];
    render(<AccountsRow />);
    expect(await screen.findByText("octocat")).toBeTruthy();
    expect(screen.getByText("token; wider than needed: sudo")).toBeTruthy();
    await userEvent.click(
      screen.getByRole("button", { name: "Sign out of octocat on github.com" }),
    );
    expect(sent.find((one) => one.cmd === "forge_sign_out")?.args).toEqual({ id: "01J9OCTOCAT" });
    await waitFor(() => expect(screen.queryByText("octocat")).toBeNull());
    expect(screen.getByRole("status").textContent).toContain("still works on github.com");
  });

  it("signs in through the token dialog and lists the new account", async () => {
    render(<AccountsRow />);
    expect(await screen.findByText("No forge account is signed in on this machine.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Sign in with a token…" }));
    await screen.findByText("read:org");
    await userEvent.type(screen.getByLabelText("Token"), TOKEN);
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByText("jdoe")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
