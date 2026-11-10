// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat, Shown } from "./bindings";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";

/**
 * **The ✋ counts every open project's Blocking asks** (#5999, ADR 0121 decision 4), against the
 * whole window with the core mocked: two projects open, the first in front. An ask from the
 * second is counted on the ✋ as "+1 elsewhere", the Dock badge shows the same total, Asks opens
 * on all projects, a narrower scope narrows the list and never the ✋, and a snoozed chat's ask
 * is never counted.
 */

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
const scopes = () => within(screen.getByRole("radiogroup", { name: "Show what needs you in" }));
const picked = () => scopes().getByRole("radio", { checked: true }).getAttribute("aria-label");

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

describe("the ✋ across every open project (#5999)", () => {
  it("counts an ask from a second open project as +1 elsewhere", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });

    await waitFor(() =>
      expect(hand()).toHaveAccessibleName("Asks 2 things wait on you, +1 elsewhere"),
    );
    expect(hand().querySelector('[data-part="count"]')).toHaveAttribute("data-count", "2");
  });

  it("counts only the other project's ask where none waits in the project in front", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });

    await waitFor(() =>
      expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you, +1 elsewhere"),
    );
  });

  it("opens Asks on all projects, and a narrower scope never changes the ✋", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });
    await screen.findByRole("tabpanel", { name: "Asks" });
    await waitFor(() => expect(picked()).toBe("All projects, 2 waiting on you"));

    await userEvent.click(scopes().getByRole("radio", { name: /^This project/ }));

    await waitFor(() => expect(picked()).toBe("This project, 1 waiting on you"));
    expect(hand()).toHaveAccessibleName("Asks 2 things wait on you, +1 elsewhere");
  });

  it("never counts a snoozed chat's ask", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [permission(1, "b-1", "Run npm test")] },
    });

    await waitFor(() => expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you"));
  });

  it("shows the same total on the Dock badge", async () => {
    const { badge } = open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });

    await waitFor(() => expect(badge.at(-1)).toBe(2));
  });

  it("puts no number on the Dock where nothing waits", async () => {
    const { badge } = open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [permission(1, "b-1", "Run npm test")] },
    });
    await screen.findByRole("tabpanel", { name: "Asks" });

    await waitFor(() => expect(badge.length).toBeGreaterThan(0));
    expect(badge.every((value) => value === null)).toBe(true);
  });
});
