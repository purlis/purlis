// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import { stripNamed } from "../test-strips";

/**
 * **Focus mode and the cheat sheet, from the window** (#2921): their keys press their palette
 * rows in the project in front, as a person's key would land on the page.
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function core() {
  mockIPC((cmd) => {
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "plane_sidebar")
      return { root: PLANE, personas: [], persona: null, unfiled: [], workspaces: [] };
    if (cmd === "about_charter") return new Promise(() => {});
    return null;
  });
}

/** A key as the page hears it, on the element that has the keyboard (the page, here). */
function key(init: KeyboardEventInit) {
  (document.activeElement ?? document.body).dispatchEvent(
    new KeyboardEvent("keydown", { ...init, bubbles: true, cancelable: true }),
  );
}

const MAC = navigator.platform.startsWith("Mac");

describe("focus mode and the cheat sheet, from the window", () => {
  it("toggles focus mode on its key, and opens the cheat sheet on its", async () => {
    core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    const window = () => document.querySelector("main.window");
    await waitFor(() => expect(document.querySelector(".region-frame")).not.toBeNull());
    expect(
      document.querySelector('[data-scope="dialog"][data-part="root"], [role="alertdialog"]'),
    ).toBeNull();
    act(() => key({ key: "Enter", code: "Enter", shiftKey: true, metaKey: MAC, ctrlKey: !MAC }));
    await waitFor(() => expect(window()).toHaveAttribute("data-focus-mode"));
    act(() => key({ key: "Enter", code: "Enter", shiftKey: true, metaKey: MAC, ctrlKey: !MAC }));
    await waitFor(() => expect(window()).not.toHaveAttribute("data-focus-mode"));
    act(() => key({ key: "/", code: "Slash", metaKey: MAC, ctrlKey: !MAC }));
    await screen.findByTestId("shortcuts-view");
    expect(
      within(stripNamed("Tabs"))
        .getAllByRole("tab")
        .filter((tab) => tab.textContent?.includes("Keyboard shortcuts")),
    ).toHaveLength(1);
  });
});
