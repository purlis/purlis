// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { ChatBlocked, Moved, SandboxState, Shown } from "./bindings";
import { askOfBlock } from "./test-asks";
import { askRows, forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";
import { forgetInbox } from "./inboxRules";
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { sandboxCommandReturned } from "./sandboxAsked";
import { SANDBOX_ALLOW_SAID } from "./allowLabels";
import { SCOPE_LABEL } from "./InboxAsks";

/**
 * **Asks holds asks only** (R-240, R-171, R-195, R-205), on the real window with the core
 * mocked as the asks registry: what Asks lists, and what the ✋ counts, is the registry's list
 * and nothing else. A block is never an ask: it adds no row and no count. A sandbox refusal the
 * chat still waits on is an ask, with its Allow. The sandbox consent is in Asks once, only while
 * it can be answered, and no Notice carries it beside its row.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

/** A block of purlis's own operation: a purlis bug, never an ask. */
const OURS: ChatBlocked = {
  plane: PLANE,
  session: 4,
  operation: "write",
  kind: "project-files",
  ours: true,
  harness: "claude",
  said: "a write to the project's own files",
  offer: "none",
  target: null,
  route: null,
  levels: [],
  held: false,
  ruled: null,
};

/** A block of the chat's own work that no grant can name. */
const THEIRS: ChatBlocked = {
  ...OURS,
  kind: "toolchain-cache",
  ours: false,
  said: "a write to a toolchain's package cache",
};

/** A host the chat's sandbox refused, its connection held while the chat waits on it (#1666). */
const HELD: ChatBlocked = {
  ...OURS,
  operation: "connect",
  kind: "host",
  ours: false,
  said: "a connection to api.example.com",
  offer: "host",
  target: "api.example.com",
  levels: ["you", "chat", "project"],
  held: true,
};

/** Chat 4 moved on the board: what a held connection's block written leaves the window told. */
const MOVED: Moved = {
  plane: PLANE,
  session: 4,
  state: "running",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
};

/** A permission prompt chat 4 holds on its hook, as the registry lists it. */
const PERMISSION: Shown = {
  session: 4,
  ask: "permission:4:1",
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["claude 4"],
  answer: { via: "hook" },
};

const ITEM = {
  what: "service",
  target: "com.example.helper",
  named: "com.example.helper",
  says: "It is a service on this machine; purlis can't tell what it does.",
  refused: null,
} as const;

const SANDBOX: SandboxState = {
  on: true,
  offer: false,
  said: null,
  never: [],
  hosts_changed: null,
  presets_changed: null,
  presets: [],
  persona_hosts: [],
  besides: { project_hosts: 0, your_hosts: 0, folders: 0 },
  policy: null,
};

/** What the project expects and this machine has not allowed, still to be asked. */
const EXPECTING: SandboxState = {
  ...SANDBOX,
  expects: { asked: true, locked: null, items: [ITEM] },
};

/** The same, once an administrator's policy locks it: nothing is left to answer. */
const LOCKED: SandboxState = {
  ...SANDBOX,
  expects: {
    asked: true,
    locked:
      "Policy forbids allowing a chat a local socket or a system service. Locked by policy, set by IT in /etc/purlis/policy.json.",
    items: [ITEM],
  },
};

/** The sandbox consent as the core lists it among the project's asks (`asking::consents`). */
const CONSENT: Shown = {
  session: 0,
  ask: "sandbox-expects",
  says: "This project expects more than this machine allows: the system service com.example.helper",
  options: [
    { id: "allow", label: "Allow on this machine", allows: true },
    { id: "not-now", label: "Not now", allows: false },
  ],
  source: "sandbox-expects",
  chain: ["purlis"],
  answer: { via: "sandbox-expects", items: [ITEM] },
};

/**
 * The core, with chat 4 open on its tab unless `chats` says none. `registry` is what the asks
 * registry lists each time it is asked, and `sandbox` the project's sandbox state: each read
 * again whenever the window asks.
 */
function core(world: {
  registry: () => Shown[];
  sandbox?: () => SandboxState;
  chats?: (typeof CHAT)[];
}) {
  const chats = world.chats ?? [CHAT];
  const asked: string[] = [];
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return chats;
      if (cmd === "reopened_layout") return aTabForEachChat(chats, () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats }],
        };
      if (cmd === "plane_doctor") return { rows: [], app_rows: [], full: false, path: "" };
      if (cmd === "sandbox_state") return (world.sandbox ?? (() => SANDBOX))();
      if (cmd === "asks_waiting")
        return { plane: PLANE, asks: world.registry(), with_asking_chat: [] };
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      if (cmd === "inbox_updates" || cmd === "note_inbox_updates") return [];
      if (cmd === "chat_states" || cmd === "running_sessions") return [];
      if (cmd === "chats_that_would_not_start" || cmd === "owed_restarts") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return { reads: () => asked.filter((cmd) => cmd === "asks_waiting").length };
}

const hand = () =>
  within(screen.getByRole("tablist", { name: "Attention" })).getByRole("tab", { name: /^Asks/ });
const handCount = () => hand().querySelector('[data-part="count"]');
const LABEL = "This project expects more than this machine allows";

beforeEach(() => inboxOpenAtLaunch());

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInboxOpen();
  forgetInbox();
});

