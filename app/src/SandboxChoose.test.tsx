// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SandboxBlockNotice } from "./SandboxBlockNotice";
import { blocked } from "./sandboxBlocks";
import { commands, type BlockChoices, type ChatBlocked, type PlaneId } from "./bindings";
import * as link from "./uiRpc";
import { askOfBlock } from "./test-asks";
import { inTheWindow } from "./personaHostsAllow";

/**
 * **The person names what a sandbox block did not** (#1884): the Notice of a socket or a
 * service the sandbox did not name offers the choice beside Start without the sandbox, lists
 * what the app found and nothing a chat wrote, and sends the choice down the window's own
 * route. And a block a chat only read back is a quiet line that asks nothing.
 */

const PLANE = "/projects/acme" as PlaneId;

/** A local socket the sandbox refused without saying which: `make up 2>&1 | tail`. */
const UNNAMED: ChatBlocked = {
  plane: PLANE,
  session: 4,
  operation: "connect",
  kind: "local-socket",
  ours: false,
  harness: "claude",
  said: "a connection to a local socket",
  offer: "unsandboxed",
  target: null,
  route:
    "The sandbox did not say which socket it refused, so you can choose it. purlis checks the " +
    "socket you choose as it checks one the sandbox names.",
  levels: [],
  held: false,
  ruled: null,
  choose: "socket",
};

// No service is listed by purlis today (the certificate check is the project's setting, never a
// grant): this draws the list a later one would be chosen from.
const SERVICE: ChatBlocked = {
  ...UNNAMED,
  operation: "lookup",
  kind: "system-service",
  said: "a lookup of a system service",
  route:
    "The sandbox did not say which system service it refused, so you can choose it from the " +
    "ones purlis can allow.",
  choose: "service",
};

const DOCKER = "/Users/op/.docker/run/docker.sock";
const AGENT = "/private/tmp/com.apple.launchd.AbC/Listeners";

/** What the app found on this machine, in its own order. */
const FOUND: BlockChoices = {
  choices: [
    {
      target: DOCKER,
      named: "Container engine",
      says: "Allowing it lets this chat control containers, which can reach your whole machine: about the same as no sandbox.",
    },
    { target: AGENT, named: "SSH agent", says: "Allowing it lets this chat use your SSH keys." },
  ],
  typed: true,
};

const SERVICES: BlockChoices = {
  choices: [
    {
      target: "com.example.helper",
      named: "Local service",
      says: "It is a service on this machine; purlis can't tell what it does.",
    },
  ],
  typed: false,
};

afterEach(() => {
  cleanup();
  clearMocks();
});

function drawn(
  block: ChatBlocked,
  answers: { choices?: BlockChoices; chosen?: () => ChatBlocked; takenUp?: () => null } = {},
) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "sandbox_block_choices") return answers.choices ?? FOUND;
    if (cmd === "choose_sandbox_block_target") {
      if (answers.chosen === undefined) throw "That is not a local socket on this machine now.";
      return answers.chosen();
    }
    if (cmd === "take_up_sandbox_block") return (answers.takenUp ?? (() => null))();
    return null;
  });
  const onDismiss = vi.fn();
  render(
    <SandboxBlockNotice
      block={block}
      asks={askOfBlock(block)}
      more={0}
      onDismiss={onDismiss}
      onAllowed={vi.fn()}
      onRestarted={vi.fn()}
    />,
  );
  return {
    onDismiss,
    sent: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
  };
}

const notice = () => screen.getByRole("status", { name: "Sandbox block" });

