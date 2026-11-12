// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ForgeAccountRow } from "../bindings";
import { accountsGroup } from "./accounts";
import type { LiveSetting } from "./groups";

/**
 * **You › Accounts › Forge accounts, as a table** (#6301, after Your hosts in #2919): a row an
 * account, its login heading the row, its host, how it signed in, and Sign out at its end.
 */

let accounts: ForgeAccountRow[];
let sent: { cmd: string; args: Record<string, unknown> }[];

function account(login: string, host: string, more: Partial<ForgeAccountRow> = {}) {
  return {
    id: `github:${host}:${login}`,
    kind: "github",
    host,
    login,
    method: "browser",
    signed_in: true,
    scopes: [],
    wider: [],
    ...more,
  } satisfies ForgeAccountRow;
}

beforeEach(() => {
  accounts = [
    account("octocat", "github.com"),
    account("sam", "gitlab.example.org", { kind: "gitlab", method: "token", wider: ["api"] }),
  ];
  sent = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "forge_accounts") return structuredClone(accounts);
    if (cmd === "forge_sign_out") {
      accounts = accounts.filter((one) => one.id !== (args as { id: string }).id);
      return null;
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The Forge accounts row's control, as Settings draws it. */
function Accounts() {
  const setting = accountsGroup().settings[0] as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id: "accounts", labelledBy: "accounts-label" })}</>;
}

describe("You › Accounts › Forge accounts", () => {
  it("draws the accounts as a table: login, host, how it signed in, then Sign out", async () => {
    render(<Accounts />);
    const table = await screen.findByRole("grid", { name: "Forge accounts" });
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((one) => one.textContent),
    ).toEqual(["Account", "Host", "Signed in with", "Sign out"]);
    expect(
      within(table)
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["octocat", "sam"]);
    const sam = within(table).getAllByRole("row")[2] as HTMLElement;
    expect(within(sam).getByText("gitlab.example.org")).toBeTruthy();
    expect(within(sam).getByText("token; wider than needed: api")).toBeTruthy();
    expect(
      within(sam).getByRole("button", { name: "Sign out of sam on gitlab.example.org" }),
    ).toBeTruthy();
  });

  it("signs an account out from its row, and lists what is left", async () => {
    render(<Accounts />);
    const table = await screen.findByRole("grid", { name: "Forge accounts" });
    await userEvent.click(
      within(table).getByRole("button", { name: "Sign out of octocat on github.com" }),
    );
    await waitFor(() => expect(sent.some((one) => one.cmd === "forge_sign_out")).toBe(true));
    await waitFor(() =>
      expect(
        within(table)
          .getAllByRole("rowheader")
          .map((one) => one.textContent),
      ).toEqual(["sam"]),
    );
  });
});
