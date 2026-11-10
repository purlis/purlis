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
 * **All chats, in the whole window** (#6013, ADR 0121 decision 18), with the core mocked: two
 * projects open, the first in front. The Chats sidebar's switch lists the chats of both, the one
 * waiting on you first, and several chosen across the two are stopped, each in its own project.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" data-session={session}>
      session {session}
    </div>
  ),
}));

afterEach(() => {
  sent.length = 0;
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

/** Every command the core was sent, with its arguments. */
const sent: { cmd: string; args: Record<string, unknown> }[] = [];

/** The core: both projects open in this window, ONE in front, each with what it holds. */
function core(projects: Record<string, Project>) {
  const badge: unknown[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    sent.push({ cmd, args: a });
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

function open(projects: Record<string, Project>) {
  core(projects);
  mockWindows("main");
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

const TWO_PROJECTS = () => ({
  [ONE]: { chats: [chat(ONE, 1)], asks: [], snoozed: [] },
  [TWO]: {
    chats: [chat(TWO, 1), chat(TWO, 2)],
    asks: [permission(2, "b-2", "Run npm test")],
    snoozed: [],
  },
});

const list = () => screen.getByRole("grid", { name: "All chats" });
const row = (name: RegExp) => within(list()).getByRole("row", { name });

describe("All chats across every open project (#6013)", () => {
  it("lists every chat of both projects", async () => {
    open(TWO_PROJECTS());
    await userEvent.click(await screen.findByRole("button", { name: "All chats", pressed: false }));

    await waitFor(() => expect(within(list()).getAllByRole("row")).toHaveLength(3));
    // The same number in two projects is two chats, each said with its project.
    expect(row(/^steward 1, .* · one · alpha$/)).toBeTruthy();
    expect(row(/^steward 1, .* · two · alpha$/)).toBeTruthy();
    expect(row(/^steward 2, .* · two · alpha$/)).toBeTruthy();
  });

  it("stops several chosen chats, each in its own project", async () => {
    open(TWO_PROJECTS());
    await userEvent.click(await screen.findByRole("button", { name: "All chats", pressed: false }));
    await waitFor(() => expect(within(list()).getAllByRole("row")).toHaveLength(3));

    await userEvent.click(within(row(/^steward 1, .* · one ·/)).getByRole("checkbox"));
    await userEvent.click(within(row(/^steward 2, .* · two ·/)).getByRole("checkbox"));
    const chosen = screen.getByRole("group", { name: "Chosen chats" });
    await userEvent.click(within(chosen).getByRole("button", { name: "Stop" }));
    const asked = screen.getByRole("group", { name: "Stop chats" });
    expect(asked).toHaveTextContent("Stop 2 chats?");
    await userEvent.click(within(asked).getByRole("button", { name: "Stop" }));

    await waitFor(() =>
      expect(
        sent
          .filter((one) => one.cmd === "stop_chat")
          .map((one) => `${String(one.args.plane)}#${String(one.args.session)}`)
          .sort(),
      ).toEqual([`${ONE}#1`, `${TWO}#2`]),
    );
  });

  it("goes back to this project's chats by its switch", async () => {
    open(TWO_PROJECTS());
    await userEvent.click(await screen.findByRole("button", { name: "All chats", pressed: false }));
    await userEvent.click(await screen.findByRole("button", { name: "All chats", pressed: true }));
    await waitFor(() => expect(screen.queryByRole("grid", { name: "All chats" })).toBeNull());
  });
});
