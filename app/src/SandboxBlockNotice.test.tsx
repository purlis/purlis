// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SandboxBlockNotice } from "./SandboxBlockNotice";
import type { ChatBlocked, PlaneId } from "./bindings";
import { askOfBlock } from "./test-asks";
import { ReadOnlySaid } from "./ReadOnlyLine";

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

function drawn(block: ChatBlocked = UNNAMED, readOnly?: string) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "allow_sandbox_block")
      return { said: "Allowed for me on this machine.", live: false };
    return null;
  });
  render(
    <ReadOnlySaid.Provider value={readOnly}>
      <SandboxBlockNotice
        block={block}
        asks={askOfBlock(block)}
        more={0}
        onDismiss={vi.fn()}
        onAllowed={vi.fn()}
        onRestarted={vi.fn()}
      />
    </ReadOnlySaid.Provider>,
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

describe("a host refused while a brokered command ran (#1353)", () => {
  const NAMED: ChatBlocked = { ...UNNAMED, target: "api.cluster.example-k8s.com:6443" };

  it("says the vault the command ran with, before any Allow", () => {
    drawn({ ...NAMED, vault: "team" });
    const notice = screen.getByRole("status", { name: "Sandbox block" });
    expect(notice).toHaveTextContent(
      "The sandbox blocked a connection to a host while running a command with vault team.",
    );
    expect(within(notice).getByText("team")).toHaveClass("block-vault");
  });

  it("says no vault for a block of the chat's own command", () => {
    drawn(NAMED);
    const notice = screen.getByRole("status", { name: "Sandbox block" });
    expect(notice).not.toHaveTextContent("vault");
  });
});

describe("the project-level Allow on a read-only project (#1759)", () => {
  const SENTENCE =
    "this project requires the feature memory-proposals, which purlis 0.8.0 does not have; " +
    "purlis 0.9.0 or later has it. purlis opens the project read-only and writes nothing to it.";
  const NAMED: ChatBlocked = { ...UNNAMED, target: "api.example.com" };

  it("is drawn and off with the sentence, and sends nothing", async () => {
    const { sent } = drawn({ ...NAMED, levels: ["project"] }, SENTENCE);
    const everyone = screen.getByRole("button", { name: "Allow for everyone in this project" });
    expect(everyone).toHaveAttribute("aria-disabled", "true");
    expect(everyone).toHaveAttribute("title", SENTENCE);
    await userEvent.click(everyone);
    expect(sent("allow_sandbox_block")).toEqual([]);
  });

  it("is off under Other scopes too, while the others still allow", async () => {
    const { sent } = drawn(NAMED, SENTENCE);
    await userEvent.click(screen.getByRole("button", { name: "Other scopes…" }));
    const everyone = screen.getByRole("button", { name: "Allow for everyone in this project" });
    expect(everyone).toBeDisabled();
    expect(everyone).toHaveAttribute("title", SENTENCE);
    await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));
    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
          level: "chat",
        },
      ]),
    );
  });

  it("is on where the project is not read-only", () => {
    drawn({ ...NAMED, levels: ["project"] });
    const everyone = screen.getByRole("button", { name: "Allow for everyone in this project" });
    expect(everyone).not.toHaveAttribute("aria-disabled");
  });
});

/**
 * **A way out says what it answered, and which asks** (#1700, 2026-10-10 21:09 item 2): the
 * pane's Inbox lists an answer given on a pane once its ask goes. An Allow or Keep blocked says
 * its words with each ask it answers; the Report and the scopes' toggle answer nothing.
 */
describe("the block Notice's ways out", () => {
  const NAMED: ChatBlocked = { ...UNNAMED, target: "api.example.com" };

  function answered(block: ChatBlocked) {
    mockIPC((cmd) => {
      if (cmd === "allow_sandbox_block")
        return { said: "Allowed for me on this machine.", live: true };
      if (cmd === "keep_sandbox_block") return null;
      return null;
    });
    const ways: { words: string; ask: string }[] = [];
    render(
      <SandboxBlockNotice
        block={block}
        asks={askOfBlock(block)}
        more={0}
        onDismiss={vi.fn()}
        onAllowed={vi.fn()}
        onRestarted={vi.fn()}
        onWay={(words, ask) => ways.push({ words, ask: ask.ask })}
      />,
    );
    return ways;
  }

  it("says an Allow's words with the block's ask", async () => {
    const ways = answered(NAMED);

    await userEvent.click(screen.getByRole("button", { name: "Allow for me on this machine" }));

    expect(ways).toEqual([
      { words: "Allow for me on this machine", ask: "block:4:connect:host:api.example.com" },
    ]);
  });

  it("says an Allow pressed under Other scopes", async () => {
    const ways = answered(NAMED);

    await userEvent.click(screen.getByRole("button", { name: "Other scopes…" }));
    expect(ways, "opening the scopes answers nothing").toEqual([]);
    await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));

    expect(ways.map((one) => one.words)).toEqual(["Allow only for this chat"]);
  });

  it("says Keep blocked", async () => {
    const ways = answered(NAMED);

    await userEvent.click(screen.getByRole("button", { name: /^Keep blocked/ }));

    expect(ways.map((one) => one.ask)).toEqual(["block:4:connect:host:api.example.com"]);
    expect(ways[0].words).toMatch(/^Keep blocked/);
  });

  it("says nothing for an Allow that sends nothing", async () => {
    // A block that named no host, with nothing typed: the press allows nothing.
    const ways = answered(UNNAMED);

    await userEvent.click(screen.getByRole("button", { name: "Allow for me on this machine" }));

    expect(ways).toEqual([]);
  });
});
