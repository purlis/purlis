// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import App, { TAB_WINDOW } from "./App";
import type { LaidOut, LaidTab, OpenChat } from "./bindings";
import { chatsOf } from "./laidOut";
import { LAYOUT_MOVED } from "./tabWindows";

/**
 * **Tab windows, as the window draws them** (#4768): a tab torn off into a bare window of its
 * project draws that project's tabs that are its own, with no sidebar and no project strip,
 * and says nothing to the other windows; the project window lets go of a tab the core moved
 * away, and remakes the tab windows its record names. Which window holds which tab is the
 * core's (`layout_windows.rs`, `chats/tests/windows.rs`) and tested there.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const ONE = "/home/dev/one";

function chat(session: number): OpenChat {
  return {
    session,
    name: String(session),
    cwd: null,
    harness: null,
    in_front: false,
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

function tab(window: number, front: boolean, session: number): LaidTab {
  return {
    workspace: null,
    pinned: false,
    window,
    front,
    focused: "",
    tree: { kind: "chat", chat: session },
  };
}

function core(over: { tabWindow?: number; layout: LaidOut; chats: number[] }) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = (given.plane as string | undefined) ?? ONE;
    if (cmd === "plane_at_launch") return { plane: ONE, from: ONE, why: null };
    if (cmd === "planes_to_restore") return { windows: [], dropped: [] };
    if (cmd === "tab_window_handed")
      return over.tabWindow === undefined ? null : { plane: ONE, number: over.tabWindow };
    if (cmd === "charter_windows") return ["main", "window-1"];
    if (cmd === "plane_sidebar")
      return {
        root: plane,
        workspaces: [],
        personas: [],
        persona: null,
        unfiled: over.chats.map(chat),
      };
    if (cmd === "opened_chats") return over.chats.map(chat);
    if (cmd === "reopened_layout") return over.layout;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "tab_windows_back") return 0;
    return null;
  });
  const fire = (event: string, payload: unknown) => {
    const handlers = listeners.get(event) ?? [];
    if (handlers.length === 0) throw new Error(`the window is not listening for ${event}`);
    act(() => {
      for (const handler of handlers)
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
    });
  };
  const said = () =>
    asked.filter((one) => one.cmd === "window_layout").map((one) => one.args.layout as LaidOut);
  return { asked, fire, said, sent: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

/** Every chat each tab a layout names, with its window. */
const shape = (laid: LaidOut | undefined) =>
  laid?.tabs.map((one) => [one.window, chatsOf(one.tree)]) ?? [];

describe("a tab window (#4768)", () => {
  it("draws only its own tabs, with no sidebar and no project strip, and says only those", async () => {
    mockWindows("window-1");
    const { said, sent } = core({
      tabWindow: 1,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);

    await screen.findByText("session 2");
    expect(screen.queryByText("session 1")).toBeNull();
    expect(document.querySelector("main.window")?.getAttribute(TAB_WINDOW)).toBe("1");
    expect(screen.queryByRole("tablist", { name: "Projects" })).toBeNull();
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[1, [2]]]));
    // It holds no project of its own, and remakes no window.
    expect(sent("window_holds_planes")).toEqual([]);
    expect(sent("tab_windows_back")).toEqual([]);
  });
});

describe("the project window of a project with a tab window (#4768)", () => {
  it("remakes the tab windows its record names, and gives no tab to a chat one of them shows", async () => {
    const { said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
      chats: [1, 2],
    });
    render(<App />);

    await screen.findByText("session 1");
    await waitFor(() => expect(sent("tab_windows_back")).toHaveLength(1));
    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [1]]]));
  });

  it("lets go of a tab the core moved to a tab window, and ends nothing", async () => {
    const { fire, said, sent } = core({
      layout: { strip: null, tabs: [tab(0, true, 1), tab(0, false, 2)] },
      chats: [1, 2],
    });
    render(<App />);
    await screen.findByText("session 1");
    await waitFor(() =>
      expect(shape(said().at(-1))).toEqual([
        [0, [1]],
        [0, [2]],
      ]),
    );

    fire(LAYOUT_MOVED, {
      plane: ONE,
      layout: { strip: null, tabs: [tab(0, true, 1), tab(1, true, 2)] },
    });

    await waitFor(() => expect(shape(said().at(-1))).toEqual([[0, [1]]]));
    expect(sent("close_session")).toEqual([]);
  });
});
