// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ForgeApplicationRow, ProjectAccountRow } from "../bindings";
import type { LiveSetting } from "./groups";
import { applications, projectAccounts } from "./oneLogin";
import { findGrid } from "../ui/table/table.testkit";

/**
 * **One login, in Settings › You › Accounts** (ADR 0128): the pinned applications, and which
 * account serves each repo. The core is faked at its command boundary.
 */

let pins: ForgeApplicationRow[];
let repos: ProjectAccountRow[];
let sent: { cmd: string; args: Record<string, unknown> }[];
let refusal: string | undefined;

const PIN: ForgeApplicationRow = {
  kind: "github",
  host: "ghe.example.com",
  client_id: "Iv1.0123456789abcdef",
  owner: "acme-platform",
  consent:
    "On the next page, check that the application is owned by acme-platform on ghe.example.com. Sign in only if it is.",
  signed_in: 0,
};

function repo(path: string, more: Partial<ProjectAccountRow> = {}): ProjectAccountRow {
  return {
    clone_key: `key-${path}`,
    repo_id: "1001",
    host: "github.com",
    path,
    serving: { id: "personal", login: "octocat" },
    chosen_is_signed_out: false,
    choices: [{ id: "personal", login: "octocat" }],
    ...more,
  };
}

beforeEach(() => {
  pins = [];
  repos = [];
  sent = [];
  refusal = undefined;
  mockIPC((cmd, args) => {
    const said = (args ?? {}) as Record<string, unknown>;
    sent.push({ cmd, args: said });
    if (cmd === "forge_applications") return structuredClone(pins);
    if (cmd === "forge_project_accounts") return structuredClone(repos);
    if (cmd === "forge_use_application") {
      if (refusal !== undefined) return Promise.reject(refusal);
      pins = [{ ...PIN, host: String(said.host), owner: String(said.owner) }];
      return { host: said.host, change: "new", replaced: null, consent: PIN.consent };
    }
    if (cmd === "forge_remove_application") {
      pins = [];
      return null;
    }
    if (cmd === "forge_application_request") return "Please register an OAuth app for purlis.";
    if (cmd === "forge_choose_project_account") return null;
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

function Row({ setting }: { setting: LiveSetting }) {
  const { control } = setting.useControl();
  return <>{control({ id: "row", labelledBy: "row-label" })}</>;
}

describe("You › Accounts › Your organization's application", () => {
  it("says none is pinned, and that github.com and gitlab.com need none", async () => {
    render(<Row setting={applications} />);
    expect(await screen.findByText(/No application is pinned/)).toBeTruthy();
    expect(screen.getByText(/github\.com and gitlab\.com need none/)).toBeTruthy();
  });

  it("lists a pin with its owner, and what to check on the forge's page before signing in", async () => {
    pins = [PIN];
    render(<Row setting={applications} />);
    const table = await findGrid(screen, "Pinned applications");
    expect(
      within(table)
        .getAllByRole("rowheader")
        .map((one) => one.textContent),
    ).toEqual(["ghe.example.com"]);
    expect(within(table).getByText(/owned by acme-platform on ghe\.example\.com/)).toBeTruthy();
    expect(within(table).getByText("Iv1.0123456789abcdef")).toBeTruthy();
    expect(within(table).getByRole("button", { name: "Sign in to ghe.example.com" })).toBeTruthy();
  });

  it("pins the application only on the press, with the owner the person typed", async () => {
    render(<Row setting={applications} />);
    await userEvent.click(await screen.findByRole("button", { name: "Use an application…" }));
    const main = screen.getByRole("button", { name: "Use this application" });
    expect(main.hasAttribute("disabled") || main.getAttribute("aria-disabled") === "true").toBe(
      true,
    );
    await userEvent.type(screen.getByRole("textbox", { name: "Host" }), "ghe.example.com");
    await userEvent.type(
      screen.getByRole("textbox", { name: "Client ID" }),
      "Iv1.0123456789abcdef",
    );
    await userEvent.type(screen.getByRole("textbox", { name: "Owner" }), "acme-platform");
    expect(sent.some((one) => one.cmd === "forge_use_application")).toBe(false);
    await userEvent.click(screen.getByRole("button", { name: "Use this application" }));
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "forge_use_application")?.args).toMatchObject({
        kind: "github",
        host: "ghe.example.com",
        clientId: "Iv1.0123456789abcdef",
        owner: "acme-platform",
      }),
    );
    expect(await screen.findByText(/will use this application/)).toBeTruthy();
    expect(await findGrid(screen, "Pinned applications")).toBeTruthy();
  });

  it("keeps the dialog open and says why when the core refuses", async () => {
    refusal = "Name the organization or group that owns the application on ghe.example.com.";
    render(<Row setting={applications} />);
    await userEvent.click(await screen.findByRole("button", { name: "Use an application…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Host" }), "ghe.example.com");
    await userEvent.type(screen.getByRole("textbox", { name: "Client ID" }), "Iv1.abc");
    await userEvent.type(screen.getByRole("textbox", { name: "Owner" }), "a b");
    await userEvent.click(screen.getByRole("button", { name: "Use this application" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Name the organization");
    expect(screen.getByRole("button", { name: "Use this application" })).toBeTruthy();
  });

  it("copies a request for an owner who has not made the application", async () => {
    const written: string[] = [];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: (text: string) => (written.push(text), Promise.resolve()) },
    });
    render(<Row setting={applications} />);
    await userEvent.click(await screen.findByRole("button", { name: "Use an application…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Host" }), "ghe.example.com");
    await userEvent.click(screen.getByRole("button", { name: "Copy a request for an owner" }));
    expect(await screen.findByText(/The request is on the clipboard/)).toBeTruthy();
    expect(written).toEqual(["Please register an OAuth app for purlis."]);
  });

  it("removes a pin from its row", async () => {
    pins = [PIN];
    render(<Row setting={applications} />);
    const table = await findGrid(screen, "Pinned applications");
    await userEvent.click(
      within(table).getByRole("button", { name: "Remove the application for ghe.example.com" }),
    );
    expect(await screen.findByText(/no longer has an application/)).toBeTruthy();
    expect(sent.some((one) => one.cmd === "forge_remove_application")).toBe(true);
  });
});

describe("You › Accounts › Account for each project", () => {
  it("says no repo is recorded yet, and how one is", async () => {
    render(<Row setting={projectAccounts} />);
    expect(await screen.findByText(/No repo is recorded yet/)).toBeTruthy();
  });

  it("asks nothing where one account is signed in on the host", async () => {
    repos = [repo("octocat/hello")];
    render(<Row setting={projectAccounts} />);
    const table = await findGrid(screen, "Account for each repo");
    expect(within(table).getByText("octocat")).toBeTruthy();
    expect(within(table).queryByRole("button", { name: /Account for/ })).toBeNull();
  });

  it("offers the choice where two accounts are signed in, and sends the one picked", async () => {
    repos = [
      repo("acme/platform", {
        repo_id: "2002",
        serving: { id: "emu", login: "octocat_acme" },
        choices: [
          { id: "personal", login: "octocat" },
          { id: "emu", login: "octocat_acme" },
        ],
      }),
    ];
    render(<Row setting={projectAccounts} />);
    const table = await findGrid(screen, "Account for each repo");
    await userEvent.click(
      within(table).getByRole("button", { name: /Account for acme\/platform on github\.com/ }),
    );
    await userEvent.click(await screen.findByRole("option", { name: "octocat" }));
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "forge_choose_project_account")?.args).toEqual({
        cloneKey: "key-acme/platform",
        repoId: "2002",
        accountId: "personal",
      }),
    );
  });

  it("says a signed-out account is not replaced by the other one", async () => {
    repos = [repo("acme/platform", { serving: null, chosen_is_signed_out: true, choices: [] })];
    render(<Row setting={projectAccounts} />);
    const table = await findGrid(screen, "Account for each repo");
    expect(within(table).getByText("signed out")).toBeTruthy();
    expect(within(table).getByText(/No other account serves this repo meanwhile/)).toBeTruthy();
  });
});
