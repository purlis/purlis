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
import { heardMore } from "./refusedMore";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

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
    expect(box.closest('[data-scope="field"]')).toHaveClass("ui-setting-box");
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
    expect(tipOf(everyone)).toEqual(SENTENCE);
    await userEvent.click(everyone);
    expect(sent("allow_sandbox_block")).toEqual([]);
  });

  it("is off under Other scopes too, while the others still allow", async () => {
    const { sent } = drawn(NAMED, SENTENCE);
    await userEvent.click(screen.getByRole("button", { name: "Other scopes…" }));
    const everyone = screen.getByRole("button", { name: "Allow for everyone in this project" });
    expect(everyone).toHaveAttribute("aria-disabled", "true");
    expect(tipOf(everyone)).toEqual(SENTENCE);
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

  it("draws each Allow as a grant and Keep blocked plain, with equal weight (ADR 0097 §6)", async () => {
    answered(NAMED);
    const notice = screen.getByRole("status", { name: "Sandbox block" });
    const ways = within(notice).getAllByRole("button");
    const allow = screen.getByRole("button", { name: "Allow for me on this machine" });
    const keep = screen.getByRole("button", { name: /^Keep blocked/ });
    const scopes = screen.getByRole("button", { name: "Other scopes…" });
    expect(allow).toHaveAttribute("data-variant", "grants");
    // The open lock it leads with is hidden from a screen reader: its name is its words.
    expect(allow.querySelector('[data-part="icon"] svg.lucide-lock-open')).toHaveAttribute(
      "aria-hidden",
      "true",
    );
    expect(keep).toHaveAttribute("data-scope", "button");
    expect(keep).not.toHaveAttribute("data-variant", "grants");
    expect(keep.querySelector("svg")).toBeNull();
    // Neither is the answer purlis suggests.
    for (const one of ways) expect(one).not.toHaveAttribute("data-variant", "primary");
    // Opening the scopes grants nothing, so it is drawn as Keep blocked is.
    expect(scopes).not.toHaveAttribute("data-variant", "grants");
    // The order the keyboard meets them in is the line's, Allow first and Keep blocked last.
    expect(ways.indexOf(allow)).toBeLessThan(ways.indexOf(keep));
    await userEvent.click(scopes);
    expect(screen.getByRole("button", { name: "Allow only for this chat" })).toHaveAttribute(
      "data-variant",
      "grants",
    );
  });

  it("says nothing for an Allow that sends nothing", async () => {
    // A block that named no host, with nothing typed: the press allows nothing.
    const ways = answered(UNNAMED);

    await userEvent.click(screen.getByRole("button", { name: "Allow for me on this machine" }));

    expect(ways).toEqual([]);
  });
});

describe("a local socket's Notice (ADR 0093)", () => {
  const DOCKER: ChatBlocked = {
    ...UNNAMED,
    kind: "local-socket",
    said: "a connection to a local socket",
    offer: "socket",
    target: "/Users/op/.docker/run/docker.sock",
    route:
      "Allowing it lets this chat control containers, which can reach your whole machine: about " +
      "the same as no sandbox.",
    levels: ["once", "chat", "you"],
  };

  it("names the socket whole, says what allowing it means, and offers once first", () => {
    drawn(DOCKER);

    expect(screen.getByText("/Users/op/.docker/run/docker.sock")).toBeTruthy();
    expect(screen.getByText(/about the same as no sandbox/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Allow once" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Other scopes…" })).toBeTruthy();
  });

  it("sends the socket and the scope the person chose", async () => {
    const { sent } = drawn(DOCKER);

    await userEvent.click(screen.getByRole("button", { name: "Other scopes…" }));
    await userEvent.click(screen.getByRole("button", { name: "Allow for me on this machine" }));

    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: {
            operation: "connect",
            kind: "local-socket",
            what: "socket",
            target: "/Users/op/.docker/run/docker.sock",
          },
          level: "you",
        },
      ]),
    );
  });

  // R2 (2026-10-13): one press can cover this machine for every project.
  it("offers every project on this machine beside Allow once, and this project under Other scopes", async () => {
    const { sent } = drawn({ ...DOCKER, levels: ["once", "machine", "chat", "you"] });

    // Side by side, with nothing to open first.
    expect(screen.getByRole("button", { name: "Allow once" })).toBeTruthy();
    const everywhere = screen.getByRole("button", {
      name: "Allow on this machine, for every project",
    });
    expect(screen.queryByRole("button", { name: "Allow for me on this machine" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Other scopes…" }));
    expect(screen.getByRole("button", { name: "Allow for me on this machine" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Allow only for this chat" })).toBeTruthy();
    // Once in the Notice, never a second time under the menu.
    expect(
      screen.getAllByRole("button", { name: "Allow on this machine, for every project" }),
    ).toHaveLength(1);

    await userEvent.click(everywhere);

    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: {
            operation: "connect",
            kind: "local-socket",
            what: "socket",
            target: "/Users/op/.docker/run/docker.sock",
          },
          level: "machine",
        },
      ]),
    );
  });

  it("offers no every-project answer where policy leaves that scope out", () => {
    drawn({ ...DOCKER, levels: ["chat", "you"] });

    expect(
      screen.queryByRole("button", { name: "Allow on this machine, for every project" }),
    ).toBeNull();
    expect(screen.getByRole("button", { name: "Allow only for this chat" })).toBeTruthy();
  });
});

