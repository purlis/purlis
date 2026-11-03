// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidOut, LaidTab, OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";

/**
 * **Keys and focus once a chat isn't a tab** (#4767, ruled on #4699), against the whole window:
 * the pane in a direction, the next and previous tab, panes named by where they are, one polite
 * line where a press lands in a pane the keyboard did not ask for, and Ctrl+Tab out of one
 * pane's terminal onto the next pane's head.
 *
 * Off a Mac, as jsdom's platform is. The chords on a Mac are the registry's (`registry.test.ts`)
 * and are recognised by the same functions.
 */

// A terminal with a text box of its own, inside the holder a chat's keyboard is marked with,
// leaving by Ctrl+Tab as the real one does.
vi.mock("./SessionPane", async (original) => {
  const real = await original<typeof import("./SessionPane")>();
  const { CHAT_KEYBOARD } = await import("./actions");
  return {
    ...real,
    SessionPane: ({ session, focused }: { session: number; focused?: boolean }) => (
      <div
        className={focused ? "pane focused" : "pane"}
        data-testid="pane"
        data-session={session}
        {...{ [CHAT_KEYBOARD]: "" }}
        onKeyDownCapture={real.leavesTheChat}
      >
        session {session}
        <textarea aria-label={`Terminal of ide.${session}`} />
      </div>
    ),
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

function chat(session: number): OpenChat {
  return {
    session,
    name: `ide.${session}`,
    cwd: "/home/dev/plane/workspaces/ide",
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

function core(open: OpenChat[], laid: LaidOut) {
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "plane_at_launch")
      return { plane: "/home/dev/plane", from: "/home/dev/plane", why: null };
    if (cmd === "opened_chats") return open;
    if (cmd === "reopened_layout") return laid;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    void args;
    return null;
  });
}

const tab = (tree: LaidTab["tree"], focused: string, front = true): LaidTab => ({
  workspace: null,
  pinned: false,
  window: 0,
  front,
  focused,
  tree,
});

/** 1 on the left, 2 over 3 on the right; 1 has the focus. */
const threePanes: LaidTab["tree"] = {
  kind: "split",
  direction: "row",
  share: 500,
  a: { kind: "chat", chat: 1 },
  b: {
    kind: "split",
    direction: "column",
    share: 500,
    a: { kind: "chat", chat: 2 },
    b: { kind: "chat", chat: 3 },
  },
};

/** The session the focused pane shows. */
const focusedSession = () => document.querySelector(".pane.focused")?.getAttribute("data-session");

const tabNames = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .map((one) => one.querySelector(".tab-name")?.textContent);
const frontTab = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .find((one) => one.getAttribute("aria-selected") === "true")
    ?.querySelector(".tab-name")?.textContent;

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the pane in a direction (Alt and an arrow off a Mac)", () => {
  it("moves the focus to the pane beside, above and below, and not past the edge", async () => {
    core([chat(1), chat(2), chat(3)], { strip: null, tabs: [tab(threePanes, "a")] });
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));

    await userEvent.keyboard("{Alt>}{ArrowRight}{/Alt}");
    await waitFor(() => expect(focusedSession()).toBe("2"));
    await userEvent.keyboard("{Alt>}{ArrowDown}{/Alt}");
    await waitFor(() => expect(focusedSession()).toBe("3"));
    await userEvent.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    await waitFor(() => expect(focusedSession()).toBe("1"));
    // No wrap: at the left edge nothing moves.
    await userEvent.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    expect(focusedSession()).toBe("1");
  });

  it("leaves Alt and an arrow to a chat's terminal, where it is a byte", async () => {
    core([chat(1), chat(2), chat(3)], { strip: null, tabs: [tab(threePanes, "a")] });
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));
    screen.getByRole("textbox", { name: "Terminal of ide.1" }).focus();

    await userEvent.keyboard("{Alt>}{ArrowRight}{/Alt}");

    expect(focusedSession()).toBe("1");
  });
});

describe("the next and previous tab (Ctrl+Shift+] and [ off a Mac)", () => {
  it("goes along the strip and round its ends, from a chat's terminal too", async () => {
    core([chat(1), chat(2), chat(3)], {
      strip: null,
      tabs: [
        tab({ kind: "chat", chat: 1 }, ""),
        tab({ kind: "chat", chat: 2 }, "", false),
        tab({ kind: "chat", chat: 3 }, "", false),
      ],
    });
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1", "ide.2", "ide.3"]));
    expect(frontTab()).toBe("ide.1");

    await userEvent.keyboard("{Control>}{Shift>}[BracketRight]{/Shift}{/Control}");
    await waitFor(() => expect(frontTab()).toBe("ide.2"));
    await userEvent.keyboard("{Control>}{Shift>}[BracketLeft]{/Shift}{/Control}");
    await waitFor(() => expect(frontTab()).toBe("ide.1"));
    // Round the start to the end, with a chat's terminal holding the keyboard.
    screen.getByRole("textbox", { name: "Terminal of ide.1" }).focus();
    await userEvent.keyboard("{Control>}{Shift>}[BracketLeft]{/Shift}{/Control}");
    await waitFor(() => expect(frontTab()).toBe("ide.3"));
    // Nothing else of the window's answered the keys.
    expect(tabNames()).toEqual(["ide.1", "ide.2", "ide.3"]);
  });
});

describe("panes named by where they are", () => {
  it("makes each pane of a split a region named by its place", async () => {
    core([chat(1), chat(2), chat(3)], { strip: null, tabs: [tab(threePanes, "a")] });
    render(<App />);

    expect(await screen.findByRole("region", { name: "Left pane" })).toHaveTextContent("session 1");
    expect(screen.getByRole("region", { name: "Top right pane" })).toHaveTextContent("session 2");
    expect(screen.getByRole("region", { name: "Bottom right pane" })).toHaveTextContent(
      "session 3",
    );
  });
});

// What the line says where a press jumps to another pane is `paneSaid.test.ts`'s.
describe("one polite line where the keyboard lands in a pane it did not ask for", () => {
  it("says nothing when the keys move the keyboard, which a terminal's own name says", async () => {
    core([chat(1), chat(2), chat(3)], { strip: null, tabs: [tab(threePanes, "a")] });
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));

    await userEvent.keyboard("{Alt>}{ArrowRight}{/Alt}");

    await waitFor(() => expect(focusedSession()).toBe("2"));
    expect(screen.getByTestId("pane-said")).toHaveTextContent("");
  });
});

describe("Ctrl+Tab out of a pane's terminal", () => {
  it("lands on the next pane's head, not in its terminal", async () => {
    core([chat(1), chat(2), chat(3)], { strip: null, tabs: [tab(threePanes, "a")] });
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));
    screen.getByRole("textbox", { name: "Terminal of ide.1" }).focus();

    await userEvent.keyboard("{Control>}{Tab}{/Control}");

    const next = screen.getByRole("region", { name: "Top right pane" });
    const head = next.querySelector(".pane-top");
    expect(head?.contains(document.activeElement)).toBe(true);
  });
});