describe("a socket the sandbox did not name", () => {
  it("offers the choice beside Start without the sandbox, and is never a dead end", () => {
    const { sent } = drawn(UNNAMED);
    const buttons = within(notice())
      .getAllByRole("button")
      .map((one) => one.textContent);
    expect(buttons).toEqual([
      "Choose the socket…",
      "Start without the sandbox for this chat",
      "Dismiss",
    ]);
    expect(notice()).toHaveTextContent("so you can choose it");
    expect(notice()).not.toHaveTextContent("there is no socket to allow");
    // Nothing is looked for, and nothing is drawn to choose, before the press.
    expect(sent("sandbox_block_choices")).toEqual([]);
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });

  it("lists what the app found, in the app's order, each with what allowing it means", async () => {
    const { sent } = drawn(UNNAMED);
    await userEvent.click(screen.getByRole("button", { name: "Choose the socket…" }));

    const list = await screen.findByRole("radiogroup", { name: "Socket to allow" });
    expect(within(list).getAllByRole("radio")).toHaveLength(3);
    expect(
      within(list)
        .getAllByRole("radio")
        .map((one) => one.getAttribute("value")),
    ).toEqual([DOCKER, AGENT, "another"]);
    expect(within(list).getByRole("radio", { name: DOCKER })).toHaveAccessibleDescription(
      /about the same as no sandbox/,
    );
    expect(within(list).getByRole("radio", { name: AGENT })).toHaveAccessibleDescription(
      /use your SSH keys/,
    );
    // Asked of the block shown, naming nothing: the list is the app's, not the chat's.
    expect(sent("sandbox_block_choices")).toEqual([
      {
        plane: PLANE,
        session: 4,
        shown: { operation: "connect", kind: "local-socket", what: "socket", target: "" },
      },
    ]);
    // Nothing is picked for the person, so nothing can be sent yet.
    expect(within(list).queryByRole("radio", { checked: true })).toBeNull();
    expect(screen.getByRole("button", { name: "Use this socket" })).toBeDisabled();
  });

  it("sends the socket the person picked down the window's own route", async () => {
    const named: ChatBlocked = { ...UNNAMED, offer: "socket", target: DOCKER, choose: null };
    const { sent } = drawn(UNNAMED, { chosen: () => named });
    await userEvent.click(screen.getByRole("button", { name: "Choose the socket…" }));
    await userEvent.click(await screen.findByRole("radio", { name: DOCKER }));
    await userEvent.click(screen.getByRole("button", { name: "Use this socket" }));

    await waitFor(() =>
      expect(sent("choose_sandbox_block_target")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "connect", kind: "local-socket", what: "socket", target: "" },
          target: DOCKER,
        },
      ]),
    );
    // Choosing allows nothing: no Allow was sent.
    expect(sent("allow_sandbox_block")).toEqual([]);
  });

  it("takes a path the person types, in a box that starts empty", async () => {
    // A block whose words name a path: none of it reaches the box.
    const said = { ...UNNAMED, route: `${UNNAMED.route} /tmp/planted.sock` };
    const { sent } = drawn(said, { chosen: () => ({ ...UNNAMED, offer: "socket" }) });
    await userEvent.click(screen.getByRole("button", { name: "Choose the socket…" }));
    expect(screen.queryByRole("textbox")).toBeNull();
    await userEvent.click(await screen.findByRole("radio", { name: "Another socket" }));

    const box = screen.getByRole("textbox", { name: "Socket path" });
    expect(box).toHaveValue("");
    expect(box.closest('[data-scope="field"]')).toHaveClass("ui-setting-box");
    expect(box.closest(".ui-setting-row")).not.toBeNull();
    expect(screen.getByRole("button", { name: "Use this socket" })).toBeDisabled();

    await userEvent.type(box, "  /Users/op/.colima/default/docker.sock ");
    await userEvent.click(screen.getByRole("button", { name: "Use this socket" }));
    await waitFor(() =>
      expect(sent("choose_sandbox_block_target")).toMatchObject([
        { target: "/Users/op/.colima/default/docker.sock" },
      ]),
    );
  });

  it("opens the box at once where the app found no socket", async () => {
    drawn(UNNAMED, { choices: { choices: [], typed: true } });
    await userEvent.click(screen.getByRole("button", { name: "Choose the socket…" }));
    expect(await screen.findByRole("textbox", { name: "Socket path" })).toHaveValue("");
    expect(
      screen.getByText(/purlis found no socket it recognises on this machine\./),
    ).toBeInTheDocument();
  });

  it("says why a choice was refused, and lets the person choose again", async () => {
    const { sent } = drawn(UNNAMED);
    await userEvent.click(screen.getByRole("button", { name: "Choose the socket…" }));
    await userEvent.click(await screen.findByRole("radio", { name: AGENT }));
    await userEvent.click(screen.getByRole("button", { name: "Use this socket" }));

    // Said in the form it opened, under the Notice's line.
    expect(
      await screen.findByText("That is not a local socket on this machine now."),
    ).toBeInTheDocument();
    expect(sent("choose_sandbox_block_target")).toHaveLength(1);
    expect(screen.getByRole("radio", { name: DOCKER })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Use this socket" })).toBeEnabled();
  });

  it("draws the choice from the shared controls", async () => {
    drawn(UNNAMED);
    const choose = screen.getByRole("button", { name: "Choose the socket…" });
    expect(choose).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(choose);
    expect(choose).toHaveAttribute("aria-expanded", "true");
    const list = await screen.findByRole("radiogroup", { name: "Socket to allow" });
    expect(list).toHaveAttribute("data-scope", "radio-group");
    expect(screen.getByRole("button", { name: "Use this socket" })).toHaveAttribute(
      "data-variant",
      "grants",
    );
    expect(screen.getByRole("button", { name: "Use this socket" })).toHaveAttribute(
      "data-size",
      "sm",
    );
    // It grants: the grant's style, never the suggested answer's (ADR 0097 §6).
    expect(screen.getByRole("button", { name: "Use this socket" })).not.toHaveAttribute(
      "data-variant",
      "primary",
    );
  });

  it("is replaced in the window by the block that names what was chosen", () => {
    const named: ChatBlocked = {
      ...UNNAMED,
      offer: "socket",
      target: DOCKER,
      levels: ["once", "chat", "you"],
      choose: null,
    };
    const held = blocked(blocked({}, UNNAMED), named);
    expect(held[4]).toEqual([named]);
  });
});

