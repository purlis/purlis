// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";

/**
 * A terminal `$VISUAL`/`$EDITOR` opens in a shell tab of its own (#1042). The core starts it
 * and says so on `editor-tab-opened`; the window draws the tab **in front**, since the person
 * pressed Open in your editor for it.
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

/** The core, holding `open` chats, and a way to say an editor opened in a tab. */
function core(chats: OpenChat[]): { open: (payload: unknown) => void } {
  const listeners = new Map<string, number>();
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, handler);
      return 1;
    }
    if (cmd === "plane_at_launch")
      return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
    if (cmd === "opened_chats") return chats;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return {
    open: (payload) => {
      const handler = listeners.get("editor-tab-opened");
      if (handler === undefined) throw new Error("the window is not listening for editor tabs");
      window.__TAURI_INTERNALS__.runCallback(handler, {
        event: "editor-tab-opened",
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

describe("a terminal editor's tab (#1042)", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it("is drawn in front, named for the editor and the file", async () => {
    const { open } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    open({ plane: "/home/dev/plane", session: 2, name: "vim lib.rs", workspace: "ide" });

    await waitFor(() => expect(tabNames()).toEqual(["ide.1", "vim lib.rs"]));
    expect(selected()).toEqual(["vim lib.rs"]);
  });

  it("is not drawn for another project", async () => {
    const { open } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    open({ plane: "/home/dev/other", session: 2, name: "vim lib.rs", workspace: "ide" });

    await new Promise((done) => setTimeout(done, 50));
    expect(tabNames()).toEqual(["ide.1"]);
  });
});