describe("Asks holds asks only", () => {
  it("a block adds no row and no count to Asks", async () => {
    // R-171: the registry lists one ask, chat 4's permission prompt, and goes on listing it
    // alone however many blocks the core sends.
    const { reads } = core({ registry: () => [PERMISSION] });
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });
    await waitFor(() => expect(askRows()).toHaveLength(1));
    await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
    const before = reads();

    // A block of purlis's own, one of the chat's work no grant names, and a held connection's
    // block written once the chat stopped waiting on it (T1-09), which tells only that chat 4
    // moved.
    await act(() => emit("chat-sandbox-blocked", OURS));
    await act(() => emit("chat-sandbox-blocked", THEIRS));
    await act(() => emit(...onTheWire("chat-moved", MOVED)));
    // The window read the list again on each, so what follows is read after them.
    await waitFor(() => expect(reads()).toBeGreaterThan(before));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 120));
    });

    expect(askRows()).toHaveLength(1);
    expect(askRows()[0]).toHaveTextContent("Run cargo test");
    expect(handCount()).toHaveAttribute("data-count", "1");
    expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you");
  });

  it("a sandbox refusal is an ask with its Allow while the chat waits", async () => {
    // R-195: while chat 4 waits on its held connection, the registry lists the refusal as one
    // sandbox ask, as `asking.rs` derives it for the block it holds.
    let waiting = false;
    core({ registry: () => (waiting ? askOfBlock(HELD, ["claude 4"]) : []) });
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });
    await screen.findByTestId("pane");

    waiting = true;
    await act(() => emit("chat-sandbox-blocked", HELD));

    const row = await waitFor(() => {
      const rows = askRows();
      expect(rows).toHaveLength(1);
      return rows[0];
    });
    expect(row).toHaveTextContent("api.example.com");
    // Its Allow on the row, the narrowest first and the wider ones behind Other scopes… (#5998).
    expect(within(row).getByRole("button", { name: SANDBOX_ALLOW_SAID.chat })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Other scopes…" })).toBeInTheDocument();
    await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
    // One ask, and no row of a block beside it.
    expect(askRows()).toHaveLength(1);
  });

  it("the sandbox consent is in Asks only while it can be answered", async () => {
    // R-205: answerable, the consent is one row in Asks, counted on the ✋, and no Notice
    // carries it. Once policy locks it, nothing can answer it: it leaves Asks and the ✋. No
    // chat is open, so Asks lists the project's, the consent among them.
    let sandbox = EXPECTING;
    core({
      registry: () => (sandbox === EXPECTING ? [CONSENT] : []),
      sandbox: () => sandbox,
      chats: [],
    });
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });
    const notices = await screen.findByRole("tabpanel", { name: "Notices" });

    await waitFor(() => expect(handCount()).toHaveAttribute("data-count", "1"));
    const row = await waitFor(() => {
      const one = askRows().find((each) => each.textContent?.includes("com.example.helper"));
      expect(one).toBeDefined();
      return one as HTMLElement;
    });
    expect(within(row).getByRole("button", { name: "Allow on this machine" })).toBeInTheDocument();
    // Read after the Notices have read the project's sandbox state.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 120));
    });
    // Once, in Asks: no Notice beside it, and no second Allow anywhere in the window.
    expect(within(notices).queryByRole("status", { name: LABEL })).toBeNull();
    expect(screen.getAllByRole("button", { name: "Allow on this machine" })).toHaveLength(1);

    sandbox = LOCKED;
    act(() => sandboxCommandReturned());

    await waitFor(() => expect(handCount()).toBeNull());
    await waitFor(() =>
      expect(askRows().some((each) => each.textContent?.includes("com.example.helper"))).toBe(
        false,
      ),
    );
    expect(screen.queryByRole("button", { name: "Allow on this machine" })).toBeNull();
  });

  it("with a chat focused, the Project chip counts the sandbox consent and lists its row", async () => {
    // Q255: the consent asks about no chat, so it is listed under Project alone, and Asks opens
    // on Chat. Its Notice draws nothing beside it (R-205), so the Project chip's count is how it
    // is found while chat 4 has focus.
    core({ registry: () => [CONSENT], sandbox: () => EXPECTING });
    render(<App />);
    await screen.findByRole("tabpanel", { name: "Asks" });
    await screen.findByTestId("pane");
    const scopes = () => within(screen.getByRole("radiogroup", { name: SCOPE_LABEL }));

    // Chat is picked, with the focused chat's asks: none.
    await waitFor(() =>
      expect(scopes().getByRole("radio", { checked: true })).toHaveAccessibleName("Chat"),
    );
    const project = scopes().getByRole("radio", { name: /^Project/ });
    expect(project).toHaveAccessibleName("Project, 1 waiting on you");
    expect(askRows().some((each) => each.textContent?.includes("com.example.helper"))).toBe(false);

    await userEvent.click(project);
    const row = await waitFor(() => {
      const one = askRows().find((each) => each.textContent?.includes("com.example.helper"));
      expect(one).toBeDefined();
      return one as HTMLElement;
    });
    expect(within(row).getByRole("button", { name: "Allow on this machine" })).toBeInTheDocument();
  });
});
