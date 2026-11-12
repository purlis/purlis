// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SigningAccountKeys, SigningState } from "../bindings";
import type { LiveSetting } from "./groups";
import { signedCommits } from "./signing";
import { findGrid } from "../ui/table/table.testkit";

/**
 * **You › Accounts › Signed commits, its lists as tables** (#6301, after Your hosts in #2919):
 * each account's signing, a row an account; and the other devices' keys, a row a key, with its
 * Remove… at the row's end, which asks first.
 */

const SIGNING: SigningState = {
  on: true,
  title: "purlis · studio",
  accounts: [
    {
      kind: "github",
      login: "octocat",
      host: "github.com",
      forge: "GitHub",
      may_add: true,
      verified_as: "octocat@users.noreply.github.com",
      verifies: true,
    },
    {
      kind: "gitlab",
      login: "sam",
      host: "gitlab.example.org",
      forge: "GitLab",
      may_add: true,
      verified_as: null,
      verifies: true,
    },
  ],
};

const KEYS: SigningAccountKeys[] = [
  {
    kind: "github",
    host: "github.com",
    has_key: true,
    others: [
      { id: "17", title: "t", device: "Studio Mac mini", added: "2026-10-01", older_setup: false },
      { id: "21", title: "t", device: "MacBook Air", added: null, older_setup: true },
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

let sent: string[];

beforeEach(() => {
  sent = [];
  mockIPC((cmd) => {
    sent.push(cmd);
    if (cmd === "signing_state") return SIGNING;
    if (cmd === "signing_accounts_keys") return KEYS;
    if (cmd === "signing_remove_other") throw "GitHub refused: the key is not yours.";
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

function Card() {
  const { control } = (signedCommits as LiveSetting).useControl();
  return <>{control({ id: "signing", labelledBy: "signing-label" })}</>;
}

const names = (table: HTMLElement, role: "columnheader" | "rowheader") =>
  within(table)
    .getAllByRole(role)
    .map((one) => one.textContent);

describe("You › Accounts › Signed commits", () => {
  it("draws each account's signing as a table, a row an account", async () => {
    render(<Card />);
    const table = await findGrid(screen, "Signing on your accounts");
    expect(names(table, "columnheader")).toEqual(["Account", "Host", "Signing"]);
    expect(names(table, "rowheader")).toEqual(["octocat", "sam"]);
    await waitFor(() =>
      expect(
        within(table).getByRole("button", { name: "Add to this account: sam on GitLab" }),
      ).toBeTruthy(),
    );
    expect(within(table).getByText("Verified on GitHub ✓")).toBeTruthy();
  });

  it("draws the other devices' keys as a table, each Remove… at its row's end", async () => {
    render(<Card />);
    const table = await findGrid(screen, "Signing keys of your other devices");
    expect(names(table, "columnheader")).toEqual(["Device", "Account", "Added", "Remove"]);
    expect(names(table, "rowheader")).toEqual(["Studio Mac mini", "MacBook AirOlder setup"]);
    const remove = within(table).getByRole("button", {
      name: "Remove Studio Mac mini's signing key from octocat on GitHub…",
    });
    await userEvent.click(remove);
    const asked = await screen.findByRole("dialog", {
      name: "Remove Studio Mac mini's signing key from octocat on GitHub?",
    });
    await userEvent.click(within(asked).getByRole("button", { name: "Remove key" }));
    // The forge's refusal is said in the key's own row.
    const row = within(table).getAllByRole("row")[1] as HTMLElement;
    expect(await within(row).findByRole("alert")).toHaveTextContent(
      "GitHub refused: the key is not yours.",
    );
  });
});