describe("the certificate check's Notice (R1, 2026-10-13)", () => {
  const CHECK: ChatBlocked = {
    ...UNNAMED,
    operation: "lookup",
    kind: "certificate-check",
    said: "a lookup of the system's certificate check",
    target: null,
    levels: [],
  };

  it("offers a restart and no Allow where the project has them on and the chat started before", async () => {
    const { sent } = drawn({
      ...CHECK,
      offer: "allowed",
      route:
        "Certificate checks are on for this project. This chat started before they were, so tools such as gh can verify a host's certificate once it restarts on the same conversation.",
    });

    expect(screen.getByText(/Certificate checks are on for this project/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Allow/ })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Restart this chat" }));

    await waitFor(() => expect(sent("ask_chat_restart")).toEqual([{ plane: PLANE, session: 4 }]));
    expect(sent("allow_sandbox_block")).toEqual([]);
  });

  it("says the project turned them off, with where to turn them on, and never an Allow", () => {
    drawn({
      ...CHECK,
      offer: "unsandboxed",
      route:
        "This project turns certificate checks off, so tools such as gh cannot verify a host's certificate in its sandboxed chats. Turn Certificate checks on in Settings › Sandbox, for everyone in the project, or start this chat without the sandbox.",
    });

    expect(screen.getByText(/This project turns certificate checks off/)).toBeTruthy();
    expect(screen.getByText(/Settings › Sandbox/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Allow/ })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Start without the sandbox for this chat" }),
    ).toBeTruthy();
  });

  it("says who locked them and offers nothing where policy forbids the way out too", () => {
    drawn({
      ...CHECK,
      offer: "policy",
      route:
        "Policy turns certificate checks off on this machine. Locked by policy, set by IT in /etc/purlis/policy.json. Policy forbids starting this chat without the sandbox too.",
    });

    expect(screen.getByText(/Policy turns certificate checks off on this machine/)).toBeTruthy();
    expect(screen.getByText(/set by IT/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Allow/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Start without the sandbox/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart this chat" })).toBeNull();
  });
});

describe("a block of purlis's own operation", () => {
  it("says it is purlis's own in a sentence of its own, after a block sentence that holds a full stop (#1416)", () => {
    drawn({
      ...UNNAMED,
      operation: "write",
      kind: "system-temp",
      ours: true,
      said: "a write to one of macOS's per-user folders, which some programs write in place of this chat's own temporary folder. Such a program can usually be told to write under \"$TMPDIR\" instead",
      offer: "none",
      levels: [],
    });
    const notice = screen.getByRole("status", { name: "Sandbox block" });
    expect(notice).toHaveTextContent(
      'write under "$TMPDIR" instead. It was purlis\'s own operation, so it is a purlis bug.',
    );
    expect(notice).not.toHaveTextContent("instead that purlis");
  });
});

describe("a host block past the hosts the chat's proxy names", () => {
  it("says how many more were refused, and that no Allow here reaches them (#1353)", () => {
    heardMore({ plane: PLANE, session: 9, more: 3 });
    drawn({ ...UNNAMED, session: 9, target: "api.example.com:443" });
    const notice = screen.getByRole("status", { name: "Sandbox block" });
    expect(notice).toHaveTextContent(
      "3 more hosts were refused after these, which purlis does not name, so no Allow here reaches them.",
    );
  });

  it("says nothing more for a chat whose proxy named every host it refused", () => {
    drawn({ ...UNNAMED, session: 10, target: "api.example.com:443" });
    expect(screen.getByRole("status", { name: "Sandbox block" })).not.toHaveTextContent(
      "more host",
    );
  });
});

describe("Keep blocked on a block that is not a host (#1987)", () => {
  const FOLDER: ChatBlocked = {
    ...UNNAMED,
    operation: "write",
    kind: "home",
    said: "a write to a folder outside the project",
    offer: "write",
    target: "/Users/dev/.cache/cargo",
    levels: ["chat", "you"],
  };

  function keeping(answer: unknown) {
    const asked: { cmd: string; args: unknown }[] = [];
    const onDismiss = vi.fn();
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "keep_sandbox_block" && answer instanceof Error) throw answer.message;
      return null;
    });
    render(
      <SandboxBlockNotice
        block={FOLDER}
        asks={askOfBlock(FOLDER)}
        more={0}
        onDismiss={onDismiss}
        onAllowed={vi.fn()}
        onRestarted={vi.fn()}
      />,
    );
    return { asked, onDismiss };
  }

  it("is kept by the core, which tells the chat, and the Notice goes", async () => {
    const { asked, onDismiss } = keeping(null);
    await userEvent.click(screen.getByRole("button", { name: /^Keep blocked/ }));
    await waitFor(() => expect(onDismiss).toHaveBeenCalled());
    expect(asked.filter((one) => one.cmd === "keep_sandbox_block").map((one) => one.args)).toEqual([
      {
        plane: PLANE,
        session: 4,
        shown: {
          operation: "write",
          kind: "home",
          what: "write",
          target: "/Users/dev/.cache/cargo",
        },
        reason: null,
      },
    ]);
  });

  it("says why and stays where the core could not keep it", async () => {
    const { onDismiss } = keeping(new Error("purlis could not read this project."));
    await userEvent.click(screen.getByRole("button", { name: /^Keep blocked/ }));
    await waitFor(() =>
      expect(screen.getByText(/purlis could not read this project\./)).toBeInTheDocument(),
    );
    expect(onDismiss).not.toHaveBeenCalled();
  });
});

