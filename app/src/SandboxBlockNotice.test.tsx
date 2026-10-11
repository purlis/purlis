// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SandboxBlockNotice } from "./SandboxBlockNotice";
import type { ChatBlocked, PlaneId } from "./bindings";
import { askOfBlock } from "./test-asks";

/**
 * **The host to allow, for a block that named none** (#626): the set's `Field`, in a form row
 * under the Notice's line, so it has the set's label, keyboard and look. The whole window's
 * cases are in `SandboxBlocks.window.test.tsx`; this is the box itself.
 */

const PLANE = "/projects/acme" as PlaneId;

/** A host the chat's sandbox refused and whose name the report did not carry. */
const UNNAMED: ChatBlocked = {
  plane: PLANE,
  session: 4,
  operation: "connect",
  kind: "host",
  ours: false,
  harness: "claude",
  said: "a connection to a host",
  offer: "host",
  target: null,
  route: null,
  levels: ["chat", "you", "project"],
  held: false,
  ruled: null,
};

afterEach(() => {
  cleanup();
  clearMocks();
});

function drawn(block: ChatBlocked = UNNAMED) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "allow_sandbox_block")
      return { said: "Allowed for me on this machine.", live: false };
    return null;
  });
  render(
    <SandboxBlockNotice
      block={block}
      asks={askOfBlock(block)}
      more={0}
      onDismiss={vi.fn()}
      onAllowed={vi.fn()}
      onRestarted={vi.fn()}
    />,
  );
  return { sent: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args) };
}

describe("the host box of a block that named no host", () => {
  it("is the set's field in a form row, named by its row, with an example in it", () => {
    drawn();
    const box = screen.getByRole("textbox", { name: "Host to allow" });
    expect(box).toHaveClass("ui-field");
    expect(box).toHaveAttribute("placeholder", "api.example.com");
    expect(box.closest(".ui-setting-row")).not.toBeNull();
    expect(box).toHaveAccessibleDescription("An Allow lets the chat reach the host you type here.");
  });

  it("sends what is typed with the Allow, and nothing while the box is empty", async () => {
    const { sent } = drawn();
    const notice = screen.getByRole("status", { name: "Sandbox block" });
    const allow = within(notice).getByRole("button", { name: "Allow for me on this machine" });
    await userEvent.click(allow);
    expect(sent("allow_sandbox_block")).toEqual([]);

    await userEvent.type(screen.getByRole("textbox", { name: "Host to allow" }), "api.example.com");
    await userEvent.click(allow);

    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
          level: "you",
        },
      ]),
    );
  });

  it("names a folder's box as a folder", () => {
    drawn({ ...UNNAMED, operation: "write", kind: "outside", offer: "write", levels: ["chat"] });
    const box = screen.getByRole("textbox", { name: "Folder to allow" });
    expect(box).toHaveAccessibleDescription(
      "An Allow lets the chat write in the folder you type here and everything in it.",
    );
  });

  it("draws no box for a block that named its host", () => {
    drawn({ ...UNNAMED, target: "api.example.com" });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.getByText("api.example.com")).toBeInTheDocument();
  });
});
