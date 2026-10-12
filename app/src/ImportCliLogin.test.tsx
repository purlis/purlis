// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ForgeAccountRow, ForgeCliLogin } from "./bindings";
import { forgetDeclinedImports, ImportCliLogin } from "./ImportCliLogin";
import { ForgeSignIn } from "./ForgeSignIn";

/**
 * **Use your existing gh or glab login** (#1376, ADR 0077 §5): offered, never taken. The offer
 * says what the import costs before it is pressed, nothing is imported until **Use my … login**
 * is, **Not now** imports nothing, and the window never holds the token: the import names the
 * forge and the host, and answers with the account.
 */

const GH: ForgeCliLogin = { kind: "github", host: "github.com", cli: "gh", title: "GitHub" };

let offers: ForgeCliLogin[];
let sent: { cmd: string; args: Record<string, unknown> }[];
let refuse: string | undefined;

const imported: ForgeAccountRow = {
  id: "01J9IMPORTED",
  kind: "github",
  host: "github.com",
  login: "octocat",
  method: "imported",
  signed_in: true,
  scopes: ["repo", "read:org", "gist"],
  wider: ["gist"],
};

beforeEach(() => {
  offers = [GH];
  sent = [];
  refuse = undefined;
  forgetDeclinedImports();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    sent.push({ cmd, args: given });
    switch (cmd) {
      case "forge_cli_logins":
        return structuredClone(offers);
      case "forge_import_cli_login":
        if (refuse !== undefined) throw refuse;
        return imported;
      case "first_run_found":
        return { harnesses: [], forges: [], templates: [], local_models: [] };
      default:
        return null;
    }
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const imports = () => sent.filter((one) => one.cmd === "forge_import_cli_login");

describe("the offer to use an existing gh or glab login", () => {
  it("says what the import does before anything is pressed, and imports nothing", async () => {
    render(<ImportCliLogin onImported={() => undefined} />);

    const offer = await screen.findByRole("region", { name: "Use your existing gh login" });
    expect(offer).toHaveTextContent("gh is signed in to github.com on this machine");
    expect(offer).toHaveTextContent(
      "purlis copies the token gh keeps for github.com into its own item in your system keychain",
    );
    expect(offer).toHaveTextContent("chats lose the access to github.com that gh gave them");
    // Declining comes first, and nothing has been imported.
    const buttons = within(offer).getAllByRole("button");
    expect(buttons.map((one) => one.textContent)).toEqual(["Not now", "Use my gh login"]);
    expect(imports()).toEqual([]);
  });

  it("imports nothing on Not now, and is not offered again in this run", async () => {
    const person = userEvent.setup();
    const { unmount } = render(<ImportCliLogin onImported={() => undefined} />);

    await person.click(await screen.findByRole("button", { name: "Not now" }));

    expect(screen.queryByRole("region", { name: "Use your existing gh login" })).toBeNull();
    expect(imports()).toEqual([]);
    unmount();
    render(<ImportCliLogin onImported={() => undefined} />);
    await waitFor(() =>
      expect(sent.filter((one) => one.cmd === "forge_cli_logins")).toHaveLength(2),
    );
    expect(screen.queryByRole("region", { name: "Use your existing gh login" })).toBeNull();
  });

  it("imports on the person's yes alone, naming the forge and the host and never a token", async () => {
    const person = userEvent.setup();
    const onImported = vi.fn();
    render(<ImportCliLogin onImported={onImported} />);

    await person.click(await screen.findByRole("button", { name: "Use my gh login" }));

    await waitFor(() => expect(onImported).toHaveBeenCalledWith(imported));
    expect(imports().map((one) => one.args)).toEqual([{ kind: "github", host: "github.com" }]);
    expect(screen.queryByRole("region", { name: "Use your existing gh login" })).toBeNull();
  });

  it("says the core's refusal beside the offer, which stays", async () => {
    refuse = "gh has no login on github.com for purlis to use.";
    const person = userEvent.setup();
    render(<ImportCliLogin onImported={() => undefined} />);

    await person.click(await screen.findByRole("button", { name: "Use my gh login" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(refuse);
    expect(screen.getByRole("region", { name: "Use your existing gh login" })).toBeVisible();
  });

  it("draws nothing where no forge tool is signed in", async () => {
    offers = [];
    const { container } = render(<ImportCliLogin onImported={() => undefined} />);
    await waitFor(() => expect(sent.some((one) => one.cmd === "forge_cli_logins")).toBe(true));
    expect(container).toBeEmptyDOMElement();
  });

  it("is on the welcome's sign-in page, which says whom the import signed in as", async () => {
    const person = userEvent.setup();
    render(<ForgeSignIn onBack={() => undefined} onSignInToForge={() => undefined} />);

    await person.click(await screen.findByRole("button", { name: "Use my gh login" }));

    expect(
      await screen.findByText("Signed in to github.com as octocat, with your gh login."),
    ).toBeVisible();
  });
});