describe("a service on this Mac's Notice (qw384)", () => {
  const CLUSTER: ChatBlocked = {
    ...UNNAMED,
    harness: "codex",
    kind: "local-service",
    said: "a connection to a service on this Mac",
    offer: "port",
    target: "50795",
    route:
      "Allowing it lets this chat connect to whatever listens on that port on this Mac, as you; " +
      "a container engine's or a cluster's port can reach much of your machine.",
    levels: ["chat", "you"],
  };

  it("names the port, says what allowing it means, and sends that one port", async () => {
    const { sent } = drawn(CLUSTER);

    expect(screen.getByText("50795")).toBeTruthy();
    expect(screen.getByText(/whatever listens on that port on this Mac/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));

    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "connect", kind: "local-service", what: "port", target: "50795" },
          level: "chat",
        },
      ]),
    );
  });

  it("pre-fills a port only the command's output named, and says to check it", async () => {
    const { sent } = drawn({ ...CLUSTER, target: null, suggested: "50795" });
    const box = screen.getByRole("textbox", { name: "Port to allow" });
    expect(box).toHaveValue("50795");
    expect(box).toHaveAccessibleDescription(/from the command's output: check it/);
    await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));
    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "connect", kind: "local-service", what: "port", target: "50795" },
          level: "chat",
        },
      ]),
    );
  });

  describe("in a Claude Code chat (qw384b)", () => {
    const CLAUDE: ChatBlocked = {
      ...CLUSTER,
      harness: "claude",
      route: `${CLUSTER.route} In a Claude Code chat a command reaches it through this chat's proxy, by the name this-mac.internal and that port.`,
    };
    const CONTEXTS = [
      { name: "kind-kind", server: "https://127.0.0.1:50795", port: 50795, refused: null },
      {
        name: "prod",
        server: "https://api.prod.example.com",
        port: null,
        refused: "https://api.prod.example.com is not a cluster on this Mac.",
      },
    ];

    function drawnClaude(block: ChatBlocked = CLAUDE) {
      const asked: { cmd: string; args: unknown }[] = [];
      mockIPC((cmd, args) => {
        asked.push({ cmd, args });
        if (cmd === "allow_sandbox_block")
          return { said: "Allowed only for this chat.", live: false };
        if (cmd === "kube_contexts") return CONTEXTS;
        if (cmd === "point_kubectl")
          return "kubectl in this chat now uses its own copy of kind-kind.";
        return null;
      });
      const onAllowed = vi.fn();
      render(
        <SandboxBlockNotice
          block={block}
          asks={askOfBlock(block)}
          more={0}
          onDismiss={vi.fn()}
          onAllowed={onAllowed}
          onRestarted={vi.fn()}
        />,
      );
      return {
        onAllowed,
        sent: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
      };
    }

    it("offers the same Allow, saying the name its proxy carries the port by", async () => {
      const { sent } = drawnClaude();
      expect(screen.getByText(/this-mac\.internal/)).toBeTruthy();
      expect(screen.queryByRole("button", { name: /without the sandbox/ })).toBeNull();
      await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));
      await waitFor(() => expect(sent("allow_sandbox_block")).toHaveLength(1));
    });

    it("once allowed, points the chat's kubectl at a context on this Mac on the person's press", async () => {
      const { sent, onAllowed } = drawnClaude();
      // Nothing is read before the person asks.
      expect(screen.queryByRole("button", { name: "Point kubectl at it…" })).toBeNull();
      await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));
      await userEvent.click(await screen.findByRole("button", { name: "Point kubectl at it…" }));
      expect(sent("kube_contexts")).toHaveLength(1);
      expect(sent("point_kubectl")).toEqual([]);
      // Before any press, it says the chat gets that context's credentials, which it can read.
      expect(
        screen.getByText(
          /gets its own copy of the context you choose, its credentials included: this chat can read them/,
        ),
      ).toBeTruthy();
      // A context elsewhere is listed with why, and never pressable.
      expect(screen.getByText(/api\.prod\.example\.com is not a cluster on this Mac/)).toBeTruthy();
      expect(screen.queryByRole("button", { name: /prod/ })).toBeNull();
      onAllowed.mockClear();
      await userEvent.click(screen.getByRole("button", { name: "Use kind-kind" }));
      await waitFor(() =>
        expect(sent("point_kubectl")).toEqual([{ plane: PLANE, session: 4, context: "kind-kind" }]),
      );
      expect(await screen.findByText(/now uses its own copy of kind-kind/)).toBeTruthy();
      // The chat restarts to take it, as after an Allow.
      expect(onAllowed).toHaveBeenCalledTimes(1);
    });

    it("never offers it in a chat of purlis's own wrap, which reaches the port as itself", async () => {
      drawnClaude({ ...CLAUDE, harness: "codex" });
      await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));
      await screen.findByText(/Allowed only for this chat/);
      expect(screen.queryByRole("button", { name: "Point kubectl at it…" })).toBeNull();
    });
  });

  it("asks for the port in a form row where the refusal named none", async () => {
    const { sent } = drawn({ ...CLUSTER, target: null });
    const box = screen.getByRole("textbox", { name: "Port to allow" });
    expect(box).toHaveAttribute("placeholder", "6443");
    await userEvent.type(box, "50795");
    await userEvent.click(screen.getByRole("button", { name: "Allow only for this chat" }));

    await waitFor(() =>
      expect(sent("allow_sandbox_block")).toEqual([
        {
          plane: PLANE,
          session: 4,
          shown: { operation: "connect", kind: "local-service", what: "port", target: "50795" },
          level: "chat",
        },
      ]),
    );
  });
});

