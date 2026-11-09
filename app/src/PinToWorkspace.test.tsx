// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactPage, ArtifactPinAsk } from "./bindings";
import { PinPage, PinToWorkspace } from "./PinToWorkspace";
import { SAID } from "./menuWords";

const ASK: ArtifactPinAsk = {
  live: true,
  folder: "backlog-state",
  copied: false,
  name: "Backlog state",
  icon: "chart",
  pinned: false,
};

const PAGE: ArtifactPage = {
  workspace: "alpha",
  repo: "",
  piece: null,
  path: "artifacts/backlog-state/index.html",
};

afterEach(() => {
  cleanup();
  clearMocks();
});

function asked(ask: ArtifactPinAsk, more: Partial<Parameters<typeof PinToWorkspace>[0]> = {}) {
  const onPin = vi.fn();
  render(
    <PinToWorkspace
      project="acme"
      workspace="alpha"
      ask={ask}
      pinning={false}
      onPin={onPin}
      onCancel={() => {}}
      {...more}
    />,
  );
  return { dialog: screen.getByRole("dialog"), onPin };
}

describe("Pin to workspace (#5953)", () => {
  it("says the pin is visible to everyone in the project, in a LIVE workspace", () => {
    const { dialog } = asked(ASK);
    expect(dialog).toHaveTextContent("Visible to everyone in acme once the project is saved.");
    expect(dialog).not.toHaveTextContent("this machine");
  });

  it("says the artifact stays on this machine, in a LOCAL workspace", () => {
    const { dialog } = asked({ ...ASK, live: false });
    expect(dialog).toHaveTextContent(
      "alpha is a LOCAL workspace, so this artifact stays on this machine.",
    );
    expect(dialog).not.toHaveTextContent("everyone");
  });

  it("offers the title's name and the icon set, and pins with what the person chose", async () => {
    const { dialog, onPin } = asked(ASK);
    const name = within(dialog).getByLabelText("Name");
    expect(name).toHaveValue("Backlog state");
    await userEvent.clear(name);
    await userEvent.type(name, "Backlog");
    await userEvent.click(within(dialog).getByRole("radio", { name: "rocket" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Pin to workspace" }));
    expect(onPin).toHaveBeenCalledWith("Backlog", "rocket");
  });

  it("names the folder a page from outside artifacts/ is copied into", () => {
    const { dialog } = asked({ ...ASK, copied: true, folder: "report" });
    expect(dialog).toHaveTextContent("The page is copied to artifacts/report/ in alpha.");
  });

  it("reads and looks unlike a tab's private Pin", () => {
    const { dialog } = asked(ASK);
    // Its words name the workspace, and its mark is people, never the pin a tab's Pin draws.
    expect(SAID.pinToWorkspace).toBe("Pin to workspace…");
    expect(within(dialog).getByRole("heading", { name: "Pin to workspace" })).toBeVisible();
    expect(dialog.querySelector(".lucide-users")).not.toBeNull();
    expect(dialog.querySelector(".lucide-pin")).toBeNull();
  });

  it("refuses a blank name", async () => {
    const { dialog, onPin } = asked(ASK);
    await userEvent.clear(within(dialog).getByLabelText("Name"));
    expect(within(dialog).getByRole("button", { name: "Pin to workspace" })).toBeDisabled();
    expect(onPin).not.toHaveBeenCalled();
  });
});

describe("Pin to workspace, for a page the window names (#5953)", () => {
  it("asks the core what to offer, pins on the answer, and closes", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      if (cmd === "artifact_pin_ask") return ASK;
      if (cmd === "artifact_pin")
        return {
          id: "x",
          folder: "backlog-state",
          name: "Backlog state",
          icon: "chart",
          missing: null,
          live: true,
          too_large: null,
          fresh: null,
        };
      return null;
    });
    const onClose = vi.fn();
    render(<PinPage plane="/x/acme" workspace="alpha" page={PAGE} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Visible to everyone in acme");
    await userEvent.click(within(dialog).getByRole("button", { name: "Pin to workspace" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(calls).toContainEqual([
      "artifact_pin",
      {
        plane: "/x/acme",
        workspace: "alpha",
        page: PAGE,
        answer: { folder: "backlog-state", name: "Backlog state", icon: "chart" },
      },
    ]);
  });

  it("keeps the dialog up with the core's sentence when the pin is refused", async () => {
    mockIPC((cmd) => {
      if (cmd === "artifact_pin_ask") return ASK;
      if (cmd === "artifact_pin")
        throw "there is already a folder 'report' in artifacts/; choose another name";
      return null;
    });
    const onClose = vi.fn();
    render(<PinPage plane="/x/acme" workspace="alpha" page={PAGE} onClose={onClose} />);
    const dialog = await screen.findByRole("dialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Pin to workspace" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "there is already a folder 'report' in artifacts/",
    );
    expect(onClose).not.toHaveBeenCalled();
  });

  it("says why a page cannot be pinned, with a way out", async () => {
    mockIPC((cmd) => {
      if (cmd === "artifact_pin_ask") throw "notes.html is not a text file, so it is not a page";
      return null;
    });
    const onClose = vi.fn();
    render(<PinPage plane="/x/acme" workspace="alpha" page={PAGE} onClose={onClose} />);
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent("not a text file");
    await userEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });
});
