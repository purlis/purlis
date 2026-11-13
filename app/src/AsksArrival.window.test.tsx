// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat, Shown } from "./bindings";
import { asksMoved } from "./asks";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";

/**
 * **A Blocking ask arriving, in the whole window** (ADR 0121 decisions 24 and 25, #6020): it is
 * said once in a polite live region, its count is in the ✋'s name, and one in the project in
 * front bounces the ✋ (#6343).
 */

// The purlis window in front, as the person using it has it.
beforeEach(() => vi.spyOn(document, "hasFocus").mockReturnValue(true));

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" data-session={session}>
      session {session}
    </div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
});

const ONE = "/home/dev/one";
const TWO = "/home/dev/two";

const chat = (plane: string, session: number): OpenChat => ({
  session,
  name: `steward ${session}`,
  cwd: `${plane}/workspaces/alpha`,
  harness: null,
  in_front: session === 1,
  resumed: null,
  fresh: null,
  profile: null,
  persona: null,
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
});

/** A permission prompt chat `session` holds on its hook, as the registry lists it. */
const permission = (session: number, ask: string, says: string): Shown => ({
  session,
  ask,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

type Project = { chats: OpenChat[]; asks: Shown[]; snoozed: Shown[] };

/** The core: both projects open in this window, ONE in front, each with what it holds. */
function core(projects: Record<string, Project>) {
  const badge: unknown[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    const here = projects[(a.plane as string | undefined) ?? ONE] ?? projects[ONE];
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "plugin:window|set_badge_count") {
      badge.push(a.value ?? null);
      return null;
    }
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: [ONE, TWO], active: 0 }], dropped: [], gone: [] };
    if (cmd === "open_plane") return { plane: a.path, ask: null };
    if (cmd === "opened_chats") return [...here.chats];
    if (cmd === "plane_sidebar")
      return structuredClone({
        root: (a.plane as string | undefined) ?? ONE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [
          {
            name: "alpha",
            path: `${(a.plane as string | undefined) ?? ONE}/workspaces/alpha`,
            vision: "",
            todos: [],
            colour: null,
            live: false,
            chats: [...here.chats],
          },
        ],
      });
    if (cmd === "asks_waiting")
      return {
        plane: a.plane,
        asks: here.asks,
        with_asking_chat: [],
        snoozed: here.snoozed,
      };
    if (cmd === "chat_states") return [];
    if (cmd === "stopping_chats") return [];
    if (cmd === "finished_tasks") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "dispatch_grants_needed") return [];
    if (cmd === "vault_refusals") return [];
    if (cmd === "owed_restarts") return [];
    if (cmd === "runners_away") return [];
    return null;
  });
  return { badge };
}

const bar = () => screen.getByRole("tablist", { name: "Attention" });
const hand = () => within(bar()).getByRole("tab", { name: /^Asks/ });

function open(projects: Record<string, Project>) {
  const held = core(projects);
  // The main window, as Tauri names it, so the badge has a window to be set on.
  mockWindows("main");
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  return held;
}

const said = () => screen.getByTestId("ask-arrived");

/** An ask arriving in ONE: the core's list grows, and the window is told its asks moved. */
async function arrives(projects: Record<string, Project>, ask: Shown, plane = ONE) {
  projects[plane].asks = [...projects[plane].asks, ask];
  await act(async () => {
    asksMoved(plane);
    await new Promise((resolve) => setTimeout(resolve, 100));
  });
}

describe("a Blocking ask arriving (#6020)", () => {
  it("is said once, politely, and the ✋'s name carries the count", async () => {
    const projects: Record<string, Project> = {
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [] },
    };
    open(projects);
    await screen.findByRole("tabpanel", { name: "Asks" });
    expect(said()).toHaveAttribute("role", "status");
    expect(said()).toHaveTextContent("");

    await arrives(projects, permission(1, "a-1", "Run cargo test"));

    await waitFor(() => expect(said()).toHaveTextContent("New ask from steward 1: Run cargo test"));
    expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you");
    // Read again with nothing new: the same line, not a new one to be said.
    const line = said().firstElementChild;
    await act(async () => {
      asksMoved(ONE);
      await new Promise((resolve) => setTimeout(resolve, 100));
    });
    expect(said().firstElementChild).toBe(line);
  });

  it("bounces the ✋ for a new ask in the project in front, and stops it as the bounce ends", async () => {
    const projects: Record<string, Project> = {
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [] },
    };
    open(projects);
    await screen.findByRole("tabpanel", { name: "Asks" });
    expect(hand()).not.toHaveAttribute("data-bounce");

    await arrives(projects, permission(1, "a-1", "Run cargo test"));

    await waitFor(() => expect(hand()).toHaveAttribute("data-bounce", "true"));
    const event = new Event("webkitAnimationEnd", { bubbles: true });
    Object.defineProperty(event, "animationName", { value: "purlis-hand-bounce" });
    act(() => void hand().dispatchEvent(event));
    await waitFor(() => expect(hand()).not.toHaveAttribute("data-bounce"));
  });

  it("does not bounce the ✋ for an ask in another project", async () => {
    const projects: Record<string, Project> = {
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [] },
    };
    open(projects);
    await screen.findByRole("tabpanel", { name: "Asks" });

    await arrives(projects, permission(1, "b-1", "Run cargo build"), TWO);

    await waitFor(() => expect(said()).toHaveTextContent("New ask in another project"));
    expect(hand()).not.toHaveAttribute("data-bounce");
  });

  it("does not bounce the ✋ while the window is behind, and still says the ask", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const projects: Record<string, Project> = {
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [] },
    };
    open(projects);
    await screen.findByRole("tabpanel", { name: "Asks" });

    await arrives(projects, permission(1, "a-1", "Run cargo test"));

    await waitFor(() => expect(said()).toHaveTextContent("New ask from steward 1"));
    expect(hand()).not.toHaveAttribute("data-bounce");
  });
});