describe("an Allow that restarts the chat says beforehand what will end (#1986)", () => {
  const FOLDER: ChatBlocked = {
    ...UNNAMED,
    operation: "write",
    kind: "outside",
    said: "a write outside the chat's folders",
    offer: "write",
    target: "/Users/op/work/notes",
    levels: ["chat", "you"],
  };

  function drawnReading(block: ChatBlocked, ends: unknown) {
    const asked: string[] = [];
    mockIPC((cmd) => {
      asked.push(cmd);
      if (cmd === "what_a_restart_ends") return ends;
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
    return asked;
  }

  it("names the dev server a folder's Allow will stop, at the chat's next pause", async () => {
    drawnReading(FOLDER, { listening: [{ program: "node", port: 5173 }], others: 0 });

    await waitFor(() =>
      expect(
        screen.getByText(/at its next pause between turns; its/, { exact: false }),
      ).toHaveTextContent("its node on port 5173 will stop."),
    );
    expect(screen.getByRole("button", { name: /^Allow/ })).toBeTruthy();
  });

  it("says plainly when purlis cannot tell what a socket's Allow will end", async () => {
    drawnReading(
      {
        ...FOLDER,
        kind: "local-socket",
        said: "a connection to a local socket",
        offer: "socket",
        target: "/Users/op/.docker/run/docker.sock",
        levels: ["once", "chat", "you"],
      },
      null,
    );

    await waitFor(() =>
      expect(screen.getByText(/purlis cannot tell what it has running/)).toBeTruthy(),
    );
  });

  it("asks nothing about a host, which the chat may take without restarting", () => {
    const asked = drawnReading({ ...UNNAMED, target: "api.example.com" }, null);
    expect(asked).not.toContain("what_a_restart_ends");
    expect(screen.queryByText(/next pause between turns/)).toBeNull();
  });
});
