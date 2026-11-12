// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";
import { aTabForEachChat } from "./laidOut.testkit";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { rowsOf, treeNamed } from "./ui/tree/tree.testkit";

/**
 * A chat a handoff opened lands in the sidebar, and on no strip (#4758; spec story 23).
 *
 * The core starts the chat and says so on `handoff-arrived`; the window's whole part is where
 * it is filed. **It takes no pane and no tab**: a handoff is work sent away from the chat on
 * screen, and a chat the person did not start never rearranges the screen. It was once a tab
 * behind the chat being read (charter-app#204); a chat is not a tab now, and the sidebar is
 * where every chat in no pane runs on. Where it came from is still said wherever it is shown,
 * as a chat the stored layout puts back in a tab shows below.
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

function chat(session: number): OpenChat {
  return {
    session,
    name: `ide.${session}`,
    cwd: "/home/dev/plane/workspaces/ide",
    // No harness and no persona, so its tab is its own name alone and the assertions below read
    // the names they gave it (the default before the name is charter-app#254's, tested there).
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

/** The core, holding `open` chats in workspace `ide`, and a way to say a handoff opened one:
 *  the core files the chat it started there, so the sidebar the window reads again lists it. */
function core(open: OpenChat[]): { arrive: (payload: unknown) => void } {
  const listeners = new Map<string, number>();
  const listed = [...open];
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, handler);
      return 1;
    }
    if (cmd === "plane_at_launch")
      return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
    if (cmd === "plane_sidebar")
      return {
        root: "/home/dev/plane",
        personas: [],
        persona: null,
        unfiled: [],
        workspaces: [
          {
            name: "ide",
            path: "/home/dev/plane/workspaces/ide",
            vision: "",
            todos: [],
            chats: [...listed],
          },
        ],
      };
    if (cmd === "opened_chats") return open;
    // The layout stored gives each chat the core put back a tab (#4758); a handoff's chat, which
    // arrives after it, is in no layout.
    if (cmd === "reopened_layout") return aTabForEachChat(open, () => "ide");
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return {
    arrive: (payload) => {
      const handler = listeners.get("handoff-arrived");
      if (handler === undefined) throw new Error("the window is not listening for handoffs");
      const one = payload as { plane: string; session: number; name: string; label?: string };
      if (one.plane === "/home/dev/plane")
        listed.push({ ...chat(one.session), name: one.name, label: one.label ?? null });
      window.__TAURI_INTERNALS__.runCallback(handler, {
        event: "handoff-arrived",
        id: 1,
        payload,
      });
    },
  };
}

const strip = () => stripNamed("Tabs");
const tabNames = () =>
  within(strip())
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);
const selected = () =>
  within(strip())
    .getAllByRole("tab")
    .filter((tab) => tab.getAttribute("aria-selected") === "true")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

/** The tab whose name is `name`. */
const tabNamed = (name: string) =>
  within(strip())
    .getAllByRole("tab")
    .find((tab) => tab.querySelector(".tab-name")?.textContent === name);

/** The rows of the Chats list, by what each says of its chat. */
const rows = () => rowsOf(treeNamed("Chats of this project")).map((row) => row.textContent ?? "");

describe("a chat a handoff opened, named for its task (charter-app#258)", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it("is listed in the sidebar by the task name the handoff gave it, and takes no tab", async () => {
    const { arrive } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    arrive({
      plane: "/home/dev/plane",
      session: 2,
      name: "2",
      label: "drop commons",
      from: { name: "steward 3", workspace: "platform-next" },
      workspace: "ide",
      persona: null,
      harness: "claude",
    });

    await waitFor(() => expect(rows().some((row) => row.includes("drop commons"))).toBe(true));
    expect(tabNames()).toEqual(["ide.1"]);
  });

  it("comes back from the record still saying where it came from, in its tab and its pane", async () => {
    core([
      {
        ...chat(1),
        label: "drop commons",
        from: {
          name: "steward 3",
          workspace: "platform-next",
          chat: 3,
          task: false,
          tab: true,
          reported: false,
          unreported: false,
        },
      },
    ]);
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["drop commons"]));
    // Under the chat's number and name (#6212).
    expect(tipOf(tabNamed("drop commons"))).toEqual(
      "#1 · drop commons\n↳ from steward 3 · platform-next",
    );
    expect(document.querySelector(".pane-from")).toHaveTextContent(
      "↳ from steward 3 · platform-next",
    );
  });

  it("keeps a task a dispatch started off the strip, the chat being read still in front (#1436)", async () => {
    const { arrive } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    arrive({
      plane: "/home/dev/plane",
      session: 2,
      name: "2",
      label: "check the queue",
      from: { name: "ide.1", workspace: "ide", chat: 1, task: true },
      workspace: "ide",
      persona: "steward",
      harness: "claude",
    });

    await waitFor(() => expect(rows().some((row) => row.includes("check the queue"))).toBe(true));
    expect(tabNames()).toEqual(["ide.1"]);
    expect(selected()).toEqual(["ide.1"]);
  });

  it("says nothing of where a chat the operator opened came from", async () => {
    core([chat(1)]);
    render(<App />);

    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));
    // Its number and name alone (#6212): nothing of where it came from.
    expect(tipOf(tabNamed("ide.1"))).toBe("#1 · ide.1");
    expect(document.querySelector(".pane-from")).toBeNull();
  });
});

describe("a chat a handoff opened", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it("takes no tab and no pane, and leaves the chat being read in front", async () => {
    const { arrive } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    for (const session of [2, 3])
      arrive({
        plane: "/home/dev/plane",
        session,
        name: `handoff ${session}`,
        workspace: "ide",
        persona: null,
      });

    await waitFor(() => expect(rows().filter((row) => row.includes("handoff")).length).toBe(2));
    expect(tabNames()).toEqual(["ide.1"]);
    expect(selected()).toEqual(["ide.1"]);
    expect(screen.getAllByTestId("pane").map((pane) => pane.textContent)).toEqual(["session 1"]);
  });

  it("is not drawn in a window showing another plane", async () => {
    // Every plane numbers its chats from one, so the plane is half the chat's identity.
    const { arrive } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    arrive({
      plane: "/home/dev/other-plane",
      session: 2,
      name: "handoff from 1",
      workspace: "ide",
      persona: null,
    });

    await new Promise((settle) => setTimeout(settle, 50));
    expect(tabNames()).toEqual(["ide.1"]);
    expect(rows().some((row) => row.includes("handoff from 1"))).toBe(false);
  });
});