describe("a system service the sandbox did not name", () => {
  it("is chosen from the ones purlis can allow, and none is typed", async () => {
    const { sent } = drawn(SERVICE, {
      choices: SERVICES,
      chosen: () => ({ ...SERVICE, offer: "service", target: "com.example.helper" }),
    });
    await userEvent.click(screen.getByRole("button", { name: "Choose the service…" }));

    const list = await screen.findByRole("radiogroup", { name: "Service to allow" });
    expect(within(list).getAllByRole("radio")).toHaveLength(1);
    expect(screen.queryByRole("radio", { name: "Another socket" })).toBeNull();
    expect(screen.queryByRole("textbox")).toBeNull();

    await userEvent.click(within(list).getByRole("radio", { name: "com.example.helper" }));
    await userEvent.click(screen.getByRole("button", { name: "Use this service" }));
    await waitFor(() =>
      expect(sent("choose_sandbox_block_target")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "lookup", kind: "system-service", what: "service", target: "" },
          target: "com.example.helper",
        },
      ]),
    );
  });
});

describe("the choice is the window's alone", () => {
  it("has its three calls on the window's client and on no link's", () => {
    for (const method of [
      "sandboxBlockChoices",
      "chooseSandboxBlockTarget",
      "takeUpSandboxBlock",
    ]) {
      expect(inTheWindow(commands, method), method).toBe(true);
      expect(inTheWindow(link.commands, method), method).toBe(false);
    }
  });

  it("offers no choice where policy left none, and keeps Start without the sandbox", () => {
    drawn({ ...UNNAMED, choose: null });
    expect(screen.queryByRole("button", { name: "Choose the socket…" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Start without the sandbox for this chat" }),
    ).toBeInTheDocument();
  });

  it("still offers the choice where policy forbids starting without the sandbox", () => {
    drawn({ ...UNNAMED, offer: "policy" });
    expect(screen.getByRole("button", { name: "Choose the socket…" })).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Start without the sandbox for this chat" }),
    ).toBeNull();
  });
});

describe("a block the chat only read back", () => {
  const QUIET: ChatBlocked = {
    ...UNNAMED,
    quiet: "It was in the output of a command that ran earlier, so nothing waits on your answer.",
  };

  it("is a quiet line that says so and is put away with Dismiss", async () => {
    const { onDismiss, sent } = drawn(QUIET);
    expect(notice()).not.toHaveClass("notice-trouble");
    expect(notice()).toHaveTextContent(
      "This chat read a sandbox refusal back: a connection to a local socket. It was in the " +
        "output of a command that ran earlier, so nothing waits on your answer.",
    );
    expect(
      within(notice())
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(["Show what you can do", "Dismiss"]);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
    expect(sent("take_up_sandbox_block")).toEqual([]);
  });

  it("becomes the block's own Notice once the person takes it up", async () => {
    const { sent } = drawn(QUIET);
    await userEvent.click(screen.getByRole("button", { name: "Show what you can do" }));

    expect(await screen.findByRole("button", { name: "Choose the socket…" })).toBeInTheDocument();
    expect(notice()).toHaveClass("notice-trouble");
    expect(sent("take_up_sandbox_block")).toEqual([
      {
        plane: PLANE,
        session: 4,
        shown: { operation: "connect", kind: "local-socket", what: "socket", target: "" },
      },
    ]);
  });

  it("takes up a host it did not name as the host's own block", async () => {
    const host: ChatBlocked = {
      ...QUIET,
      kind: "host",
      said: "a connection to an internet host this project does not allow",
      offer: "host",
      route: null,
      levels: ["chat", "you", "project"],
      choose: null,
    };
    const { sent } = drawn(host);
    expect(screen.queryByRole("textbox")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Show what you can do" }));
    expect(await screen.findByRole("textbox", { name: "Host to allow" })).toHaveValue("");
    expect(sent("take_up_sandbox_block")).toEqual([
      {
        plane: PLANE,
        session: 4,
        shown: { operation: "connect", kind: "host", what: "host", target: "" },
      },
    ]);
  });

  it("stays quiet, saying why, where the chat is no longer blocked on it", async () => {
    drawn(QUIET, {
      takenUp: () => {
        throw "Chat 4 is not blocked on what this line showed any more.";
      },
    });
    await userEvent.click(screen.getByRole("button", { name: "Show what you can do" }));
    await waitFor(() => expect(notice()).toHaveTextContent("Chat 4 is not blocked on what"));
    expect(notice()).not.toHaveClass("notice-trouble");
    expect(screen.queryByRole("button", { name: "Choose the socket…" })).toBeNull();
  });

  it("opens a block nothing holds without a word to the core", async () => {
    const { sent } = drawn({ ...QUIET, choose: null });
    await userEvent.click(screen.getByRole("button", { name: "Show what you can do" }));
    expect(
      await screen.findByRole("button", { name: "Start without the sandbox for this chat" }),
    ).toBeInTheDocument();
    expect(sent("take_up_sandbox_block")).toEqual([]);
  });
});
