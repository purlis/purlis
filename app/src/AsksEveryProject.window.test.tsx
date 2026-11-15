// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat, Shown } from "./bindings";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { keepInboxScope } from "./inboxScope";

/**
 * **The ✋ counts the project in front; the Dock badge counts every open project** (#6341, ADR
 * 0121 amended 2026-11-12, decision 1, which reverses #5999's ✋), against the whole window with
 * the core mocked: two projects open, the first in front. An ask from the second is not on the
 * ✋ but on its own project tab and on the Dock badge's total, Asks lists the project in front's
 * alone (#6340, decision 4), bringing the second forward moves the ✋ to its count, pressing the
 * ✋ brings no other project forward, and a snoozed chat's ask is never counted.
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
function core(projects: Record<string, Project>, events = false) {
  const badge: unknown[] = [];
  const answered: Record<string, unknown>[] = [];
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "answer_ask") {
        answered.push(a);
        return null;
      }
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
    },
    { shouldMockEvents: events },
  );
  return { badge, answered };
}

const bar = () => screen.getByRole("tablist", { name: "Attention" });
const hand = () => within(bar()).getByRole("tab", { name: /^Asks/ });
const scopes = () => within(screen.getByRole("radiogroup", { name: "Show what needs you in" }));
const picked = () => scopes().getByRole("radio", { checked: true }).getAttribute("aria-label");

function open(projects: Record<string, Project>, events = false) {
  const held = core(projects, events);
  // The main window, as Tauri names it, so the badge has a window to be set on.
  mockWindows("main");
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  return held;
}

/** The project tab on the rail for `name`'s project: the rail's tablist owns its tabs by
 *  `aria-owns`, so each is found by the directory it carries. */
const tab = (name: "one" | "two") => {
  const found = document.querySelector<HTMLElement>(
    `[role="tab"][data-directory="${name === "one" ? ONE : TWO}"]`,
  );
  if (found === null) throw new Error(`no project tab for ${name}`);
  return found;
};

/** The project tab in front, by its name. */
const front = () =>
  [...document.querySelectorAll<HTMLElement>('[role="tab"][data-directory]')]
    .find((one) => one.ariaSelected === "true")
    ?.getAttribute("aria-label");

describe("the ✋ counts the project in front (#6341)", () => {
  it("counts only the project in front's ask, never another's", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });

    await waitFor(() => expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you"));
    expect(hand().querySelector('[data-part="count"]')).toHaveAttribute("data-count", "1");
  });

  it("counts nothing where only another project waits", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });
    // TWO's tab says its ask, so the ✋'s having none is read, not a moment before the asks.
    await waitFor(() => expect(tab("two")).toHaveAccessibleDescription(/1 waiting/));

    expect(hand().querySelector('[data-part="count"]')).toBeNull();
    expect(hand()).toHaveAccessibleName("Asks");
  });

  it("puts each project's count on its own tab", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: {
        chats: [chat(TWO, 1)],
        asks: [permission(1, "b-1", "Run npm test"), permission(1, "b-2", "Run npm build")],
        snoozed: [],
      },
    });

    await waitFor(() => expect(tab("two")).toHaveAccessibleDescription(/2 waiting/));
    expect(tab("one")).toHaveAccessibleDescription(/1 waiting/);
  });

  it("moves to the other project's count once that project is in front", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: {
        chats: [chat(TWO, 1)],
        asks: [permission(1, "b-1", "Run npm test"), permission(1, "b-2", "Run npm build")],
        snoozed: [],
      },
    });
    await waitFor(() => expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you"));

    await userEvent.click(tab("two"));

    await waitFor(() => expect(hand()).toHaveAccessibleName("Asks 2 things wait on you"));
  });

  it("lists only the project in front's asks in Asks, and the ✋ counts that project's alone (ADR 0121, amended 2026-11-12)", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });
    await screen.findByRole("tabpanel", { name: "Asks" });
    // No chat has a pane here, so Chat is greyed and Asks lists Project: ONE's ask alone.
    await waitFor(() => expect(picked()).toBe("Project, 1 waiting on you"));
    expect(scopes().getByRole("radio", { name: "Chat" })).toBeDisabled();
    expect(scopes().queryByRole("radio", { name: /All projects/ })).toBeNull();
    expect(document.querySelector(`[data-elsewhere]`)).toBeNull();
    expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you");
  });

  it("never counts a snoozed chat's ask", async () => {
    open({
      [ONE]: { chats: [chat(ONE, 1)], asks: [permission(1, "a-1", "Run cargo test")], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [], snoozed: [permission(1, "b-1", "Run npm test")] },
    });

    await waitFor(() => expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you"));
    expect(tab("two")).not.toHaveAccessibleDescription(/waiting/);
  });

  it("keeps every open project's total on the Dock badge (#5999)", async () => {
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

  it("opens the Asks in front when the title bar's hand is pressed, and brings no other project forward, whatever scope was kept", async () => {
    // A scope kept from before Asks stopped keeping one (#6340): the press never reads it.
    keepInboxScope(ONE, "project");
    const quiet = { ...chat(ONE, 1), unreported: "a shell cannot say it waits" };
    open({
      [ONE]: { chats: [quiet], asks: [], snoozed: [] },
      [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
    });
    await waitFor(() => expect(tab("two")).toHaveAccessibleDescription(/1 waiting/));
    expect(front()).toBe("one");
    // Asks put away, so the press is what opens it.
    if (screen.queryByRole("tabpanel", { name: "Asks" }) !== null) await userEvent.click(hand());
    await waitFor(() => expect(screen.queryByRole("tabpanel", { name: "Asks" })).toBeNull());

    // Nothing waits in ONE, but its quiet chat draws the faint hand in the title bar.
    await userEvent.click(await screen.findByTestId("needs-you-button"));

    // ONE's own Asks opens and ONE stays in front. TWO's ask is reached from TWO's tab.
    const inbox = await screen.findByRole("tabpanel", { name: "Asks" });
    expect(front()).toBe("one");
    expect(within(inbox).queryByText("Run npm test")).toBeNull();
  });

  it("brings another project forward with its ask open when its notification is clicked", async () => {
    open(
      {
        [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
        [TWO]: { chats: [chat(TWO, 1)], asks: [permission(1, "b-1", "Run npm test")], snoozed: [] },
      },
      true,
    );
    await waitFor(() => expect(tab("two")).toHaveAccessibleDescription(/1 waiting/));
    expect(front()).toBe("one");

    // The window is behind another app when TWO's notification is clicked.
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    await act(async () => {
      await emit("asks-notified", { plane: TWO, session: 1, sessions: [1] });
    });
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });

    await waitFor(() => expect(front()).toBe("two"));
    await waitFor(() => expect(hand()).toHaveAccessibleName("Asks 1 thing waits on you"));
    const asks = await screen.findByRole("tabpanel", { name: "Asks" });
    await waitFor(() => expect(within(asks).getByText("Run npm test")).toBeTruthy());
  });
});
