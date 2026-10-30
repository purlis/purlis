// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import { HIDDEN_IN_FOCUS } from "./focusMode";
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

/** The commands the window sent, in order. */
const sent: string[] = [];

function core() {
  sent.length = 0;
  mockIPC((cmd) => {
    sent.push(cmd);
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

describe("a region's key in focus mode (D-2921-19)", () => {
  it("leaves focus mode and writes no layout; out of it the same key writes one", async () => {
    core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    await waitFor(() => expect(document.querySelector(".region-frame")).not.toBeNull());
    const window = () => document.querySelector("main.window");
    const focus = { key: "Enter", code: "Enter", shiftKey: true, metaKey: MAC, ctrlKey: !MAC };
    const sidebar = { key: "b", code: "KeyB", metaKey: MAC, ctrlKey: !MAC };
    act(() => key(focus));
    await waitFor(() => expect(window()).toHaveAttribute("data-focus-mode"));
    const from = sent.length;
    act(() => key(sidebar));
    await waitFor(() => expect(window()).not.toHaveAttribute("data-focus-mode"));
    await new Promise((settle) => setTimeout(settle, 300));
    expect(sent.slice(from)).not.toContain("write_layout");
    // Out of focus mode, the same key puts the sidebar away, which is kept.
    act(() => key(sidebar));
    await waitFor(() => expect(sent.slice(from)).toContain("write_layout"));
  });
});

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

describe("focus mode run from the palette, opened from a part it hides (#4072)", () => {
  /** Each part focus mode hides, by its selector in `HIDDEN_IN_FOCUS`. */
  const PARTS = HIDDEN_IN_FOCUS.split(", ");

  it.each(PARTS)("leaves the keyboard on the work, from %s", async (part) => {
    core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    await waitFor(() => expect(document.querySelector(".region-frame")).not.toBeNull());
    const root = () => document.querySelector("main.window");
    // The keyboard in the part: on the first of its controls that takes it, or on the part
    // itself where none does (a slot drawing an empty view).
    const there = document.querySelector<HTMLElement>(part);
    expect(there, `the window draws ${part}`).not.toBeNull();
    const part_ = there as HTMLElement;
    const controls = [
      ...part_.querySelectorAll<HTMLElement>("button:not([disabled]), [tabindex], a[href], input"),
    ];
    for (const control of controls) {
      act(() => control.focus());
      if (part_.contains(document.activeElement)) break;
    }
    if (!part_.contains(document.activeElement)) {
      part_.tabIndex = -1;
      act(() => part_.focus());
    }
    expect(part_.contains(document.activeElement), "the keyboard is in the part").toBe(true);

    // The palette, opened from there, runs Focus mode.
    act(() => {
      fireEvent.keyDown(window, { key: "k", metaKey: MAC, ctrlKey: !MAC });
    });
    const box = await screen.findByRole("combobox", { name: "Run an action" });
    fireEvent.change(box, { target: { value: "Focus mode" } });
    const row = await screen.findByRole("option", { name: /^Focus mode/ });
    act(() => row.click());

    await waitFor(() => expect(root()).toHaveAttribute("data-focus-mode"));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull(),
    );
    await waitFor(() => {
      const at = document.activeElement;
      expect(at, "the keyboard is not left on the page").not.toBe(document.body);
      expect(
        at?.closest(HIDDEN_IN_FOCUS),
        "the keyboard is not in a part focus mode hid",
      ).toBeNull();
    });
  });
});
