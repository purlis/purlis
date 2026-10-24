// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { Moved, OpenChat } from "./bindings";
import { chipName, openAllWorkspaces, workspaceChips } from "./test-strips";

/**
 * **What waits on you in a workspace is never hidden** (ADR 0054, charter-app#392): the
 * operator's constraint, stated before anything else was settled. The workspace strip's
 * show-more carried it until the workspaces moved to the title bar (#2916): now each pinned
 * workspace's chip says what waits in it, and every workspace's row in the ⌄'s popover does,
 * so a workspace with no chip still says its count where it is listed.
 *
 * Against the whole `App`, observed only through what the operator reads: a chip's and a row's
 * names and counts.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

/** A chat the core says a project has open, with everything not under test left plain. */
function chat(session: number, name: string, cwd: string | null): OpenChat {
  return {
    session,
    name,
    cwd,
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
  };
}

/**
 * A core holding the projects a test names, each with the workspaces and chats it names.
 *
 * Chats are filed by the directory they work in, as the real core files them: nothing on the
 * plane records a chat, so the mock does not get told where one belongs either.
 */
function core(
  projects: Record<string, { workspaces: string[]; chats: OpenChat[] }>,
  restore: { planes: string[]; active: number },
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  // Every handler: each project listens for `chat-moved` on the app, and the question is
  // what the right one does with a move about it.
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = (given.plane as string | undefined) ?? "";
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore") return { windows: [restore], dropped: [] };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar") {
      const held = projects[plane];
      return {
        root: plane,
        workspaces: (held?.workspaces ?? []).map((name) => ({
          name,
          path: `${plane}/workspaces/${name}`,
          vision: "",
          todos: [],
          chats: (held?.chats ?? []).filter((one) => one.cwd === `${plane}/workspaces/${name}`),
        })),
        personas: [],
        persona: null,
        unfiled: [],
      };
    }
    if (cmd === "opened_chats") return projects[plane]?.chats ?? [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  return {
    asked,
    /** Fires one `chat-moved`, the way the core pushes one. */
    move(moved: Moved) {
      for (const handler of listeners.get("chat-moved") ?? [])
        window.__TAURI_INTERNALS__.runCallback(handler, {
          event: "chat-moved",
          id: 1,
          payload: moved,
        });
    },
  };
}

/** A move saying `session` in `plane` is waiting, and the queue that plane now has. */
function asking(plane: string, session: number, queue: number[], sequence: number): Moved {
  return {
    plane,
    session,
    state: "waiting",
    needs_you: queue.includes(session),
    queue,
    moved_at: sequence,
    sequence,
    reports: [],
    refusals: [],
    children: [],
  };
}

const PLANE = "/home/dev/plane";
const at = (workspace: string) => `${PLANE}/workspaces/${workspace}`;

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("what waits in a workspace, in the title bar (#2916)", () => {
  /** One project with three workspaces, alpha in front and pinned beta. */
  function threeWorkspaces() {
    return core(
      {
        [PLANE]: {
          workspaces: ["alpha", "beta", "gamma"],
          chats: [
            chat(1, "one", at("alpha")),
            chat(5, "five", at("beta")),
            chat(6, "six", at("gamma")),
          ],
        },
      },
      { planes: [PLANE], active: 0 },
    );
  }

  /** The ⌄'s row for `name`, by its name and what waits in it. */
  const row = async (name: RegExp) =>
    within(await openAllWorkspaces()).getByRole("option", { name });

  it("counts a chat that needs you in a workspace with no chip, on its row in the ⌄", async () => {
    const { move } = threeWorkspaces();
    render(<App />);
    await waitFor(() => expect(workspaceChips().map(chipName)).toEqual(["alpha"]));

    move(asking(PLANE, 6, [6], 1));

    await waitFor(async () => expect(await row(/^gamma/)).toHaveAccessibleDescription("1 waiting"));
  });

  it("drops the count when that chat is ignored", async () => {
    // The count is read from the same queue as the tabs', so it goes down when they do.
    const { asked, move } = threeWorkspaces();
    render(<App />);
    await waitFor(() => expect(workspaceChips().map(chipName)).toEqual(["alpha"]));
    move(asking(PLANE, 6, [6], 1));
    await waitFor(async () => expect(await row(/^gamma/)).toHaveAccessibleDescription("1 waiting"));
    await userEvent.keyboard("{Escape}");

    // The palette's row: the hand's list that had the ✕ retired into the Inbox (#1695).
    await userEvent.keyboard("{Meta>}k{/Meta}");
    await userEvent.keyboard("Ignore six until it asks again");
    await userEvent.keyboard("{Enter}");
    await vi.waitFor(() => expect(asked.some((one) => one.cmd === "ignore_needs_you")).toBe(true));
    // What the core answers an ignore with: the chat still waiting, and a queue without it.
    move(asking(PLANE, 6, [], 2));

    await waitFor(async () => expect(await row(/^gamma/)).toHaveAccessibleName("gamma"));
  });

  it("counts what waits in the workspace in front on its chip", async () => {
    const { move } = threeWorkspaces();
    render(<App />);
    await waitFor(() => expect(workspaceChips().map(chipName)).toEqual(["alpha"]));

    move(asking(PLANE, 1, [1], 1));

    await waitFor(() => expect(workspaceChips()[0]).toHaveAccessibleDescription("1 waiting"));
  });
});
